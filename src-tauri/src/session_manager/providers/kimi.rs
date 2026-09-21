use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{messages_from_content, read_json, read_jsonl, walk_files, ContentPart};

const PROVIDER_ID: &str = "kimi";

pub fn session_roots() -> Vec<PathBuf> {
    if let Some(configured) = crate::session2md_settings::directory_override("kimi") {
        return vec![
            if configured.file_name().and_then(|name| name.to_str()) == Some("sessions") {
                configured
            } else {
                configured.join("sessions")
            },
        ];
    }
    vec![paths::kimi_dir().join("sessions")]
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let mut seen = HashSet::new();
    session_roots()
        .into_iter()
        .flat_map(|root| find_kimi_dirs(&root))
        .filter_map(|dir| parse_session(&dir).ok())
        .filter(|session| seen.insert(session.session_id.clone()))
        .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let wire = wire_path(path)?;
    let values = read_jsonl(&wire)?;
    let mut messages = visible_records(&values);
    let loop_messages = loop_event_messages(&values);
    if !messages.iter().any(|message| message.role == "assistant") {
        messages.extend(
            loop_messages
                .iter()
                .filter(|message| message.role == "assistant")
                .cloned(),
        );
    }
    if !messages.iter().any(|message| message.role == "tool") {
        messages.extend(
            loop_messages
                .into_iter()
                .filter(|message| message.role == "tool"),
        );
    }
    Ok(messages)
}

fn visible_records(values: &[Value]) -> Vec<SessionMessage> {
    let mut output = Vec::new();
    for value in values {
        let ts = record_time(value);
        if let Some(record_type) = value.get("type").and_then(Value::as_str) {
            match record_type {
                "turn.prompt" => {
                    output.extend(messages_from_content(
                        "user",
                        value.get("input").unwrap_or(&Value::Null),
                        ts,
                    ));
                }
                "context.append_message" => {
                    if let Some(message) = value.get("message") {
                        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
                        let content = message.get("content").unwrap_or(&Value::Null);
                        let normalized = super::common::normalize_role(role);
                        if normalized == "user"
                            && output.last().is_some_and(|last| {
                                last.role == "user"
                                    && last.content
                                        == super::common::messages_from_content("user", content, ts)
                                            .first()
                                            .map(|message| message.content.as_str())
                                            .unwrap_or_default()
                            })
                        {
                            continue;
                        }
                        output.extend(messages_from_content(role, content, ts));
                        if let Some(tool_calls) = message.get("toolCalls") {
                            output.extend(messages_from_content("assistant", tool_calls, ts));
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(message) = value.get("message") {
            if let Some(record_type) = message.get("type").and_then(Value::as_str) {
                let payload = message.get("payload").unwrap_or(&Value::Null);
                match record_type {
                    "TurnBegin" | "SteerInput" => output.extend(messages_from_content(
                        "user",
                        payload.get("user_input").unwrap_or(&Value::Null),
                        ts,
                    )),
                    "TextPart" | "ThinkPart" => {
                        output.extend(messages_from_content("assistant", payload, ts))
                    }
                    "ToolCall" => output.extend(messages_from_content("tool", payload, ts)),
                    "ToolResult" => output.extend(messages_from_content("tool", payload, ts)),
                    _ => {}
                }
            }
        }
    }
    output
}

fn loop_event_messages(values: &[Value]) -> Vec<SessionMessage> {
    let mut parts = Vec::new();
    let mut ts = None;
    for value in values {
        if value.get("type").and_then(Value::as_str) != Some("context.append_loop_event") {
            continue;
        }
        ts = ts.or_else(|| record_time(value));
        let Some(event) = value.get("event") else {
            continue;
        };
        match event.get("type").and_then(Value::as_str) {
            Some("content.part") => {
                if let Some(part) = event.get("part") {
                    parts.extend(super::common::normalize_content_parts(part));
                }
            }
            Some("tool.call") => {
                parts.push(ContentPart::tool_call(
                    event
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                ));
            }
            Some("tool.result") => {
                parts.push(ContentPart::tool_result(
                    event
                        .get("result")
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                ));
            }
            _ => {}
        }
    }
    super::common::messages_from_parts("assistant", &parts, ts)
}

fn record_time(value: &Value) -> Option<i64> {
    value
        .get("timestamp")
        .or_else(|| value.get("time"))
        .or_else(|| value.get("created_at"))
        .and_then(super::utils::parse_timestamp_to_ms)
}

fn parse_session(dir: &Path) -> Result<SessionMeta, String> {
    let wire = wire_path(dir)?;
    let messages = load_messages(dir)?;
    if messages.is_empty() {
        return Err("Kimi session has no conversational messages".to_string());
    }
    let state = read_json(&dir.join("state.json"), 16 * 1024 * 1024).ok();
    let title = state
        .as_ref()
        .and_then(|state| {
            state
                .get("custom_title")
                .and_then(Value::as_str)
                .or_else(|| {
                    (state.get("isCustomTitle").and_then(Value::as_bool) == Some(true))
                        .then(|| state.get("title").and_then(Value::as_str))
                        .flatten()
                })
        })
        .filter(|title| !title.trim().is_empty())
        .map(|title| super::utils::truncate_summary(title, 80))
        .or_else(|| {
            messages
                .iter()
                .find(|message| message.role == "user")
                .map(|message| super::utils::truncate_summary(&message.content, 80))
        });
    let cwd = state.as_ref().and_then(|state| {
        ["cwd", "workDir"]
            .iter()
            .find_map(|key| state.get(*key).and_then(Value::as_str))
            .map(str::to_string)
    });
    let created_at = messages.iter().find_map(|message| message.ts);
    let last_active_at = fs::metadata(&wire)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64);
    Ok(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string(),
        title,
        summary: None,
        project_dir: cwd,
        created_at,
        last_active_at: last_active_at.or(created_at),
        source_path: Some(dir.display().to_string()),
        resume_command: None,
        can_delete: false,
    })
}

fn wire_path(dir: &Path) -> Result<PathBuf, String> {
    let root = dir.join("wire.jsonl");
    if root.is_file() {
        return Ok(root);
    }
    let nested = dir.join("agents").join("main").join("wire.jsonl");
    if nested.is_file() {
        return Ok(nested);
    }
    Err(format!(
        "Kimi wire transcript not found in {}",
        dir.display()
    ))
}

fn find_kimi_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if wire_path(root).is_ok() {
        dirs.push(root.to_path_buf());
        return dirs;
    }
    for path in walk_files(root, |path| {
        path.file_name().and_then(|name| name.to_str()) == Some("wire.jsonl")
    }) {
        let Some(parent) = path.parent() else {
            continue;
        };
        let dir = if parent.file_name().and_then(|name| name.to_str()) == Some("main")
            && parent
                .parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some("agents")
        {
            parent.parent().and_then(Path::parent).unwrap_or(parent)
        } else {
            parent
        };
        if !dir.to_string_lossy().contains("subagents") {
            dirs.push(dir.to_path_buf());
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_new_and_old_kimi_visible_records() {
        let root = tempfile::tempdir().expect("root");
        fs::write(
            root.path().join("wire.jsonl"),
            [
                r#"{"type":"turn.prompt","input":"hello"}"#,
                r#"{"timestamp":1,"message":{"type":"TextPart","payload":{"text":"answer"}}}"#,
            ]
            .join("\n"),
        )
        .expect("write");
        let messages = load_messages(root.path()).expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "answer");
    }

    #[test]
    fn keeps_kimi_code_think_and_tool_records() {
        let root = tempfile::tempdir().expect("root");
        fs::write(
            root.path().join("wire.jsonl"),
            [
                r#"{"type":"context.append_message","message":{"role":"user","content":[{"type":"text","text":"hello"}]}}"#,
                r#"{"type":"context.append_message","message":{"role":"assistant","content":[{"type":"think","think":"reasoning"},{"type":"text","text":"answer"}],"toolCalls":[{"type":"function","function":{"name":"read","arguments":"{}"}}]}}"#,
                r#"{"type":"context.append_message","message":{"role":"tool","content":[{"type":"text","text":"file contents"}]}}"#,
            ]
            .join("\n"),
        )
        .expect("write");
        let messages = load_messages(root.path()).expect("messages");
        assert!(messages.iter().any(|message| {
            message.role == "assistant"
                && message.content.contains("reasoning")
                && message.content.contains("answer")
        }));
        assert!(messages
            .iter()
            .any(|message| message.role == "tool" && message.content.contains("[Tool: read]")));
        assert!(messages
            .iter()
            .any(|message| message.role == "tool" && message.content == "file contents"));
    }
}
