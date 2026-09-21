use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{messages_from_content, read_json, walk_files};

const PROVIDER_ID: &str = "cline";
const MAX_SESSION_BYTES: u64 = 128 * 1024 * 1024;

pub fn session_roots() -> Vec<PathBuf> {
    let configured = paths::cline_dir();
    let uses_override = crate::session2md_settings::directory_override("cline").is_some();
    let modern = std::env::var_os("CLINE_SESSION_DATA_DIR")
        .filter(|_| !uses_override)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("CLINE_DATA_DIR")
                .filter(|_| !uses_override)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| configured.join("data"))
                .join("sessions")
        });
    let mut roots = vec![modern];
    if !uses_override {
        roots.extend(legacy_roots());
    }
    roots
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let mut seen = HashSet::new();
    session_roots()
        .into_iter()
        .flat_map(|root| walk_files(&root, is_candidate))
        .filter_map(|path| parse_session(&path).ok())
        .filter(|session| seen.insert(session.session_id.clone()))
        .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let value = read_json(path, MAX_SESSION_BYTES)?;
    let messages = value
        .get("messages")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| format!("Cline transcript has no messages array: {}", path.display()))?;
    Ok(messages.iter().flat_map(visible_messages).collect())
}

fn visible_messages(message: &Value) -> Vec<SessionMessage> {
    let Some(role) = message.get("role").and_then(Value::as_str) else {
        return Vec::new();
    };
    if role != "user" && role != "assistant" {
        return Vec::new();
    }
    let Some(content) = message.get("content") else {
        return Vec::new();
    };
    messages_from_content(
        role,
        content,
        message
            .get("ts")
            .and_then(super::utils::parse_timestamp_to_ms),
    )
}

fn parse_session(path: &Path) -> Result<SessionMeta, String> {
    let messages_value = read_json(path, MAX_SESSION_BYTES)?;
    let agent = messages_value
        .get("agent")
        .and_then(Value::as_str)
        .unwrap_or("lead");
    if agent != "lead" {
        return Err("subagent transcript".to_string());
    }
    let legacy =
        path.file_name().and_then(|name| name.to_str()) == Some("api_conversation_history.json");
    let session_id = if legacy {
        path.parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string()
    } else {
        path.file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".messages.json"))
            .unwrap_or_default()
            .to_string()
    };
    if session_id.is_empty() {
        return Err("missing Cline session id".to_string());
    }
    let messages = load_messages(path)?;
    let manifest = if legacy {
        None
    } else {
        let manifest_path = path
            .parent()
            .map(|parent| parent.join(format!("{session_id}.json")));
        manifest_path.and_then(|path| read_json(&path, 16 * 1024 * 1024).ok())
    };
    let title = manifest
        .as_ref()
        .and_then(|manifest| {
            manifest
                .get("metadata")
                .and_then(|metadata| metadata.get("title"))
                .and_then(Value::as_str)
        })
        .filter(|title| !title.trim().is_empty())
        .map(|title| super::utils::truncate_summary(title, 80))
        .or_else(|| {
            messages
                .iter()
                .find(|message| message.role == "user")
                .map(|message| super::utils::truncate_summary(&message.content, 80))
        });
    let project_dir = manifest.as_ref().and_then(|manifest| {
        ["workspace_root", "cwd"]
            .iter()
            .find_map(|key| manifest.get(*key).and_then(Value::as_str))
            .or_else(|| {
                manifest
                    .get("metadata")
                    .and_then(|metadata| metadata.get("cwd"))
                    .and_then(Value::as_str)
            })
            .map(str::to_string)
    });
    let created_at = manifest
        .as_ref()
        .and_then(|manifest| manifest.get("started_at"))
        .and_then(super::utils::parse_timestamp_to_ms)
        .or_else(|| messages.first().and_then(|message| message.ts));
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

fn is_candidate(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if name.ends_with(".messages.json") {
        return true;
    }
    name == "api_conversation_history.json"
}

fn legacy_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let names = ["Code", "Code - Insiders", "VSCodium"];
    #[cfg(target_os = "windows")]
    {
        if let Some(app_data) = std::env::var_os("APPDATA") {
            for name in names {
                roots.push(
                    PathBuf::from(&app_data)
                        .join(name)
                        .join("User")
                        .join("globalStorage")
                        .join("saoudrizwan.claude-dev"),
                );
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        for name in names {
            roots.push(
                paths::home_dir()
                    .join("Library")
                    .join("Application Support")
                    .join(name)
                    .join("User")
                    .join("globalStorage")
                    .join("saoudrizwan.claude-dev"),
            );
        }
    }
    #[cfg(target_os = "linux")]
    {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| paths::home_dir().join(".config"));
        for name in names {
            roots.push(
                config_home
                    .join(name)
                    .join("User")
                    .join("globalStorage")
                    .join("saoudrizwan.claude-dev"),
            );
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_cline_visible_messages() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("cline-1.messages.json");
        fs::write(
            &path,
            serde_json::json!({
                "agent": "lead",
                "messages": [
                    {"role": "user", "content": [{"type": "text", "text": "hello"}]},
                    {"role": "assistant", "content": [{"type": "text", "text": "answer"}, {"type": "tool_use", "name": "read"}]}
                ]
            })
            .to_string(),
        )
        .expect("write");
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].content, "answer");
        assert_eq!(messages[2].role, "tool");
    }
}
