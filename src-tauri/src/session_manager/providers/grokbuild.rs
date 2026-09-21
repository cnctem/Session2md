use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::session_manager::{paths::grok_dir, SessionMessage, SessionMessageKind, SessionMeta};

use super::common::{messages_from_parts, normalize_content_parts, ContentPart};
use super::utils::{parse_timestamp_to_ms, truncate_summary, TITLE_MAX_CHARS};

#[derive(Debug, Deserialize)]
struct GrokSessionInfo {
    id: String,
    #[serde(default)]
    cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GrokSessionSummary {
    info: GrokSessionInfo,
    #[serde(default)]
    session_summary: Option<String>,
    #[serde(default)]
    generated_title: Option<String>,
    #[serde(default)]
    created_at: Option<Value>,
    #[serde(default)]
    updated_at: Option<Value>,
    #[serde(default)]
    last_active_at: Option<Value>,
}

#[derive(Clone)]
struct GrokToolCompletion {
    tool_name: Option<String>,
    outcome: Option<String>,
    duration_ms: Option<u64>,
}

pub fn session_roots() -> Vec<PathBuf> {
    let config_dir = grok_dir();
    vec![
        config_dir.join("sessions"),
        config_dir.join("archived_sessions"),
    ]
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let mut summaries = Vec::new();
    for root in session_roots() {
        collect_summary_files(&root, &mut summaries);
    }
    summaries
        .into_iter()
        .filter_map(|path| parse_summary(&path))
        .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let session_dir = path
        .parent()
        .ok_or_else(|| format!("Invalid Grok Build session path: {}", path.display()))?;
    let chat_path = session_dir.join("chat_history.jsonl");
    let file = File::open(&chat_path)
        .map_err(|e| format!("Failed to open Grok Build chat history: {e}"))?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let completion_list = load_tool_completions(session_dir);
    let completions = completion_list.iter().cloned().collect::<HashMap<_, _>>();
    let mut seen_completions = HashSet::new();

    for line in reader.lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
        let ts = value
            .get("timestamp")
            .or_else(|| value.get("ts"))
            .and_then(parse_timestamp_to_ms);
        let mut parts = Vec::new();
        match kind {
            "system" | "user" | "assistant" => {
                if let Some(content) = value.get("content") {
                    parts.extend(normalize_content_parts(content));
                }
                if let Some(tool_calls) = value.get("tool_calls") {
                    parts.extend(normalize_content_parts(tool_calls));
                }
            }
            "reasoning" => {
                if let Some(summary) = value.get("summary") {
                    parts.extend(
                        normalize_content_parts(summary)
                            .into_iter()
                            .map(ContentPart::as_reasoning),
                    );
                }
            }
            "tool_result" => {
                let tool_call_id = value
                    .get("tool_call_id")
                    .or_else(|| value.get("toolCallId"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let mut content = value
                    .get("content")
                    .map(|content| {
                        normalize_content_parts(content)
                            .into_iter()
                            .map(|part| part.content)
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                let completion = tool_call_id
                    .as_deref()
                    .and_then(|id| completions.get(id))
                    .cloned();
                if let Some(id) = tool_call_id.as_deref() {
                    seen_completions.insert(id.to_string());
                }
                if let Some(completion) = completion.as_ref() {
                    content = append_tool_completion(&content, completion);
                }
                parts.push(ContentPart::tool_result_with_metadata(
                    content,
                    tool_call_id,
                    completion.and_then(|completion| completion.tool_name),
                ));
            }
            "tool" => {
                if let Some(content) = value.get("content") {
                    parts.extend(normalize_content_parts(content).into_iter().map(|part| {
                        ContentPart::tool_result_with_metadata(
                            part.content,
                            value
                                .get("tool_call_id")
                                .or_else(|| value.get("toolCallId"))
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            None,
                        )
                    }));
                }
            }
            _ => continue,
        }
        let role = if kind == "reasoning" {
            "assistant"
        } else if kind == "tool_result" || kind == "tool" {
            "tool"
        } else {
            kind
        };
        messages.extend(messages_from_parts(role, &parts, ts));
    }

    for (id, completion) in completion_list {
        if seen_completions.contains(&id) {
            continue;
        }
        let content = append_tool_completion("", &completion);
        if content.is_empty() {
            continue;
        }
        messages.push(SessionMessage {
            role: "tool".to_string(),
            content,
            kind: SessionMessageKind::ToolResult,
            tool_call_id: Some(id),
            tool_name: completion.tool_name,
            ts: None,
        });
    }

    Ok(messages)
}

fn load_tool_completions(session_dir: &Path) -> Vec<(String, GrokToolCompletion)> {
    let path = session_dir.join("events.jsonl");
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let reader = BufReader::new(file);
    let mut completions = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("tool_completed") {
            continue;
        }
        let Some(id) = value
            .get("tool_call_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        completions.push((
            id.to_string(),
            GrokToolCompletion {
                tool_name: value
                    .get("tool_name")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                outcome: value
                    .get("outcome")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                duration_ms: value.get("duration_ms").and_then(Value::as_u64),
            },
        ));
    }
    completions
}

fn append_tool_completion(content: &str, completion: &GrokToolCompletion) -> String {
    let mut metadata = Vec::new();
    if let Some(outcome) = completion.outcome.as_deref() {
        metadata.push(format!("status: {outcome}"));
    }
    if let Some(duration_ms) = completion.duration_ms {
        metadata.push(format!("duration: {duration_ms} ms"));
    }
    if metadata.is_empty() {
        return content.to_string();
    }
    let metadata = format!("[{}]", metadata.join(" · "));
    if content.trim().is_empty() {
        metadata
    } else if content.contains(&metadata) {
        content.to_string()
    } else {
        format!("{}\n\n{metadata}", content.trim())
    }
}

pub fn delete_session(root: &Path, path: &Path, session_id: &str) -> Result<bool, String> {
    if !path.starts_with(root)
        || path.file_name().and_then(|name| name.to_str()) != Some("summary.json")
    {
        return Err(format!(
            "Unexpected Grok Build session source: {}",
            path.display()
        ));
    }
    let summary = read_summary(path)?;
    if summary.info.id != session_id {
        return Err(format!(
            "Grok Build session ID mismatch: expected {session_id}, found {}",
            summary.info.id
        ));
    }
    let session_dir = path
        .parent()
        .ok_or_else(|| format!("Invalid Grok Build session path: {}", path.display()))?;
    if session_dir == root
        || !session_dir.starts_with(root)
        || session_dir.file_name().and_then(|name| name.to_str()) != Some(session_id)
    {
        return Err(format!(
            "Grok Build session directory does not match session ID: {}",
            session_dir.display()
        ));
    }
    std::fs::remove_dir_all(session_dir).map_err(|error| {
        format!(
            "Failed to delete Grok Build session directory {}: {error}",
            session_dir.display()
        )
    })?;
    Ok(true)
}

fn collect_summary_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_summary_files(&path, files);
        } else if path.file_name().and_then(|name| name.to_str()) == Some("summary.json") {
            files.push(path);
        }
    }
}

fn read_summary(path: &Path) -> Result<GrokSessionSummary, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read Grok Build session summary: {e}"))?;
    serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse Grok Build session summary: {e}"))
}

fn parse_summary(path: &Path) -> Option<SessionMeta> {
    let summary = read_summary(path).ok()?;
    let session_id = summary.info.id;
    let title = summary
        .generated_title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            summary
                .session_summary
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        })
        .map(|value| truncate_summary(value, TITLE_MAX_CHARS));
    let session_summary = summary
        .session_summary
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| truncate_summary(value, 160));
    let created_at = summary.created_at.as_ref().and_then(parse_timestamp_to_ms);
    let last_active_at = summary
        .last_active_at
        .as_ref()
        .or(summary.updated_at.as_ref())
        .and_then(parse_timestamp_to_ms);

    Some(SessionMeta {
        provider_id: "grokbuild".to_string(),
        session_id: session_id.clone(),
        title,
        summary: session_summary,
        project_dir: summary.info.cwd,
        created_at,
        last_active_at,
        source_path: Some(path.to_string_lossy().to_string()),
        can_delete: true,
        resume_command: Some(format!("grok --resume {session_id}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn scans_native_grokbuild_session_layout() {
        let temp = tempdir().expect("tempdir");
        let sessions_dir = temp.path().join("sessions");
        let session_id = "019f6af2-18b0-7673-958e-d25be650e172";
        let session_dir = sessions_dir.join("encoded-project").join(session_id);
        std::fs::create_dir_all(&session_dir).expect("create session dir");
        std::fs::write(
            session_dir.join("summary.json"),
            format!(
                r#"{{"info":{{"id":"{session_id}","cwd":"C:/work"}},"session_summary":"hello grok","generated_title":"Grok session","created_at":"2026-07-16T12:00:00Z","last_active_at":"2026-07-16T12:00:01Z"}}"#
            ),
        )
        .expect("write summary");
        let mut files = Vec::new();
        collect_summary_files(&sessions_dir, &mut files);
        let sessions = files
            .iter()
            .filter_map(|path| parse_summary(path))
            .collect::<Vec<_>>();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].provider_id, "grokbuild");
        assert_eq!(sessions[0].session_id, session_id);
        assert_eq!(sessions[0].title.as_deref(), Some("Grok session"));
    }

    #[test]
    fn loads_native_grokbuild_chat_history() {
        let temp = tempdir().expect("tempdir");
        let summary_path = temp.path().join("summary.json");
        std::fs::write(&summary_path, "{}").expect("write summary placeholder");
        std::fs::write(
            temp.path().join("chat_history.jsonl"),
            concat!(
                "{\"type\":\"user\",\"content\":[{\"type\":\"text\",\"text\":\"hello\"}]}\n",
                "{\"type\":\"reasoning\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"private\"}]}\n",
                "{\"type\":\"assistant\",\"content\":\"Hi there\"}\n"
            ),
        )
        .expect("write chat history");
        let messages = load_messages(&summary_path).expect("load messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
        assert_eq!(
            messages[1].kind,
            crate::session_manager::SessionMessageKind::Reasoning
        );
        assert_eq!(messages[1].content, "private");
        assert_eq!(messages[2].content, "Hi there");
    }

    #[test]
    fn keeps_grok_tool_calls_and_results() {
        let temp = tempfile::tempdir().expect("tempdir");
        let summary_path = temp.path().join("summary.json");
        std::fs::write(&summary_path, "{}").expect("write summary placeholder");
        std::fs::write(
            temp.path().join("chat_history.jsonl"),
            concat!(
                "{\"type\":\"assistant\",\"content\":\"\",\"tool_calls\":[{\"id\":\"call-1\",\"name\":\"bash\",\"arguments\":\"{\\\"command\\\":\\\"pwd\\\"}\"}]}\n",
                "{\"type\":\"tool_result\",\"tool_call_id\":\"call-1\",\"content\":[{\"type\":\"text\",\"text\":\"/tmp/project\"}]}\n"
            ),
        )
        .expect("write chat history");
        std::fs::write(
            temp.path().join("events.jsonl"),
            "{\"type\":\"tool_completed\",\"tool_name\":\"bash\",\"duration_ms\":7,\"outcome\":\"success\",\"tool_call_id\":\"call-1\"}\n",
        )
        .expect("write events");

        let messages = load_messages(&summary_path).expect("load messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0].kind,
            crate::session_manager::SessionMessageKind::ToolCall
        );
        assert_eq!(messages[0].tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(messages[0].tool_name.as_deref(), Some("bash"));
        assert!(messages[0].content.contains("pwd"));
        assert_eq!(
            messages[1].kind,
            crate::session_manager::SessionMessageKind::ToolResult
        );
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-1"));
        assert_eq!(messages[1].tool_name.as_deref(), Some("bash"));
        assert!(messages[1].content.contains("/tmp/project"));
        assert!(messages[1].content.contains("status: success"));
        assert!(messages[1].content.contains("duration: 7 ms"));
    }

    #[test]
    fn keeps_event_only_grok_tool_completions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let summary_path = temp.path().join("summary.json");
        std::fs::write(&summary_path, "{}").expect("write summary placeholder");
        std::fs::write(temp.path().join("chat_history.jsonl"), "").expect("write chat history");
        std::fs::write(
            temp.path().join("events.jsonl"),
            "{\"type\":\"tool_completed\",\"tool_name\":\"read_file\",\"duration_ms\":3,\"outcome\":\"success\",\"tool_call_id\":\"call-2\"}\n",
        )
        .expect("write events");

        let messages = load_messages(&summary_path).expect("load messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].kind,
            crate::session_manager::SessionMessageKind::ToolResult
        );
        assert_eq!(messages[0].tool_name.as_deref(), Some("read_file"));
        assert_eq!(messages[0].tool_call_id.as_deref(), Some("call-2"));
        assert!(messages[0].content.contains("status: success"));
        assert!(messages[0].content.contains("duration: 3 ms"));
    }
}
