pub mod antigravity;
pub mod claude;
pub mod cline;
pub mod codex;
pub(crate) mod common;
pub mod continue_session;
pub mod crush;
pub mod cursor;
pub mod deveco;
pub mod dsh;
pub mod gemini;
pub mod goose;
pub mod grokbuild;
pub mod hermes;
pub mod kilocode;
pub mod kimi;
pub mod mcode;
pub mod mimocode;
pub mod openclaw;
pub mod opencode;
mod opencode_family;
pub mod pi;
pub mod qoder;
pub mod qwen;
pub mod reasonix;
pub mod teleagent;
pub mod trae;
pub mod traework;
mod utils;

use crate::session_manager::{SessionMessage, SessionMessageMetadata};

const MAX_MODEL_LABEL_BYTES: usize = 512;

pub(crate) fn clean_model_id(value: &str) -> Option<String> {
    let value = clean_model_label(value)?;
    let normalized = value.to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "unknown" | "<synthetic>" | "__secondary__"
    ) {
        return None;
    }
    Some(value)
}

pub(crate) fn model(id: Option<String>) -> Option<String> {
    clean_model_id(id.as_deref()?)
}

pub(crate) fn attach_models(messages: &mut [SessionMessage], models: Vec<String>) {
    let models = dedupe_models(models);
    if models.is_empty() {
        return;
    }
    for message in messages {
        let is_model_message = message.role.eq_ignore_ascii_case("assistant")
            || (message.role.eq_ignore_ascii_case("tool")
                && message.kind == crate::session_manager::SessionMessageKind::ToolCall);
        if !is_model_message {
            continue;
        }
        let mut merged = message
            .metadata
            .take()
            .and_then(|metadata| metadata.models)
            .unwrap_or_default();
        merged.extend(models.iter().cloned());
        let merged = dedupe_models(merged);
        message.metadata = Some(SessionMessageMetadata {
            models: (!merged.is_empty()).then_some(merged),
        });
    }
}

pub(crate) fn dedupe_models(models: Vec<String>) -> Vec<String> {
    let mut output = Vec::new();
    for candidate in models {
        if !output.iter().any(|existing| existing == &candidate) {
            output.push(candidate);
        }
    }
    output
}

fn clean_model_label(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    let mut output = String::new();
    for character in value.chars() {
        if output.len() + character.len_utf8() > MAX_MODEL_LABEL_BYTES {
            break;
        }
        output.push(character);
    }
    (!output.is_empty()).then_some(output)
}

pub(crate) fn push_message(
    messages: &mut Vec<crate::session_manager::SessionMessage>,
    role: &str,
    content: &str,
    ts: Option<i64>,
) {
    push_message_part(
        messages,
        role,
        content,
        crate::session_manager::SessionMessageKind::Text,
        ts,
        None,
        None,
    );
}

pub(crate) fn push_message_part(
    messages: &mut Vec<crate::session_manager::SessionMessage>,
    role: &str,
    content: &str,
    kind: crate::session_manager::SessionMessageKind,
    ts: Option<i64>,
    tool_call_id: Option<String>,
    tool_name: Option<String>,
) {
    if content.trim().is_empty() {
        return;
    }
    messages.push(crate::session_manager::SessionMessage {
        role: role.to_string(),
        content: content.trim().to_string(),
        kind,
        tool_call_id,
        tool_name,
        ts,
        metadata: None,
    });
}
pub mod workbuddy;
pub mod zcode;
pub mod zed;

#[cfg(test)]
mod tests {
    use super::clean_model_id;

    #[test]
    fn model_ids_filter_placeholders_and_trim_values() {
        assert_eq!(clean_model_id("  gpt-5.4  ").as_deref(), Some("gpt-5.4"));
        assert_eq!(clean_model_id("unknown"), None);
        assert_eq!(clean_model_id("<synthetic>"), None);
        assert_eq!(clean_model_id("__secondary__"), None);
    }
}
