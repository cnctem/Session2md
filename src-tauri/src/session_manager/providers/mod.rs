pub mod antigravity;
pub mod claude;
pub mod cline;
pub mod codex;
mod common;
pub mod continue_session;
pub mod crush;
pub mod cursor;
pub mod dsh;
pub mod gemini;
pub mod goose;
pub mod grokbuild;
pub mod hermes;
pub mod kilocode;
pub mod kimi;
pub mod mimocode;
pub mod openclaw;
pub mod opencode;
mod opencode_family;
pub mod pi;
pub mod qoder;
pub mod qwen;
pub mod reasonix;
pub mod teleagent;
mod utils;

pub(crate) fn push_message(
    messages: &mut Vec<crate::session_manager::SessionMessage>,
    role: &str,
    content: &str,
    ts: Option<i64>,
) {
    if content.trim().is_empty() {
        return;
    }
    messages.push(crate::session_manager::SessionMessage {
        role: role.to_string(),
        content: content.trim().to_string(),
        ts,
    });
}
pub mod workbuddy;
pub mod zcode;
pub mod zed;
