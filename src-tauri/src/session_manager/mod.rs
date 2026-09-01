pub mod paths;
pub mod providers;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionRequest {
    pub provider_id: String,
    pub session_id: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionOutcome {
    pub provider_id: String,
    pub session_id: String,
    pub source_path: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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

pub fn delete_session(
    provider_id: &str,
    session_id: &str,
    source_path: &str,
) -> Result<bool, String> {
    if provider_id == "opencode" && source_path.starts_with("sqlite:") {
        return opencode::delete_session_sqlite(session_id, source_path);
    }
    if provider_id == "hermes" && source_path.starts_with("sqlite:") {
        return hermes::delete_session_sqlite(session_id, source_path);
    }

    let roots = provider_roots(provider_id)?;
    delete_session_with_roots(provider_id, session_id, Path::new(source_path), &roots)
}

pub fn delete_sessions(requests: &[DeleteSessionRequest]) -> Vec<DeleteSessionOutcome> {
    requests
        .iter()
        .map(|request| {
            let result = delete_session(
                &request.provider_id,
                &request.session_id,
                &request.source_path,
            );
            match result {
                Ok(true) => DeleteSessionOutcome {
                    provider_id: request.provider_id.clone(),
                    session_id: request.session_id.clone(),
                    source_path: request.source_path.clone(),
                    success: true,
                    error: None,
                },
                Ok(false) => DeleteSessionOutcome {
                    provider_id: request.provider_id.clone(),
                    session_id: request.session_id.clone(),
                    source_path: request.source_path.clone(),
                    success: false,
                    error: Some("Session was not deleted".to_string()),
                },
                Err(error) => DeleteSessionOutcome {
                    provider_id: request.provider_id.clone(),
                    session_id: request.session_id.clone(),
                    source_path: request.source_path.clone(),
                    success: false,
                    error: Some(error),
                },
            }
        })
        .collect()
}

fn delete_session_with_roots(
    provider_id: &str,
    session_id: &str,
    source_path: &Path,
    roots: &[PathBuf],
) -> Result<bool, String> {
    let validated_source = canonicalize_existing_path(source_path, "session source")?;

    let mut saw_existing_root = false;
    for root in roots {
        if !root.exists() {
            continue;
        }

        saw_existing_root = true;
        let validated_root = canonicalize_existing_path(root, "session root")?;
        if validated_source.starts_with(&validated_root) {
            return match provider_id {
                "codex" => codex::delete_session(&validated_root, &validated_source, session_id),
                "claude" => claude::delete_session(&validated_root, &validated_source, session_id),
                "opencode" => {
                    opencode::delete_session(&validated_root, &validated_source, session_id)
                }
                "openclaw" => {
                    openclaw::delete_session(&validated_root, &validated_source, session_id)
                }
                "gemini" => gemini::delete_session(&validated_root, &validated_source, session_id),
                "grokbuild" => {
                    grokbuild::delete_session(&validated_root, &validated_source, session_id)
                }
                "hermes" => hermes::delete_session(&validated_root, &validated_source, session_id),
                "pi" => pi::delete_session(&validated_root, &validated_source, session_id),
                _ => Err(format!("Unsupported provider: {provider_id}")),
            };
        }
    }

    if !saw_existing_root {
        return Err(format!("Session root not found for provider {provider_id}"));
    }

    Err(format!(
        "Session source path is outside provider roots: {}",
        source_path.display()
    ))
}

fn provider_roots(provider_id: &str) -> Result<Vec<PathBuf>, String> {
    let roots = match provider_id {
        "codex" => codex::session_roots(),
        "claude" => vec![paths::claude_dir().join("projects")],
        "opencode" => vec![opencode::get_opencode_data_dir()],
        "openclaw" => vec![paths::openclaw_dir().join("agents")],
        "gemini" => vec![paths::gemini_dir().join("tmp")],
        "grokbuild" => grokbuild::session_roots(),
        "hermes" => vec![paths::hermes_dir().join("sessions")],
        "pi" => pi::session_roots(),
        _ => return Err(format!("Unsupported provider: {provider_id}")),
    };
    Ok(roots)
}

fn canonicalize_existing_path(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("{label} not found: {}", path.display()));
    }
    path.canonicalize()
        .map_err(|error| format!("Failed to resolve {label} {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_source_path_outside_provider_roots() {
        let root = tempdir().expect("root");
        let outside = tempdir().expect("outside");
        let source = outside.path().join("session.jsonl");
        std::fs::write(&source, "{}").expect("write source");

        let error = delete_session_with_roots("codex", "session-1", &source, &[root.path().into()])
            .expect_err("outside path should be rejected");

        assert!(error.contains("outside provider roots"));
    }

    #[test]
    fn batch_delete_reports_each_failure_without_stopping() {
        let requests = vec![
            DeleteSessionRequest {
                provider_id: "unsupported".to_string(),
                session_id: "one".to_string(),
                source_path: "/missing/one".to_string(),
            },
            DeleteSessionRequest {
                provider_id: "unsupported".to_string(),
                session_id: "two".to_string(),
                source_path: "/missing/two".to_string(),
            },
        ];

        let outcomes = delete_sessions(&requests);
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(|outcome| !outcome.success));
    }
}
