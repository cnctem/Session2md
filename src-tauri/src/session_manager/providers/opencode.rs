use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde_json::Value;

use crate::session_manager::{paths::opencode_dir, SessionMessage, SessionMeta};

use super::common::{messages_from_parts, normalize_content_parts, ContentPart};
use super::utils::{parse_timestamp_to_ms, path_basename, truncate_summary};

const PROVIDER_ID: &str = "opencode";

/// OpenCode SQLite schema layout.
///
/// V1 uses the `session` / `message` / `part` tables. OpenCode 2.x migrates
/// sessions into `session_v2` / `session_message` and stops writing the V1
/// tables, so both layouts have to be supported (including databases that hold
/// a mix of the two).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenCodeSchema {
    V1,
    V2,
}

/// Return the OpenCode base directory (`$XDG_DATA_HOME/opencode`).
///
/// Respects `XDG_DATA_HOME` on all platforms; falls back to
/// `~/.local/share/opencode/`.
pub(crate) fn get_opencode_base_dir() -> PathBuf {
    opencode_dir()
}

/// Return the OpenCode JSON storage directory (legacy flat-file layout).
pub(crate) fn get_opencode_data_dir() -> PathBuf {
    get_opencode_base_dir().join("storage")
}

fn get_opencode_db_path() -> PathBuf {
    get_opencode_base_dir().join("opencode.db")
}

fn detect_schema(conn: &Connection) -> OpenCodeSchema {
    if sqlite_table_exists(conn, "session_v2") && sqlite_table_exists(conn, "session_message") {
        OpenCodeSchema::V2
    } else {
        OpenCodeSchema::V1
    }
}

fn sqlite_table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
    .unwrap_or(false)
}

/// Read the V1→V2 migration marker timestamp (`migration.v1-v2`) from `kv`.
///
/// Databases that tried OpenCode 2.x keep V1 rows that predate the migration.
/// Only sessions updated after the marker are considered live V1 sessions.
fn get_v2_migration_marker_ts(conn: &Connection) -> Option<i64> {
    if !sqlite_table_exists(conn, "kv") {
        return None;
    }
    conn.query_row(
        "SELECT max(coalesce(time_created, 0), coalesce(time_updated, 0)) \
         FROM kv WHERE key = 'migration.v1-v2'",
        [],
        |row| row.get(0),
    )
    .ok()
    .filter(|&ts| ts > 0)
}

/// Scan sessions from both the legacy JSON files and the newer SQLite database,
/// merging results with SQLite taking precedence on ID conflicts.
pub fn scan_sessions() -> Vec<SessionMeta> {
    let json_sessions = scan_sessions_json();
    let sqlite_sessions = scan_sessions_sqlite();

    if sqlite_sessions.is_empty() {
        return json_sessions;
    }
    if json_sessions.is_empty() {
        return sqlite_sessions;
    }

    // Deduplicate: keep SQLite version when the same session_id exists in both
    let sqlite_ids: std::collections::HashSet<String> = sqlite_sessions
        .iter()
        .map(|s| s.session_id.clone())
        .collect();

    let mut merged = sqlite_sessions;
    for s in json_sessions {
        if !sqlite_ids.contains(&s.session_id) {
            merged.push(s);
        }
    }
    merged
}

fn scan_sessions_json() -> Vec<SessionMeta> {
    let storage = get_opencode_data_dir();
    let session_dir = storage.join("session");
    if !session_dir.exists() {
        return Vec::new();
    }

    let mut json_files = Vec::new();
    collect_json_files(&session_dir, &mut json_files);

    let mut sessions = Vec::new();
    for path in json_files {
        if let Some(meta) = parse_session(&storage, &path) {
            sessions.push(meta);
        }
    }
    sessions
}

/// Parse a SQLite source reference in the format `sqlite:<db_path>:<session_id>`.
///
/// Uses `rfind(":ses_")` to split the path from the session ID because the
/// db path itself may contain colons (e.g. `C:\Users\...` on Windows).
/// This relies on the OpenCode convention that session IDs start with `ses_`.
fn parse_sqlite_source(source: &str) -> Option<(PathBuf, String)> {
    let rest = source.strip_prefix("sqlite:")?;
    let sep = rest.rfind(":ses_")?;
    let db_path = PathBuf::from(&rest[..sep]);
    let session_id = rest[sep + 1..].to_string();
    Some((db_path, session_id))
}

fn scan_sessions_sqlite() -> Vec<SessionMeta> {
    let db_path = get_opencode_db_path();
    if !db_path.exists() {
        return Vec::new();
    }

    let conn = match Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let db_display = db_path.display().to_string();
    match detect_schema(&conn) {
        OpenCodeSchema::V1 => scan_sessions_sqlite_v1(&conn, &db_display),
        OpenCodeSchema::V2 => scan_sessions_sqlite_v2(&conn, &db_display),
    }
}

fn parse_sqlite_session_row(
    row: &rusqlite::Row<'_>,
    db_display: &str,
) -> rusqlite::Result<SessionMeta> {
    let session_id: String = row.get(0)?;
    let title: String = row.get(1)?;
    let directory: String = row.get(2)?;
    let created: i64 = row.get(3)?;
    let updated: i64 = row.get(4)?;

    let display_title = if title.trim().is_empty() {
        path_basename(&directory)
    } else {
        Some(title)
    };
    Ok(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: session_id.clone(),
        title: display_title.clone(),
        summary: display_title,
        project_dir: if directory.is_empty() {
            None
        } else {
            Some(directory)
        },
        created_at: Some(created),
        last_active_at: Some(updated),
        source_path: Some(format!("sqlite:{db_display}:{session_id}")),
        can_delete: true,
        models: None,
        resume_command: Some(format!("opencode -s {session_id}")),
    })
}

fn scan_sessions_sqlite_v1(conn: &Connection, db_display: &str) -> Vec<SessionMeta> {
    let mut stmt = match conn.prepare(
        "SELECT id, COALESCE(title, ''), directory, time_created, time_updated \
         FROM session ORDER BY time_updated DESC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let iter = match stmt.query_map([], |row| parse_sqlite_session_row(row, db_display)) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();
    for meta in iter.flatten() {
        if !session_has_v1_messages(conn, &meta.session_id) {
            continue;
        }
        sessions.push(meta);
    }
    sessions
}

fn scan_sessions_sqlite_v2(conn: &Connection, db_display: &str) -> Vec<SessionMeta> {
    let sql = "SELECT s.id, \
                      COALESCE(s.title, ''), \
                      s.directory, \
                      s.time_created, \
                      MAX(s.time_updated, COALESCE(MAX(m.time_updated), s.time_updated)) AS last_active_at \
               FROM session_v2 s \
               LEFT JOIN session_message m ON m.session_id = s.id \
               GROUP BY s.id \
               ORDER BY last_active_at DESC";

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let iter = match stmt.query_map([], |row| parse_sqlite_session_row(row, db_display)) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();
    for meta in iter.flatten() {
        if !session_has_v2_messages(conn, &meta.session_id) {
            continue;
        }
        sessions.push(meta);
    }

    // Mixed V1/V2 database: a user may have tested 2.x and then continued on
    // 1.x. Only V1 sessions written after the migration marker are live.
    if sqlite_table_exists(conn, "session") {
        if let Some(marker_ts) = get_v2_migration_marker_ts(conn) {
            let v1_sql = "SELECT id, COALESCE(title, ''), directory, time_created, time_updated \
                          FROM session \
                          WHERE NOT EXISTS (SELECT 1 FROM session_v2 WHERE session_v2.id = session.id) \
                            AND time_updated > ?1 \
                          ORDER BY time_updated DESC";
            if let Ok(mut v1_stmt) = conn.prepare(v1_sql) {
                if let Ok(rows) =
                    v1_stmt.query_map([marker_ts], |row| parse_sqlite_session_row(row, db_display))
                {
                    for meta in rows.flatten() {
                        if !session_has_v1_messages(conn, &meta.session_id) {
                            continue;
                        }
                        sessions.push(meta);
                    }
                }
            }
        }
    }

    sessions.sort_by_key(|session| std::cmp::Reverse(session.last_active_at));
    sessions
}

fn session_has_v1_messages(conn: &Connection, session_id: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM message WHERE session_id = ?1",
        [session_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

fn session_has_v2_messages(conn: &Connection, session_id: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM session_message WHERE session_id = ?1",
        [session_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

pub fn load_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    // `path` is the message directory: storage/message/{sessionID}/
    if !path.is_dir() {
        return Err(format!("Message directory not found: {}", path.display()));
    }

    let storage = path
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| "Cannot determine storage root from message path".to_string())?;

    let mut msg_files = Vec::new();
    collect_json_files(path, &mut msg_files);

    // (created_ts, message_id, role, parts, model)
    let mut entries = Vec::<(i64, String, String, Vec<ContentPart>, Option<String>)>::new();

    for msg_path in &msg_files {
        let data = match std::fs::read_to_string(msg_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_id = match value.get("id").and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => continue,
        };

        let role = value
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();

        let created_ts = value
            .get("time")
            .and_then(|t| t.get("created"))
            .and_then(parse_timestamp_to_ms)
            .unwrap_or(0);

        // Collect all conversational parts from storage/part/{messageID}/.
        // Part ids are time-ordered, so sorting the file names restores the
        // generation order that the directory listing does not guarantee.
        let part_dir = storage.join("part").join(&msg_id);
        let mut part_files = Vec::new();
        collect_json_files(&part_dir, &mut part_files);
        part_files.sort();

        let parts: Vec<ContentPart> = part_files
            .iter()
            .filter_map(|part_path| std::fs::read_to_string(part_path).ok())
            .filter_map(|raw| serde_json::from_str::<Value>(&raw).ok())
            .flat_map(|part| opencode_part_to_content(&part))
            .collect();
        if parts.is_empty() {
            continue;
        }

        let model = (role == "assistant")
            .then(|| message_model(&value))
            .flatten();
        entries.push((created_ts, msg_id, role, parts, model));
    }

    // Sort by created timestamp, then by (time-ordered) message id
    entries.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));

    let messages = entries
        .into_iter()
        .flat_map(|(ts, _, role, parts, model)| {
            let mut normalized = messages_from_parts(&role, &parts, (ts > 0).then_some(ts));
            if let Some(model) = model {
                super::attach_models(&mut normalized, vec![model]);
            }
            normalized
        })
        .collect();

    Ok(messages)
}

/// Load messages from the OpenCode SQLite database for a given source reference.
///
/// Supports both V1 (`message` + `part` tables) and V2 (`session_message` table).
pub fn load_messages_sqlite(source: &str) -> Result<Vec<SessionMessage>, String> {
    let (db_path, session_id) = parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid SQLite source reference: {source}"))?;

    let conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("Failed to open OpenCode database: {e}"))?;

    match detect_schema(&conn) {
        OpenCodeSchema::V1 => load_messages_sqlite_v1(&conn, &session_id),
        OpenCodeSchema::V2 => {
            // In a mixed V1/V2 database, read from whichever table the session
            // actually lives in.
            let in_session_v2 = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM session_v2 WHERE id = ?1)",
                    [&session_id],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if in_session_v2 {
                load_messages_sqlite_v2(&conn, &session_id)
            } else if sqlite_table_exists(&conn, "message") {
                load_messages_sqlite_v1(&conn, &session_id)
            } else {
                load_messages_sqlite_v2(&conn, &session_id)
            }
        }
    }
}

fn load_messages_sqlite_v1(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<SessionMessage>, String> {
    let mut msg_stmt = conn
        .prepare(
            "SELECT id, time_created, data FROM message WHERE session_id = ?1 \
             ORDER BY time_created ASC, id ASC",
        )
        .map_err(|e| format!("Failed to prepare message query: {e}"))?;

    let msg_rows = msg_stmt
        .query_map([session_id], |row| {
            let id: String = row.get(0)?;
            let ts: i64 = row.get(1)?;
            let data: String = row.get(2)?;
            Ok((id, ts, data))
        })
        .map_err(|e| format!("Failed to query messages: {e}"))?;

    let mut part_stmt = conn
        .prepare(
            "SELECT message_id, data FROM part WHERE session_id = ?1 \
             ORDER BY time_created ASC, id ASC",
        )
        .map_err(|e| format!("Failed to prepare part query: {e}"))?;

    let part_rows = part_stmt
        .query_map([session_id], |row| {
            let message_id: String = row.get(0)?;
            let data: String = row.get(1)?;
            Ok((message_id, data))
        })
        .map_err(|e| format!("Failed to query parts: {e}"))?;

    let mut parts_map: std::collections::HashMap<String, Vec<ContentPart>> =
        std::collections::HashMap::new();
    for (message_id, data) in part_rows.flatten() {
        let Ok(part_value) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        parts_map
            .entry(message_id)
            .or_default()
            .extend(opencode_part_to_content(&part_value));
    }

    let mut messages = Vec::new();
    for (msg_id, ts, data) in msg_rows.flatten() {
        let Ok(msg_value) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        let role = msg_value
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let parts = parts_map.remove(&msg_id).unwrap_or_default();
        let mut normalized = messages_from_parts(role, &parts, Some(ts));
        if role == "assistant" {
            if let Some(model) = message_model(&msg_value) {
                super::attach_models(&mut normalized, vec![model]);
            }
        }
        messages.extend(normalized);
    }

    Ok(messages)
}

fn load_messages_sqlite_v2(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<SessionMessage>, String> {
    // `seq` is the authoritative ordering; retries can rewrite `time_created`,
    // so never sort V2 messages by time.
    let mut stmt = conn
        .prepare(
            "SELECT id, type, time_created, data FROM session_message \
             WHERE session_id = ?1 ORDER BY seq ASC, rowid ASC",
        )
        .map_err(|e| format!("Failed to prepare V2 message query: {e}"))?;

    let rows = stmt
        .query_map([session_id], |row| {
            let id: String = row.get(0)?;
            let msg_type: String = row.get(1)?;
            let ts: i64 = row.get(2)?;
            let data: String = row.get(3)?;
            Ok((id, msg_type, ts, data))
        })
        .map_err(|e| format!("Failed to query V2 messages: {e}"))?;

    let mut messages = Vec::new();
    for (_row_id, msg_type, ts, data) in rows.flatten() {
        if !matches!(msg_type.as_str(), "user" | "assistant" | "system") {
            continue;
        }
        let Ok(msg_value) = serde_json::from_str::<Value>(&data) else {
            continue;
        };

        // User/system messages carry their prompt in `text`; assistant messages
        // store a v1-shaped `content[]` array of parts.
        let parts: Vec<ContentPart> = match (
            msg_type.as_str(),
            msg_value.get("text").and_then(Value::as_str),
            msg_value.get("content"),
        ) {
            ("user" | "system", Some(text), _) => {
                normalize_content_parts(&Value::String(text.to_string()))
            }
            (_, _, Some(Value::Array(items))) if msg_type != "system" => {
                items.iter().flat_map(opencode_part_to_content).collect()
            }
            (_, _, Some(Value::String(text))) if msg_type == "user" => {
                normalize_content_parts(&Value::String(text.clone()))
            }
            (_, Some(text), _) => normalize_content_parts(&Value::String(text.to_string())),
            _ => Vec::new(),
        };

        let mut normalized = messages_from_parts(&msg_type, &parts, Some(ts));
        if msg_type == "assistant" {
            if let Some(model) = message_model(&msg_value) {
                super::attach_models(&mut normalized, vec![model]);
            }
        }
        messages.extend(normalized);
    }

    Ok(messages)
}

/// Convert an OpenCode part (V1 `part.data`, V2 `content[]` item or a file-store
/// part) into normalized content parts.
pub(crate) fn opencode_part_to_content(part_value: &Value) -> Vec<ContentPart> {
    match part_value.get("type").and_then(Value::as_str) {
        Some("tool") => {
            let name = part_value
                .get("tool")
                .or_else(|| part_value.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let input = part_value
                .get("state")
                .and_then(|state| state.get("input"))
                .or_else(|| part_value.get("input"))
                .map(|value| match value {
                    Value::String(value) => value.clone(),
                    _ => value.to_string(),
                })
                .unwrap_or_else(|| "{}".to_string());
            let tool_call_id = part_value
                .get("callID")
                .or_else(|| part_value.get("callId"))
                .or_else(|| part_value.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let mut parts = vec![ContentPart::tool_call_with_metadata(
                name,
                input,
                tool_call_id.clone(),
            )];
            if let Some(output) = part_value
                .get("state")
                .and_then(|state| state.get("output"))
            {
                parts.push(ContentPart::tool_result_with_metadata(
                    output
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| output.to_string()),
                    tool_call_id,
                    Some(name.to_string()),
                ));
            }
            parts
        }
        Some("text") => {
            // `synthetic` prompts are injected by OpenCode itself and `ignored`
            // text was dropped from the conversation; neither belongs in a
            // transcript.
            if part_value.get("synthetic").and_then(Value::as_bool) == Some(true)
                || part_value.get("ignored").and_then(Value::as_bool) == Some(true)
            {
                return Vec::new();
            }
            normalize_content_parts(part_value)
        }
        _ => normalize_content_parts(part_value),
    }
}

fn message_model(message: &Value) -> Option<String> {
    let nested = message.get("model").and_then(Value::as_object);
    super::model(
        nested
            .and_then(|model| model.get("modelID"))
            .or_else(|| message.get("modelID"))
            .and_then(Value::as_str)
            .map(str::to_string),
    )
}

pub fn delete_session(storage: &Path, path: &Path, session_id: &str) -> Result<bool, String> {
    let message_root = storage.join("message");
    if path.parent() != Some(message_root.as_path()) {
        return Err(format!(
            "OpenCode session source must be a direct message directory: {}",
            path.display()
        ));
    }
    if path.file_name().and_then(|name| name.to_str()) != Some(session_id) {
        return Err(format!(
            "OpenCode session path does not match session ID: expected {session_id}, found {}",
            path.display()
        ));
    }

    let mut message_files = Vec::new();
    collect_json_files(path, &mut message_files);
    let mut message_ids = Vec::new();
    for message_path in message_files {
        let Ok(data) = std::fs::read_to_string(message_path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        if let Some(message_id) = value.get("id").and_then(Value::as_str) {
            message_ids.push(message_id.to_string());
        }
    }

    for message_id in message_ids {
        let part_dir = storage.join("part").join(message_id);
        remove_dir_all_if_exists(&part_dir).map_err(|error| {
            format!(
                "Failed to delete OpenCode part directory {}: {error}",
                part_dir.display()
            )
        })?;
    }
    let session_diff_path = storage
        .join("session_diff")
        .join(format!("{session_id}.json"));
    remove_file_if_exists(&session_diff_path).map_err(|error| {
        format!(
            "Failed to delete OpenCode session diff {}: {error}",
            session_diff_path.display()
        )
    })?;
    remove_dir_all_if_exists(path).map_err(|error| {
        format!(
            "Failed to delete OpenCode message directory {}: {error}",
            path.display()
        )
    })?;
    if let Some(session_file) = find_session_file(storage, session_id) {
        remove_file_if_exists(&session_file).map_err(|error| {
            format!(
                "Failed to delete OpenCode session file {}: {error}",
                session_file.display()
            )
        })?;
    }
    Ok(true)
}

pub fn delete_session_sqlite(session_id: &str, source: &str) -> Result<bool, String> {
    let (db_path, ref_session_id) = parse_sqlite_source(source)
        .ok_or_else(|| format!("Invalid SQLite source reference: {source}"))?;
    let db_path = db_path
        .canonicalize()
        .map_err(|error| format!("Failed to canonicalize SQLite database path: {error}"))?;
    let expected_db_path = get_opencode_db_path().canonicalize().map_err(|error| {
        format!("Failed to canonicalize expected OpenCode database path: {error}")
    })?;
    if ref_session_id != session_id {
        return Err(format!(
            "OpenCode SQLite session ID mismatch: expected {session_id}, found {ref_session_id}"
        ));
    }
    if db_path != expected_db_path {
        return Err("SQLite path does not match expected OpenCode database".to_string());
    }

    let mut conn = Connection::open(&db_path)
        .map_err(|error| format!("Failed to open OpenCode database: {error}"))?;
    let tx = conn
        .transaction()
        .map_err(|error| format!("Failed to begin transaction: {error}"))?;

    let deleted = match detect_schema(&tx) {
        OpenCodeSchema::V2 => {
            tx.execute(
                "DELETE FROM session_message WHERE session_id = ?1",
                [session_id],
            )
            .map_err(|error| format!("Failed to delete OpenCode V2 messages: {error}"))?;

            // Best-effort cleanup of auxiliary V2 tables whose schema may evolve.
            for child in [
                "session_pending",
                "session_inbox",
                "instruction_entry",
                "instruction_state",
            ] {
                if sqlite_table_exists(&tx, child) {
                    let _ = tx.execute(
                        &format!("DELETE FROM {child} WHERE session_id = ?1"),
                        [session_id],
                    );
                }
            }
            if sqlite_table_exists(&tx, "event_sequence") {
                let _ = tx.execute(
                    "DELETE FROM event_sequence WHERE aggregate_id = ?1",
                    [session_id],
                );
            }

            let mut deleted_count = tx
                .execute("DELETE FROM session_v2 WHERE id = ?1", [session_id])
                .map_err(|error| format!("Failed to delete OpenCode V2 session: {error}"))?;

            // Migrated sessions can also have leftover V1 rows.
            if sqlite_table_exists(&tx, "session") {
                if sqlite_table_exists(&tx, "part") {
                    tx.execute("DELETE FROM part WHERE session_id = ?1", [session_id])
                        .map_err(|error| format!("Failed to delete OpenCode parts: {error}"))?;
                }
                if sqlite_table_exists(&tx, "message") {
                    tx.execute("DELETE FROM message WHERE session_id = ?1", [session_id])
                        .map_err(|error| format!("Failed to delete OpenCode messages: {error}"))?;
                }
                deleted_count += tx
                    .execute("DELETE FROM session WHERE id = ?1", [session_id])
                    .map_err(|error| {
                        format!("Failed to delete OpenCode legacy session: {error}")
                    })?;
            }

            deleted_count
        }
        OpenCodeSchema::V1 => {
            if sqlite_table_exists(&tx, "part") {
                tx.execute("DELETE FROM part WHERE session_id = ?1", [session_id])
                    .map_err(|error| format!("Failed to delete OpenCode parts: {error}"))?;
            }
            if sqlite_table_exists(&tx, "message") {
                tx.execute("DELETE FROM message WHERE session_id = ?1", [session_id])
                    .map_err(|error| format!("Failed to delete OpenCode messages: {error}"))?;
            }
            tx.execute("DELETE FROM session WHERE id = ?1", [session_id])
                .map_err(|error| format!("Failed to delete OpenCode session: {error}"))?
        }
    };

    tx.commit()
        .map_err(|error| format!("Failed to commit session deletion: {error}"))?;
    Ok(deleted > 0)
}

fn parse_session(storage: &Path, path: &Path) -> Option<SessionMeta> {
    let data = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;

    let session_id = value.get("id").and_then(Value::as_str)?.to_string();
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let directory = value
        .get("directory")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let created_at = value
        .get("time")
        .and_then(|t| t.get("created"))
        .and_then(parse_timestamp_to_ms);
    let updated_at = value
        .get("time")
        .and_then(|t| t.get("updated"))
        .and_then(parse_timestamp_to_ms);

    // Derive title from directory basename if no explicit title
    let has_title = title.is_some();
    let display_title = title.or_else(|| {
        directory
            .as_deref()
            .and_then(path_basename)
            .map(|s| s.to_string())
    });

    // Build source_path = message directory for this session
    let msg_dir = storage.join("message").join(&session_id);
    let source_path = msg_dir.to_string_lossy().to_string();

    // Skip expensive I/O if title already available from session JSON
    let summary = if has_title {
        display_title.clone()
    } else {
        get_first_user_summary(storage, &session_id)
    };

    Some(SessionMeta {
        provider_id: PROVIDER_ID.to_string(),
        session_id: session_id.clone(),
        title: display_title,
        summary,
        project_dir: directory,
        created_at,
        last_active_at: updated_at.or(created_at),
        source_path: Some(source_path),
        can_delete: true,
        models: None,
        resume_command: Some(format!("opencode -s {session_id}")),
    })
}

/// Read the first user message's first text part to use as summary.
fn get_first_user_summary(storage: &Path, session_id: &str) -> Option<String> {
    let msg_dir = storage.join("message").join(session_id);
    if !msg_dir.is_dir() {
        return None;
    }

    let mut msg_files = Vec::new();
    collect_json_files(&msg_dir, &mut msg_files);

    // Collect user messages with timestamps for ordering
    let mut user_msgs: Vec<(i64, String)> = Vec::new();
    for msg_path in &msg_files {
        let data = match std::fs::read_to_string(msg_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if value.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }

        let msg_id = match value.get("id").and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => continue,
        };

        let ts = value
            .get("time")
            .and_then(|t| t.get("created"))
            .and_then(parse_timestamp_to_ms)
            .unwrap_or(0);

        user_msgs.push((ts, msg_id));
    }

    user_msgs.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));

    // Take first user message and get its parts
    let (_, first_id) = user_msgs.first()?;
    let part_dir = storage.join("part").join(first_id);
    let text = collect_parts_text(&part_dir);
    if text.trim().is_empty() {
        return None;
    }
    Some(truncate_summary(&text, 160))
}

/// Collect text content from all parts in a part directory.
fn extract_part_text(part_value: &Value) -> Option<String> {
    match part_value.get("type").and_then(Value::as_str) {
        Some("text") => {
            if part_value.get("synthetic").and_then(Value::as_bool) == Some(true)
                || part_value.get("ignored").and_then(Value::as_bool) == Some(true)
            {
                return None;
            }
            part_value
                .get("text")
                .and_then(Value::as_str)
                .filter(|t| !t.trim().is_empty())
                .map(|t| t.to_string())
        }
        Some("tool") => {
            let tool = part_value
                .get("tool")
                .or_else(|| part_value.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            Some(format!("[Tool: {tool}]"))
        }
        _ => None,
    }
}

fn collect_parts_text(part_dir: &Path) -> String {
    if !part_dir.is_dir() {
        return String::new();
    }

    let mut parts = Vec::new();
    collect_json_files(part_dir, &mut parts);
    parts.sort();

    let mut texts = Vec::new();
    for part_path in &parts {
        let data = match std::fs::read_to_string(part_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let value: Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(text) = extract_part_text(&value) {
            texts.push(text);
        }
    }

    texts.join("\n")
}

fn collect_json_files(root: &Path, files: &mut Vec<PathBuf>) {
    if !root.exists() {
        return;
    }

    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            files.push(path);
        }
    }
}

fn find_session_file(storage: &Path, session_id: &str) -> Option<PathBuf> {
    let mut files = Vec::new();
    collect_json_files(&storage.join("session"), &mut files);
    let expected = format!("{session_id}.json");
    files
        .into_iter()
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some(expected.as_str()))
}

fn remove_file_if_exists(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_dir_all_if_exists(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn opencode_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn create_sqlite_schema(conn: &Connection) {
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE session (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                directory TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL
            );
            CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                data TEXT NOT NULL,
                FOREIGN KEY(session_id) REFERENCES session(id) ON DELETE CASCADE
            );
            CREATE TABLE part (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                data TEXT NOT NULL,
                FOREIGN KEY(session_id) REFERENCES session(id) ON DELETE CASCADE,
                FOREIGN KEY(message_id) REFERENCES message(id) ON DELETE CASCADE
            );
            ",
        )
        .expect("create sqlite schema");
    }

    fn create_sqlite_schema_v2(conn: &Connection) {
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE session_v2 (
                id TEXT PRIMARY KEY,
                title TEXT,
                directory TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL
            );
            CREATE TABLE session_message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                type TEXT NOT NULL,
                seq INTEGER NOT NULL,
                data TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                FOREIGN KEY(session_id) REFERENCES session_v2(id) ON DELETE CASCADE
            );
            ",
        )
        .expect("create v2 sqlite schema");
    }

    fn restore_xdg(original: Option<std::ffi::OsString>) {
        #[allow(deprecated)]
        if let Some(value) = original {
            std::env::set_var("XDG_DATA_HOME", value);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }
    }

    #[test]
    fn load_messages_includes_tool_parts() {
        let temp = tempdir().expect("tempdir");
        let storage = temp.path();
        let session_id = "ses_test";
        let msg_id = "msg_1";

        let msg_dir = storage.join("message").join(session_id);
        let part_dir = storage.join("part").join(msg_id);
        std::fs::create_dir_all(&msg_dir).expect("create msg dir");
        std::fs::create_dir_all(&part_dir).expect("create part dir");

        std::fs::write(
            msg_dir.join(format!("{msg_id}.json")),
            r#"{"id":"msg_1","role":"assistant","time":{"created":"2026-03-06T10:00:00Z"}}"#,
        )
        .expect("write msg");

        std::fs::write(
            part_dir.join("prt_1.json"),
            r#"{"id":"prt_1","type":"tool","tool":"bash","state":{"status":"completed","input":{"command":"ls"},"output":"file.txt"}}"#,
        )
        .expect("write tool part");

        std::fs::write(
            part_dir.join("prt_2.json"),
            r#"{"id":"prt_2","type":"text","text":"Here are the files."}"#,
        )
        .expect("write text part");

        let msgs = load_messages(&msg_dir).expect("load");
        assert_eq!(msgs.len(), 3);
        assert!(msgs.iter().any(|message| {
            message.role == "tool"
                && message.kind == crate::session_manager::SessionMessageKind::ToolCall
                && message.tool_name.as_deref() == Some("bash")
                && message.content.contains("ls")
        }));
        assert!(msgs.iter().any(|message| {
            message.role == "tool"
                && message.kind == crate::session_manager::SessionMessageKind::ToolResult
                && message.content.contains("file.txt")
        }));
        assert!(msgs.iter().any(|message| {
            message.role == "assistant" && message.content.contains("Here are the files.")
        }));
    }

    #[test]
    fn load_messages_skips_synthetic_text_parts() {
        let temp = tempdir().expect("tempdir");
        let storage = temp.path();
        let session_id = "ses_synthetic";
        let msg_id = "msg_1";

        let msg_dir = storage.join("message").join(session_id);
        let part_dir = storage.join("part").join(msg_id);
        std::fs::create_dir_all(&msg_dir).expect("create msg dir");
        std::fs::create_dir_all(&part_dir).expect("create part dir");

        std::fs::write(
            msg_dir.join(format!("{msg_id}.json")),
            r#"{"id":"msg_1","role":"user","time":{"created":"2026-03-06T10:00:00Z"}}"#,
        )
        .expect("write msg");
        std::fs::write(
            part_dir.join("prt_1.json"),
            r#"{"id":"prt_1","type":"text","text":"injected","synthetic":true}"#,
        )
        .expect("write synthetic part");
        std::fs::write(
            part_dir.join("prt_2.json"),
            r#"{"id":"prt_2","type":"text","text":"real prompt"}"#,
        )
        .expect("write text part");

        let msgs = load_messages(&msg_dir).expect("load");
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "real prompt");
    }

    #[test]
    fn delete_session_removes_legacy_transcript_and_related_data() {
        let temp = tempdir().expect("tempdir");
        let storage = temp.path();
        let session_id = "ses_delete";
        let message_id = "msg_delete";
        let message_dir = storage.join("message").join(session_id);
        let part_dir = storage.join("part").join(message_id);
        let session_file = storage
            .join("session")
            .join("project")
            .join(format!("{session_id}.json"));
        let session_diff = storage
            .join("session_diff")
            .join(format!("{session_id}.json"));

        std::fs::create_dir_all(&message_dir).expect("create message dir");
        std::fs::create_dir_all(&part_dir).expect("create part dir");
        std::fs::create_dir_all(session_file.parent().expect("session parent"))
            .expect("create session dir");
        std::fs::create_dir_all(session_diff.parent().expect("diff parent"))
            .expect("create diff dir");
        std::fs::write(
            message_dir.join(format!("{message_id}.json")),
            format!(r#"{{"id":"{message_id}","sessionID":"{session_id}"}}"#),
        )
        .expect("write message");
        std::fs::write(part_dir.join("part.json"), "{}").expect("write part");
        std::fs::write(&session_file, format!(r#"{{"id":"{session_id}"}}"#))
            .expect("write session");
        std::fs::write(&session_diff, "[]").expect("write diff");

        assert!(delete_session(storage, &message_dir, session_id).expect("delete session"));
        assert!(!message_dir.exists());
        assert!(!part_dir.exists());
        assert!(!session_file.exists());
        assert!(!session_diff.exists());
    }

    #[test]
    fn delete_session_rejects_non_message_directories() {
        let temp = tempdir().expect("tempdir");
        let storage = temp.path();
        let path = storage.join("other").join("ses_delete");
        std::fs::create_dir_all(&path).expect("create unrelated directory");

        let error = delete_session(storage, &path, "ses_delete")
            .expect_err("unrelated directory should be rejected");
        assert!(error.contains("direct message directory"));
        assert!(path.exists());
    }

    #[test]
    fn parse_sqlite_source_accepts_valid_references() {
        let parsed = parse_sqlite_source("sqlite:/tmp/opencode.db:ses_123").expect("valid source");

        assert_eq!(parsed.0, PathBuf::from("/tmp/opencode.db"));
        assert_eq!(parsed.1, "ses_123");
    }

    #[test]
    fn parse_sqlite_source_rejects_invalid_references() {
        assert!(parse_sqlite_source("/tmp/opencode.db:ses_123").is_none());
        assert!(parse_sqlite_source("sqlite:/tmp/opencode.db:msg_123").is_none());
        assert!(parse_sqlite_source("sqlite:/tmp/opencode.db").is_none());
    }

    #[test]
    #[allow(deprecated)] // set_var/remove_var deprecated since Rust 1.81; safe here under mutex
    fn scan_sessions_sqlite_reads_temp_database() {
        let _guard = opencode_env_lock().lock().expect("lock");
        let temp = tempdir().expect("tempdir");
        let original_xdg = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", temp.path());

        let base_dir = temp.path().join("opencode");
        std::fs::create_dir_all(&base_dir).expect("create base dir");
        let db_path = base_dir.join("opencode.db");
        let conn = Connection::open(&db_path).expect("open sqlite db");
        create_sqlite_schema(&conn);

        conn.execute(
            "INSERT INTO session (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_1", "", "/tmp/project-a", 1_771_061_953_033_i64, 1_771_061_954_033_i64),
        )
        .expect("insert session 1");
        conn.execute(
            "INSERT INTO session (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_2", "Named Session", "/tmp/project-b", 1_771_061_950_000_i64, 1_771_061_955_000_i64),
        )
        .expect("insert session 2");
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, data) VALUES ('msg_1', 'ses_1', 1, '{\"role\":\"user\"}')",
            [],
        )
        .expect("insert session 1 message");
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, data) VALUES ('msg_2', 'ses_2', 2, '{\"role\":\"user\"}')",
            [],
        )
        .expect("insert session 2 message");
        drop(conn);

        let sessions = scan_sessions_sqlite();

        restore_xdg(original_xdg);

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, "ses_2");
        assert_eq!(sessions[0].title.as_deref(), Some("Named Session"));
        assert_eq!(sessions[1].session_id, "ses_1");
        assert_eq!(sessions[1].title.as_deref(), Some("project-a"));
        assert_eq!(sessions[1].project_dir.as_deref(), Some("/tmp/project-a"));
        let expected_source = format!("sqlite:{}:ses_1", db_path.display());
        assert_eq!(
            sessions[1].source_path.as_deref(),
            Some(expected_source.as_str())
        );
        assert_eq!(
            sessions[1].resume_command.as_deref(),
            Some("opencode -s ses_1")
        );
    }

    #[test]
    fn load_messages_sqlite_reads_messages_and_parts() {
        let temp = tempdir().expect("tempdir");
        let db_path = temp.path().join("opencode.db");
        let conn = Connection::open(&db_path).expect("open sqlite db");
        create_sqlite_schema(&conn);

        conn.execute(
            "INSERT INTO session (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_1", "Session", "/tmp/project-a", 1000_i64, 3000_i64),
        )
        .expect("insert session");
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4)",
            ("msg_1", "ses_1", 1000_i64, r#"{"role":"user"}"#),
        )
        .expect("insert message 1");
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4)",
            ("msg_2", "ses_1", 2000_i64, r#"{"role":"assistant","modelID":"deepseek-v4-flash","providerID":"deepseek","variant":"max"}"#),
        )
        .expect("insert message 2");
        conn.execute(
            "INSERT INTO part (id, session_id, message_id, time_created, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("prt_1", "ses_1", "msg_1", 1000_i64, r#"{"type":"text","text":"Hello"}"#),
        )
        .expect("insert part 1");
        conn.execute(
            "INSERT INTO part (id, session_id, message_id, time_created, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                "prt_2",
                "ses_1",
                "msg_2",
                2000_i64,
                r#"{"type":"tool","tool":"bash"}"#,
            ),
        )
        .expect("insert part 2");
        conn.execute(
            "INSERT INTO part (id, session_id, message_id, time_created, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                "prt_3",
                "ses_1",
                "msg_2",
                2001_i64,
                r#"{"type":"text","text":"Done"}"#,
            ),
        )
        .expect("insert part 3");
        drop(conn);

        let source = format!("sqlite:{}:ses_1", db_path.display());
        let messages = load_messages_sqlite(&source).expect("load sqlite messages");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[0].ts, Some(1000));
        assert_eq!(messages[1].role, "tool");
        assert_eq!(
            messages[1].kind,
            crate::session_manager::SessionMessageKind::ToolCall
        );
        assert_eq!(messages[1].tool_name.as_deref(), Some("bash"));
        assert_eq!(
            messages[1]
                .metadata
                .as_ref()
                .unwrap()
                .models
                .as_ref()
                .unwrap()[0]
                .as_str(),
            "deepseek-v4-flash"
        );
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[2].content, "Done");
        assert_eq!(messages[2].ts, Some(2000));
        assert_eq!(
            messages[2]
                .metadata
                .as_ref()
                .unwrap()
                .models
                .as_ref()
                .unwrap()[0],
            "deepseek-v4-flash"
        );
    }

    #[test]
    #[allow(deprecated)]
    fn scan_sessions_sqlite_v2_reads_temp_database() {
        let _guard = opencode_env_lock().lock().expect("lock");
        let temp = tempdir().expect("tempdir");
        let original_xdg = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", temp.path());

        let base_dir = temp.path().join("opencode");
        std::fs::create_dir_all(&base_dir).expect("create base dir");
        let db_path = base_dir.join("opencode.db");
        let conn = Connection::open(&db_path).expect("open sqlite db");
        // V1 tables also exist (empty) after migration
        create_sqlite_schema(&conn);
        create_sqlite_schema_v2(&conn);

        conn.execute(
            "INSERT INTO session_v2 (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_v2_1", Option::<&str>::None, "/tmp/project-v2-a", 1_000_i64, 2_000_i64),
        )
        .expect("insert v2 session 1");
        conn.execute(
            "INSERT INTO session_v2 (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_v2_2", "V2 Named Session", "/tmp/project-v2-b", 1_500_i64, 2_500_i64),
        )
        .expect("insert v2 session 2");
        // The message on ses_v2_1 is newer (3_000), so it must sort first
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            ("msg_v2_1", "ses_v2_1", "user", 1_i64, r#"{"text":"hi"}"#, 1_100_i64, 3_000_i64),
        )
        .expect("insert v2 message 1");
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            ("msg_v2_2", "ses_v2_2", "user", 1_i64, r#"{"text":"hello"}"#, 1_600_i64, 2_500_i64),
        )
        .expect("insert v2 message 2");
        drop(conn);

        let sessions = scan_sessions_sqlite();

        restore_xdg(original_xdg);

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, "ses_v2_1");
        assert_eq!(sessions[0].title.as_deref(), Some("project-v2-a"));
        assert_eq!(sessions[0].last_active_at, Some(3_000));
        assert_eq!(sessions[1].session_id, "ses_v2_2");
        assert_eq!(sessions[1].title.as_deref(), Some("V2 Named Session"));
        assert_eq!(sessions[1].last_active_at, Some(2_500));
    }

    #[test]
    fn load_messages_sqlite_v2_reads_messages_and_parts() {
        let temp = tempdir().expect("tempdir");
        let db_path = temp.path().join("opencode.db");
        let conn = Connection::open(&db_path).expect("open sqlite db");
        create_sqlite_schema_v2(&conn);

        conn.execute(
            "INSERT INTO session_v2 (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_v2_1", "Session V2", "/tmp/project-a", 1000_i64, 3000_i64),
        )
        .expect("insert session_v2");
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            ("msg_1", "ses_v2_1", "user", 1_i64, r#"{"text":"Hello V2"}"#, 1000_i64, 1000_i64),
        )
        .expect("insert user message");
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                "msg_2",
                "ses_v2_1",
                "assistant",
                2_i64,
                r#"{"modelID":"deepseek-v4-flash","content":[{"type":"reasoning","text":"thinking"},{"type":"tool","name":"shell","id":"call_1","state":{"input":{"command":"ls"},"output":"file.txt"}},{"type":"text","text":"All done in V2"}]}"#,
                2000_i64,
                2500_i64,
            ),
        )
        .expect("insert assistant message");
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            ("msg_3", "ses_v2_1", "compaction", 3_i64, r#"{"status":"completed"}"#, 2600_i64, 2600_i64),
        )
        .expect("insert compaction message");
        drop(conn);

        let source = format!("sqlite:{}:ses_v2_1", db_path.display());
        let messages = load_messages_sqlite(&source).expect("load v2 sqlite messages");

        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hello V2");
        assert_eq!(messages[0].ts, Some(1000));
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(
            messages[1].kind,
            crate::session_manager::SessionMessageKind::Reasoning
        );
        assert_eq!(messages[1].content, "thinking");
        assert_eq!(messages[2].role, "tool");
        assert_eq!(
            messages[2].kind,
            crate::session_manager::SessionMessageKind::ToolCall
        );
        assert_eq!(messages[2].tool_name.as_deref(), Some("shell"));
        assert_eq!(
            messages[2]
                .metadata
                .as_ref()
                .unwrap()
                .models
                .as_ref()
                .unwrap()[0],
            "deepseek-v4-flash"
        );
        // The tool result is emitted from the tool's state.output
        assert_eq!(messages[3].role, "tool");
        assert_eq!(
            messages[3].kind,
            crate::session_manager::SessionMessageKind::ToolResult
        );
        assert_eq!(messages[3].content, "file.txt");
        assert_eq!(messages[4].role, "assistant");
        assert_eq!(messages[4].content, "All done in V2");
    }

    #[test]
    fn load_messages_sqlite_v2_orders_by_seq_instead_of_time_created() {
        let temp = tempdir().expect("tempdir");
        let db_path = temp.path().join("opencode.db");
        let conn = Connection::open(&db_path).expect("open sqlite db");
        create_sqlite_schema_v2(&conn);

        conn.execute(
            "INSERT INTO session_v2 (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_v2_seq", "Seq Order Test", "/tmp/project-a", 1000_i64, 5000_i64),
        )
        .expect("insert session_v2");
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            ("msg_1", "ses_v2_seq", "user", 1_i64, r#"{"text":"First question"}"#, 1000_i64, 1000_i64),
        )
        .expect("insert msg_1");
        // Retried reply: time_created was rewritten to 5000 but seq stays 2
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            ("msg_2", "ses_v2_seq", "assistant", 2_i64, r#"{"content":[{"type":"text","text":"Retried first answer"}]}"#, 5000_i64, 5000_i64),
        )
        .expect("insert msg_2");
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            ("msg_3", "ses_v2_seq", "user", 3_i64, r#"{"text":"Follow-up question"}"#, 3000_i64, 3000_i64),
        )
        .expect("insert msg_3");
        drop(conn);

        let source = format!("sqlite:{}:ses_v2_seq", db_path.display());
        let messages = load_messages_sqlite(&source).expect("load v2 sqlite messages");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "First question");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Retried first answer");
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content, "Follow-up question");
    }

    #[test]
    #[allow(deprecated)]
    fn delete_session_sqlite_v2_removes_session_and_messages() {
        let _guard = opencode_env_lock().lock().expect("lock");
        let temp = tempdir().expect("tempdir");
        let original_xdg = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", temp.path());

        let base_dir = temp.path().join("opencode");
        std::fs::create_dir_all(&base_dir).expect("create base dir");
        let db_path = base_dir.join("opencode.db");
        let conn = Connection::open(&db_path).expect("open sqlite db");
        create_sqlite_schema_v2(&conn);

        conn.execute(
            "INSERT INTO session_v2 (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_v2_1", "Session V2", "/tmp/project-a", 1000_i64, 3000_i64),
        )
        .expect("insert session_v2");
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            ("msg_1", "ses_v2_1", "user", 1_i64, r#"{"text":"Hello"}"#, 1000_i64, 1000_i64),
        )
        .expect("insert message");
        drop(conn);

        let source = format!("sqlite:{}:ses_v2_1", db_path.display());
        assert!(delete_session_sqlite("ses_v2_1", &source).expect("delete v2 sqlite session"));

        let conn = Connection::open(&db_path).expect("re-open sqlite db");
        let remaining_sessions: i64 = conn
            .query_row("SELECT COUNT(*) FROM session_v2", [], |row| row.get(0))
            .expect("count v2 sessions");
        let remaining_messages: i64 = conn
            .query_row("SELECT COUNT(*) FROM session_message", [], |row| row.get(0))
            .expect("count v2 messages");

        restore_xdg(original_xdg);

        assert_eq!(remaining_sessions, 0);
        assert_eq!(remaining_messages, 0);
    }

    #[test]
    #[allow(deprecated)]
    fn mixed_v1_v2_database_scans_loads_and_deletes_post_migration_sessions() {
        let _guard = opencode_env_lock().lock().expect("lock");
        let temp = tempdir().expect("tempdir");
        let original_xdg = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", temp.path());

        let base_dir = temp.path().join("opencode");
        std::fs::create_dir_all(&base_dir).expect("create base dir");
        let db_path = base_dir.join("opencode.db");
        let conn = Connection::open(&db_path).expect("open sqlite db");
        create_sqlite_schema(&conn);
        create_sqlite_schema_v2(&conn);
        conn.execute_batch(
            "CREATE TABLE kv (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL
            );",
        )
        .expect("create kv table");
        conn.execute(
            "INSERT INTO kv (key, value, time_created, time_updated) VALUES (?1, ?2, ?3, ?4)",
            (
                "migration.v1-v2",
                r#"{"phase":"completed"}"#,
                1900_i64,
                2000_i64,
            ),
        )
        .expect("insert migration marker");

        // Pre-migration V1 session absent from V2 (deleted while on 2.x) -> hidden
        conn.execute(
            "INSERT INTO session (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_v1_deleted_in_v2", "Old Deleted", "/tmp/old", 1000_i64, 1500_i64),
        )
        .expect("insert old v1 session");

        // Migrated session present in both tables -> listed once, from V2
        conn.execute(
            "INSERT INTO session (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_migrated", "Migrated Session", "/tmp/migrated", 1200_i64, 1800_i64),
        )
        .expect("insert migrated v1 row");
        conn.execute(
            "INSERT INTO session_v2 (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_migrated", "Migrated Session", "/tmp/migrated", 1200_i64, 2500_i64),
        )
        .expect("insert migrated v2 row");
        conn.execute(
            "INSERT INTO session_message (id, session_id, type, seq, data, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            ("msg_v2_mig", "ses_migrated", "user", 1_i64, r#"{"text":"From V2 table"}"#, 1200_i64, 2500_i64),
        )
        .expect("insert v2 message");

        // Post-migration V1 session (user went back to 1.x) -> listed
        conn.execute(
            "INSERT INTO session (id, title, directory, time_created, time_updated) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("ses_v1_post", "Post Migration 1.x", "/tmp/v1-post", 2800_i64, 3000_i64),
        )
        .expect("insert post-migration v1 session");
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, data) VALUES (?1, ?2, ?3, ?4)",
            ("msg_v1_post", "ses_v1_post", 2800_i64, r#"{"role":"user"}"#),
        )
        .expect("insert v1 message");
        conn.execute(
            "INSERT INTO part (id, session_id, message_id, time_created, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            ("prt_v1_post", "ses_v1_post", "msg_v1_post", 2800_i64, r#"{"type":"text","text":"Hello from 1.x after migration"}"#),
        )
        .expect("insert v1 part");
        drop(conn);

        let sessions = scan_sessions_sqlite();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, "ses_v1_post");
        assert_eq!(sessions[0].title.as_deref(), Some("Post Migration 1.x"));
        assert_eq!(sessions[0].last_active_at, Some(3000));
        assert_eq!(sessions[1].session_id, "ses_migrated");
        assert_eq!(sessions[1].last_active_at, Some(2500));

        // Post-migration V1 session reads from the V1 tables
        let v1_source = format!("sqlite:{}:ses_v1_post", db_path.display());
        let v1_msgs = load_messages_sqlite(&v1_source).expect("load post-migration v1 messages");
        assert_eq!(v1_msgs.len(), 1);
        assert_eq!(v1_msgs[0].role, "user");
        assert_eq!(v1_msgs[0].content, "Hello from 1.x after migration");

        // Migrated session reads from session_message
        let v2_source = format!("sqlite:{}:ses_migrated", db_path.display());
        let v2_msgs = load_messages_sqlite(&v2_source).expect("load migrated v2 messages");
        assert_eq!(v2_msgs.len(), 1);
        assert_eq!(v2_msgs[0].content, "From V2 table");

        assert!(delete_session_sqlite("ses_v1_post", &v1_source).expect("delete v1_post"));
        assert!(delete_session_sqlite("ses_migrated", &v2_source).expect("delete migrated"));

        let conn = Connection::open(&db_path).expect("re-open sqlite db");
        let remaining_v2: i64 = conn
            .query_row("SELECT COUNT(*) FROM session_v2", [], |row| row.get(0))
            .expect("count v2");
        let remaining_v1_active: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM session WHERE id IN ('ses_v1_post', 'ses_migrated')",
                [],
                |row| row.get(0),
            )
            .expect("count v1");
        let remaining_v1_messages: i64 = conn
            .query_row("SELECT COUNT(*) FROM message", [], |row| row.get(0))
            .expect("count v1 messages");

        restore_xdg(original_xdg);

        assert_eq!(remaining_v2, 0);
        assert_eq!(remaining_v1_active, 0);
        assert_eq!(remaining_v1_messages, 0);
    }
}
