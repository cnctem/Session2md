use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::common::{
    messages_from_parts, normalize_content_parts, read_jsonl, walk_files, ContentPart,
};

const PROVIDER_ID: &str = "reasonix";

pub fn session_roots() -> Vec<PathBuf> {
    if let Some(configured) = crate::session2md_settings::directory_override("reasonix") {
        return vec![
            if configured.file_name().and_then(|name| name.to_str()) == Some("sessions") {
                configured
            } else {
                configured.join("sessions")
            },
        ];
    }
    let mut roots = vec![paths::reasonix_dir().join("sessions")];
    if let Some(app_data) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(app_data).join("reasonix"));
    }
    roots
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let mut seen = std::collections::HashSet::new();
    session_roots()
        .into_iter()
        .flat_map(|root| walk_files(&root, is_candidate))
        .filter_map(|path| parse_session(&path).ok())
        .filter(|session| seen.insert(session.session_id.clone()))
        .collect()
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let values = load_values_with_wal(path)?;
    Ok(values
        .iter()
        .flat_map(|value| {
            let role = value.get("role").and_then(Value::as_str)?;
            let ts = value
                .get("createdAt")
                .and_then(super::utils::parse_timestamp_to_ms);
            let mut parts = value
                .get("content")
                .map(normalize_content_parts)
                .unwrap_or_default();
            if let Some(reasoning) = value.get("reasoning_content").and_then(Value::as_str) {
                parts.push(ContentPart::reasoning(reasoning));
            }
            if let Some(tool_calls) = value.get("tool_calls") {
                parts.extend(normalize_content_parts(tool_calls));
            }
            Some(messages_from_parts(role, &parts, ts))
        })
        .flatten()
        .collect())
}

fn load_values_with_wal(path: &Path) -> Result<Vec<Value>, String> {
    let mut values = read_jsonl(path)?;
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return Ok(values);
    };
    let wal_path = path.with_file_name(format!("{stem}.events.jsonl"));
    if !wal_path.is_file() {
        return Ok(values);
    }
    let wal = read_jsonl(&wal_path)?;
    for event in wal {
        if event.get("type").and_then(Value::as_str) == Some("replace") {
            if let Some(messages) = event.get("messages").and_then(Value::as_array) {
                values = messages.clone();
            }
        } else if event.get("role").is_some() {
            values.push(event);
        }
    }
    Ok(values)
}

fn parse_session(path: &Path) -> Result<SessionMeta, String> {
    let values = read_jsonl(path)?;
    let session_id = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    if session_id.starts_with("subagent-") {
        return Err("subagent transcript".to_string());
    }
    let messages = load_messages(path)?;
    let explicit_title = desktop_title(path, &session_id);
    let title = explicit_title.or_else(|| {
        messages
            .iter()
            .find(|message| message.role == "user")
            .map(|message| super::utils::truncate_summary(&message.content, 80))
            .filter(|title| !title.is_empty())
    });
    let created_at = values.iter().find_map(|value| {
        value
            .get("createdAt")
            .and_then(super::utils::parse_timestamp_to_ms)
    });
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
        project_dir: path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(str::to_string),
        created_at,
        last_active_at: last_active_at.or(created_at),
        source_path: Some(path.display().to_string()),
        resume_command: None,
        can_delete: false,
    })
}

fn desktop_title(path: &Path, session_id: &str) -> Option<String> {
    let titles = path.parent()?.join(".titles.json");
    let raw = fs::read_to_string(titles).ok()?;
    let values: Value = serde_json::from_str(&raw).ok()?;
    values
        .get(session_id)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| super::utils::truncate_summary(title, 80))
}

fn is_candidate(path: &Path) -> bool {
    if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if name.ends_with(".events.jsonl")
        || name.ends_with(".conflicts.jsonl")
        || name.ends_with(".guardian.jsonl")
    {
        return false;
    }
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let desktop = path
        .to_string_lossy()
        .replace('\\', "/")
        .contains("/projects/");
    desktop || stem.starts_with("desktop-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_visible_reasonix_text() {
        let root = tempfile::tempdir().expect("root");
        let path = root.path().join("desktop-202601010101-1.jsonl");
        fs::write(
            &path,
            [
                r#"{"role":"user","content":"hello"}"#,
                r#"{"role":"assistant","content":"answer","reasoning_content":"hidden","tool_calls":[]}"#,
            ]
            .join("\n"),
        )
        .expect("write");
        let messages = load_messages(&path).expect("messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].content, "answer\n\nhidden");
    }
}
