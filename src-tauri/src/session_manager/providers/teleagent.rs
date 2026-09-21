use std::fs;
use std::path::PathBuf;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::opencode_family::{self, Family};

const PROVIDER_ID: &str = "teleagent";

pub fn database_paths() -> Vec<PathBuf> {
    let root = paths::teleagent_data_dir();
    let direct = root.join("teleagent.db");
    if direct.is_file() {
        return vec![direct];
    }
    let users = if root.file_name().and_then(|name| name.to_str()) == Some("users") {
        root
    } else {
        root.join("users")
    };
    let Ok(entries) = fs::read_dir(users) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
        .map(|entry| entry.path().join("teleagent.db"))
        .filter(|path| path.is_file())
        .collect()
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    database_paths()
        .iter()
        .flat_map(|path| opencode_family::scan_database(path, PROVIDER_ID, Family::Teleagent))
        .collect()
}

pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    opencode_family::load_database(source, PROVIDER_ID)
}
