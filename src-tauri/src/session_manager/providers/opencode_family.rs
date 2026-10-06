use std::path::Path;
use std::time::Duration;

use rusqlite::{params_from_iter, Connection, OpenFlags, TransactionBehavior};
use serde_json::Value;

use crate::session_manager::{SessionMessage, SessionMeta};

use super::common::{
    messages_from_parts, normalize_content_parts, open_sqlite_readonly, row_to_json, sqlite_source,
    table_column_names, table_columns, ContentPart,
};

#[derive(Debug, Clone, Copy)]
pub enum Family {
    Deveco,
    Mimocode,
    Kilocode,
    Zcode,
    Teleagent,
}

pub fn scan_database(
    db_path: &Path,
    provider_id: &str,
    family: Family,
    can_delete: bool,
) -> Vec<SessionMeta> {
    let Ok(conn) = open_sqlite_readonly(db_path) else {
        return Vec::new();
    };
    let session_column_names = table_column_names(&conn, "session");
    let session_columns: std::collections::HashSet<String> =
        session_column_names.iter().cloned().collect();
    if session_column_names.is_empty() {
        return Vec::new();
    }
    let order_column = if session_columns.contains("time_created") {
        "time_created"
    } else {
        "rowid"
    };
    let query = format!("SELECT * FROM session ORDER BY {order_column}, id");
    let Ok(mut stmt) = conn.prepare(&query) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| Ok(row_to_json(row, &session_column_names))) else {
        return Vec::new();
    };
    rows.flatten()
        .filter(|session| !is_filtered_session(session, family))
        .filter_map(|session| session_to_meta(&conn, db_path, provider_id, &session, can_delete))
        .collect()
}

pub fn delete_database(
    session_id: &str,
    source: &str,
    expected_db_path: &Path,
    provider_id: &str,
    family: Family,
) -> Result<bool, String> {
    let (db_path, ref_session_id) = super::common::parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid {provider_id} SQLite source reference: {source}"))?;
    if ref_session_id != session_id {
        return Err(format!(
            "{provider_id} SQLite session ID mismatch: expected {session_id}, found {ref_session_id}"
        ));
    }

    let db_path = db_path
        .canonicalize()
        .map_err(|error| format!("Failed to canonicalize {provider_id} database path: {error}"))?;
    let expected_db_path = expected_db_path.canonicalize().map_err(|error| {
        format!("Failed to canonicalize expected {provider_id} database path: {error}")
    })?;
    if db_path != expected_db_path {
        return Err(format!(
            "SQLite path does not match expected {provider_id} database"
        ));
    }

    let mut conn = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|error| format!("Failed to open {provider_id} database: {error}"))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("Failed to configure {provider_id} database timeout: {error}"))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|error| format!("Failed to enable foreign keys: {error}"))?;
    let foreign_keys: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(|error| format!("Failed to verify foreign keys: {error}"))?;
    if foreign_keys != 1 {
        return Err("Refusing to delete session without foreign-key cascades".to_string());
    }

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Failed to begin {provider_id} delete transaction: {error}"))?;

    let session_ids = {
        let mut statement = tx
            .prepare(
                "WITH RECURSIVE descendants(id) AS (
                    SELECT id FROM session WHERE id = ?1
                    UNION
                    SELECT session.id
                    FROM session
                    JOIN descendants ON session.parent_id = descendants.id
                )
                SELECT id FROM descendants",
            )
            .map_err(|error| {
                format!("Failed to query {provider_id} session descendants: {error}")
            })?;
        let rows = statement
            .query_map([session_id], |row| row.get::<_, String>(0))
            .map_err(|error| {
                format!("Failed to read {provider_id} session descendants: {error}")
            })?;
        rows.filter_map(Result::ok).collect::<Vec<_>>()
    };

    if session_ids.is_empty() {
        return Ok(false);
    }

    if matches!(family, Family::Mimocode) {
        for (table, column) in [
            ("history_fts", "session_id"),
            ("external_import", "session_id"),
            ("claude_import", "session_id"),
        ] {
            delete_matching_rows(&tx, table, column, &session_ids, provider_id)?;
        }
    }

    delete_matching_rows(&tx, "event", "aggregate_id", &session_ids, provider_id)?;
    delete_matching_rows(
        &tx,
        "event_sequence",
        "aggregate_id",
        &session_ids,
        provider_id,
    )?;
    let deleted = delete_matching_rows(&tx, "session", "id", &session_ids, provider_id)?;

    tx.commit()
        .map_err(|error| format!("Failed to commit {provider_id} session deletion: {error}"))?;
    Ok(deleted > 0)
}

fn delete_matching_rows(
    conn: &Connection,
    table: &str,
    column: &str,
    ids: &[String],
    provider_id: &str,
) -> Result<usize, String> {
    if !table_columns(conn, table).contains(column) {
        return Ok(0);
    }

    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!("DELETE FROM {table} WHERE {column} IN ({placeholders})");
    conn.execute(&sql, params_from_iter(ids.iter().map(String::as_str)))
        .map_err(|error| format!("Failed to delete {provider_id} rows from {table}: {error}"))
}

pub fn load_database(source: &str, provider_id: &str) -> Result<Vec<SessionMessage>, String> {
    let (db_path, session_id) = super::common::parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid {provider_id} SQLite source reference: {source}"))?;
    let conn = open_sqlite_readonly(&db_path)?;
    let mut statement = conn
        .prepare("SELECT id, time_created, data FROM message WHERE session_id = ?1 ORDER BY time_created, id")
        .map_err(|error| format!("Failed to query {provider_id} messages: {error}"))?;
    let rows = statement
        .query_map([session_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("Failed to read {provider_id} messages: {error}"))?;

    let part_columns = table_columns(&conn, "part");
    let part_order = if part_columns.contains("time_created") {
        "time_created"
    } else {
        "rowid"
    };
    let part_query = format!(
        "SELECT message_id, data FROM part WHERE session_id = ?1 ORDER BY {part_order}, id"
    );
    let mut part_statement = conn
        .prepare(&part_query)
        .map_err(|error| format!("Failed to query {provider_id} parts: {error}"))?;
    let part_rows = part_statement
        .query_map([session_id.as_str()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("Failed to read {provider_id} parts: {error}"))?;
    let mut parts_by_message = std::collections::HashMap::<String, Vec<String>>::new();
    for row in part_rows.flatten() {
        parts_by_message.entry(row.0).or_default().push(row.1);
    }

    let mut messages = Vec::new();
    for row in rows.flatten() {
        let Ok(message) = serde_json::from_str::<Value>(&row.2) else {
            continue;
        };
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        let mut parts = Vec::<ContentPart>::new();
        for raw_part in parts_by_message.get(&row.0).into_iter().flatten() {
            let Ok(part) = serde_json::from_str::<Value>(raw_part) else {
                continue;
            };
            match part.get("type").and_then(Value::as_str) {
                Some("tool") => {
                    let name = part
                        .get("tool")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let input = part
                        .get("state")
                        .and_then(|state| state.get("input"))
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "{}".to_string());
                    let tool_call_id = part
                        .get("callID")
                        .or_else(|| part.get("callId"))
                        .or_else(|| part.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    parts.push(ContentPart::tool_call_with_metadata(
                        name,
                        input,
                        tool_call_id.clone(),
                    ));
                    if let Some(output) = part.get("state").and_then(|state| state.get("output")) {
                        parts.push(ContentPart::tool_result_with_metadata(
                            output
                                .as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| output.to_string()),
                            tool_call_id,
                            Some(name.to_string()),
                        ));
                    }
                }
                Some("compaction") => {
                    if let Some(summary) = part
                        .get("summary")
                        .and_then(|summary| summary.get("body"))
                        .and_then(Value::as_str)
                    {
                        parts.push(ContentPart::system(summary));
                    }
                }
                _ => parts.extend(normalize_content_parts(&part)),
            }
        }
        messages.extend(messages_from_parts(
            role,
            &parts,
            (row.1 > 0).then_some(row.1),
        ));
    }
    Ok(messages)
}

fn session_to_meta(
    conn: &rusqlite::Connection,
    db_path: &Path,
    provider_id: &str,
    session: &Value,
    can_delete: bool,
) -> Option<SessionMeta> {
    let session_id = session.get("id").and_then(Value::as_str)?.to_string();
    let source = sqlite_source(db_path, &session_id);
    let first_user = first_user_text(conn, &session_id);
    let title = session
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
        .map(|title| super::utils::truncate_summary(title, 80))
        .or_else(|| first_user.clone());
    let project_dir = session
        .get("directory")
        .and_then(Value::as_str)
        .filter(|directory| !directory.trim().is_empty())
        .map(str::to_string);
    let created_at = session
        .get("time_created")
        .and_then(super::utils::parse_timestamp_to_ms);
    let updated_at = session
        .get("time_updated")
        .and_then(super::utils::parse_timestamp_to_ms)
        .or(created_at);
    Some(SessionMeta {
        provider_id: provider_id.to_string(),
        session_id,
        title,
        summary: first_user,
        project_dir,
        created_at,
        last_active_at: updated_at,
        source_path: Some(source),
        resume_command: None,
        can_delete,
    })
}

fn first_user_text(conn: &rusqlite::Connection, session_id: &str) -> Option<String> {
    let mut stmt = conn
        .prepare("SELECT id, data FROM message WHERE session_id = ?1 ORDER BY time_created, id")
        .ok()?;
    let rows = stmt
        .query_map([session_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .ok()?;
    for row in rows.flatten() {
        let Ok(message) = serde_json::from_str::<Value>(&row.1) else {
            continue;
        };
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let mut part_stmt = conn
            .prepare("SELECT data FROM part WHERE message_id = ?1 ORDER BY time_created, id")
            .ok()?;
        let part_rows = part_stmt
            .query_map([row.0.as_str()], |part_row| part_row.get::<_, String>(0))
            .ok()?;
        let mut texts = Vec::new();
        for raw in part_rows.flatten() {
            let Ok(part) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            if part.get("type").and_then(Value::as_str) == Some("text") {
                let text = super::common::normalize_content_parts(&part)
                    .into_iter()
                    .map(|part| part.content)
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_string();
                if !text.is_empty() {
                    texts.push(text);
                }
            }
        }
        if !texts.is_empty() {
            return Some(super::utils::truncate_summary(&texts.join("\n"), 160));
        }
    }
    None
}

fn is_filtered_session(session: &Value, family: Family) -> bool {
    let title = session
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if matches!(family, Family::Kilocode | Family::Zcode)
        && session
            .get("parent_id")
            .and_then(Value::as_str)
            .is_some_and(|parent| !parent.is_empty())
    {
        return true;
    }
    if matches!(family, Family::Kilocode)
        && session
            .get("time_archived")
            .is_some_and(|value| !value.is_null())
    {
        return true;
    }
    if matches!(family, Family::Mimocode) {
        let lower = title.to_ascii_lowercase();
        if lower.starts_with("checkpoint-writer")
            || lower.starts_with("checkpoint writer")
            || lower.starts_with("auto dream")
            || lower.starts_with("auto distill")
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn create_db(path: &Path) {
        let conn = Connection::open(path).expect("db");
        conn.execute_batch(
            "CREATE TABLE session (id TEXT, title TEXT, directory TEXT, time_created INTEGER, time_updated INTEGER);
             CREATE TABLE message (id TEXT, session_id TEXT, time_created INTEGER, data TEXT);
             CREATE TABLE part (id TEXT, message_id TEXT, session_id TEXT, time_created INTEGER, data TEXT);",
        )
        .expect("schema");
        conn.execute(
            "INSERT INTO session VALUES ('ses-1','Demo','/tmp/project',1000,2000)",
            [],
        )
        .expect("session");
        conn.execute(
            "INSERT INTO message VALUES ('msg-1','ses-1',1000,'{\"role\":\"user\"}')",
            [],
        )
        .expect("user message");
        conn.execute(
            "INSERT INTO message VALUES ('msg-2','ses-1',2000,'{\"role\":\"assistant\"}')",
            [],
        )
        .expect("assistant message");
        conn.execute(
            "INSERT INTO part VALUES ('part-1','msg-1','ses-1',1000,'{\"type\":\"text\",\"text\":\"hello\"}')",
            [],
        )
        .expect("user part");
        conn.execute(
            "INSERT INTO part VALUES ('part-2','msg-2','ses-1',2000,'{\"type\":\"reasoning\",\"text\":\"hidden\"}')",
            [],
        )
        .expect("reasoning part");
        conn.execute(
            "INSERT INTO part VALUES ('part-3','msg-2','ses-1',2001,'{\"type\":\"text\",\"text\":\"answer\"}')",
            [],
        )
        .expect("assistant part");
    }

    fn create_delete_db(path: &Path) {
        let conn = Connection::open(path).expect("db");
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE session (
                id TEXT PRIMARY KEY,
                title TEXT,
                directory TEXT,
                parent_id TEXT,
                time_created INTEGER,
                time_updated INTEGER
             );
             CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT REFERENCES session(id) ON DELETE CASCADE,
                time_created INTEGER,
                data TEXT
             );
             CREATE TABLE part (
                id TEXT PRIMARY KEY,
                message_id TEXT REFERENCES message(id) ON DELETE CASCADE,
                session_id TEXT,
                time_created INTEGER,
                data TEXT
             );
             CREATE TABLE todo (
                session_id TEXT REFERENCES session(id) ON DELETE CASCADE,
                content TEXT
             );
             CREATE TABLE event_sequence (
                aggregate_id TEXT PRIMARY KEY,
                seq INTEGER NOT NULL
             );
             CREATE TABLE event (
                id TEXT PRIMARY KEY,
                aggregate_id TEXT REFERENCES event_sequence(aggregate_id) ON DELETE CASCADE,
                seq INTEGER NOT NULL,
                type TEXT NOT NULL,
                data TEXT NOT NULL
             );
             CREATE TABLE history_fts (
                part_id TEXT PRIMARY KEY,
                session_id TEXT,
                body TEXT
             );
             CREATE TABLE external_import (
                source TEXT,
                source_key TEXT,
                session_id TEXT,
                source_path TEXT
             );
             CREATE TABLE claude_import (
                source_uuid TEXT PRIMARY KEY,
                session_id TEXT,
                source_path TEXT
             );
             INSERT INTO session VALUES
                ('ses-root','Root','/tmp/root',NULL,1000,2000),
                ('ses-child','Child','/tmp/root','ses-root',1000,2000),
                ('ses-keep','Keep','/tmp/keep',NULL,1000,2000);
             INSERT INTO message VALUES
                ('msg-root','ses-root',1000,'{\"role\":\"user\"}'),
                ('msg-child','ses-child',1000,'{\"role\":\"user\"}'),
                ('msg-keep','ses-keep',1000,'{\"role\":\"user\"}');
             INSERT INTO part VALUES
                ('part-root','msg-root','ses-root',1000,'{\"type\":\"text\",\"text\":\"root\"}'),
                ('part-child','msg-child','ses-child',1000,'{\"type\":\"text\",\"text\":\"child\"}'),
                ('part-keep','msg-keep','ses-keep',1000,'{\"type\":\"text\",\"text\":\"keep\"}');
             INSERT INTO todo VALUES ('ses-root','do it'), ('ses-keep','keep it');
             INSERT INTO event_sequence VALUES ('ses-root',0), ('ses-child',0), ('ses-keep',0);
             INSERT INTO event VALUES
                ('event-root','ses-root',0,'session.created.1','{}'),
                ('event-child','ses-child',0,'session.created.1','{}'),
                ('event-keep','ses-keep',0,'session.created.1','{}');
             INSERT INTO history_fts VALUES
                ('part-root','ses-root','root'),
                ('part-child','ses-child','child'),
                ('part-keep','ses-keep','keep');
             INSERT INTO external_import VALUES
                ('opencode','root','ses-root','/tmp/source-root'),
                ('opencode','keep','ses-keep','/tmp/source-keep');
             INSERT INTO claude_import VALUES ('root','ses-root','/tmp/claude-root');",
        )
        .expect("schema");
    }

    #[test]
    fn reads_visible_text_from_opencode_family_database() {
        let root = tempdir().expect("root");
        let path = root.path().join("mimocode.db");
        create_db(&path);
        let sessions = scan_database(&path, "mimocode", Family::Mimocode, false);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].can_delete, false);
        let source = sessions[0].source_path.as_deref().expect("source");
        let messages = load_database(source, "mimocode").expect("messages");
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

    #[test]
    fn deletes_session_descendants_and_provider_owned_rows() {
        let root = tempdir().expect("root");
        let path = root.path().join("mimocode.db");
        create_delete_db(&path);

        let sessions = scan_database(&path, "mimocode", Family::Mimocode, true);
        assert_eq!(sessions.len(), 3);
        let source = sqlite_source(&path, "ses-root");
        assert!(
            delete_database("ses-root", &source, &path, "mimocode", Family::Mimocode,)
                .expect("delete session")
        );

        let conn = Connection::open(&path).expect("db");
        let remaining_sessions: i64 = conn
            .query_row("SELECT count(*) FROM session", [], |row| row.get(0))
            .expect("remaining sessions");
        let remaining_messages: i64 = conn
            .query_row("SELECT count(*) FROM message", [], |row| row.get(0))
            .expect("remaining messages");
        let remaining_parts: i64 = conn
            .query_row("SELECT count(*) FROM part", [], |row| row.get(0))
            .expect("remaining parts");
        let remaining_events: i64 = conn
            .query_row("SELECT count(*) FROM event", [], |row| row.get(0))
            .expect("remaining events");
        let remaining_history: i64 = conn
            .query_row("SELECT count(*) FROM history_fts", [], |row| row.get(0))
            .expect("remaining history");
        let remaining_imports: i64 = conn
            .query_row("SELECT count(*) FROM external_import", [], |row| row.get(0))
            .expect("remaining imports");
        let remaining_claude_imports: i64 = conn
            .query_row("SELECT count(*) FROM claude_import", [], |row| row.get(0))
            .expect("remaining claude imports");

        assert_eq!(remaining_sessions, 1);
        assert_eq!(remaining_messages, 1);
        assert_eq!(remaining_parts, 1);
        assert_eq!(remaining_events, 1);
        assert_eq!(remaining_history, 1);
        assert_eq!(remaining_imports, 1);
        assert_eq!(remaining_claude_imports, 0);
        assert_eq!(
            conn.query_row("SELECT id FROM session", [], |row| row.get::<_, String>(0))
                .expect("remaining session ID"),
            "ses-keep"
        );
    }
}
