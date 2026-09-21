use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{messages_from_content, read_jsonl, walk_files};

const PROVIDER_ID: &str = "cursor";

pub fn session_roots() -> Vec<PathBuf> {
    vec![paths::cursor_dir().join("projects")]
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    walk_files(&session_roots()[0], is_transcript)
        .into_iter()
        .filter_map(|path| parse_session(&path).ok())
        .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let values = read_jsonl(path)?;
    let messages = values
        .iter()
        .flat_map(|value| {
            let role = value.get("role").and_then(Value::as_str)?;
            if role != "user" && role != "assistant" {
                return None;
            }
            let content = value.get("message")?.get("content")?;
            Some(messages_from_content(role, content, None))
        })
        .flatten()
        .filter_map(|mut message| {
            message.content = if message.role == "user" {
                strip_cursor_wrappers(&message.content)
            } else if message.role == "assistant" {
                message.content.replace("[REDACTED]", "").trim().to_string()
            } else {
                message.content
            };
            (!message.content.is_empty()).then_some(message)
        })
        .collect();
    Ok(messages)
}

fn parse_session(path: &Path) -> Result<SessionMeta, String> {
    let session_id = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let messages = load_messages(path)?;
    let title = messages
        .iter()
        .find(|message| message.role == "user")
        .map(|message| super::utils::truncate_summary(&message.content, 80))
        .filter(|title| !title.is_empty());
    let created_at = messages.iter().find_map(|message| {
        super::common::between_tags(&message.content, "<timestamp>", "</timestamp>")
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value.trim()).ok())
            .map(|value| value.timestamp_millis())
    });
    let metadata = fs::metadata(path).map_err(|error| {
        format!(
            "Failed to inspect Cursor transcript {}: {error}",
            path.display()
        )
    })?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64);

    Ok(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id,
        title,
        summary: None,
        project_dir: cursor_project_slug(path),
        created_at,
        last_active_at: mtime.or(created_at),
        source_path: Some(path.display().to_string()),
        resume_command: None,
        can_delete: false,
    })
}

fn strip_cursor_wrappers(text: &str) -> String {
    let without_timestamp = super::common::strip_tagged_blocks(text, &["timestamp"]);
    super::common::between_tags(&without_timestamp, "<user_query>", "</user_query>")
        .unwrap_or(without_timestamp)
        .replace("</user_query>", "")
        .replace("<user_query>", "")
        .trim()
        .to_string()
}

fn cursor_project_slug(path: &Path) -> Option<String> {
    let components = path.components().collect::<Vec<_>>();
    let index = components
        .iter()
        .position(|component| component.as_os_str() == "projects")?;
    components
        .get(index + 1)
        .map(|component| component.as_os_str().to_string_lossy().to_string())
}

fn is_transcript(path: &Path) -> bool {
    if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
        return false;
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    if parent
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        != Some("agent-transcripts")
    {
        return false;
    }
    path.file_stem() == parent.file_name()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_cursor_prompt_wrappers() {
        assert_eq!(
            strip_cursor_wrappers(
                "<timestamp>2026-01-01T00:00:00Z</timestamp>\n<user_query>hello</user_query>"
            ),
            "hello"
        );
    }

    #[test]
    fn visible_messages_skip_tools() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("cursor.jsonl");
        fs::write(
            &path,
            [
                json!({"role":"user","message":{"content":[{"type":"text","text":"<user_query>hello</user_query>"}]}}),
                json!({"role":"assistant","message":{"content":[{"type":"tool_use","name":"read"},{"type":"text","text":"answer"}]}}),
            ]
            .into_iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        )
        .expect("write");
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].role, "tool");
        assert_eq!(messages[2].content, "answer");
    }
}
