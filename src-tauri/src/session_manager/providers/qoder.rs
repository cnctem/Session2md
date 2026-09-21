use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{messages_from_content, read_jsonl, walk_files};

const PROVIDER_ID: &str = "qoder";

pub fn session_roots() -> Vec<PathBuf> {
    vec![paths::qoder_dir().join("projects")]
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    walk_files(&session_roots()[0], is_main_transcript)
        .into_iter()
        .filter_map(|path| parse_session(&path).ok())
        .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let values = read_jsonl(path)?;
    let messages = values.iter().flat_map(visible_records).collect();
    Ok(messages)
}

fn visible_records(value: &Value) -> Vec<SessionMessage> {
    let Some(record_type) = value.get("type").and_then(Value::as_str) else {
        return Vec::new();
    };
    let role = match record_type {
        "user" => "user",
        "assistant" => "assistant",
        _ => return Vec::new(),
    };
    let Some(content) = value
        .get("message")
        .and_then(|message| message.get("content"))
    else {
        return Vec::new();
    };
    messages_from_content(
        role,
        content,
        value
            .get("timestamp")
            .and_then(super::utils::parse_timestamp_to_ms),
    )
}

fn parse_session(path: &Path) -> Result<SessionMeta, String> {
    let values = read_jsonl(path)?;
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let session_id = values
        .iter()
        .find_map(|value| value.get("sessionId").and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .unwrap_or(stem)
        .to_string();
    if session_id != stem {
        return Err("auxiliary transcript".to_string());
    }
    let messages = load_messages(path)?;
    let title = values
        .iter()
        .rev()
        .find_map(|value| {
            (value.get("type").and_then(Value::as_str) == Some("ai-title"))
                .then(|| value.get("aiTitle").and_then(Value::as_str))
                .flatten()
        })
        .or_else(|| {
            values.iter().rev().find_map(|value| {
                (value.get("type").and_then(Value::as_str) == Some("last-prompt"))
                    .then(|| value.get("lastPrompt").and_then(Value::as_str))
                    .flatten()
            })
        })
        .map(|title| super::utils::truncate_summary(title, 80))
        .or_else(|| {
            messages
                .iter()
                .find(|message| message.role == "user")
                .map(|message| super::utils::truncate_summary(&message.content, 80))
        });
    let cwd = values
        .iter()
        .find_map(|value| value.get("cwd").and_then(Value::as_str))
        .map(str::to_string);
    let created_at = messages.first().and_then(|message| message.ts);
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    let last_active_at = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64);
    Ok(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id,
        title,
        summary: None,
        project_dir: cwd,
        created_at,
        last_active_at: last_active_at.or(created_at),
        source_path: Some(path.display().to_string()),
        resume_command: None,
        can_delete: false,
    })
}

fn is_main_transcript(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("jsonl")
        && !path
            .to_string_lossy()
            .replace('\\', "/")
            .contains("/subagents/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_visible_qoder_text() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("qoder-1.jsonl");
        fs::write(
            &path,
            [
                r#"{"type":"user","message":{"content":"hello"}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"answer"},{"type":"tool_use","name":"read"}]}}"#,
            ]
            .join("\n"),
        )
        .expect("write");
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].content, "answer");
        assert_eq!(messages[2].role, "tool");
    }
}
