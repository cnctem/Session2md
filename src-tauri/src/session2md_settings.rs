use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub const DIRECTORY_IDS: [&str; 8] = [
    "claude",
    "codex",
    "gemini",
    "grokbuild",
    "opencode",
    "openclaw",
    "hermes",
    "pi",
];

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session2mdSettings {
    #[serde(default)]
    pub directory_overrides: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session2mdSettingsSnapshot {
    pub directory_overrides: BTreeMap<String, String>,
    pub resolved_directories: BTreeMap<String, String>,
}

pub fn load_settings() -> Result<Session2mdSettings, String> {
    let path = settings_path();
    if !path.exists() {
        return Ok(Session2mdSettings::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read Session2md settings: {error}"))?;
    let mut settings = serde_json::from_str::<Session2mdSettings>(&content)
        .map_err(|error| format!("Failed to parse Session2md settings: {error}"))?;
    normalize_settings(&mut settings);
    Ok(settings)
}

pub fn save_settings(
    mut settings: Session2mdSettings,
) -> Result<Session2mdSettingsSnapshot, String> {
    normalize_settings(&mut settings);

    let path = settings_path();
    let parent = path
        .parent()
        .ok_or_else(|| "Failed to resolve Session2md settings directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to create Session2md settings directory: {error}"))?;

    let serialized = serde_json::to_vec_pretty(&settings)
        .map_err(|error| format!("Failed to serialize Session2md settings: {error}"))?;
    let temporary_path = path.with_extension("json.tmp");
    fs::write(&temporary_path, serialized)
        .map_err(|error| format!("Failed to write Session2md settings: {error}"))?;
    fs::rename(&temporary_path, &path)
        .map_err(|error| format!("Failed to save Session2md settings: {error}"))?;

    Ok(snapshot_from(settings))
}

pub fn get_settings_snapshot() -> Result<Session2mdSettingsSnapshot, String> {
    load_settings().map(snapshot_from)
}

pub fn directory_override(directory_id: &str) -> Option<PathBuf> {
    load_settings()
        .ok()?
        .directory_overrides
        .get(directory_id)
        .map(|value| resolve_user_path(value))
}

fn snapshot_from(settings: Session2mdSettings) -> Session2mdSettingsSnapshot {
    Session2mdSettingsSnapshot {
        directory_overrides: settings.directory_overrides,
        resolved_directories: crate::session_manager::paths::resolved_directories(),
    }
}

fn normalize_settings(settings: &mut Session2mdSettings) {
    settings.directory_overrides.retain(|directory_id, value| {
        if !DIRECTORY_IDS.contains(&directory_id.as_str()) {
            return false;
        }

        let trimmed = value.trim();
        if trimmed.is_empty() {
            return false;
        }

        *value = trimmed.to_string();
        true
    });
}

fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("session2md")
        .join("settings.json")
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn resolve_user_path(value: &str) -> PathBuf {
    if value == "~" {
        home_dir()
    } else if let Some(rest) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        home_dir().join(rest)
    } else {
        PathBuf::from(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_settings, Session2mdSettings};
    use std::collections::BTreeMap;

    #[test]
    fn normalization_keeps_only_known_non_empty_directory_overrides() {
        let mut settings = Session2mdSettings {
            directory_overrides: BTreeMap::from([
                ("codex".to_string(), "  /tmp/codex  ".to_string()),
                ("unknown".to_string(), "/tmp/unknown".to_string()),
                ("claude".to_string(), "   ".to_string()),
            ]),
        };

        normalize_settings(&mut settings);

        assert_eq!(
            settings.directory_overrides,
            BTreeMap::from([("codex".to_string(), "/tmp/codex".to_string())])
        );
    }
}
