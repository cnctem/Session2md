use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{
    messages_from_content, open_sqlite_readonly, row_to_json, sqlite_source, table_column_names,
};

const PROVIDER_ID: &str = "goose";

pub fn database_path() -> PathBuf {
    paths::goose_data_dir().join("sessions").join("sessions.db")
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    scan_database(&database_path()).unwrap_or_default()
}

pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    let (db_path, session_id) = super::common::parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid Goose SQLite source reference: {source}"))?;
    let conn = open_sqlite_readonly(&db_path)?;
    load_session_messages(&conn, &session_id)
}

fn scan_database(path: &Path) -> Result<Vec<SessionMeta>, String> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let conn = open_sqlite_readonly(path)?;
    let columns = table_column_names(&conn, "sessions");
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    let order = if columns.iter().any(|column| column == "updated_at") {
        "updated_at"
    } else {
        "rowid"
    };
    let mut stmt = conn
        .prepare(&format!("SELECT * FROM sessions ORDER BY {order} DESC"))
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok(row_to_json(row, &columns)))
        .map_err(|error| error.to_string())?;
    let mut sessions = Vec::new();
    for row in rows.flatten() {
        let Some(id) = row.get("id").and_then(Value::as_str) else {
            continue;
        };
        let session_type = row
            .get("session_type")
            .and_then(Value::as_str)
            .unwrap_or("");
        if matches!(session_type, "sub_agent" | "hidden")
            || row
                .get("parent_session_id")
                .and_then(Value::as_str)
                .is_some_and(|parent| !parent.is_empty())
        {
            continue;
        }
        let messages = load_session_messages(&conn, id).unwrap_or_default();
        let first_user = messages
            .iter()
            .find(|message| message.role == "user")
            .map(|message| super::utils::truncate_summary(&message.content, 160));
        let title = row
            .get("name")
            .and_then(Value::as_str)
            .filter(|title| !title.trim().is_empty())
            .or_else(|| row.get("description").and_then(Value::as_str))
            .map(|title| super::utils::truncate_summary(title, 80))
            .or_else(|| first_user.clone());
        sessions.push(SessionMeta {
            provider_id: PROVIDER_ID.to_string(),
            session_id: id.to_string(),
            title,
            summary: first_user,
            project_dir: row
                .get("working_dir")
                .and_then(Value::as_str)
                .filter(|path| !path.trim().is_empty())
                .map(str::to_string),
            created_at: row.get("created_at").and_then(parse_sql_time),
            last_active_at: row
                .get("updated_at")
                .and_then(parse_sql_time)
                .or_else(|| row.get("created_at").and_then(parse_sql_time)),
            source_path: Some(sqlite_source(path, id)),
            resume_command: None,
            can_delete: false,
        });
    }
    Ok(sessions)
}

fn load_session_messages(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<SessionMessage>, String> {
    let columns = table_column_names(conn, "messages");
    if columns.is_empty() {
        return Err("Goose messages table is missing".to_string());
    }
    let order = if columns.iter().any(|column| column == "created_timestamp") {
        "created_timestamp"
    } else {
        "rowid"
    };
    let mut stmt = conn
        .prepare(&format!(
            "SELECT * FROM messages WHERE session_id = ?1 ORDER BY {order}, id"
        ))
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([session_id], |row| Ok(row_to_json(row, &columns)))
        .map_err(|error| error.to_string())?;
    let mut messages = Vec::new();
    for row in rows.flatten() {
        if row
            .get("metadata_json")
            .and_then(Value::as_str)
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|metadata| metadata.get("userVisible").and_then(Value::as_bool))
            == Some(false)
        {
            continue;
        }
        let role = row.get("role").and_then(Value::as_str).unwrap_or("");
        let Some(raw_content) = row.get("content_json").and_then(Value::as_str) else {
            continue;
        };
        let Ok(content) = serde_json::from_str::<Value>(raw_content) else {
            continue;
        };
        messages.extend(messages_from_content(
            role,
            &content,
            row.get("created_timestamp")
                .and_then(super::utils::parse_timestamp_to_ms),
        ));
    }
    Ok(messages)
}

fn parse_sql_time(value: &Value) -> Option<i64> {
    if let Some(timestamp) = super::utils::parse_timestamp_to_ms(value) {
        return Some(timestamp);
    }
    let raw = value.as_str()?;
    if raw.len() == 19 && raw.as_bytes().get(10) == Some(&b' ') {
        chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
            .ok()
            .map(|value| value.and_utc().timestamp_millis())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    #[test]
    fn scans_goose_visible_messages() {
        let root = tempdir().expect("root");
        let path = root.path().join("sessions.db");
        let conn = Connection::open(&path).expect("db");
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT, name TEXT, description TEXT, working_dir TEXT, session_type TEXT, parent_session_id TEXT, created_at TEXT, updated_at TEXT);
             CREATE TABLE messages (id INTEGER, session_id TEXT, role TEXT, content_json TEXT, created_timestamp INTEGER, metadata_json TEXT);",
        )
        .expect("schema");
        conn.execute(
            "INSERT INTO sessions VALUES ('goose-1','Demo','','/tmp/project','','','2026-01-01 00:00:00','2026-01-01 00:00:01')",
            [],
        )
        .expect("session");
        conn.execute(
            "INSERT INTO messages VALUES (1,'goose-1','user','[{\"type\":\"text\",\"text\":\"hello\"}]',1000,NULL)",
            [],
        )
        .expect("user");
        conn.execute(
            "INSERT INTO messages VALUES (2,'goose-1','assistant','[{\"type\":\"thinking\",\"thinking\":\"hidden\"},{\"type\":\"text\",\"text\":\"answer\"}]',2000,NULL)",
            [],
        )
        .expect("assistant");
        let sessions = scan_database(&path).expect("scan");
        assert_eq!(sessions.len(), 1);
        let messages =
            load_messages(sessions[0].source_path.as_deref().expect("source")).expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "hidden\n\nanswer");
    }
}
