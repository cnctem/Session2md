use std::fs::{self, File};
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{messages_from_content, walk_files, MAX_SESSION_BYTES};

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
    values
        .iter()
        .flat_map(|value| {
            let event_type = value.get("type").and_then(Value::as_str)?;
            let role = match event_type {
                "user/message" => "user",
                "assistant/message" => "assistant",
                "tool/result" => "tool",
                _ => return None,
            };
            let content = if role == "user" {
                value.get("data")?.get("content")
            } else if role == "tool" {
                value.get("data")?.get("message")?.get("content")
            } else {
                value.get("data")?.get("message")?.get("content")
            }?;
            let ts = value
                .get("time")
                .and_then(crate::session_manager::providers::utils::parse_timestamp_to_ms);
            Some(messages_from_content(role, content, ts))
        })
        .flatten()
        .collect()
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
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "hidden\n\nworld");
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
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "hidden\n\nworld");
    }
}
