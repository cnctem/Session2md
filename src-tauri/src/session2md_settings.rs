use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

pub const SESSION_PROVIDER_IDS: [&str; 25] = [
    "codex",
    "claude",
    "gemini",
    "grokbuild",
    "opencode",
    "openclaw",
    "hermes",
    "pi",
    "dsh",
    "kimi",
    "zcode",
    "qoder",
    "qwen",
    "workbuddy",
    "antigravity",
    "cursor",
    "kilocode",
    "cline",
    "goose",
    "zed",
    "mimocode",
    "reasonix",
    "continue",
    "crush",
    "teleagent",
];

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session2mdSettings {
    #[serde(default)]
    pub directory_overrides: BTreeMap<String, String>,
    #[serde(default)]
    pub hidden_providers: BTreeSet<String>,
    #[serde(default)]
    pub export_thinking: bool,
    #[serde(default)]
    pub export_tool_inputs: bool,
    #[serde(default)]
    pub export_tool_outputs: bool,
    #[serde(default)]
    pub default_expand_thinking: bool,
    #[serde(default)]
    pub default_expand_tools: bool,
    #[serde(default)]
    pub default_expand_system: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session2mdSettingsSnapshot {
    pub directory_overrides: BTreeMap<String, String>,
    pub hidden_providers: BTreeSet<String>,
    pub export_thinking: bool,
    pub export_tool_inputs: bool,
    pub export_tool_outputs: bool,
    pub default_expand_thinking: bool,
    pub default_expand_tools: bool,
    pub default_expand_system: bool,
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
        hidden_providers: settings.hidden_providers,
        export_thinking: settings.export_thinking,
        export_tool_inputs: settings.export_tool_inputs,
        export_tool_outputs: settings.export_tool_outputs,
        default_expand_thinking: settings.default_expand_thinking,
        default_expand_tools: settings.default_expand_tools,
        default_expand_system: settings.default_expand_system,
        resolved_directories: crate::session_manager::paths::resolved_directories(),
    }
}

fn normalize_settings(settings: &mut Session2mdSettings) {
    settings.directory_overrides.retain(|directory_id, value| {
        if !SESSION_PROVIDER_IDS.contains(&directory_id.as_str()) {
            return false;
        }

        let trimmed = value.trim();
        if trimmed.is_empty() {
            return false;
        }

        *value = trimmed.to_string();
        true
    });
    settings
        .hidden_providers
        .retain(|provider_id| SESSION_PROVIDER_IDS.contains(&provider_id.as_str()));
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
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn normalization_keeps_only_known_non_empty_directory_overrides() {
        let mut settings = Session2mdSettings {
            directory_overrides: BTreeMap::from([
                ("codex".to_string(), "  /tmp/codex  ".to_string()),
                ("unknown".to_string(), "/tmp/unknown".to_string()),
                ("claude".to_string(), "   ".to_string()),
            ]),
            hidden_providers: BTreeSet::new(),
            ..Session2mdSettings::default()
        };

        normalize_settings(&mut settings);

        assert_eq!(
            settings.directory_overrides,
            BTreeMap::from([("codex".to_string(), "/tmp/codex".to_string())])
        );
    }

    #[test]
    fn normalization_keeps_only_known_hidden_providers() {
        let mut settings = Session2mdSettings {
            directory_overrides: BTreeMap::new(),
            hidden_providers: BTreeSet::from([
                "codex".to_string(),
                "unknown".to_string(),
                "dsh".to_string(),
            ]),
            ..Session2mdSettings::default()
        };

        normalize_settings(&mut settings);

        assert_eq!(
            settings.hidden_providers,
            BTreeSet::from(["codex".to_string(), "dsh".to_string()])
        );
    }

    #[test]
    fn missing_hidden_providers_defaults_to_empty() {
        let settings: Session2mdSettings =
            serde_json::from_str(r#"{"directoryOverrides":{}}"#).expect("settings");

        assert!(settings.hidden_providers.is_empty());
        assert!(!settings.export_thinking);
        assert!(!settings.export_tool_inputs);
        assert!(!settings.export_tool_outputs);
        assert!(!settings.default_expand_thinking);
        assert!(!settings.default_expand_tools);
        assert!(!settings.default_expand_system);
    }
}
