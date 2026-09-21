use std::io::Read;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{
    messages_from_parts, open_sqlite_readonly, row_to_json, sqlite_source, table_column_names,
    ContentPart,
};

const PROVIDER_ID: &str = "zed";
const MAX_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;

pub fn database_path() -> PathBuf {
    paths::zed_data_dir().join("threads").join("threads.db")
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    scan_database(&database_path()).unwrap_or_default()
}

pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    let (db_path, session_id) = super::common::parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid Zed SQLite source reference: {source}"))?;
    let thread = read_thread(&db_path, &session_id)?;
    Ok(visible_messages(&thread))
}

fn scan_database(path: &Path) -> Result<Vec<SessionMeta>, String> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let conn = open_sqlite_readonly(path)?;
    let columns = table_column_names(&conn, "threads");
    if columns.is_empty() {
        return Ok(Vec::new());
    }
    let order = if columns.iter().any(|column| column == "updated_at") {
        "updated_at"
    } else {
        "rowid"
    };
    let mut stmt = conn
        .prepare(&format!("SELECT * FROM threads ORDER BY {order} DESC"))
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok(row_to_json(row, &columns)))
        .map_err(|error| error.to_string())?;
    let mut sessions = Vec::new();
    for row in rows.flatten() {
        if row
            .get("parent_id")
            .and_then(Value::as_str)
            .is_some_and(|parent| !parent.is_empty())
        {
            continue;
        }
        let Some(id) = row.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Ok(thread) = read_thread(path, id) else {
            continue;
        };
        let messages = visible_messages(&thread);
        let first_user = messages
            .iter()
            .find(|message| message.role == "user")
            .map(|message| super::utils::truncate_summary(&message.content, 160));
        let title = row
            .get("summary")
            .and_then(Value::as_str)
            .filter(|title| !title.trim().is_empty())
            .or_else(|| thread.get("title").and_then(Value::as_str))
            .map(|title| super::utils::truncate_summary(title, 80))
            .or_else(|| first_user.clone());
        sessions.push(SessionMeta {
            provider_id: PROVIDER_ID.to_string(),
            session_id: id.to_string(),
            title,
            summary: first_user,
            project_dir: folder_path(&row),
            created_at: row
                .get("created_at")
                .and_then(super::utils::parse_timestamp_to_ms),
            last_active_at: row
                .get("updated_at")
                .and_then(super::utils::parse_timestamp_to_ms),
            source_path: Some(sqlite_source(path, id)),
            resume_command: None,
            can_delete: false,
        });
    }
    Ok(sessions)
}

fn read_thread(path: &Path, id: &str) -> Result<Value, String> {
    let conn = open_sqlite_readonly(path)?;
    let (data_type, bytes): (String, Vec<u8>) = conn
        .query_row(
            "SELECT data_type, data FROM threads WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| format!("Failed to read Zed thread {id}: {error}"))?;
    let raw = if data_type.eq_ignore_ascii_case("zstd") {
        let decoder = ruzstd::decoding::StreamingDecoder::new(std::io::Cursor::new(bytes))
            .map_err(|error| format!("Failed to initialize Zed zstd decoder: {error}"))?;
        let mut limited = decoder.take(MAX_DECOMPRESSED_BYTES + 1);
        let mut output = Vec::new();
        limited
            .read_to_end(&mut output)
            .map_err(|error| format!("Failed to decompress Zed thread {id}: {error}"))?;
        if output.len() as u64 > MAX_DECOMPRESSED_BYTES {
            return Err(format!("Zed thread {id} exceeds the decompression limit"));
        }
        output
    } else {
        bytes
    };
    serde_json::from_slice(&raw)
        .map_err(|error| format!("Failed to parse Zed thread {id}: {error}"))
}

fn visible_messages(thread: &Value) -> Vec<SessionMessage> {
    let Some(messages) = thread.get("messages").and_then(Value::as_array) else {
        return Vec::new();
    };
    let version = thread.get("version").and_then(Value::as_str).unwrap_or("");
    if version == "0.3.0" {
        return visible_v3_messages(messages);
    }
    visible_legacy_messages(messages)
}

fn visible_v3_messages(messages: &[Value]) -> Vec<SessionMessage> {
    let mut output = Vec::new();
    for message in messages {
        if let Some(user) = message.get("User") {
            let parts = user
                .get("content")
                .and_then(Value::as_array)
                .map(|blocks| zed_block_parts(blocks))
                .unwrap_or_default();
            output.extend(messages_from_parts("user", &parts, None));
        } else if let Some(agent) = message.get("Agent") {
            let mut parts = agent
                .get("content")
                .and_then(Value::as_array)
                .map(|blocks| zed_block_parts(blocks))
                .unwrap_or_default();
            if let Some(results) = agent.get("tool_results").and_then(Value::as_object) {
                for (id, result) in results {
                    let content = result
                        .get("content")
                        .or_else(|| result.get("output"))
                        .map(ToString::to_string)
                        .unwrap_or_else(|| format!("[Tool result: {id}]"));
                    parts.push(ContentPart::tool_result_with_metadata(
                        content,
                        Some(id.clone()),
                        None,
                    ));
                }
            }
            output.extend(messages_from_parts("assistant", &parts, None));
        }
    }
    output
}

fn visible_legacy_messages(messages: &[Value]) -> Vec<SessionMessage> {
    let mut output = Vec::new();
    for message in messages {
        if message.get("is_visible").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        let mut parts = message
            .get("segments")
            .and_then(Value::as_array)
            .map(|segments| zed_block_parts(segments))
            .unwrap_or_default();
        if let Some(tool_uses) = message.get("tool_uses").and_then(Value::as_array) {
            for call in tool_uses {
                let name = call
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let input = call
                    .get("input")
                    .map(ToString::to_string)
                    .unwrap_or_default();
                parts.push(ContentPart::tool_call_with_metadata(
                    name,
                    input,
                    call.get("id").and_then(Value::as_str).map(str::to_string),
                ));
            }
        }
        if let Some(results) = message.get("tool_results").and_then(Value::as_array) {
            for result in results {
                let content = result
                    .get("content")
                    .or_else(|| result.get("output"))
                    .map(ToString::to_string)
                    .unwrap_or_default();
                parts.push(ContentPart::tool_result_with_metadata(
                    content,
                    result
                        .get("tool_use_id")
                        .or_else(|| result.get("toolCallId"))
                        .or_else(|| result.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    result
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                ));
            }
        }
        output.extend(messages_from_parts(role, &parts, None));
    }
    output
}

fn zed_block_parts(blocks: &[Value]) -> Vec<ContentPart> {
    let mut parts = Vec::new();
    for block in blocks {
        if let Some(text) = block.get("Text").and_then(Value::as_str) {
            parts.push(ContentPart::text(text));
        } else if let Some(thinking) = block.get("Thinking") {
            let text = thinking
                .get("text")
                .and_then(Value::as_str)
                .or_else(|| block.get("text").and_then(Value::as_str))
                .unwrap_or("");
            parts.push(ContentPart::reasoning(text));
        } else if let Some(call) = block.get("ToolUse") {
            let name = call
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let input = call
                .get("raw_input")
                .or_else(|| call.get("input"))
                .map(ToString::to_string)
                .unwrap_or_default();
            parts.push(ContentPart::tool_call_with_metadata(
                name,
                input,
                call.get("id").and_then(Value::as_str).map(str::to_string),
            ));
        } else if block.get("type").and_then(Value::as_str) == Some("text") {
            parts.push(ContentPart::text(
                block.get("text").and_then(Value::as_str).unwrap_or(""),
            ));
        } else if block.get("type").and_then(Value::as_str) == Some("thinking") {
            parts.push(ContentPart::reasoning(
                block.get("text").and_then(Value::as_str).unwrap_or(""),
            ));
        }
    }
    parts
}

fn folder_path(row: &Value) -> Option<String> {
    let raw = row.get("folder_paths")?.as_str()?;
    let parsed: Value = serde_json::from_str(raw).ok()?;
    parsed
        .as_array()
        .and_then(|paths| paths.first())
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    #[test]
    fn reads_visible_zed_thread_text() {
        let root = tempdir().expect("root");
        let path = root.path().join("threads.db");
        let conn = Connection::open(&path).expect("db");
        conn.execute_batch(
            "CREATE TABLE threads (id TEXT, summary TEXT, updated_at TEXT, data_type TEXT, data BLOB, parent_id TEXT, folder_paths TEXT, created_at TEXT);",
        )
        .expect("schema");
        let thread = serde_json::json!({
            "version": "0.3.0",
            "messages": [
                {"User": {"content": [{"Text": "hello"}]}},
                {"Agent": {"content": [{"Thinking": {"text": "hidden"}}, {"Text": "answer"}]}}
            ]
        });
        conn.execute(
            "INSERT INTO threads VALUES (?1,?2,?3,'json',?4,NULL,?5,?3)",
            rusqlite::params![
                "zed-1",
                "Demo",
                "2026-01-01T00:00:00Z",
                thread.to_string().as_bytes(),
                "[\"/tmp/project\"]"
            ],
        )
        .expect("thread");
        let sessions = scan_database(&path).expect("scan");
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
