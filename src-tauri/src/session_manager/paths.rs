use std::path::{Path, PathBuf};

const CODEX_STATE_DB_FILENAME: &str = "state_5.sqlite";

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn claude_dir() -> PathBuf {
    home_dir().join(".claude")
}

pub fn codex_dir() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".codex"))
}

pub fn gemini_dir() -> PathBuf {
    home_dir().join(".gemini")
}

pub fn grok_dir() -> PathBuf {
    home_dir().join(".grok")
}

pub fn openclaw_dir() -> PathBuf {
    home_dir().join(".openclaw")
}

pub fn hermes_dir() -> PathBuf {
    std::env::var_os("HERMES_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".hermes"))
}

pub fn pi_sessions_dir() -> PathBuf {
    home_dir().join(".pi").join("agent").join("sessions")
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
