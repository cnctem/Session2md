pub mod paths;
pub mod providers;

use serde::Serialize;
use std::path::Path;

use providers::{claude, codex, gemini, grokbuild, hermes, openclaw, opencode, pi};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub provider_id: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<i64>,
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let (r1, r2, r3, r4, r5, r6, r7, r8) = std::thread::scope(|scope| {
        let codex = scope.spawn(codex::scan_sessions);
        let claude = scope.spawn(claude::scan_sessions);
        let opencode = scope.spawn(opencode::scan_sessions);
        let openclaw = scope.spawn(openclaw::scan_sessions);
        let gemini = scope.spawn(gemini::scan_sessions);
        let hermes = scope.spawn(hermes::scan_sessions);
        let grokbuild = scope.spawn(grokbuild::scan_sessions);
        let pi = scope.spawn(pi::scan_sessions);
        (
            codex.join().unwrap_or_default(),
            claude.join().unwrap_or_default(),
            opencode.join().unwrap_or_default(),
            openclaw.join().unwrap_or_default(),
            gemini.join().unwrap_or_default(),
            hermes.join().unwrap_or_default(),
            grokbuild.join().unwrap_or_default(),
            pi.join().unwrap_or_default(),
        )
    });

    let mut sessions = Vec::new();
    sessions.extend(r1);
    sessions.extend(r2);
    sessions.extend(r3);
    sessions.extend(r4);
    sessions.extend(r5);
    sessions.extend(r6);
    sessions.extend(r7);
    sessions.extend(r8);
    sessions.sort_by(|a, b| {
        let a_ts = a.last_active_at.or(a.created_at).unwrap_or(0);
        let b_ts = b.last_active_at.or(b.created_at).unwrap_or(0);
        b_ts.cmp(&a_ts)
    });
    sessions
}

pub fn load_messages(provider_id: &str, source_path: &str) -> Result<Vec<SessionMessage>, String> {
    if provider_id == "opencode" && source_path.starts_with("sqlite:") {
        return opencode::load_messages_sqlite(source_path);
    }
    if provider_id == "hermes" && source_path.starts_with("sqlite:") {
        return hermes::load_messages_sqlite(source_path);
    }

    let path = Path::new(source_path);
    match provider_id {
        "codex" => codex::load_messages(path),
        "claude" => claude::load_messages(path),
        "opencode" => opencode::load_messages(path),
        "openclaw" => openclaw::load_messages(path),
        "gemini" => gemini::load_messages(path),
        "grokbuild" => grokbuild::load_messages(path),
        "hermes" => hermes::load_messages(path),
        "pi" => pi::load_messages(path),
        _ => Err(format!("Unsupported provider: {provider_id}")),
    }
}
