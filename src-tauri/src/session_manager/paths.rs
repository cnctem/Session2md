use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const CODEX_STATE_DB_FILENAME: &str = "state_5.sqlite";

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn claude_dir() -> PathBuf {
    directory_override_or("claude", || home_dir().join(".claude"))
}

pub fn codex_dir() -> PathBuf {
    directory_override_or("codex", || {
        std::env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".codex"))
    })
}

pub fn gemini_dir() -> PathBuf {
    directory_override_or("gemini", || home_dir().join(".gemini"))
}

pub fn grok_dir() -> PathBuf {
    directory_override_or("grokbuild", || home_dir().join(".grok"))
}

pub fn openclaw_dir() -> PathBuf {
    directory_override_or("openclaw", || home_dir().join(".openclaw"))
}

pub fn hermes_dir() -> PathBuf {
    directory_override_or("hermes", || {
        std::env::var_os("HERMES_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".hermes"))
    })
}

pub fn pi_sessions_dir() -> PathBuf {
    directory_override_or("pi", || home_dir().join(".pi"))
        .join("agent")
        .join("sessions")
}

pub fn opencode_dir() -> PathBuf {
    directory_override_or("opencode", || {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("opencode");
            }
        }
        home_dir().join(".local").join("share").join("opencode")
    })
}

pub fn resolved_directories() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("claude".to_string(), claude_dir().display().to_string()),
        ("codex".to_string(), codex_dir().display().to_string()),
        ("gemini".to_string(), gemini_dir().display().to_string()),
        ("grokbuild".to_string(), grok_dir().display().to_string()),
        ("opencode".to_string(), opencode_dir().display().to_string()),
        ("openclaw".to_string(), openclaw_dir().display().to_string()),
        ("hermes".to_string(), hermes_dir().display().to_string()),
        (
            "pi".to_string(),
            pi_sessions_dir()
                .parent()
                .and_then(Path::parent)
                .unwrap_or_else(|| Path::new("."))
                .display()
                .to_string(),
        ),
    ])
}

fn directory_override_or(directory_id: &str, default: impl FnOnce() -> PathBuf) -> PathBuf {
    crate::session2md_settings::directory_override(directory_id).unwrap_or_else(default)
}

pub fn read_codex_config_text(config_dir: &Path) -> String {
    std::fs::read_to_string(config_dir.join("config.toml")).unwrap_or_default()
}

pub fn codex_state_db_paths(config_dir: &Path, config_text: &str) -> Vec<PathBuf> {
    let mut paths = vec![config_dir.join(CODEX_STATE_DB_FILENAME)];
    let sqlite_home = toml::from_str::<toml::Value>(config_text)
        .ok()
        .and_then(|value| value.get("sqlite_home")?.as_str().map(str::to_owned))
        .or_else(|| std::env::var("CODEX_SQLITE_HOME").ok());

    if let Some(path) = sqlite_home
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(resolve_user_path)
    {
        let db_path = path.join(CODEX_STATE_DB_FILENAME);
        if !paths.contains(&db_path) {
            paths.push(db_path);
        }
    }

    paths
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
