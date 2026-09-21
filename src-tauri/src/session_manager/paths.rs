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

pub fn cursor_dir() -> PathBuf {
    directory_override_or("cursor", || home_dir().join(".cursor"))
}

pub fn antigravity_dir() -> PathBuf {
    directory_override_or("antigravity", || {
        home_dir().join(".gemini").join("antigravity-cli")
    })
}

pub fn reasonix_dir() -> PathBuf {
    directory_override_or("reasonix", || home_dir().join(".reasonix"))
}

pub fn mimocode_dir() -> PathBuf {
    directory_override_or("mimocode", || xdg_data_home().join("mimocode"))
}

pub fn zcode_dir() -> PathBuf {
    directory_override_or("zcode", || home_dir().join(".zcode"))
}

pub fn kimi_dir() -> PathBuf {
    directory_override_or("kimi", || home_dir().join(".kimi-code"))
}

pub fn kilocode_dir() -> PathBuf {
    directory_override_or("kilocode", || xdg_data_home().join("kilo"))
}

pub fn qoder_dir() -> PathBuf {
    directory_override_or("qoder", || home_dir().join(".qoder"))
}

pub fn workbuddy_dir() -> PathBuf {
    directory_override_or("workbuddy", || home_dir().join(".workbuddy"))
}

pub fn qwen_dir() -> PathBuf {
    directory_override_or("qwen", || home_dir().join(".qwenworkcn"))
}

pub fn continue_dir() -> PathBuf {
    directory_override_or("continue", || {
        std::env::var_os("CONTINUE_GLOBAL_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".continue"))
    })
}

pub fn cline_dir() -> PathBuf {
    directory_override_or("cline", || home_dir().join(".cline"))
}

pub fn goose_data_dir() -> PathBuf {
    directory_override_or("goose", || {
        if let Some(root) = std::env::var_os("GOOSE_PATH_ROOT") {
            let path = PathBuf::from(root);
            if path.is_absolute() {
                return path.join("data");
            }
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home_dir().join("AppData").join("Roaming"))
                .join("Block")
                .join("goose")
                .join("data")
        }
        #[cfg(target_os = "macos")]
        {
            home_dir()
                .join("Library")
                .join("Application Support")
                .join("Block")
                .join("goose")
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            home_dir().join(".local").join("share").join("goose")
        }
    })
}

pub fn zed_data_dir() -> PathBuf {
    directory_override_or("zed", || {
        #[cfg(target_os = "windows")]
        {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home_dir().join("AppData").join("Local"))
                .join("Zed")
        }
        #[cfg(target_os = "macos")]
        {
            home_dir()
                .join("Library")
                .join("Application Support")
                .join("Zed")
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            xdg_data_home().join("zed")
        }
    })
}

pub fn crush_data_dir() -> PathBuf {
    directory_override_or("crush", || {
        if let Some(root) = std::env::var_os("CRUSH_GLOBAL_DATA") {
            let path = PathBuf::from(root);
            if path.is_absolute() {
                return path;
            }
        }
        #[cfg(target_os = "windows")]
        {
            if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
                return PathBuf::from(local_app_data).join("crush");
            }
        }
        xdg_data_home().join("crush")
    })
}

pub fn teleagent_data_dir() -> PathBuf {
    directory_override_or("teleagent", || {
        std::env::var_os("TELEAGENT_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".local").join("share").join("TeleAgent"))
    })
}

pub fn dsh_dir() -> PathBuf {
    directory_override_or("dsh", || {
        std::env::var_os("DSH_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".dsh"))
    })
}

fn xdg_data_home() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".local").join("share"))
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
        ("cursor".to_string(), cursor_dir().display().to_string()),
        (
            "antigravity".to_string(),
            antigravity_dir().display().to_string(),
        ),
        ("reasonix".to_string(), reasonix_dir().display().to_string()),
        ("mimocode".to_string(), mimocode_dir().display().to_string()),
        ("zcode".to_string(), zcode_dir().display().to_string()),
        ("kimi".to_string(), kimi_dir().display().to_string()),
        ("kilocode".to_string(), kilocode_dir().display().to_string()),
        ("qoder".to_string(), qoder_dir().display().to_string()),
        (
            "workbuddy".to_string(),
            workbuddy_dir().display().to_string(),
        ),
        ("qwen".to_string(), qwen_dir().display().to_string()),
        ("continue".to_string(), continue_dir().display().to_string()),
        ("cline".to_string(), cline_dir().display().to_string()),
        ("goose".to_string(), goose_data_dir().display().to_string()),
        ("zed".to_string(), zed_data_dir().display().to_string()),
        ("crush".to_string(), crush_data_dir().display().to_string()),
        (
            "teleagent".to_string(),
            teleagent_data_dir().display().to_string(),
        ),
        ("dsh".to_string(), dsh_dir().display().to_string()),
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
