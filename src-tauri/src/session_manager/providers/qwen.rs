use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{clean_prompt_text, messages_from_content, read_jsonl, walk_files};

const PROVIDER_ID: &str = "qwen";

pub fn session_roots() -> Vec<PathBuf> {
    vec![paths::qwen_dir().join("projects")]
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let mut sessions = HashMap::<String, SessionMeta>::new();
    for path in walk_files(&session_roots()[0], |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("jsonl")
    }) {
        let Ok(session) = parse_session(&path) else {
            continue;
        };
        match sessions.get(&session.session_id) {
            Some(existing)
                if existing.last_active_at.unwrap_or(0) >= session.last_active_at.unwrap_or(0) => {}
            _ => {
                sessions.insert(session.session_id.clone(), session);
            }
        }
    }
    sessions.into_values().collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let values = read_jsonl(path)?;
    Ok(values
        .iter()
        .flat_map(|value| {
            let record_type = value.get("type").and_then(Value::as_str)?;
            let role = match record_type {
                "user" => "user",
                "assistant" => "assistant",
                _ => return None,
            };
            let content = value.get("message")?.get("content")?;
            let ts = value
                .get("timestamp")
                .and_then(super::utils::parse_timestamp_to_ms);
            if role == "user" {
                if let Some(prompt) = value
                    .get("humanInput")
                    .and_then(|input| input.get("text"))
                    .and_then(Value::as_str)
                    .filter(|text| !text.trim().is_empty())
                {
                    Some(messages_from_content(
                        "user",
                        &Value::String(prompt.trim().to_string()),
                        ts,
                    ))
                } else {
                    let mut parts = super::common::normalize_content_parts(content);
                    for part in &mut parts {
                        if part.kind == super::common::ContentPartKind::Text {
                            part.content = clean_prompt_text(&part.content);
                        }
                    }
                    Some(super::common::messages_from_parts("user", &parts, ts))
                }
            } else {
                Some(messages_from_content(role, content, ts))
            }
        })
        .flatten()
        .collect())
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
        .unwrap_or(stem)
        .to_string();
    if session_id != stem {
        return Err("auxiliary transcript".to_string());
    }
    let messages = load_messages(path)?;
    let title = messages
        .iter()
        .find(|message| message.role == "user")
        .map(|message| super::utils::truncate_summary(&message.content, 80));
    let project_dir = workspace_dir(&values);
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
        project_dir,
        created_at,
        last_active_at: last_active_at.or(created_at),
        source_path: Some(path.display().to_string()),
        resume_command: None,
        can_delete: false,
    })
}

fn workspace_dir(values: &[Value]) -> Option<String> {
    values.iter().find_map(|value| {
        let directories = value.get("directories")?.as_array()?;
        directories.iter().find_map(|directory| {
            let directory = directory.as_str()?;
            (!directory.replace('\\', "/").contains("/.qwenworkcn/")).then(|| directory.to_string())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_qwen_human_input_and_skips_injected_text() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("session.jsonl");
        fs::write(
            &path,
            [
                r#"{"type":"user","humanInput":{"text":"hello"},"message":{"content":[{"type":"text","text":"<system-reminder>ctx</system-reminder>"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"answer"},{"type":"thinking","text":"hidden"}]}}"#,
            ]
            .join("\n"),
        )
        .expect("write");
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].content, "answer\n\nhidden");
    }
}
