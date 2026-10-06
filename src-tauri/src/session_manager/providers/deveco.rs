use std::path::PathBuf;

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{self, normalize_content_parts, ContentPart};
use super::opencode_family::{self, Family};

const PROVIDER_ID: &str = "deveco";

pub fn database_path() -> PathBuf {
    paths::deveco_dir().join("deveco.db")
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    opencode_family::scan_database(&database_path(), PROVIDER_ID, Family::Deveco, true)
}

pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    if let Some(messages) = load_v2_messages(source)? {
        return Ok(messages);
    }
    opencode_family::load_database(source, PROVIDER_ID)
}

pub fn delete_session(session_id: &str, source: &str) -> Result<bool, String> {
    opencode_family::delete_database(
        session_id,
        source,
        &database_path(),
        PROVIDER_ID,
        Family::Deveco,
    )
}

fn load_v2_messages(source: &str) -> Result<Option<Vec<SessionMessage>>, String> {
    let (db_path, session_id) = common::parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid {PROVIDER_ID} SQLite source reference: {source}"))?;
    let conn = common::open_sqlite_readonly(&db_path)?;
    if common::table_column_names(&conn, "session_message").is_empty() {
        return Ok(None);
    }

    let mut statement = conn
        .prepare(
            "SELECT id, type, time_created, data
             FROM session_message
             WHERE session_id = ?1
             ORDER BY seq, id",
        )
        .map_err(|error| format!("Failed to query {PROVIDER_ID} V2 messages: {error}"))?;
    let rows = statement
        .query_map([session_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<i64>>(2)?.unwrap_or_default(),
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| format!("Failed to read {PROVIDER_ID} V2 messages: {error}"))?;

    let mut messages = Vec::new();
    let mut has_conversation_content = false;
    for row in rows.flatten() {
        let (_, message_type, timestamp, raw) = row;
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let ts = (timestamp > 0).then_some(timestamp);

        match message_type.as_str() {
            "user" => {
                let parts = user_parts(&value);
                if !parts.is_empty() {
                    has_conversation_content = true;
                }
                messages.extend(common::messages_from_parts("user", &parts, ts));
            }
            "assistant" => {
                let parts = value
                    .get("content")
                    .map(normalize_content_parts)
                    .unwrap_or_default();
                if !parts.is_empty() {
                    has_conversation_content = true;
                }
                messages.extend(common::messages_from_parts("assistant", &parts, ts));
            }
            "system" | "synthetic" => {
                if let Some(text) = value.get("text").and_then(Value::as_str) {
                    messages.extend(common::messages_from_parts(
                        "system",
                        &[ContentPart::system(text)],
                        ts,
                    ));
                }
            }
            "shell" => {
                let parts = shell_parts(&value);
                if !parts.is_empty() {
                    has_conversation_content = true;
                }
                messages.extend(common::messages_from_parts("tool", &parts, ts));
            }
            "compaction" => {
                if let Some(summary) = value.get("summary").and_then(Value::as_str) {
                    messages.extend(common::messages_from_parts(
                        "system",
                        &[ContentPart::system(summary)],
                        ts,
                    ));
                }
            }
            _ => {}
        }
    }

    if has_conversation_content && !messages.is_empty() {
        Ok(Some(messages))
    } else {
        Ok(None)
    }
}

fn user_parts(value: &Value) -> Vec<ContentPart> {
    let mut parts = Vec::new();
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        if !text.trim().is_empty() {
            parts.push(ContentPart::text(text));
        }
    }
    if let Some(files) = value.get("files").and_then(Value::as_array) {
        for file in files {
            let name = file
                .get("name")
                .or_else(|| file.get("uri"))
                .and_then(Value::as_str)
                .unwrap_or("Attachment");
            parts.push(ContentPart::attachment(format!("[Attachment: {name}]")));
        }
    }
    parts
}

fn shell_parts(value: &Value) -> Vec<ContentPart> {
    let call_id = value
        .get("callID")
        .or_else(|| value.get("callId"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let command = value
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let output = value
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut parts = Vec::new();
    if !command.trim().is_empty() {
        parts.push(ContentPart::tool_call_with_metadata(
            "shell",
            command,
            call_id.clone(),
        ));
    }
    if !output.trim().is_empty() {
        parts.push(ContentPart::tool_result_with_metadata(
            output,
            call_id,
            Some("shell".to_string()),
        ));
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn create_db(path: &std::path::Path) {
        let conn = Connection::open(path).expect("db");
        conn.execute_batch(
            "CREATE TABLE session (
                id TEXT PRIMARY KEY,
                title TEXT,
                directory TEXT,
                parent_id TEXT,
                time_created INTEGER,
                time_updated INTEGER
             );
             CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT,
                time_created INTEGER,
                time_updated INTEGER,
                data TEXT
             );
             CREATE TABLE part (
                id TEXT PRIMARY KEY,
                message_id TEXT,
                session_id TEXT,
                time_created INTEGER,
                time_updated INTEGER,
                data TEXT
             );
             CREATE TABLE session_message (
                id TEXT PRIMARY KEY,
                session_id TEXT,
                type TEXT,
                seq INTEGER,
                time_created INTEGER,
                time_updated INTEGER,
                data TEXT
             );",
        )
        .expect("schema");
        conn.execute(
            "INSERT INTO session VALUES ('ses-v2','V2 demo','/tmp/project',NULL,1000,3000)",
            [],
        )
        .expect("session");
    }

    #[test]
    fn loads_v2_user_and_assistant_messages() {
        let root = tempdir().expect("root");
        let path = root.path().join("deveco.db");
        create_db(&path);
        let conn = Connection::open(&path).expect("db");
        conn.execute(
            "INSERT INTO session_message VALUES (
                'msg-user','ses-v2','user',1,1000,1000,
                '{\"text\":\"hello\",\"files\":[{\"name\":\"notes.md\"}]}'
             )",
            [],
        )
        .expect("user message");
        conn.execute(
            "INSERT INTO session_message VALUES (
                'msg-assistant','ses-v2','assistant',2,2000,2000,
                '{\"content\":[{\"type\":\"text\",\"text\":\"answer\"},{\"type\":\"reasoning\",\"text\":\"think\"}]}'
             )",
            [],
        )
        .expect("assistant message");
        drop(conn);

        let sessions = opencode_family::scan_database(&path, PROVIDER_ID, Family::Deveco, true);
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].can_delete);

        let source = format!("sqlite:{}#ses-v2", path.display());
        let messages = load_messages(&source).expect("messages");

        assert!(messages.iter().any(|message| message.content == "hello"));
        assert!(messages.iter().any(|message| message.content == "answer"));
        assert!(messages.iter().any(|message| {
            message.kind == crate::session_manager::SessionMessageKind::Reasoning
                && message.content == "think"
        }));
    }

    #[test]
    fn falls_back_to_legacy_messages_when_v2_has_only_control_rows() {
        let root = tempdir().expect("root");
        let path = root.path().join("deveco.db");
        create_db(&path);
        let conn = Connection::open(&path).expect("db");
        conn.execute(
            "INSERT INTO message VALUES ('msg-1','ses-v2',1000,1000,'{\"role\":\"user\"}')",
            [],
        )
        .expect("legacy message");
        conn.execute(
            "INSERT INTO part VALUES ('part-1','msg-1','ses-v2',1000,1000,'{\"type\":\"text\",\"text\":\"legacy\"}')",
            [],
        )
        .expect("legacy part");
        conn.execute(
            "INSERT INTO session_message VALUES (
                'control-1','ses-v2','model-switched',1,2000,2000,
                '{\"model\":{\"providerID\":\"p\",\"modelID\":\"m\"}}'
             )",
            [],
        )
        .expect("control message");
        drop(conn);

        let source = format!("sqlite:{}#ses-v2", path.display());
        let messages = load_messages(&source).expect("messages");

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "legacy");
    }
}
