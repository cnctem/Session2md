use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use rusqlite::{types::ValueRef, Connection, OptionalExtension};
use serde_json::{Map, Value};
use url::Url;

use crate::session_manager::{
    paths, SessionMessage, SessionMessageKind, SessionMeta, SessionRecovery,
};

use super::common::open_sqlite_readonly;
use super::{attach_models, dedupe_models, model, push_message_part};

const PROVIDER_TRAE: &str = "trae";
const PROVIDER_TRAEWORK: &str = "traework";
const STATE_DB_NAME: &str = "state.vscdb";
const ACTIVE_SESSION_KEY: &str = "ai-chat-v2.lastActiveSessionId";
const INPUT_HISTORY_KEY: &str = "icube-ai-agent-storage-input-history";
const QUERY_COMPLETION_PREFIX: &str = "ai-chat.chatQueryCompletion.v2.";
const MAX_JSON_BYTES: usize = 256 * 1024 * 1024;

const EXACT_CHAT_KEYS: &[&str] = &[
    "memento/icube-ai-agent-storage",
    "chat.ChatSessionStore.index",
    "ChatStore",
];
const CHAT_KEY_PREFIXES: &[&str] = &[
    "memento/icube-ai-agent-storage-",
    "memento/icube-ai-chat-storage-",
    "memento/icube-ai-ng-chat-storage-",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraeProduct {
    Trae,
    TraeWork,
}

impl TraeProduct {
    fn provider_id(self) -> &'static str {
        match self {
            Self::Trae => PROVIDER_TRAE,
            Self::TraeWork => PROVIDER_TRAEWORK,
        }
    }

    fn default_root(self) -> PathBuf {
        match self {
            Self::Trae => paths::trae_dir(),
            Self::TraeWork => paths::traework_dir(),
        }
    }
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    scan_product(TraeProduct::Trae)
}

pub(crate) fn scan_product(product: TraeProduct) -> Vec<SessionMeta> {
    scan_root(product, &product.default_root())
}

fn scan_root(product: TraeProduct, root: &Path) -> Vec<SessionMeta> {
    let mut sessions = BTreeMap::<String, RawSession>::new();
    for db_path in database_paths(root) {
        scan_database(product, &db_path, &mut sessions);
    }

    sessions
        .into_values()
        .filter_map(finish_session)
        .collect::<Vec<_>>()
}

pub fn load_messages(source: &str) -> Result<Vec<SessionMessage>, String> {
    let (db_path, session_id) = super::common::parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid Trae SQLite source reference: {source}"))?;
    let conn = open_db(&db_path)?;
    let mut sessions = BTreeMap::<String, RawSession>::new();
    scan_connection(TraeProduct::Trae, &db_path, &conn, &mut sessions);
    let Some(raw) = sessions.remove(&session_id) else {
        return Ok(Vec::new());
    };
    let mut messages = raw.messages;
    attach_models(&mut messages, raw.models);
    Ok(messages)
}

fn scan_database(
    product: TraeProduct,
    db_path: &Path,
    sessions: &mut BTreeMap<String, RawSession>,
) {
    let Ok(conn) = open_db(db_path) else {
        return;
    };
    scan_connection(product, db_path, &conn, sessions);
}

fn scan_connection(
    product: TraeProduct,
    db_path: &Path,
    conn: &Connection,
    sessions: &mut BTreeMap<String, RawSession>,
) {
    let keys = match item_keys(conn) {
        Ok(keys) => keys,
        Err(_) => return,
    };
    let project_dir = workspace_project(db_path);
    let mtime = file_mtime_ms(db_path);
    let mut active_session_id = None;
    let mut query_completions = Vec::<(String, Value)>::new();
    let mut input_history = Vec::<Value>::new();

    for key in keys {
        if key == ACTIVE_SESSION_KEY {
            if let Some(value) = read_json_value(conn, &key) {
                active_session_id = value.as_str().map(str::to_string);
            }
            continue;
        }
        if let Some(session_id) = key.strip_prefix(QUERY_COMPLETION_PREFIX) {
            if let Some(value) = read_json_value(conn, &key) {
                query_completions.push((session_id.to_string(), value));
            }
            continue;
        }
        if key == INPUT_HISTORY_KEY {
            if let Some(value) = read_json_value(conn, &key) {
                input_history.push(value);
            }
            continue;
        }
        if !is_chat_key(&key) {
            continue;
        }
        let Some(value) = read_json_value(conn, &key) else {
            continue;
        };
        if active_session_id.is_none() {
            active_session_id = value
                .get("currentSessionId")
                .or_else(|| value.get("current_session_id"))
                .and_then(Value::as_str)
                .filter(|session_id| !session_id.trim().is_empty())
                .map(str::to_string);
        }
        for (fallback_id, record) in session_records(&value) {
            let Some(mut raw) = parse_session_record(
                product,
                db_path,
                fallback_id.as_deref(),
                project_dir.as_deref(),
                mtime,
                &record,
            ) else {
                continue;
            };
            merge_raw_session(sessions, &mut raw);
        }
    }

    if let Some(session_id) = active_session_id {
        let mut raw =
            RawSession::metadata_only(product, db_path, &session_id, project_dir.clone(), mtime);
        merge_raw_session(sessions, &mut raw);
    }

    for (session_id, value) in query_completions {
        let messages = parse_query_completion_messages(&value);
        if messages.is_empty() {
            continue;
        }
        let mut raw =
            RawSession::metadata_only(product, db_path, &session_id, project_dir.clone(), mtime);
        raw.messages = messages;
        merge_raw_session(sessions, &mut raw);
    }

    for value in input_history {
        for (session_id, record) in input_history_records(&value) {
            let Some(session_id) = session_id else {
                continue;
            };
            let Some(message) = parse_user_message(&record) else {
                continue;
            };
            let mut raw = RawSession::metadata_only(
                product,
                db_path,
                &session_id,
                project_dir.clone(),
                mtime,
            );
            raw.messages.push(message);
            merge_raw_session(sessions, &mut raw);
        }
    }
}

#[derive(Debug, Clone)]
struct RawSession {
    provider_id: &'static str,
    session_id: String,
    title: Option<String>,
    project_dir: Option<String>,
    created_at: Option<i64>,
    last_active_at: Option<i64>,
    source_path: String,
    models: Vec<String>,
    messages: Vec<SessionMessage>,
    full_messages: bool,
}

impl RawSession {
    fn metadata_only(
        product: TraeProduct,
        db_path: &Path,
        session_id: &str,
        project_dir: Option<String>,
        mtime: Option<i64>,
    ) -> Self {
        Self {
            provider_id: product.provider_id(),
            session_id: session_id.to_string(),
            title: None,
            project_dir,
            created_at: None,
            last_active_at: mtime,
            source_path: super::common::sqlite_source(db_path, session_id),
            models: Vec::new(),
            messages: Vec::new(),
            full_messages: false,
        }
    }
}

fn merge_raw_session(sessions: &mut BTreeMap<String, RawSession>, incoming: &mut RawSession) {
    let Some(existing) = sessions.get_mut(&incoming.session_id) else {
        sessions.insert(incoming.session_id.clone(), incoming.clone());
        return;
    };

    let incoming_rank = (incoming.full_messages, incoming.last_active_at.unwrap_or(0));
    let existing_rank = (existing.full_messages, existing.last_active_at.unwrap_or(0));
    if incoming_rank > existing_rank {
        incoming.title = incoming.title.take().or_else(|| existing.title.clone());
        if incoming.project_dir.is_none() {
            incoming.project_dir = existing.project_dir.clone();
        }
        incoming.created_at = incoming.created_at.or(existing.created_at);
        incoming.last_active_at = incoming.last_active_at.or(existing.last_active_at);
        incoming.models = dedupe_models(
            incoming
                .models
                .iter()
                .chain(existing.models.iter())
                .cloned()
                .collect(),
        );
        if incoming.messages.is_empty() {
            incoming.messages = existing.messages.clone();
        } else {
            append_unique_messages(&mut incoming.messages, &existing.messages);
        }
        *existing = incoming.clone();
        return;
    }

    existing.title = existing.title.take().or_else(|| incoming.title.clone());
    existing.project_dir = existing
        .project_dir
        .take()
        .or_else(|| incoming.project_dir.clone());
    existing.created_at = existing.created_at.or(incoming.created_at);
    existing.last_active_at = existing.last_active_at.or(incoming.last_active_at);
    existing.models = dedupe_models(
        existing
            .models
            .iter()
            .chain(incoming.models.iter())
            .cloned()
            .collect(),
    );
    append_unique_messages(&mut existing.messages, &incoming.messages);
    existing.full_messages |= incoming.full_messages;
}

fn append_unique_messages(target: &mut Vec<SessionMessage>, incoming: &[SessionMessage]) {
    for message in incoming {
        let duplicate = target.iter().any(|existing| {
            existing.role == message.role
                && existing.kind == message.kind
                && existing.content == message.content
        });
        if !duplicate {
            target.push(message.clone());
        }
    }
}

fn finish_session(raw: RawSession) -> Option<SessionMeta> {
    if raw.session_id.trim().is_empty() {
        return None;
    }
    let mut messages = raw.messages;
    attach_models(&mut messages, raw.models.clone());
    let first_user = messages
        .iter()
        .find(|message| message.role == "user")
        .map(|message| super::utils::truncate_summary(&message.content, 160));
    let title = raw
        .title
        .filter(|title| !title.trim().is_empty())
        .map(|title| super::utils::truncate_summary(&title, 80))
        .or_else(|| first_user.clone());
    let created_at = raw
        .created_at
        .or_else(|| messages.first().and_then(|message| message.ts));
    let last_active_at = raw
        .last_active_at
        .or(created_at)
        .or_else(|| messages.last().and_then(|message| message.ts));
    Some(SessionMeta {
        provider_id: raw.provider_id.to_string(),
        session_id: raw.session_id,
        title,
        summary: first_user,
        project_dir: raw.project_dir,
        created_at,
        last_active_at,
        source_path: Some(raw.source_path),
        resume_command: None,
        can_delete: false,
        models: (!raw.models.is_empty()).then_some(raw.models),
        recovery: Some(if raw.full_messages {
            SessionRecovery::Complete
        } else {
            SessionRecovery::Partial
        }),
    })
}

fn parse_session_record(
    product: TraeProduct,
    db_path: &Path,
    fallback_id: Option<&str>,
    workspace_project: Option<&str>,
    mtime: Option<i64>,
    value: &Value,
) -> Option<RawSession> {
    let object = value.as_object()?;
    let session_id = first_string(
        object,
        &["sessionId", "session_id", "id", "chat_session_id", "key"],
    )
    .or_else(|| fallback_id.map(str::to_string))?;
    if session_id.trim().is_empty() {
        return None;
    }

    let message_values = message_values(object);
    let messages = message_values
        .iter()
        .flat_map(|message| parse_message(message))
        .collect::<Vec<_>>();
    let has_assistant_placeholder = message_values.iter().any(|message| {
        message
            .get("role")
            .and_then(Value::as_str)
            .is_some_and(|role| {
                matches!(
                    role.to_ascii_lowercase().as_str(),
                    "assistant" | "ai" | "model" | "bot" | "agent"
                )
            })
    });
    let full_messages = !message_values.is_empty()
        && !messages.is_empty()
        && (!has_assistant_placeholder
            || messages.iter().any(|message| message.role == "assistant"));
    let title = first_string(object, &["title", "name", "chatTitle", "chat_title"]);
    let project_dir = first_string(
        object,
        &[
            "directory",
            "cwd",
            "projectPath",
            "project_path",
            "projectDir",
        ],
    )
    .or_else(|| workspace_project.map(str::to_string));
    let created_at = first_timestamp(
        object,
        &[
            "createdAt",
            "created_at",
            "createdTime",
            "created_time",
            "timestamp",
        ],
    );
    let last_active_at = first_timestamp(
        object,
        &[
            "updatedAt",
            "updated_at",
            "updatedTime",
            "updated_time",
            "lastActiveAt",
        ],
    )
    .or_else(|| messages.last().and_then(|message| message.ts))
    .or(created_at)
    .or(mtime);

    let models = session_models(object, &messages);

    Some(RawSession {
        provider_id: product.provider_id(),
        session_id: session_id.clone(),
        title,
        project_dir,
        created_at,
        last_active_at,
        source_path: super::common::sqlite_source(db_path, &session_id),
        models,
        messages,
        full_messages,
    })
}

fn session_records(value: &Value) -> Vec<(Option<String>, Value)> {
    if let Some(array) = value.as_array() {
        return array.iter().cloned().map(|item| (None, item)).collect();
    }
    let Some(object) = value.as_object() else {
        return Vec::new();
    };

    for key in [
        "list",
        "sessions",
        "conversations",
        "entries",
        "items",
        "data",
    ] {
        let Some(container) = object.get(key) else {
            continue;
        };
        if let Some(array) = container.as_array() {
            let records = array
                .iter()
                .cloned()
                .map(|item| (None, item))
                .collect::<Vec<_>>();
            if !records.is_empty() {
                return records;
            }
        } else if let Some(map) = container.as_object() {
            let records = map
                .iter()
                .filter(|(_, item)| item.is_object())
                .map(|(key, item)| (Some(key.clone()), item.clone()))
                .collect::<Vec<_>>();
            if !records.is_empty() {
                return records;
            }
        }
    }

    let records = object
        .iter()
        .filter(|(_, item)| item.is_object())
        .map(|(key, item)| (Some(key.clone()), item.clone()))
        .collect::<Vec<_>>();
    if records.len() == 1
        && object.keys().all(|key| {
            matches!(
                key.as_str(),
                "currentSessionId" | "current_session_id" | "version"
            )
        })
    {
        return Vec::new();
    }
    records
}

fn input_history_records(value: &Value) -> Vec<(Option<String>, Value)> {
    let records = session_records(value);
    records
        .into_iter()
        .filter_map(|(fallback_id, item)| {
            let object = item.as_object()?;
            let session_id = first_string(
                object,
                &[
                    "sessionId",
                    "session_id",
                    "chat_session_id",
                    "conversationId",
                    "conversation_id",
                ],
            )
            .or(fallback_id);
            Some((session_id, item))
        })
        .collect()
}

fn message_values(object: &Map<String, Value>) -> Vec<Value> {
    for key in ["messages", "conversation", "history", "bubbles", "list"] {
        let Some(value) = object.get(key) else {
            continue;
        };
        if let Some(array) = value.as_array() {
            return array.clone();
        }
        if let Some(map) = value.as_object() {
            return map.values().cloned().collect();
        }
    }
    Vec::new()
}

fn parse_message(value: &Value) -> Vec<SessionMessage> {
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    let Some(role) = first_string(object, &["role", "type", "sender"]) else {
        return Vec::new();
    };
    let role = role.to_ascii_lowercase();
    let ts = first_timestamp(
        object,
        &[
            "timestamp",
            "createdAt",
            "created_at",
            "time",
            "createdTime",
        ],
    );
    if matches!(role.as_str(), "user" | "human" | "human_message") {
        return parse_user_message(value).into_iter().collect();
    }
    if !matches!(
        role.as_str(),
        "assistant" | "ai" | "model" | "bot" | "agent"
    ) {
        return Vec::new();
    }

    let agent = object
        .get("agentTaskContent")
        .or_else(|| object.get("agent_task_content"))
        .and_then(parse_json_object);
    let mut messages = Vec::new();

    if let Some(items) = agent
        .as_ref()
        .and_then(|agent| agent.get("guideline"))
        .and_then(|guideline| {
            guideline
                .get("planItems")
                .or_else(|| guideline.get("plan_items"))
        })
        .and_then(values_of)
    {
        for item in items {
            append_plan_item(&mut messages, &item, ts);
        }
    }

    let proposal = agent
        .as_ref()
        .and_then(|agent| first_string(agent, &["proposal", "content", "text"]))
        .or_else(|| first_string(object, &["content", "text", "response", "output"]));
    if let Some(proposal) = proposal {
        push_unique_message(
            &mut messages,
            "assistant",
            &proposal,
            SessionMessageKind::Text,
            ts,
            None,
            None,
        );
    }
    attach_models(&mut messages, record_models(object));
    messages
}

fn parse_user_message(value: &Value) -> Option<SessionMessage> {
    let object = value.as_object()?;
    let text = first_string(
        object,
        &[
            "content",
            "parsedQuery",
            "parsed_query",
            "text",
            "message",
            "body",
            "prompt",
            "query",
        ],
    )?;
    let ts = first_timestamp(
        object,
        &[
            "timestamp",
            "createdAt",
            "created_at",
            "time",
            "createdTime",
        ],
    );
    Some(SessionMessage {
        role: "user".to_string(),
        content: text.trim().to_string(),
        kind: SessionMessageKind::Text,
        tool_call_id: None,
        tool_name: None,
        ts,
        metadata: None,
    })
}

fn append_plan_item(messages: &mut Vec<SessionMessage>, item: &Value, ts: Option<i64>) {
    let Some(object) = item.as_object() else {
        return;
    };
    let reasoning = first_string(
        object,
        &[
            "thought",
            "reasoning",
            "reasoningContent",
            "reasoning_content",
            "thinking",
        ],
    );
    if let Some(reasoning) = reasoning {
        push_unique_message(
            messages,
            "assistant",
            &reasoning,
            SessionMessageKind::Reasoning,
            ts,
            None,
            None,
        );
    }

    let tool_name = first_string(object, &["toolName", "tool_name", "tool", "name"]);
    let tool_call_id = first_string(object, &["id", "toolCallId", "tool_call_id", "callId"]);
    if let Some(tool_name) = tool_name {
        let arguments = object
            .get("params")
            .or_else(|| object.get("arguments"))
            .or_else(|| object.get("input"))
            .map(value_to_text)
            .unwrap_or_default();
        push_unique_message(
            messages,
            "tool",
            &arguments,
            SessionMessageKind::ToolCall,
            ts,
            tool_call_id.clone(),
            Some(tool_name.clone()),
        );

        if let Some(result) = object
            .get("result")
            .or_else(|| object.get("finish"))
            .or_else(|| object.get("output"))
        {
            let result = value_to_text(result);
            push_unique_message(
                messages,
                "tool",
                &result,
                SessionMessageKind::ToolResult,
                ts,
                tool_call_id,
                Some(tool_name),
            );
        }
    }

    if let Some(text) = first_string(object, &["content", "text"]) {
        push_unique_message(
            messages,
            "assistant",
            &text,
            SessionMessageKind::Text,
            ts,
            None,
            None,
        );
    }
}

fn push_unique_message(
    messages: &mut Vec<SessionMessage>,
    role: &str,
    content: &str,
    kind: SessionMessageKind,
    ts: Option<i64>,
    tool_call_id: Option<String>,
    tool_name: Option<String>,
) {
    let content = content.trim();
    if content.is_empty() {
        return;
    }
    if messages.iter().any(|message| {
        message.role == role
            && message.kind == kind
            && message.content == content
            && message.ts == ts
    }) {
        return;
    }
    push_message_part(messages, role, content, kind, ts, tool_call_id, tool_name);
}

fn parse_query_completion_messages(value: &Value) -> Vec<SessionMessage> {
    let Some(result) = value
        .get("response")
        .and_then(|response| response.get("result"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut messages = Vec::new();
    for item in result {
        let text = item
            .get("text")
            .or_else(|| item.get("content"))
            .map(value_to_text)
            .unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        let ts = item
            .as_object()
            .and_then(|object| first_timestamp(object, &["timestamp", "createdAt", "created_at"]));
        push_unique_message(
            &mut messages,
            "user",
            &text,
            SessionMessageKind::Text,
            ts,
            None,
            None,
        );
    }
    messages
}

fn session_models(object: &Map<String, Value>, messages: &[SessionMessage]) -> Vec<String> {
    let mut models = record_models(object);
    for message in messages {
        if let Some(metadata) = &message.metadata {
            if let Some(values) = &metadata.models {
                models.extend(values.iter().cloned());
            }
        }
    }
    dedupe_models(models)
}

fn record_models(object: &Map<String, Value>) -> Vec<String> {
    let mut models = Vec::new();
    for key in ["model", "modelId", "modelID", "modelName"] {
        if let Some(value) = object.get(key).and_then(model_from_value) {
            models.push(value);
        }
    }
    if let Some(model_info) = object.get("modelInfo").and_then(Value::as_object) {
        for key in ["display_model_name", "config_name", "id", "name"] {
            if let Some(value) = model_info
                .get(key)
                .and_then(Value::as_str)
                .and_then(|value| model(Some(value.to_string())))
            {
                models.push(value);
            }
        }
    }
    dedupe_models(models)
}

fn model_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => model(Some(value.clone())),
        Value::Object(object) => ["id", "name", "model", "modelId", "config_name"]
            .iter()
            .find_map(|key| {
                object
                    .get(*key)
                    .and_then(Value::as_str)
                    .and_then(|value| model(Some(value.to_string())))
            }),
        _ => None,
    }
}

fn first_string(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(value_text)
            .filter(|value| !value.trim().is_empty())
    })
}

fn first_timestamp(object: &Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(parse_timestamp))
}

fn parse_timestamp(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => {
            let raw = number
                .as_i64()
                .or_else(|| number.as_f64().map(|value| value as i64))?;
            Some(if raw.abs() < 100_000_000_000 {
                raw.saturating_mul(1000)
            } else {
                raw
            })
        }
        Value::String(raw) => {
            if let Ok(number) = raw.parse::<f64>() {
                return parse_timestamp(&Value::Number(serde_json::Number::from_f64(number)?));
            }
            chrono::DateTime::parse_from_rfc3339(raw)
                .ok()
                .map(|value| value.timestamp_millis())
        }
        _ => None,
    }
}

fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(value_text)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            (!text.trim().is_empty()).then_some(text)
        }
        Value::Object(object) => {
            for key in ["text", "content", "summary", "proposal", "value"] {
                if let Some(text) = object.get(key).and_then(value_text) {
                    if !text.trim().is_empty() {
                        return Some(text);
                    }
                }
            }
            if let Some(data) = object.get("data").and_then(Value::as_object) {
                if let Some(text) = data.get("summary").and_then(value_text) {
                    if !text.trim().is_empty() {
                        return Some(text);
                    }
                }
            }
            serde_json::to_string_pretty(value).ok()
        }
        Value::Null => None,
    }
}

fn value_to_text(value: &Value) -> String {
    if let Value::String(raw) = value {
        if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
            if !parsed.is_string() {
                return value_text(&parsed).unwrap_or_else(|| raw.clone());
            }
        }
    }
    value_text(value).unwrap_or_default()
}

fn parse_json_object(value: &Value) -> Option<Map<String, Value>> {
    match value {
        Value::Object(object) => Some(object.clone()),
        Value::String(raw) => serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|value| value.as_object().cloned()),
        _ => None,
    }
}

fn values_of(value: &Value) -> Option<Vec<Value>> {
    if let Some(array) = value.as_array() {
        return Some(array.clone());
    }
    value
        .as_object()
        .map(|object| object.values().cloned().collect())
}

fn database_paths(root: &Path) -> Vec<PathBuf> {
    if root.is_file() {
        return (root.file_name().and_then(|name| name.to_str()) == Some(STATE_DB_NAME))
            .then(|| vec![root.to_path_buf()])
            .unwrap_or_default();
    }
    if !root.is_dir() {
        return Vec::new();
    }

    let mut paths = BTreeSet::new();
    let basename = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if basename.eq_ignore_ascii_case("workspaceStorage") {
        collect_workspace_databases(root, &mut paths);
        return paths.into_iter().collect();
    }
    if basename.eq_ignore_ascii_case("globalStorage") {
        add_database(root.join(STATE_DB_NAME), &mut paths);
        return paths.into_iter().collect();
    }

    let user_dir = if basename.eq_ignore_ascii_case("User") {
        root.to_path_buf()
    } else if root.join("User").is_dir() {
        root.join("User")
    } else {
        root.to_path_buf()
    };
    add_database(
        user_dir.join("globalStorage").join(STATE_DB_NAME),
        &mut paths,
    );
    collect_workspace_databases(&user_dir.join("workspaceStorage"), &mut paths);
    paths.into_iter().collect()
}

fn collect_workspace_databases(root: &Path, paths: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        add_database(path.join(STATE_DB_NAME), paths);
    }
}

fn add_database(path: PathBuf, paths: &mut BTreeSet<PathBuf>) {
    if path.is_file() {
        paths.insert(path);
    }
}

fn open_db(path: &Path) -> Result<Connection, String> {
    let conn = open_sqlite_readonly(path)?;
    let _ = conn.busy_timeout(Duration::from_secs(5));
    let _ = conn.execute_batch("PRAGMA query_only = ON;");
    Ok(conn)
}

fn item_keys(conn: &Connection) -> Result<Vec<String>, String> {
    let mut statement = conn
        .prepare("SELECT key FROM ItemTable")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    Ok(rows.flatten().collect())
}

fn is_chat_key(key: &str) -> bool {
    EXACT_CHAT_KEYS.contains(&key)
        || CHAT_KEY_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}

fn read_json_value(conn: &Connection, key: &str) -> Option<Value> {
    let mut statement = conn
        .prepare("SELECT value FROM ItemTable WHERE key = ?1")
        .ok()?;
    let bytes = statement
        .query_row([key], |row| match row.get_ref(0) {
            Ok(ValueRef::Text(bytes)) => Ok(bytes.to_vec()),
            Ok(ValueRef::Blob(bytes)) => Ok(bytes.to_vec()),
            _ => Ok(Vec::new()),
        })
        .optional()
        .ok()
        .flatten()?;
    if bytes.is_empty() || bytes.len() > MAX_JSON_BYTES {
        return None;
    }
    let value = serde_json::from_slice::<Value>(&bytes).ok()?;
    if let Value::String(inner) = &value {
        if let Ok(parsed) = serde_json::from_str::<Value>(inner) {
            return Some(parsed);
        }
    }
    Some(value)
}

fn workspace_project(db_path: &Path) -> Option<String> {
    let workspace_dir = db_path.parent()?;
    let workspace_json = workspace_dir.join("workspace.json");
    let value = std::fs::read_to_string(workspace_json)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())?;
    resolve_workspace_project(&value, workspace_dir)
}

fn resolve_workspace_project(value: &Value, workspace_dir: &Path) -> Option<String> {
    let object = value.as_object()?;
    if let Some(folder) = object.get("folder").and_then(value_text) {
        if let Some(path) =
            decode_file_uri(&folder).or_else(|| (!folder.trim().is_empty()).then_some(folder))
        {
            return Some(path);
        }
    }
    if let Some(folder) = object
        .get("configuration")
        .and_then(Value::as_object)
        .and_then(|configuration| configuration.get("folder"))
        .and_then(value_text)
    {
        if let Some(path) =
            decode_file_uri(&folder).or_else(|| (!folder.trim().is_empty()).then_some(folder))
        {
            return Some(path);
        }
    }
    let workspace = object.get("workspace").and_then(value_text)?;
    let workspace_path = decode_file_uri(&workspace)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let path = PathBuf::from(&workspace);
            if path.is_absolute() {
                path
            } else {
                workspace_dir.join(path)
            }
        });
    let raw = std::fs::read_to_string(workspace_path).ok()?;
    let parsed = jsonc_parse(&raw)?;
    parsed
        .get("folders")
        .and_then(Value::as_array)
        .and_then(|folders| folders.first())
        .and_then(|folder| folder.get("uri").or_else(|| folder.get("path")))
        .and_then(value_text)
        .and_then(|folder| {
            decode_file_uri(&folder).or_else(|| (!folder.trim().is_empty()).then_some(folder))
        })
}

fn jsonc_parse(raw: &str) -> Option<Value> {
    serde_json::from_str(raw).ok().or_else(|| {
        let cleaned = raw
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        serde_json::from_str(&cleaned).ok()
    })
}

fn decode_file_uri(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path()
        .ok()
        .map(|path| path.display().to_string())
}

fn file_mtime_ms(path: &Path) -> Option<i64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;
    use tempfile::tempdir;

    fn write_db(path: &Path, rows: &[(&str, Value)]) {
        let conn = Connection::open(path).expect("db");
        conn.execute_batch("CREATE TABLE ItemTable (key TEXT PRIMARY KEY, value BLOB NOT NULL);")
            .expect("schema");
        for (key, value) in rows {
            conn.execute(
                "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
                rusqlite::params![key, serde_json::to_vec(value).expect("json")],
            )
            .expect("row");
        }
    }

    fn fixture_session() -> Value {
        json!({
            "sessionId": "session-1",
            "title": "Fix login",
            "createdAt": 1_747_985_945_000_i64,
            "updatedAt": 1_747_987_789_000_i64,
            "model": "gpt-5",
            "messages": [
                {"role": "user", "content": "why does login fail?", "timestamp": 1_747_985_967_000_i64},
                {"role": "assistant", "content": "", "agentTaskContent": {
                    "proposal": "The token is expired.",
                    "guideline": {"planItems": [
                        {"thought": "Inspect auth", "toolName": "read_file", "params": {"file_path": "auth.rs"}, "result": "expired token"}
                    ]}
                }}
            ]
        })
    }

    #[test]
    fn scans_full_memento_and_loads_structured_messages() {
        let root = tempdir().expect("root");
        let workspace = root.path().join("workspaceStorage").join("hash-1");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::write(
            workspace.join("workspace.json"),
            r#"{"folder":"file:///tmp/trae-project"}"#,
        )
        .expect("workspace json");
        write_db(
            &workspace.join("state.vscdb"),
            &[(
                "memento/icube-ai-agent-storage",
                json!({"list": [fixture_session()]}),
            )],
        );

        let sessions = scan_root(TraeProduct::Trae, root.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].provider_id, "trae");
        assert_eq!(
            sessions[0].project_dir.as_deref(),
            Some("/tmp/trae-project")
        );
        assert_eq!(sessions[0].recovery, Some(SessionRecovery::Complete));
        assert_eq!(
            sessions[0].models.as_deref(),
            Some(&["gpt-5".to_string()][..])
        );

        let messages =
            load_messages(sessions[0].source_path.as_deref().expect("source")).expect("messages");
        assert_eq!(messages[0].content, "why does login fail?");
        assert_eq!(messages[1].content, "Inspect auth");
        assert_eq!(messages[1].kind, SessionMessageKind::Reasoning);
        assert_eq!(messages[2].tool_name.as_deref(), Some("read_file"));
        assert_eq!(messages[2].kind, SessionMessageKind::ToolCall);
        assert_eq!(messages[3].kind, SessionMessageKind::ToolResult);
        assert_eq!(messages[4].content, "The token is expired.");
        assert_eq!(
            messages[4]
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.models.as_deref()),
            Some(&["gpt-5".to_string()][..])
        );
    }

    #[test]
    fn scans_query_cache_as_partial_session() {
        let root = tempdir().expect("root");
        let global = root.path().join("User").join("globalStorage");
        std::fs::create_dir_all(&global).expect("global");
        write_db(
            &global.join("state.vscdb"),
            &[
                (
                    "chat.ChatSessionStore.index",
                    json!({"entries": {"session-2": {"title": "Cached prompt"}}}),
                ),
                (
                    "ai-chat.chatQueryCompletion.v2.session-2",
                    json!({"response": {"result": [{"text": "recover this prompt"}]}}),
                ),
            ],
        );

        let sessions = scan_root(TraeProduct::TraeWork, root.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].provider_id, "traework");
        assert_eq!(sessions[0].recovery, Some(SessionRecovery::Partial));
        let messages =
            load_messages(sessions[0].source_path.as_deref().expect("source")).expect("messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "recover this prompt");
    }

    #[test]
    fn scans_account_scoped_memento_storage() {
        let root = tempdir().expect("root");
        let workspace = root.path().join("workspaceStorage").join("hash-2");
        std::fs::create_dir_all(&workspace).expect("workspace");
        write_db(
            &workspace.join("state.vscdb"),
            &[(
                "memento/icube-ai-agent-storage-123456789",
                json!({"list": [fixture_session()]}),
            )],
        );

        let sessions = scan_root(TraeProduct::Trae, root.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "session-1");
        assert_eq!(sessions[0].recovery, Some(SessionRecovery::Complete));
    }

    #[test]
    fn marks_empty_assistant_placeholder_as_partial() {
        let root = tempdir().expect("root");
        let workspace = root.path().join("workspaceStorage").join("hash-4");
        std::fs::create_dir_all(&workspace).expect("workspace");
        write_db(
            &workspace.join("state.vscdb"),
            &[(
                "memento/icube-ai-agent-storage",
                json!({"list": [{
                    "sessionId": "session-placeholder",
                    "messages": [
                        {"role": "user", "content": "hello"},
                        {"role": "assistant", "content": ""}
                    ]
                }]}),
            )],
        );

        let sessions = scan_root(TraeProduct::Trae, root.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].recovery, Some(SessionRecovery::Partial));
    }

    #[test]
    fn keeps_current_memento_session_when_messages_are_encrypted() {
        let root = tempdir().expect("root");
        let workspace = root.path().join("workspaceStorage").join("hash-3");
        std::fs::create_dir_all(&workspace).expect("workspace");
        write_db(
            &workspace.join("state.vscdb"),
            &[(
                "memento/icube-ai-agent-storage",
                json!({"list": [], "currentSessionId": "session-encrypted"}),
            )],
        );

        let sessions = scan_root(TraeProduct::Trae, root.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "session-encrypted");
        assert_eq!(sessions[0].recovery, Some(SessionRecovery::Partial));
    }

    #[test]
    fn tolerates_malformed_databases_and_numeric_timestamps() {
        let root = tempdir().expect("root");
        let storage = root.path().join("User").join("workspaceStorage");
        let good = storage.join("good");
        let bad = storage.join("bad");
        std::fs::create_dir_all(&good).expect("good");
        std::fs::create_dir_all(&bad).expect("bad");
        std::fs::write(bad.join("state.vscdb"), b"not sqlite").expect("bad db");
        write_db(
            &good.join("state.vscdb"),
            &[(
                "memento/icube-ai-agent-storage",
                json!({"list": [{
                    "sessionId": "session-3",
                    "createdAt": "1747985945",
                    "messages": [{"role": "user", "content": "hello"}]
                }]}),
            )],
        );

        let sessions = scan_root(TraeProduct::Trae, root.path());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].created_at, Some(1_747_985_945_000));
    }
}
