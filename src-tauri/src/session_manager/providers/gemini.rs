use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

use crate::session_manager::{paths::gemini_dir, SessionMessage, SessionMeta};

use super::common::{messages_from_parts, ContentPart};
use super::utils::{parse_timestamp_to_ms, truncate_summary};

const PROVIDER_ID: &str = "gemini";

pub fn scan_sessions() -> Vec<SessionMeta> {
    let gemini_dir = gemini_dir();
    let tmp_dir = gemini_dir.join("tmp");
    if !tmp_dir.exists() {
        return Vec::new();
    }

    let mut sessions = Vec::new();

    // Iterate over project directories: tmp/<project_name>/chats/session-*.json(l)
    let project_dirs = match std::fs::read_dir(&tmp_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for entry in project_dirs.flatten() {
        let chats_dir = entry.path().join("chats");
        if !chats_dir.is_dir() {
            continue;
        }

        let chat_files = match std::fs::read_dir(&chats_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        let project_root_file = entry.path().join(".project_root");
        let project_dir = std::fs::read_to_string(project_root_file).ok();

        for file_entry in chat_files.flatten() {
            let path = file_entry.path();
            if !is_session_file(&path) {
                continue;
            }
            if let Some(meta) = parse_session(&path) {
                sessions.push(SessionMeta {
                    project_dir: project_dir.clone(),
                    ..meta
                });
            }
        }
    }

    sessions
}

/// A session file is either the legacy `.json` document or the `.jsonl` log that
/// newer Gemini CLI releases write. After a `.json` is migrated to `.jsonl` on
/// resume, the legacy file can remain beside it; prefer the `.jsonl` so the same
/// session is not listed twice.
pub(crate) fn is_session_file(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some("jsonl") => true,
        Some("json") => !path.with_extension("jsonl").exists(),
        _ => false,
    }
}

/// Rebuild the legacy single-object shape `{sessionId, ..., messages: [...]}`
/// from a session file.
///
/// A `.json` file already has that shape and is returned as-is. A `.jsonl` file
/// is replayed the way Gemini CLI's `loadConversationRecord` does: a line with
/// `sessionId` is metadata, a line with a string `id` is a message (a later line
/// with the same id overwrites in place), `{"$set": {...}}` merges metadata and
/// replaces messages when it carries `messages`, and `{"$rewindTo": id}` drops
/// that message and everything after it.
pub(crate) fn parse_session_document(data: &str) -> Option<Value> {
    // Metadata-only `.jsonl` files are still valid single objects; default messages.
    if let Ok(Value::Object(mut map)) = serde_json::from_str::<Value>(data) {
        map.entry("messages")
            .or_insert_with(|| Value::Array(Vec::new()));
        return Some(Value::Object(map));
    }

    let mut metadata = serde_json::Map::new();
    let mut log = MessageLog::default();
    let mut seen_record = false;

    for line in data.lines() {
        let Ok(Value::Object(mut record)) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        seen_record = true;
        if let Some(id) = record.get("$rewindTo").and_then(Value::as_str) {
            log.rewind_to(id);
        } else if record.get("id").is_some_and(Value::is_string) {
            log.upsert(Value::Object(record));
        } else if let Some(Value::Object(mut set)) = record.remove("$set") {
            if let Some(Value::Array(list)) = set.remove("messages") {
                log.reset(list);
            }
            metadata.extend(set);
        } else if record.get("sessionId").is_some_and(Value::is_string) {
            if let Some(Value::Array(list)) = record.remove("messages") {
                for msg in list {
                    log.upsert(msg);
                }
            }
            metadata.extend(record);
        }
    }

    if !seen_record {
        return None;
    }
    metadata.insert("messages".to_string(), Value::Array(log.messages));
    Some(Value::Object(metadata))
}

/// Replayed JSONL messages: same-id lines overwrite in place while preserving
/// first-seen order (like the upstream JS `Map`), with an id index so replay
/// stays linear.
#[derive(Default)]
struct MessageLog {
    messages: Vec<Value>,
    index: HashMap<String, usize>,
}

impl MessageLog {
    /// Lines without a string `id` are not messages and are ignored.
    fn upsert(&mut self, msg: Value) {
        let Some(id) = msg.get("id").and_then(Value::as_str).map(str::to_string) else {
            return;
        };
        match self.index.get(&id) {
            Some(&position) => self.messages[position] = msg,
            None => {
                self.index.insert(id, self.messages.len());
                self.messages.push(msg);
            }
        }
    }

    /// Drop the message with `id` and everything after it; clear when unknown.
    fn rewind_to(&mut self, id: &str) {
        let len = self.index.get(id).copied().unwrap_or(0);
        self.messages.truncate(len);
        self.index.retain(|_, position| *position < len);
    }

    /// `$set.messages` checkpoint: replace the whole log.
    fn reset(&mut self, list: Vec<Value>) {
        self.messages.clear();
        self.index.clear();
        for msg in list {
            self.upsert(msg);
        }
    }
}

/// Gemini CLI injected context or non-prompt user text (matches upstream's
/// `isIgnoredUserContent`).
fn is_ignored_user_text(text: &str) -> bool {
    let text = text.trim();
    text.is_empty() || text.starts_with('/') || text.starts_with('?') || is_injected_user_text(text)
}

/// CLI-injected context (environment info, hook output), not a user prompt.
fn is_injected_user_text(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with("<session_context>") || text.starts_with("<hook_context>")
}

/// Flatten a Gemini user message `content` (string or part array) into text.
fn visible_user_text(content: &Value) -> Option<String> {
    match content {
        Value::String(text) => Some(text.clone()),
        _ => {
            let text = super::common::normalize_content_parts(content)
                .into_iter()
                .map(|part| part.content)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
    }
}

fn read_session_document(path: &Path) -> Result<Value, String> {
    let data = std::fs::read_to_string(path).map_err(|e| format!("Failed to read session: {e}"))?;
    parse_session_document(&data).ok_or_else(|| "Failed to parse session JSON".to_string())
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let value = read_session_document(path)?;

    let messages = value
        .get("messages")
        .and_then(Value::as_array)
        .ok_or_else(|| "No messages array found".to_string())?;

    let mut result = Vec::new();
    for msg in messages {
        let role = match msg.get("type").and_then(Value::as_str) {
            Some("gemini") => "assistant",
            Some("user") => "user",
            Some("info") | Some("error") => "system",
            Some(_) | None => continue,
        };

        let mut parts = msg
            .get("content")
            .map(super::common::normalize_content_parts)
            .unwrap_or_default();
        if let Some(thoughts) = msg.get("thoughts") {
            match thoughts {
                Value::String(text) => parts.push(ContentPart::reasoning(text)),
                Value::Array(_) | Value::Object(_) => {
                    parts.extend(
                        super::common::normalize_content_parts(thoughts)
                            .into_iter()
                            .map(|part| ContentPart::reasoning(part.content)),
                    );
                }
                _ => {}
            }
        }
        if let Some(Value::Array(calls)) = msg.get("toolCalls") {
            for call in calls {
                if let Some(name) = call.get("name").and_then(Value::as_str) {
                    let args = call
                        .get("args")
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    parts.push(ContentPart::tool_call_with_metadata(
                        name,
                        args,
                        call.get("id")
                            .or_else(|| call.get("callId"))
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    ));
                }
            }
        }
        let ts = msg.get("timestamp").and_then(parse_timestamp_to_ms);
        let mut normalized = messages_from_parts(role, &parts, ts);
        if role == "assistant" {
            if let Some(model) =
                super::model(msg.get("model").and_then(Value::as_str).map(str::to_string))
            {
                super::attach_models(&mut normalized, vec![model]);
            }
        }
        result.extend(normalized);
    }

    Ok(result)
}

pub fn delete_session(_root: &Path, path: &Path, session_id: &str) -> Result<bool, String> {
    let meta = parse_session(path).ok_or_else(|| {
        format!(
            "Failed to parse Gemini session metadata: {}",
            path.display()
        )
    })?;
    if meta.session_id != session_id {
        return Err(format!(
            "Gemini session ID mismatch: expected {session_id}, found {}",
            meta.session_id
        ));
    }
    // Remove a leftover migrated `.json` first; otherwise it would reappear in
    // the list after its `.jsonl` is gone.
    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
        let legacy = path.with_extension("json");
        if legacy.is_file() {
            std::fs::remove_file(&legacy).map_err(|error| {
                format!(
                    "Failed to delete legacy Gemini session file {}: {error}",
                    legacy.display()
                )
            })?;
        }
    }
    std::fs::remove_file(path).map_err(|error| {
        format!(
            "Failed to delete Gemini session file {}: {error}",
            path.display()
        )
    })?;
    Ok(true)
}

fn parse_session(path: &Path) -> Option<SessionMeta> {
    let value = read_session_document(path).ok()?;

    let session_id = value.get("sessionId").and_then(Value::as_str)?.to_string();

    let created_at = value.get("startTime").and_then(parse_timestamp_to_ms);
    let last_active_at = value.get("lastUpdated").and_then(parse_timestamp_to_ms);

    // Derive title from the first real user prompt, skipping injected context
    // and slash commands.
    let title = value
        .get("messages")
        .and_then(Value::as_array)
        .and_then(|msgs| {
            msgs.iter()
                .filter(|m| m.get("type").and_then(Value::as_str) == Some("user"))
                .filter_map(|m| m.get("content"))
                .filter_map(visible_user_text)
                .find(|text| !is_ignored_user_text(text))
                .map(|text| truncate_summary(&text, 160))
        });

    let source_path = path.to_string_lossy().to_string();

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: session_id.clone(),
        title: title.clone(),
        summary: title,
        project_dir: None, // (optionally) populated later
        created_at,
        last_active_at: last_active_at.or(created_at),
        source_path: Some(source_path),
        can_delete: true,
        models: None,
        resume_command: Some(format!("gemini --resume {session_id}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_messages_handles_array_content() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("session.json");
        std::fs::write(
            &path,
            r#"{
              "sessionId": "test",
              "messages": [
                {"id":"1","timestamp":"2026-03-06T10:00:00Z","type":"user","content":[{"text":"hello"}]},
                {"id":"2","timestamp":"2026-03-06T10:00:01Z","type":"gemini","content":"world"},
                {"id":"3","timestamp":"2026-03-06T10:00:02Z","type":"info","content":"system info"},
                {"id":"4","timestamp":"2026-03-06T10:00:03Z","type":"error","content":"MCP ERROR"}
              ]
            }"#,
        )
        .expect("write");

        let msgs = load_messages(&path).expect("load");
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content, "hello");
        assert_eq!(msgs[1].role, "assistant");
        assert_eq!(msgs[1].content, "world");
        assert_eq!(msgs[2].role, "system");
        assert_eq!(msgs[3].role, "system");
    }

    #[test]
    fn load_messages_includes_tool_calls() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("session.json");
        std::fs::write(
            &path,
            r#"{
              "sessionId": "test",
              "messages": [
                {"id":"1","timestamp":"2026-03-10T08:24:50Z","type":"gemini","model":"gemini-3-pro","content":"","toolCalls":[{"id":"call_1","name":"web_search","args":{"query":"test"}}]},
                {"id":"2","timestamp":"2026-03-10T08:25:00Z","type":"gemini","model":"gemini-3-pro","content":"Here are the results.","toolCalls":[{"id":"call_2","name":"web_fetch","args":{"url":"http://example.com"}}]}
              ]
            }"#,
        )
        .expect("write");

        let msgs = load_messages(&path).expect("load");
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, "tool");
        assert_eq!(
            msgs[0].kind,
            crate::session_manager::SessionMessageKind::ToolCall
        );
        assert_eq!(msgs[0].tool_name.as_deref(), Some("web_search"));
        assert_eq!(msgs[0].tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(msgs[1].role, "assistant");
        assert!(msgs[1].content.contains("Here are the results."));
        assert_eq!(
            &msgs[1].metadata.as_ref().unwrap().models.as_ref().unwrap()[0],
            "gemini-3-pro"
        );
        assert_eq!(
            msgs[2].kind,
            crate::session_manager::SessionMessageKind::ToolCall
        );
        assert_eq!(msgs[2].tool_name.as_deref(), Some("web_fetch"));
    }

    #[test]
    fn is_session_file_prefers_migrated_jsonl_over_legacy_json() {
        let temp = tempdir().expect("tempdir");
        let legacy = temp.path().join("session-1.json");
        let jsonl = temp.path().join("session-1.jsonl");

        std::fs::write(&legacy, "{}").expect("write legacy json");
        assert!(is_session_file(&legacy));

        std::fs::write(&jsonl, "").expect("write jsonl");
        assert!(is_session_file(&jsonl));
        assert!(!is_session_file(&legacy));
    }

    #[test]
    fn parse_session_document_replays_jsonl_log() {
        let data = concat!(
            "{\"sessionId\":\"s1\",\"startTime\":\"2026-03-06T10:00:00Z\"}\n",
            "{\"id\":\"1\",\"type\":\"user\",\"content\":\"first\"}\n",
            "{\"id\":\"2\",\"type\":\"gemini\",\"content\":\"one\"}\n",
            "{\"$set\":{\"lastUpdated\":\"2026-03-06T10:05:00Z\"}}\n",
            "{\"id\":\"2\",\"type\":\"gemini\",\"content\":\"one-updated\"}\n",
            "{\"id\":\"3\",\"type\":\"user\",\"content\":\"second\"}\n",
            "{\"$rewindTo\":\"3\"}\n",
        );

        let doc = parse_session_document(data).expect("parse document");
        assert_eq!(doc.get("sessionId").and_then(Value::as_str), Some("s1"));
        assert_eq!(
            doc.get("lastUpdated").and_then(Value::as_str),
            Some("2026-03-06T10:05:00Z")
        );
        let messages = doc
            .get("messages")
            .and_then(Value::as_array)
            .expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0].get("content").and_then(Value::as_str),
            Some("first")
        );
        assert_eq!(
            messages[1].get("content").and_then(Value::as_str),
            Some("one-updated")
        );
    }

    #[test]
    fn load_messages_reads_jsonl_session() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("session-abc.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"sessionId\":\"s1\"}\n",
                "{\"id\":\"1\",\"timestamp\":\"2026-03-06T10:00:00Z\",\"type\":\"user\",\"content\":\"hello jsonl\"}\n",
                "{\"id\":\"2\",\"timestamp\":\"2026-03-06T10:00:01Z\",\"type\":\"gemini\",\"model\":\"gemini-3-pro\",\"content\":\"hi\"}\n",
            ),
        )
        .expect("write");

        let msgs = load_messages(&path).expect("load");
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].content, "hello jsonl");
        assert_eq!(msgs[1].role, "assistant");
    }

    #[test]
    fn parse_session_skips_injected_context_and_slash_commands() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("session-title.json");
        std::fs::write(
            &path,
            r#"{
              "sessionId": "s1",
              "messages": [
                {"id":"1","type":"user","content":"<session_context>env</session_context>"},
                {"id":"2","type":"user","content":"/help"},
                {"id":"3","type":"user","content":[{"text":"real question"}]}
              ]
            }"#,
        )
        .expect("write");

        let meta = parse_session(&path).expect("parse");
        assert_eq!(meta.title.as_deref(), Some("real question"));
    }

    #[test]
    fn delete_session_removes_migrated_legacy_json() {
        let temp = tempdir().expect("tempdir");
        let jsonl = temp.path().join("session-abc.jsonl");
        let legacy = temp.path().join("session-abc.json");
        std::fs::write(&jsonl, "{\"sessionId\":\"s1\"}\n").expect("write jsonl");
        std::fs::write(&legacy, "{\"sessionId\":\"s1\"}").expect("write legacy json");

        assert!(delete_session(temp.path(), &jsonl, "s1").expect("delete"));
        assert!(!jsonl.exists());
        assert!(!legacy.exists());
    }
}
