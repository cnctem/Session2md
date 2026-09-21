use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_SESSION_BYTES: u64 = 256 * 1024 * 1024;
const MAX_WALK_FILES: usize = 100_000;
const MAX_WALK_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentPartKind {
    Text,
    Reasoning,
    ToolCall,
    ToolResult,
    System,
    Attachment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentPart {
    pub kind: ContentPartKind,
    pub content: String,
}

impl ContentPart {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            kind: ContentPartKind::Text,
            content: content.into(),
        }
    }

    pub fn reasoning(content: impl Into<String>) -> Self {
        Self {
            kind: ContentPartKind::Reasoning,
            content: content.into(),
        }
    }

    pub fn tool_call(content: impl Into<String>) -> Self {
        Self {
            kind: ContentPartKind::ToolCall,
            content: content.into(),
        }
    }

    pub fn tool_result(content: impl Into<String>) -> Self {
        Self {
            kind: ContentPartKind::ToolResult,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            kind: ContentPartKind::System,
            content: content.into(),
        }
    }

    pub fn attachment(content: impl Into<String>) -> Self {
        Self {
            kind: ContentPartKind::Attachment,
            content: content.into(),
        }
    }
}

pub fn normalize_role(role: &str) -> &'static str {
    match role.to_ascii_lowercase().as_str() {
        "user" | "human" => "user",
        "assistant" | "model" | "ai" => "assistant",
        "tool" | "toolresult" | "tool_result" | "function" => "tool",
        "system" | "developer" | "instruction" => "system",
        _ => "system",
    }
}

pub fn normalize_content_parts(content: &Value) -> Vec<ContentPart> {
    match content {
        Value::String(text) => vec![ContentPart::text(text.clone())],
        Value::Array(items) => items.iter().flat_map(normalize_content_parts).collect(),
        Value::Object(object) => normalize_object_part(object),
        _ => Vec::new(),
    }
}

pub fn messages_from_content(
    role: &str,
    content: &Value,
    ts: Option<i64>,
) -> Vec<crate::session_manager::SessionMessage> {
    let role = normalize_role(role);
    let parts = normalize_content_parts(content);
    messages_from_parts(role, &parts, ts)
}

pub fn messages_from_parts(
    source_role: &str,
    parts: &[ContentPart],
    ts: Option<i64>,
) -> Vec<crate::session_manager::SessionMessage> {
    let source_role = normalize_role(source_role);
    let mut messages = Vec::new();
    let mut text_buffer = Vec::<String>::new();

    let flush_text = |messages: &mut Vec<crate::session_manager::SessionMessage>,
                      text_buffer: &mut Vec<String>| {
        if text_buffer.is_empty() {
            return;
        }
        let content = text_buffer.join("\n\n");
        text_buffer.clear();
        super::push_message(messages, source_role, &content, ts);
    };

    for part in parts {
        match part.kind {
            ContentPartKind::Text | ContentPartKind::Reasoning => {
                if !part.content.trim().is_empty() {
                    text_buffer.push(part.content.trim().to_string());
                }
            }
            ContentPartKind::ToolCall | ContentPartKind::ToolResult => {
                flush_text(&mut messages, &mut text_buffer);
                super::push_message(&mut messages, "tool", &part.content, ts);
            }
            ContentPartKind::System => {
                flush_text(&mut messages, &mut text_buffer);
                super::push_message(&mut messages, "system", &part.content, ts);
            }
            ContentPartKind::Attachment => {
                if !part.content.trim().is_empty() {
                    text_buffer.push(part.content.trim().to_string());
                }
            }
        }
    }
    flush_text(&mut messages, &mut text_buffer);
    messages
}

fn normalize_object_part(object: &serde_json::Map<String, Value>) -> Vec<ContentPart> {
    let item_type = object.get("type").and_then(Value::as_str).unwrap_or("");
    let effective = object
        .get("data")
        .and_then(Value::as_object)
        .unwrap_or(object);
    match item_type {
        "text" | "Text" | "input_text" | "output_text" | "markdown" => {
            return text_part(effective, &["text", "Text", "content"]);
        }
        "thinking" | "Thinking" | "reasoning" | "think" | "reasoning_text"
        | "redacted_thinking" | "redactedThinking" | "RedactedThinking" => {
            return text_part(
                effective,
                &[
                    "thinking",
                    "think",
                    "reasoning",
                    "text",
                    "content",
                    "summary",
                ],
            )
            .into_iter()
            .map(|part| ContentPart::reasoning(part.content))
            .collect();
        }
        "tool_use" | "toolCall" | "tool-call" | "toolRequest" | "function_call" | "function"
        | "ToolUse" => {
            return vec![ContentPart::tool_call(format_tool_call(effective))];
        }
        "tool_result"
        | "tool-result"
        | "toolResponse"
        | "function_call_output"
        | "tool_result_output"
        | "ToolResult" => {
            return vec![ContentPart::tool_result(
                value_text(
                    effective
                        .get("content")
                        .or_else(|| effective.get("output"))
                        .or_else(|| effective.get("value"))
                        .or_else(|| effective.get("result")),
                )
                .unwrap_or_else(|| format_tool_result(effective)),
            )];
        }
        "tool" => {
            let name = effective
                .get("tool")
                .or_else(|| effective.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let input = effective
                .get("state")
                .and_then(|state| state.get("input"))
                .or_else(|| effective.get("input"))
                .map(ToString::to_string)
                .unwrap_or_default();
            let mut parts = vec![ContentPart::tool_call(format!("[Tool: {name}]\n{input}"))];
            if let Some(output) = effective
                .get("state")
                .and_then(|state| state.get("output"))
                .or_else(|| effective.get("output"))
            {
                parts.push(ContentPart::tool_result(output.to_string()));
            }
            return parts;
        }
        "system" | "developer" | "instruction" => {
            return text_part(effective, &["text", "content"])
                .into_iter()
                .map(|part| ContentPart::system(part.content))
                .collect();
        }
        "image" | "image_url" | "file" | "document" | "binary" | "image_blob_ref" => {
            return vec![ContentPart::attachment(format_attachment(object))];
        }
        "finish" | "step-start" | "step-finish" | "step.begin" | "step.end" | "compaction"
        | "timeline" => return Vec::new(),
        _ => {}
    }

    if let Some(content) = object.get("content") {
        if matches!(content, Value::Array(_) | Value::Object(_)) {
            let nested = normalize_content_parts(content);
            if !nested.is_empty() {
                return nested;
            }
        }
    }
    text_part(
        object,
        &["text", "content", "summary", "reasoning", "think"],
    )
}

fn text_part(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Vec<ContentPart> {
    keys.iter()
        .find_map(|key| value_text(object.get(*key)))
        .filter(|text| !text.trim().is_empty())
        .map(ContentPart::text)
        .into_iter()
        .collect()
}

fn value_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => {
            let parts = normalize_content_parts(value);
            let text = parts
                .into_iter()
                .map(|part| part.content)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        }
        Value::Null => None,
    }
}

fn format_tool_call(object: &serde_json::Map<String, Value>) -> String {
    let name = object
        .get("name")
        .or_else(|| object.get("tool"))
        .or_else(|| {
            object
                .get("function")
                .and_then(Value::as_object)
                .and_then(|function| function.get("name"))
        })
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let arguments = object
        .get("arguments")
        .or_else(|| object.get("input"))
        .or_else(|| object.get("args"))
        .or_else(|| {
            object
                .get("function")
                .and_then(Value::as_object)
                .and_then(|function| function.get("arguments"))
        });
    let arguments = value_text(arguments).unwrap_or_else(|| "{}".to_string());
    format!("[Tool: {name}]\n{arguments}")
}

fn format_tool_result(object: &serde_json::Map<String, Value>) -> String {
    object
        .get("tool_call_id")
        .or_else(|| object.get("toolCallId"))
        .and_then(Value::as_str)
        .map(|id| format!("[Tool result: {id}]"))
        .unwrap_or_else(|| "[Tool result]".to_string())
}

fn format_attachment(object: &serde_json::Map<String, Value>) -> String {
    let name = object
        .get("name")
        .or_else(|| object.get("filename"))
        .or_else(|| object.get("path"))
        .and_then(Value::as_str);
    match name {
        Some(name) => format!("[Attachment: {name}]"),
        None => "[Attachment]".to_string(),
    }
}

pub fn parse_sqlite_source(source: &str) -> Option<(PathBuf, String)> {
    let rest = source.strip_prefix("sqlite:")?;
    let hash_pos = rest.rfind('#')?;
    let db_path = PathBuf::from(&rest[..hash_pos]);
    let session_id = rest[hash_pos + 1..].to_string();
    (!db_path.as_os_str().is_empty() && !session_id.is_empty()).then_some((db_path, session_id))
}

pub fn sqlite_source(path: &Path, session_id: &str) -> String {
    format!("sqlite:{}#{session_id}", path.display())
}

pub fn open_sqlite_readonly(path: &Path) -> Result<rusqlite::Connection, String> {
    rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
            | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|error| format!("Failed to open SQLite database {}: {error}", path.display()))
}

pub fn table_columns(conn: &rusqlite::Connection, table: &str) -> HashSet<String> {
    table_column_names(conn, table).into_iter().collect()
}

pub fn table_column_names(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
    let query = format!("PRAGMA table_info({table})");
    let Ok(mut stmt) = conn.prepare(&query) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

pub fn row_to_json(row: &rusqlite::Row, columns: &[String]) -> Value {
    use rusqlite::types::ValueRef;
    use serde_json::{Map, Number};

    let mut map = Map::new();
    for (index, column) in columns.iter().enumerate() {
        let value = match row.get_ref(index) {
            Ok(ValueRef::Null) => Value::Null,
            Ok(ValueRef::Integer(value)) => Value::Number(value.into()),
            Ok(ValueRef::Real(value)) => Number::from_f64(value)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            Ok(ValueRef::Text(value)) => Value::String(String::from_utf8_lossy(value).to_string()),
            Ok(ValueRef::Blob(_)) | Err(_) => Value::Null,
        };
        map.insert(column.clone(), value);
    }
    Value::Object(map)
}

pub fn visible_content_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(visible_item_text)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(_) => visible_item_text(content).unwrap_or_default(),
        _ => String::new(),
    }
}

fn visible_item_text(item: &Value) -> Option<String> {
    let object = item.as_object()?;
    let item_type = object.get("type").and_then(Value::as_str).unwrap_or("");
    match item_type {
        "text" | "input_text" | "output_text" | "markdown" => {
            return object
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        "thinking"
        | "reasoning"
        | "redacted_thinking"
        | "redactedThinking"
        | "tool_use"
        | "tool_result"
        | "tool-call"
        | "tool-result"
        | "toolCall"
        | "toolRequest"
        | "toolResponse"
        | "image"
        | "image_url"
        | "file"
        | "document"
        | "binary"
        | "finish"
        | "step-start"
        | "step-finish"
        | "compaction"
        | "shell_command"
        | "systemNotification"
        | "error"
        | "actionRequired"
        | "toolConfirmationRequest" => return None,
        _ => {}
    }

    if let Some(text) = object.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = object.get("input_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = object.get("output_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(content) = object.get("content") {
        let text = visible_content_text(content);
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

pub fn clean_prompt_text(text: &str) -> String {
    strip_tagged_blocks(
        text,
        &[
            "system-reminder",
            "project_context",
            "connector-status",
            "expert_selection",
        ],
    )
}

pub fn strip_tagged_blocks(text: &str, tags: &[&str]) -> String {
    let mut output = text.to_string();
    for tag in tags {
        output = remove_tagged_block(&output, tag);
    }
    output.trim().to_string()
}

fn remove_tagged_block(input: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut remaining = input;
    let mut output = String::new();
    while let Some(start) = remaining.find(&open) {
        output.push_str(&remaining[..start]);
        let after = &remaining[start..];
        if let Some(end) = after.find(&close) {
            remaining = &after[end + close.len()..];
        } else {
            remaining = "";
            break;
        }
    }
    output.push_str(remaining);
    output
}

pub fn between_tags(text: &str, open: &str, close: &str) -> Option<String> {
    let start = text.find(open)? + open.len();
    let rest = &text[start..];
    let end = rest.find(close)?;
    Some(rest[..end].trim().to_string())
}

pub fn read_jsonl(path: &Path) -> Result<Vec<Value>, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to inspect session {}: {error}", path.display()))?;
    if metadata.len() > MAX_SESSION_BYTES {
        return Err(format!(
            "Session exceeds the {MAX_SESSION_BYTES}-byte safety limit: {}",
            path.display()
        ));
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read session {}: {error}", path.display()))?;
    Ok(content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect())
}

pub fn read_json(path: &Path, max_bytes: u64) -> Result<Value, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to inspect session {}: {error}", path.display()))?;
    if metadata.len() > max_bytes {
        return Err(format!(
            "Session exceeds the {}-byte safety limit: {}",
            max_bytes,
            path.display()
        ));
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read session {}: {error}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("Failed to parse session {}: {error}", path.display()))
}

pub fn walk_files(root: &Path, predicate: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut visited = HashSet::new();
    walk_files_inner(root, &predicate, 0, &mut files, &mut visited);
    files.sort();
    files
}

fn walk_files_inner(
    root: &Path,
    predicate: &dyn Fn(&Path) -> bool,
    depth: usize,
    files: &mut Vec<PathBuf>,
    visited: &mut HashSet<PathBuf>,
) {
    if depth > MAX_WALK_DEPTH || files.len() >= MAX_WALK_FILES {
        return;
    }
    let Ok(canonical) = root.canonicalize() else {
        return;
    };
    if !visited.insert(canonical) {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= MAX_WALK_FILES {
            break;
        }
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                ".git" | "node_modules" | "target" | "dist" | ".cache"
            ) {
                continue;
            }
            walk_files_inner(&path, predicate, depth + 1, files, visited);
        } else if file_type.is_file() && predicate(&path) {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn visible_text_excludes_reasoning_and_tools() {
        assert_eq!(
            visible_content_text(&json!([
                {"type": "thinking", "text": "secret"},
                {"type": "text", "text": "answer"},
                {"type": "tool_use", "text": "tool"}
            ])),
            "answer"
        );
    }

    #[test]
    fn normalized_messages_keep_reasoning_and_separate_tools() {
        let messages = messages_from_content(
            "assistant",
            &json!([
                {"type": "think", "think": "reasoning"},
                {"type": "text", "text": "answer"},
                {"type": "function", "function": {"name": "read", "arguments": "{}"}}
            ]),
            None,
        );
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "assistant");
        assert_eq!(messages[0].content, "reasoning\n\nanswer");
        assert_eq!(messages[1].role, "tool");
        assert!(messages[1].content.contains("[Tool: read]"));
    }
}
