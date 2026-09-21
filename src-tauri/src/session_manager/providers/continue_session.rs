use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{messages_from_content, read_json, walk_files};

const PROVIDER_ID: &str = "continue";
const MAX_SESSION_BYTES: u64 = 64 * 1024 * 1024;

pub fn session_roots() -> Vec<PathBuf> {
    vec![paths::continue_dir().join("sessions")]
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let root = &session_roots()[0];
    let index = read_index(&root.join("sessions.json"));
    walk_files(root, |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("json")
            && path.file_name().and_then(|name| name.to_str()) != Some("sessions.json")
    })
    .into_iter()
    .filter_map(|path| parse_session(&path, &index).ok())
    .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let value = read_json(path, MAX_SESSION_BYTES)?;
    let history = value
        .get("history")
        .and_then(Value::as_array)
        .ok_or_else(|| "Continue session has no history array".to_string())?;
    Ok(history
        .iter()
        .flat_map(|item| {
            let message = item.get("message")?;
            let role = message.get("role").and_then(Value::as_str)?;
            let normalized_role = match role {
                "thinking" => "assistant",
                "tool" => "tool",
                "user" | "assistant" => role,
                _ => return None,
            };
            let mut messages = messages_from_content(
                normalized_role,
                message.get("content").unwrap_or(&Value::Null),
                None,
            );
            if let Some(tool_calls) = message.get("toolCalls") {
                messages.extend(messages_from_content("assistant", tool_calls, None));
            }
            if let Some(reasoning) = item
                .get("reasoning")
                .and_then(|reasoning| reasoning.get("text"))
                .and_then(Value::as_str)
                .filter(|text| !text.trim().is_empty())
            {
                messages.extend(messages_from_content(
                    "assistant",
                    &Value::String(reasoning.to_string()),
                    None,
                ));
            }
            Some(messages)
        })
        .flatten()
        .collect())
}

fn parse_session(path: &Path, index: &HashMap<String, IndexEntry>) -> Result<SessionMeta, String> {
    let value = read_json(path, MAX_SESSION_BYTES)?;
    let session_id = value
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| path.file_stem()?.to_str().map(str::to_string))
        .ok_or_else(|| "missing Continue session id".to_string())?;
    let messages = load_messages(path)?;
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty() && *title != "New Session")
        .map(|title| super::utils::truncate_summary(title, 80))
        .or_else(|| {
            messages
                .iter()
                .find(|message| message.role == "user")
                .map(|message| super::utils::truncate_summary(&message.content, 80))
        });
    let indexed = index.get(&session_id);
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64);
    Ok(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id,
        title: title.or_else(|| indexed.and_then(|entry| entry.title.clone())),
        summary: None,
        project_dir: value
            .get("workspaceDirectory")
            .and_then(Value::as_str)
            .or_else(|| indexed.and_then(|entry| entry.cwd.as_deref()))
            .map(str::to_string),
        created_at: indexed.and_then(|entry| entry.created_at),
        last_active_at: indexed
            .and_then(|entry| entry.last_active_at)
            .or(mtime)
            .or_else(|| indexed.and_then(|entry| entry.created_at)),
        source_path: Some(path.display().to_string()),
        resume_command: None,
        can_delete: false,
    })
}

#[derive(Default)]
struct IndexEntry {
    title: Option<String>,
    cwd: Option<String>,
    created_at: Option<i64>,
    last_active_at: Option<i64>,
}

fn read_index(path: &Path) -> HashMap<String, IndexEntry> {
    let Ok(raw) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    let Ok(values) = serde_json::from_str::<Value>(&raw) else {
        return HashMap::new();
    };
    let Some(items) = values.as_array() else {
        return HashMap::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("sessionId").and_then(Value::as_str)?.to_string();
            let entry = IndexEntry {
                title: item
                    .get("title")
                    .and_then(Value::as_str)
                    .map(|title| super::utils::truncate_summary(title, 80)),
                cwd: item
                    .get("workspaceDirectory")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                created_at: item
                    .get("dateCreated")
                    .and_then(super::utils::parse_timestamp_to_ms),
                last_active_at: item
                    .get("dateLastActivity")
                    .or_else(|| item.get("lastMessageAt"))
                    .and_then(super::utils::parse_timestamp_to_ms),
            };
            Some((id, entry))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_continue_user_and_assistant_text() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("session.json");
        fs::write(
            &path,
            serde_json::json!({
                "sessionId": "continue-1",
                "title": "Demo",
                "workspaceDirectory": "/tmp/project",
                "history": [
                    {"message": {"role": "user", "content": "hello"}},
                    {"message": {"role": "assistant", "content": [{"type": "text", "text": "answer"}]}},
                    {"message": {"role": "thinking", "content": "hidden"}}
                ]
            })
            .to_string(),
        )
        .expect("write");
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].content, "answer");
        assert_eq!(messages[2].content, "hidden");
    }
}
