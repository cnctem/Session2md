use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{
    between_tags, clean_prompt_text, messages_from_content, read_jsonl, visible_content_text,
    walk_files, ContentPart,
};

const PROVIDER_ID: &str = "workbuddy";

pub fn session_roots() -> Vec<PathBuf> {
    vec![paths::workbuddy_dir().join("projects")]
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    walk_files(&session_roots()[0], |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("jsonl")
    })
    .into_iter()
    .filter_map(|path| parse_session(&path).ok())
    .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let values = read_jsonl(path)?;
    let mut messages = Vec::new();
    for value in &values {
        let ts = value
            .get("timestamp")
            .and_then(super::utils::parse_timestamp_to_ms);
        match value.get("type").and_then(Value::as_str) {
            Some("message") => {
                let Some(role) = value.get("role").and_then(Value::as_str) else {
                    continue;
                };
                if role == "user" {
                    let raw = value
                        .get("content")
                        .map(visible_content_text)
                        .unwrap_or_default();
                    let prompt = workbuddy_prompt(&raw);
                    if !prompt.is_empty() {
                        messages.extend(messages_from_content("user", &Value::String(prompt), ts));
                    }
                } else {
                    messages.extend(messages_from_content(
                        role,
                        value.get("content").unwrap_or(&Value::Null),
                        ts,
                    ));
                }
            }
            Some("reasoning") => {
                let parts = value
                    .get("rawContent")
                    .map(super::common::normalize_content_parts)
                    .unwrap_or_default();
                messages.extend(super::common::messages_from_parts("assistant", &parts, ts));
            }
            Some("function_call") => {
                let name = value
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let arguments = value
                    .get("arguments")
                    .map(|value| match value {
                        Value::String(value) => value.clone(),
                        _ => value.to_string(),
                    })
                    .unwrap_or_default();
                messages.extend(super::common::messages_from_parts(
                    "tool",
                    &[ContentPart::tool_call_with_metadata(
                        name,
                        arguments,
                        value
                            .get("call_id")
                            .or_else(|| value.get("callId"))
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    )],
                    ts,
                ));
            }
            Some("function_call_result") => {
                let content = value
                    .get("output")
                    .map(ToString::to_string)
                    .unwrap_or_default();
                messages.extend(super::common::messages_from_parts(
                    "tool",
                    &[ContentPart::tool_result_with_metadata(
                        content,
                        value
                            .get("call_id")
                            .or_else(|| value.get("callId"))
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        value
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    )],
                    ts,
                ));
            }
            _ => {}
        }
    }
    Ok(messages)
}

fn workbuddy_prompt(text: &str) -> String {
    between_tags(text, "<user_query>", "</user_query>").unwrap_or_else(|| clean_prompt_text(text))
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
    let messages = load_messages(path)?;
    let title = messages
        .iter()
        .find(|message| message.role == "user")
        .map(|message| super::utils::truncate_summary(&message.content, 80));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_workbuddy_user_query_and_visible_reply() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("session.jsonl");
        fs::write(
            &path,
            [
                r#"{"type":"message","role":"user","content":[{"type":"input_text","text":"<system-reminder>ctx</system-reminder><user_query>hello</user_query>"}]}"#,
                r#"{"type":"message","role":"assistant","content":[{"type":"output_text","text":"answer"}]}"#,
            ]
            .join("\n"),
        )
        .expect("write");
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].content, "answer");
    }
}
