use std::fs::{self, File};
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{
    messages_from_content, messages_from_parts, walk_files, ContentPart, MAX_SESSION_BYTES,
};

const PROVIDER_ID: &str = "dsh";
const MAX_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;

pub fn session_roots() -> Vec<PathBuf> {
    vec![paths::dsh_dir().join("sessions")]
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    session_roots()
        .into_iter()
        .flat_map(|root| scan_root(&root))
        .collect()
}

fn scan_root(root: &Path) -> Vec<SessionMeta> {
    walk_files(root, is_session_file)
        .into_iter()
        .filter_map(|path| parse_session(&path).ok())
        .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    if !is_session_file(path) {
        return Err(format!(
            "Invalid DeepSeek Harness session file: {}",
            path.display()
        ));
    }
    let values = read_session_values(path)?;
    let messages = visible_messages(&values);
    if messages.is_empty() {
        return Err("DeepSeek Harness session has no conversational messages".to_string());
    }
    if messages.is_empty() {
        return Err("DeepSeek Harness session has no visible messages".to_string());
    }
    Ok(messages)
}

fn parse_session(path: &Path) -> Result<SessionMeta, String> {
    let values = read_session_values(path)?;
    let header = values.iter().find(|value| {
        value.get("type").and_then(Value::as_str) == Some("session")
            && value.get("id").and_then(Value::as_str).is_some()
    });
    let header = header.ok_or_else(|| "missing DSH session header".to_string())?;
    let session_id = header
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let cwd = header
        .get("cwd")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let created_at = header
        .get("createdAt")
        .and_then(crate::session_manager::providers::utils::parse_timestamp_to_ms);
    let messages = visible_messages(&values);
    if messages.is_empty() {
        return Err("DeepSeek Harness session has no conversational messages".to_string());
    }
    let title = values
        .iter()
        .rev()
        .find_map(|value| {
            (value.get("type").and_then(Value::as_str) == Some("session/title"))
                .then(|| {
                    value
                        .get("data")?
                        .get("title")?
                        .as_str()
                        .map(str::to_string)
                })
                .flatten()
        })
        .or_else(|| {
            messages
                .iter()
                .find(|message| message.role == "user")
                .map(|message| super::utils::truncate_summary(&message.content, 80))
                .filter(|title| !title.is_empty())
        });
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to inspect DSH session {}: {error}", path.display()))?;
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
        project_dir: cwd,
        created_at,
        last_active_at: mtime.or(created_at),
        source_path: Some(path.display().to_string()),
        resume_command: None,
        can_delete: false,
    })
}

fn visible_messages(values: &[Value]) -> Vec<SessionMessage> {
    let canonical_steps = values
        .iter()
        .filter(|value| value.get("type").and_then(Value::as_str) == Some("assistant/message"))
        .filter_map(|value| dsh_step_key(value))
        .collect::<std::collections::HashSet<_>>();
    let mut block_steps = std::collections::HashSet::new();
    let mut partial_chunks = std::collections::BTreeMap::<(u64, u64, u64, String), String>::new();
    let mut output = Vec::new();
    let mut seen_tool_call_ids = std::collections::HashSet::<String>::new();

    for value in values {
        let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        let ts = value
            .get("time")
            .and_then(crate::session_manager::providers::utils::parse_timestamp_to_ms);
        match event_type {
            "user/message" => {
                if let Some(content) = value.get("data").and_then(|data| data.get("content")) {
                    output.extend(messages_from_content("user", content, ts));
                }
            }
            "assistant/message" => {
                if let Some(content) = value
                    .get("data")
                    .and_then(|data| data.get("message"))
                    .and_then(|message| message.get("content"))
                {
                    collect_tool_call_ids(content, &mut seen_tool_call_ids);
                    output.extend(messages_from_content("assistant", content, ts));
                }
            }
            "assistant/chunk" => {
                let Some(step) = dsh_step_key(value) else {
                    continue;
                };
                if canonical_steps.contains(&step) {
                    continue;
                }
                let Some(chunk) = value.get("data").and_then(|data| data.get("chunk")) else {
                    continue;
                };
                if chunk.get("type").and_then(Value::as_str) != Some("block-end") {
                    continue;
                }
                block_steps.insert(step);
                if let Some(block) = chunk.get("block") {
                    collect_tool_call_ids(block, &mut seen_tool_call_ids);
                    output.extend(messages_from_content("assistant", block, ts));
                }
            }
            "tool/call" => {
                let Some(data) = value.get("data") else {
                    continue;
                };
                let id = data
                    .get("callId")
                    .or_else(|| data.get("call_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if id
                    .as_ref()
                    .is_some_and(|id| seen_tool_call_ids.contains(id))
                {
                    continue;
                }
                if let Some(id) = id.clone() {
                    seen_tool_call_ids.insert(id.clone());
                }
                let name = data
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let arguments = data
                    .get("arguments")
                    .map(|value| match value {
                        Value::String(value) => value.clone(),
                        _ => value.to_string(),
                    })
                    .unwrap_or_else(|| "{}".to_string());
                output.extend(messages_from_parts(
                    "tool",
                    &[ContentPart::tool_call_with_metadata(name, arguments, id)],
                    ts,
                ));
            }
            "tool/result" => {
                if let Some(content) = value
                    .get("data")
                    .and_then(|data| data.get("message"))
                    .and_then(|message| message.get("content"))
                {
                    output.extend(messages_from_content("tool", content, ts));
                }
            }
            "reasoning-chunks" | "text-chunks" => {
                let Some((turn, step, index)) = dsh_chunk_key(value) else {
                    continue;
                };
                let Some(texts) = value
                    .get("data")
                    .and_then(|data| data.get("texts"))
                    .and_then(Value::as_array)
                else {
                    continue;
                };
                let text = texts.iter().filter_map(Value::as_str).collect::<String>();
                if text.is_empty() {
                    continue;
                }
                let kind = if event_type == "reasoning-chunks" {
                    "reasoning"
                } else {
                    "text"
                };
                partial_chunks
                    .entry((turn, step, index, kind.to_string()))
                    .or_default()
                    .push_str(&text);
            }
            _ => {}
        }
    }

    let mut partial_by_step = std::collections::BTreeMap::<(u64, u64), Vec<ContentPart>>::new();
    for ((turn, step, _index, kind), content) in partial_chunks {
        if canonical_steps.contains(&(turn, step)) || block_steps.contains(&(turn, step)) {
            continue;
        }
        partial_by_step
            .entry((turn, step))
            .or_default()
            .push(if kind == "reasoning" {
                ContentPart::reasoning(content)
            } else {
                ContentPart::text(content)
            });
    }
    for (_, parts) in partial_by_step {
        output.extend(messages_from_parts("assistant", &parts, None));
    }

    output
}

fn dsh_step_key(value: &Value) -> Option<(u64, u64)> {
    let data = value.get("data")?;
    Some((data.get("turn")?.as_u64()?, data.get("step")?.as_u64()?))
}

fn dsh_chunk_key(value: &Value) -> Option<(u64, u64, u64)> {
    let data = value.get("data")?;
    Some((
        data.get("turn")?.as_u64()?,
        data.get("step")?.as_u64()?,
        data.get("index")?.as_u64()?,
    ))
}

fn collect_tool_call_ids(value: &Value, ids: &mut std::collections::HashSet<String>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_tool_call_ids(value, ids);
            }
        }
        Value::Object(object) => {
            let kind = object.get("type").and_then(Value::as_str).unwrap_or("");
            if matches!(
                kind,
                "tool-call" | "toolCall" | "tool_use" | "function_call"
            ) {
                if let Some(id) = object
                    .get("id")
                    .or_else(|| object.get("callId"))
                    .or_else(|| object.get("call_id"))
                    .and_then(Value::as_str)
                {
                    ids.insert(id.to_string());
                }
            }
            for value in object.values() {
                collect_tool_call_ids(value, ids);
            }
        }
        _ => {}
    }
}

fn read_session_values(path: &Path) -> Result<Vec<Value>, String> {
    let bytes = if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zstd"))
    {
        decompress_zstd(path)?
    } else {
        let metadata = fs::metadata(path).map_err(|error| {
            format!("Failed to inspect DSH session {}: {error}", path.display())
        })?;
        if metadata.len() > MAX_SESSION_BYTES {
            return Err(format!(
                "DSH session exceeds the {MAX_SESSION_BYTES}-byte safety limit: {}",
                path.display()
            ));
        }
        fs::read(path)
            .map_err(|error| format!("Failed to read DSH session {}: {error}", path.display()))?
    };
    let text = String::from_utf8_lossy(&bytes);
    Ok(text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect())
}

fn decompress_zstd(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|error| {
        format!(
            "Failed to open compressed DSH session {}: {error}",
            path.display()
        )
    })?;
    let file_len = file
        .metadata()
        .map_err(|error| format!("Failed to inspect DSH session {}: {error}", path.display()))?
        .len();
    let mut output = Vec::new();
    let mut position = 0u64;

    while position < file_len {
        if output.len() as u64 >= MAX_DECOMPRESSED_BYTES {
            return Err(format!(
                "Decompressed DSH session exceeds the {MAX_DECOMPRESSED_BYTES}-byte safety limit: {}",
                path.display()
            ));
        }
        let remaining = MAX_DECOMPRESSED_BYTES - output.len() as u64;
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(file).map_err(|error| {
            format!(
                "Failed to initialize zstd decoder for {} at byte {position}: {error}",
                path.display()
            )
        })?;
        let mut frame = Vec::new();
        {
            let mut limited = decoder.by_ref().take(remaining + 1);
            limited.read_to_end(&mut frame).map_err(|error| {
                format!(
                    "Failed to decompress DSH session {} at byte {position}: {error}",
                    path.display()
                )
            })?;
        }
        let (mut next_file, _) = decoder.into_parts();
        let next_position = next_file
            .stream_position()
            .map_err(|error| format!("Failed to inspect DSH session position: {error}"))?;
        if next_position <= position {
            return Err(format!(
                "DSH zstd decoder made no progress at byte {position}: {}",
                path.display()
            ));
        }
        output.extend_from_slice(&frame);
        if output.len() as u64 > MAX_DECOMPRESSED_BYTES {
            return Err(format!(
                "Decompressed DSH session exceeds the {MAX_DECOMPRESSED_BYTES}-byte safety limit: {}",
                path.display()
            ));
        }
        file = next_file;
        position = next_position;
    }
    Ok(output)
}

fn is_session_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    let stem = name.strip_suffix(".zstd").unwrap_or(&name);
    stem == "session.jsonl"
        || (stem.starts_with("session.v")
            && stem.ends_with(".jsonl")
            && stem[9..stem.len() - 6]
                .chars()
                .all(|ch| ch.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn write_session(path: &Path) {
        let lines = [
            json!({"type":"session","id":"dsh-1","cwd":"/tmp/project","createdAt":1_700_000_000_000_i64}),
            json!({"type":"user/message","seq":0,"time":1_700_000_000_100_i64,"data":{"content":[{"type":"text","text":"hello"}]}}),
            json!({"type":"assistant/message","seq":1,"time":1_700_000_000_200_i64,"data":{"message":{"content":[{"type":"thinking","thinking":"hidden"},{"type":"text","text":"world"}]}}}),
        ];
        fs::write(
            path,
            lines
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .expect("write session");
    }

    #[test]
    fn loads_visible_dsh_messages() {
        let root = tempdir().expect("root");
        let path = root.path().join("session.jsonl");
        write_session(&path);
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages[1].kind,
            crate::session_manager::SessionMessageKind::Reasoning
        );
        assert_eq!(messages[1].content, "hidden");
        assert_eq!(
            messages[2].kind,
            crate::session_manager::SessionMessageKind::Text
        );
        assert_eq!(messages[2].content, "world");
    }

    #[test]
    fn scans_dsh_session_layout() {
        let root = tempdir().expect("root");
        let dir = root.path().join("workspace").join("session");
        fs::create_dir_all(&dir).expect("mkdir");
        write_session(&dir.join("session.jsonl"));
        let sessions = scan_root(root.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "dsh-1");
    }

    #[test]
    fn falls_back_to_stream_blocks_and_keeps_tool_commands() {
        let root = tempdir().expect("root");
        let path = root.path().join("session.jsonl");
        let lines = [
            json!({"type":"user/message","seq":0,"time":1_700_000_000_100_i64,"data":{"content":[{"type":"text","text":"hello"}]}}),
            json!({"type":"assistant/chunk","seq":1,"time":1_700_000_000_200_i64,"data":{"turn":1,"step":1,"chunk":{"type":"block-end","index":0,"block":{"type":"reasoning","text":"plan"}}}}),
            json!({"type":"tool/call","seq":2,"time":1_700_000_000_300_i64,"data":{"turn":1,"step":1,"callId":"call-1","name":"bash","arguments":"{\"command\":\"ls\"}"}}),
            json!({"type":"tool/result","seq":3,"time":1_700_000_000_400_i64,"data":{"turn":1,"step":1,"message":{"content":[{"type":"tool-result","toolCallId":"call-1","content":[{"type":"text","text":"file.txt"}]}]}}}),
        ];
        fs::write(
            &path,
            lines
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .expect("write");

        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 4);
        assert_eq!(
            messages[1].kind,
            crate::session_manager::SessionMessageKind::Reasoning
        );
        assert_eq!(messages[1].content, "plan");
        assert_eq!(
            messages[2].kind,
            crate::session_manager::SessionMessageKind::ToolCall
        );
        assert_eq!(messages[2].tool_name.as_deref(), Some("bash"));
        assert_eq!(messages[2].tool_call_id.as_deref(), Some("call-1"));
        assert!(messages[2].content.contains("ls"));
        assert_eq!(
            messages[3].kind,
            crate::session_manager::SessionMessageKind::ToolResult
        );
        assert_eq!(messages[3].content, "file.txt");
    }

    #[test]
    fn does_not_duplicate_canonical_tool_calls() {
        let root = tempdir().expect("root");
        let path = root.path().join("session.jsonl");
        let lines = [
            json!({"type":"assistant/chunk","time":1_i64,"data":{"turn":1,"step":1,"chunk":{"type":"block-end","index":0,"block":{"type":"tool-call","id":"call-1","name":"bash","arguments":"{}"}}}}),
            json!({"type":"tool/call","time":2_i64,"data":{"turn":1,"step":1,"callId":"call-1","name":"bash","arguments":"{}"}}),
        ];
        fs::write(
            &path,
            lines
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .expect("write");

        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].kind,
            crate::session_manager::SessionMessageKind::ToolCall
        );
    }

    #[test]
    fn recovers_partial_reasoning_and_text_chunks() {
        let root = tempdir().expect("root");
        let path = root.path().join("session.jsonl");
        let lines = [
            json!({"type":"user/message","time":1_i64,"data":{"content":[{"type":"text","text":"hello"}]}}),
            json!({"type":"reasoning-chunks","time":2_i64,"data":{"turn":1,"step":1,"index":0,"texts":["plan","ning"]}}),
            json!({"type":"text-chunks","time":3_i64,"data":{"turn":1,"step":1,"index":1,"texts":["partial"," answer"]}}),
        ];
        fs::write(
            &path,
            lines
                .into_iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .expect("write");

        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages[1].kind,
            crate::session_manager::SessionMessageKind::Reasoning
        );
        assert_eq!(messages[1].content, "planning");
        assert_eq!(
            messages[2].kind,
            crate::session_manager::SessionMessageKind::Text
        );
        assert_eq!(messages[2].content, "partial answer");
    }

    #[test]
    fn reads_all_concatenated_zstd_frames() {
        let root = tempdir().expect("root");
        let path = root.path().join("session.jsonl.zstd");
        let lines = [
            json!({"type":"session","id":"dsh-multi","cwd":"/tmp/project","createdAt":1_700_000_000_000_i64}),
            json!({"type":"user/message","time":1_700_000_000_100_i64,"data":{"content":[{"type":"text","text":"hello"}]}}),
            json!({"type":"assistant/message","time":1_700_000_000_200_i64,"data":{"message":{"content":[{"type":"reasoning","text":"hidden"},{"type":"text","text":"world"}]}}}),
        ];
        let mut compressed = Vec::new();
        for line in lines {
            compressed.extend(ruzstd::encoding::compress_to_vec(
                std::io::Cursor::new(format!("{line}\n").into_bytes()),
                ruzstd::encoding::CompressionLevel::Uncompressed,
            ));
        }
        fs::write(&path, compressed).expect("write compressed session");
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages[1].kind,
            crate::session_manager::SessionMessageKind::Reasoning
        );
        assert_eq!(messages[1].content, "hidden");
        assert_eq!(messages[2].content, "world");
    }
}
