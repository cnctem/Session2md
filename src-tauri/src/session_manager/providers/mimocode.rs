use std::path::PathBuf;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::opencode_family::{self, Family};

const PROVIDER_ID: &str = "mimocode";

pub fn database_path() -> PathBuf {
    paths::mimocode_dir().join("mimocode.db")
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    opencode_family::scan_database(&database_path(), PROVIDER_ID, Family::Mimocode)
}

pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    opencode_family::load_database(source, PROVIDER_ID)
}
