use std::path::PathBuf;

use crate::session_manager::{paths, SessionMessage, SessionMeta};

use super::opencode_family::{self, Family};

const PROVIDER_ID: &str = "zcode";

pub fn database_path() -> PathBuf {
    paths::zcode_dir().join("cli").join("db").join("db.sqlite")
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    opencode_family::scan_database(&database_path(), PROVIDER_ID, Family::Zcode)
}

pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    opencode_family::load_database(source, PROVIDER_ID)
}
