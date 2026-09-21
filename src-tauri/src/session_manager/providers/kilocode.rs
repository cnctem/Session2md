use std::path::PathBuf;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::opencode_family::{self, Family};

const PROVIDER_ID: &str = "kilocode";

pub fn database_path() -> PathBuf {
    if let Some(value) = std::env::var_os("KILO_DB") {
        if !value.is_empty() {
            return PathBuf::from(value);
        }
    }
    paths::kilocode_dir().join("kilo.db")
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    opencode_family::scan_database(&database_path(), PROVIDER_ID, Family::Kilocode)
}

pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    opencode_family::load_database(source, PROVIDER_ID)
}
