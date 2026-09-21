use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMessageKind, SessionMeta};

use super::common::{messages_from_content, read_json, read_jsonl, walk_files, ContentPart};

const PROVIDER_ID: &str = "kimi";

type MessageKey = (String, SessionMessageKind, String, Option<String>);

struct OrderedMessage {
    record_index: usize,
    part_index: usize,
    message: SessionMessage,
}

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
    let mut messages = canonical_records(&values);
    let canonical_keys = messages
        .iter()
        .map(|entry| message_key(&entry.message))
        .collect::<HashSet<_>>();
    messages.extend(loop_event_records(&values, &canonical_keys));
    messages.sort_by_key(|entry| (entry.record_index, entry.part_index));
    Ok(messages.into_iter().map(|entry| entry.message).collect())
}

fn canonical_records(values: &[Value]) -> Vec<OrderedMessage> {
    let mut output = Vec::new();
    for (record_index, value) in values.iter().enumerate() {
        let mut part_index = 0;
        let ts = record_time(value);
        if let Some(record_type) = value.get("type").and_then(Value::as_str) {
            match record_type {
                "turn.prompt" => {
                    append_ordered(
                        &mut output,
                        record_index,
                        &mut part_index,
                        messages_from_content(
                            "user",
                            value.get("input").unwrap_or(&Value::Null),
                            ts,
                        ),
                    );
                }
                "context.append_message" => {
                    if let Some(message) = value.get("message") {
                        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
                        let content = message.get("content").unwrap_or(&Value::Null);
                        let normalized = super::common::normalize_role(role);
                        let normalized_messages = messages_from_content(role, content, ts);
                        if normalized == "user"
                            && normalized_messages.first().is_some_and(|candidate| {
                                output.last().is_some_and(|last| {
                                    last.message.role == "user"
                                        && last.message.content == candidate.content
                                })
                            })
                        {
                            continue;
                        }
                        append_ordered(
                            &mut output,
                            record_index,
                            &mut part_index,
                            normalized_messages,
                        );
                        if let Some(tool_calls) = message.get("toolCalls") {
                            append_ordered(
                                &mut output,
                                record_index,
                                &mut part_index,
                                messages_from_content("assistant", tool_calls, ts),
                            );
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
                    "TurnBegin" | "SteerInput" => append_ordered(
                        &mut output,
                        record_index,
                        &mut part_index,
                        messages_from_content(
                            "user",
                            payload.get("user_input").unwrap_or(&Value::Null),
                            ts,
                        ),
                    ),
                    "TextPart" | "ThinkPart" => append_ordered(
                        &mut output,
                        record_index,
                        &mut part_index,
                        messages_from_content("assistant", payload, ts),
                    ),
                    "ToolCall" | "ToolResult" => append_ordered(
                        &mut output,
                        record_index,
                        &mut part_index,
                        messages_from_content("tool", payload, ts),
                    ),
                    _ => {}
                }
            }
        }
    }
    output
}

fn loop_event_records(
    values: &[Value],
    canonical_keys: &HashSet<MessageKey>,
) -> Vec<OrderedMessage> {
    let mut output = Vec::new();
    let mut seen = HashSet::new();
    for (record_index, value) in values.iter().enumerate() {
        if value.get("type").and_then(Value::as_str) != Some("context.append_loop_event") {
            continue;
        }
        let mut part_index = 0;
        for message in loop_event_messages(value) {
            let key = message_key(&message);
            if canonical_keys.contains(&key) || !seen.insert(key) {
                continue;
            }
            append_ordered(&mut output, record_index, &mut part_index, vec![message]);
        }
    }
    output
}

fn loop_event_messages(value: &Value) -> Vec<SessionMessage> {
    let mut parts = Vec::new();
    let ts = record_time(value);
    let Some(event) = value.get("event") else {
        return Vec::new();
    };
    match event.get("type").and_then(Value::as_str) {
        Some("content.part") => {
            if let Some(part) = event.get("part") {
                parts.extend(super::common::normalize_content_parts(part));
            }
        }
        Some("tool.call") => {
            let arguments = event
                .get("args")
                .map(|value| match value {
                    Value::String(value) => value.clone(),
                    _ => value.to_string(),
                })
                .or_else(|| {
                    event
                        .get("display")
                        .and_then(|display| display.get("command"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default();
            parts.push(ContentPart::tool_call_with_metadata(
                event
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                arguments,
                event
                    .get("toolCallId")
                    .or_else(|| event.get("tool_call_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
            ));
        }
        Some("tool.result") => {
            let result = event
                .get("result")
                .and_then(|result| result.get("output"))
                .or_else(|| event.get("result"))
                .map(|value| match value {
                    Value::String(value) => value.clone(),
                    _ => value.to_string(),
                })
                .unwrap_or_default();
            parts.push(ContentPart::tool_result_with_metadata(
                result,
                event
                    .get("toolCallId")
                    .or_else(|| event.get("tool_call_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                event
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            ));
        }
        _ => {}
    }
    super::common::messages_from_parts("assistant", &parts, ts)
}

fn append_ordered(
    output: &mut Vec<OrderedMessage>,
    record_index: usize,
    part_index: &mut usize,
    messages: Vec<SessionMessage>,
) {
    for message in messages {
        output.push(OrderedMessage {
            record_index,
            part_index: *part_index,
            message,
        });
        *part_index += 1;
    }
}

fn message_key(message: &SessionMessage) -> MessageKey {
    (
        message.role.clone(),
        message.kind.clone(),
        message.content.clone(),
        message.tool_call_id.clone(),
    )
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
        assert_eq!(messages.len(), 5);
        assert!(messages.iter().any(|message| {
            message.role == "assistant"
                && message.kind == crate::session_manager::SessionMessageKind::Reasoning
                && message.content == "reasoning"
        }));
        assert!(messages.iter().any(|message| {
            message.role == "assistant"
                && message.kind == crate::session_manager::SessionMessageKind::Text
                && message.content == "answer"
        }));
        assert!(messages.iter().any(|message| {
            message.role == "tool"
                && message.kind == crate::session_manager::SessionMessageKind::ToolCall
                && message.tool_name.as_deref() == Some("read")
        }));
        assert!(messages.iter().any(|message| {
            message.role == "tool"
                && message.kind == crate::session_manager::SessionMessageKind::ToolResult
                && message.content == "file contents"
        }));
    }

    #[test]
    fn preserves_loop_event_order_across_turns() {
        let root = tempfile::tempdir().expect("root");
        fs::write(
            root.path().join("wire.jsonl"),
            [
                r#"{"type":"turn.prompt","input":"first"}"#,
                r#"{"type":"context.append_loop_event","timestamp":1,"event":{"type":"content.part","part":{"type":"text","text":"first answer"}}}"#,
                r#"{"type":"context.append_loop_event","timestamp":2,"event":{"type":"tool.call","name":"Bash","toolCallId":"call-1","args":{"command":"pwd"}}}"#,
                r#"{"type":"context.append_loop_event","timestamp":3,"event":{"type":"tool.result","toolCallId":"call-1","result":{"output":"/tmp"}}}"#,
                r#"{"type":"turn.prompt","input":"second"}"#,
                r#"{"type":"context.append_loop_event","timestamp":5,"event":{"type":"content.part","part":{"type":"text","text":"second answer"}}}"#,
            ]
            .join("\n"),
        )
        .expect("write");

        let messages = load_messages(root.path()).expect("messages");

        assert_eq!(
            messages
                .iter()
                .map(|message| message.content.as_str())
                .collect::<Vec<_>>(),
            vec![
                "first",
                "first answer",
                r#"{"command":"pwd"}"#,
                "/tmp",
                "second",
                "second answer",
            ]
        );
        assert_eq!(
            messages
                .iter()
                .map(|message| message.kind.clone())
                .collect::<Vec<_>>(),
            vec![
                SessionMessageKind::Text,
                SessionMessageKind::Text,
                SessionMessageKind::ToolCall,
                SessionMessageKind::ToolResult,
                SessionMessageKind::Text,
                SessionMessageKind::Text,
            ]
        );
    }

    #[test]
    fn merges_loop_tool_calls_with_canonical_messages() {
        let root = tempfile::tempdir().expect("root");
        fs::write(
            root.path().join("wire.jsonl"),
            [
                r#"{"type":"context.append_message","message":{"role":"assistant","content":[{"type":"text","text":"answer"}]}}"#,
                r#"{"type":"context.append_loop_event","event":{"type":"content.part","part":{"type":"text","text":"answer"}}}"#,
                r#"{"type":"context.append_loop_event","event":{"type":"tool.call","name":"Bash","toolCallId":"call-1","args":{"command":"pwd"}}}"#,
                r#"{"type":"context.append_loop_event","event":{"type":"tool.result","toolCallId":"call-1","result":{"output":"/tmp"}}}"#,
            ]
            .join("\n"),
        )
        .expect("write");

        let messages = load_messages(root.path()).expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].content, "answer");
        assert_eq!(messages[1].kind, SessionMessageKind::ToolCall);
        assert_eq!(messages[2].kind, SessionMessageKind::ToolResult);
        assert!(messages.iter().any(|message| {
            message.kind == crate::session_manager::SessionMessageKind::ToolCall
                && message.tool_name.as_deref() == Some("Bash")
                && message.tool_call_id.as_deref() == Some("call-1")
                && message.content.contains("pwd")
        }));
        assert!(messages.iter().any(|message| {
            message.kind == crate::session_manager::SessionMessageKind::ToolResult
                && message.tool_call_id.as_deref() == Some("call-1")
                && message.content == "/tmp"
        }));
    }
}
