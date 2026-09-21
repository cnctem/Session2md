use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{
    messages_from_content, messages_from_parts, normalize_content_parts, read_jsonl,
    strip_tagged_blocks, walk_files, ContentPart,
};

const PROVIDER_ID: &str = "antigravity";

pub fn session_roots() -> Vec<PathBuf> {
    vec![paths::antigravity_dir()]
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let root = &session_roots()[0];
    let conversations = root.join("conversations");
    walk_files(&conversations, |path| {
        path.extension().and_then(|extension| extension.to_str()) == Some("db")
    })
    .into_iter()
    .filter_map(|db| {
        let id = db.file_stem()?.to_str()?.to_string();
        parse_session(root, &id).ok()
    })
    .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let values = read_jsonl(path)?;
    let mut messages = Vec::new();
    for value in &values {
        let ts = value
            .get("created_at")
            .and_then(super::utils::parse_timestamp_to_ms);
        match value.get("type").and_then(Value::as_str) {
            Some("USER_INPUT") | Some("PLANNER_RESPONSE") => {
                let role = if value.get("type").and_then(Value::as_str) == Some("USER_INPUT") {
                    "user"
                } else {
                    "assistant"
                };
                let mut parts = value
                    .get("content")
                    .map(normalize_content_parts)
                    .unwrap_or_default();
                if let Some(thinking) = value.get("thinking").and_then(Value::as_str) {
                    parts.push(ContentPart::reasoning(thinking));
                }
                if let Some(tool_calls) = value.get("tool_calls") {
                    for call in tool_calls.as_array().into_iter().flatten() {
                        let name = call
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown");
                        let args = call
                            .get("args")
                            .map(ToString::to_string)
                            .unwrap_or_default();
                        parts.push(ContentPart::tool_call(format!("[Tool: {name}]\n{args}")));
                    }
                }
                messages.extend(messages_from_parts(role, &parts, ts));
            }
            Some("GENERIC") if value.get("status").and_then(Value::as_str) == Some("DONE") => {
                messages.extend(messages_from_content(
                    "tool",
                    value.get("content").unwrap_or(&Value::Null),
                    ts,
                ));
            }
            Some("ERROR_MESSAGE") => {
                messages.extend(messages_from_content(
                    "system",
                    value.get("content").unwrap_or(&Value::Null),
                    ts,
                ));
            }
            _ => {}
        }
    }
    Ok(messages)
}

fn parse_session(root: &Path, id: &str) -> Result<SessionMeta, String> {
    let transcript = root
        .join("brain")
        .join(id)
        .join(".system_generated")
        .join("logs")
        .join("transcript.jsonl");
    if !transcript.is_file() {
        return Err(format!("Antigravity transcript not found for {id}"));
    }
    let messages = load_messages(&transcript)?;
    let title =
        annotation_title(&root.join("annotations").join(format!("{id}.pbtxt"))).or_else(|| {
            messages
                .iter()
                .find(|message| message.role == "user")
                .map(|message| super::utils::truncate_summary(&message.content, 80))
        });
    let metadata = fs::metadata(&transcript).map_err(|error| error.to_string())?;
    let last_active_at = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64);
    Ok(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: id.to_string(),
        title,
        summary: None,
        project_dir: None,
        created_at: messages.first().and_then(|message| message.ts),
        last_active_at,
        source_path: Some(transcript.display().to_string()),
        resume_command: None,
        can_delete: false,
    })
}

fn annotation_title(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let title = raw
        .lines()
        .find_map(|line| line.trim().strip_prefix("title:"))?
        .trim()
        .trim_matches('"')
        .replace("\\\"", "\"");
    let title = strip_tagged_blocks(&title, &["system-reminder"]);
    (!title.is_empty()).then(|| super::utils::truncate_summary(&title, 80))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_antigravity_visible_messages() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("transcript.jsonl");
        fs::write(
            &path,
            [
                r#"{"type":"USER_INPUT","content":"hello"}"#,
                r#"{"type":"PLANNER_RESPONSE","content":"answer","thinking":"hidden","tool_calls":[]}"#,
            ]
            .join("\n"),
        )
        .expect("write");
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "answer\n\nhidden");
    }
}
