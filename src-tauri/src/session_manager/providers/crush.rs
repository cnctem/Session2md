use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{
    messages_from_parts, normalize_content_parts, open_sqlite_readonly, row_to_json, sqlite_source,
    table_column_names,
};

const PROVIDER_ID: &str = "crush";

pub fn project_databases() -> Vec<PathBuf> {
    let root = paths::crush_data_dir();
    let direct = root.join(".crush").join("crush.db");
    if direct.is_file() {
        return vec![direct];
    }
    let registry = root.join("projects.json");
    let Ok(raw) = fs::read_to_string(registry) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    value
        .get("projects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|project| {
            let data_dir = project.get("data_dir").and_then(Value::as_str);
            let data_dir = data_dir.map(PathBuf::from).or_else(|| {
                project
                    .get("path")
                    .and_then(Value::as_str)
                    .map(|path| PathBuf::from(path).join(".crush"))
            })?;
            let db = data_dir.join("crush.db");
            (db.is_file() && seen.insert(db.clone())).then_some(db)
        })
        .collect()
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    project_databases()
        .iter()
        .flat_map(|path| scan_database(path))
        .collect()
}

pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    let (db_path, session_id) = super::common::parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid Crush SQLite source reference: {source}"))?;
    let conn = open_sqlite_readonly(&db_path)?;
    load_session_messages(&conn, &session_id)
}

fn scan_database(path: &Path) -> Vec<SessionMeta> {
    let Ok(conn) = open_sqlite_readonly(path) else {
        return Vec::new();
    };
    let columns = table_column_names(&conn, "sessions");
    if columns.is_empty() {
        return Vec::new();
    }
    let Ok(mut stmt) = conn.prepare("SELECT * FROM sessions ORDER BY updated_at DESC") else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| Ok(row_to_json(row, &columns))) else {
        return Vec::new();
    };
    rows.flatten()
        .filter(|session| {
            session
                .get("parent_session_id")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
                && !session
                    .get("title")
                    .and_then(Value::as_str)
                    .is_some_and(|title| title == "Generate a title")
        })
        .filter_map(|session| {
            let id = session.get("id").and_then(Value::as_str)?;
            let messages = load_session_messages(&conn, id).unwrap_or_default();
            let first_user = messages
                .iter()
                .find(|message| message.role == "user")
                .map(|message| super::utils::truncate_summary(&message.content, 160));
            Some(SessionMeta {
                provider_id: PROVIDER_ID.to_string(),
                session_id: id.to_string(),
                title: session
                    .get("title")
                    .and_then(Value::as_str)
                    .filter(|title| !title.trim().is_empty())
                    .map(|title| super::utils::truncate_summary(title, 80))
                    .or_else(|| first_user.clone()),
                summary: first_user,
                project_dir: project_for_database(path),
                created_at: session
                    .get("created_at")
                    .and_then(super::utils::parse_timestamp_to_ms),
                last_active_at: session
                    .get("updated_at")
                    .and_then(super::utils::parse_timestamp_to_ms),
                source_path: Some(sqlite_source(path, id)),
                resume_command: None,
                can_delete: false,
            })
        })
        .collect()
}

fn load_session_messages(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<SessionMessage>, String> {
    let columns = table_column_names(conn, "messages");
    let mut stmt = conn
        .prepare("SELECT * FROM messages WHERE session_id = ?1 ORDER BY created_at, id")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([session_id], |row| Ok(row_to_json(row, &columns)))
        .map_err(|error| error.to_string())?;
    let mut messages = Vec::new();
    for row in rows.flatten() {
        let role = row.get("role").and_then(Value::as_str).unwrap_or("");
        let Some(raw_parts) = row.get("parts").and_then(Value::as_str) else {
            continue;
        };
        let Ok(parts) = serde_json::from_str::<Value>(raw_parts) else {
            continue;
        };
        let normalized_parts = normalize_content_parts(&parts);
        messages.extend(messages_from_parts(
            role,
            &normalized_parts,
            row.get("created_at")
                .and_then(super::utils::parse_timestamp_to_ms),
        ));
    }
    Ok(messages)
}

fn project_for_database(path: &Path) -> Option<String> {
    if path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        == Some(".crush")
    {
        path.parent()?
            .parent()
            .map(|path| path.display().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn reads_visible_crush_text_parts() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("crush.db");
        let conn = Connection::open(&path).expect("db");
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT, parent_session_id TEXT, title TEXT, created_at INTEGER, updated_at INTEGER);
             CREATE TABLE messages (id TEXT, session_id TEXT, role TEXT, parts TEXT, created_at INTEGER);",
        )
        .expect("schema");
        conn.execute(
            "INSERT INTO sessions VALUES ('crush-1',NULL,'Demo',1000,2000)",
            [],
        )
        .expect("session");
        conn.execute(
            "INSERT INTO messages VALUES ('m1','crush-1','user','[{\"type\":\"text\",\"data\":{\"text\":\"hello\"}}]',1000)",
            [],
        )
        .expect("user");
        conn.execute(
            "INSERT INTO messages VALUES ('m2','crush-1','assistant','[{\"type\":\"reasoning\",\"data\":{\"text\":\"hidden\"}},{\"type\":\"text\",\"data\":{\"text\":\"answer\"}}]',2000)",
            [],
        )
        .expect("assistant");
        let sessions = scan_database(&path);
        assert_eq!(sessions.len(), 1);
        let messages =
            load_messages(sessions[0].source_path.as_deref().expect("source")).expect("messages");
        assert_eq!(messages.len(), 3);
        assert!(messages.iter().any(|message| {
            message.kind == crate::session_manager::SessionMessageKind::Reasoning
                && message.content == "hidden"
        }));
        assert!(messages.iter().any(|message| {
            message.kind == crate::session_manager::SessionMessageKind::Text
                && message.content == "answer"
        }));
    }
}
