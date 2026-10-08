use super::providers::{common, mcode};
use super::{load_messages, SessionMessageKind, SessionMeta};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

const CACHE_BUDGET_BYTES: usize = 128 * 1024 * 1024;
const RESULT_LIMIT: usize = 100;
const HIT_LIMIT: usize = 20;
const INDEX_WORKERS: usize = 4;
const SNIPPET_MAX_CHARS: usize = 160;
const SNIPPET_LEAD_CHARS: usize = 40;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchRequest {
    pub request_id: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub provider_ids: Vec<String>,
    #[serde(default)]
    pub project_dir: Option<String>,
    #[serde(default)]
    pub active_from: Option<i64>,
    #[serde(default)]
    pub active_to: Option<i64>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub message_kinds: Vec<SessionMessageKind>,
    #[serde(default)]
    pub force_refresh: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionSearchField {
    Title,
    Summary,
    SessionId,
    ProjectDir,
    SourcePath,
    Message,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchHit {
    pub field: SessionSearchField,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<SessionMessageKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<i64>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchResult {
    pub session: SessionMeta,
    pub score: u32,
    pub total_matches: usize,
    pub hits: Vec<SessionSearchHit>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchResponse {
    pub results: Vec<SessionSearchResult>,
    pub total_sessions: usize,
    pub total_matches: usize,
    pub truncated: bool,
    pub failed_sessions: usize,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionSearchPhase {
    Scanning,
    Indexing,
    Searching,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchProgress {
    pub request_id: String,
    pub phase: SessionSearchPhase,
    pub indexed: usize,
    pub total: usize,
}

#[derive(Clone)]
pub struct SessionSearchState {
    inner: Arc<Mutex<SearchCache>>,
}

impl Default for SessionSearchState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SearchCache::default())),
        }
    }
}

impl SessionSearchState {
    pub fn clear(&self) {
        if let Ok(mut cache) = self.inner.lock() {
            cache.clear();
        }
    }
}

#[derive(Default)]
struct SearchCache {
    entries: HashMap<String, CachedSession>,
    bytes: usize,
    clock: u64,
}

struct CachedSession {
    fingerprint: SourceFingerprint,
    documents: Arc<Vec<SearchDocument>>,
    bytes: usize,
    last_used: u64,
}

impl SearchCache {
    fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
        self.clock = 0;
    }

    fn get(
        &mut self,
        key: &str,
        fingerprint: &SourceFingerprint,
    ) -> Option<Arc<Vec<SearchDocument>>> {
        if !fingerprint.is_reliable() {
            return None;
        }

        let matches = self
            .entries
            .get(key)
            .is_some_and(|entry| &entry.fingerprint == fingerprint);
        if !matches {
            self.remove(key);
            return None;
        }

        self.clock = self.clock.saturating_add(1);
        let clock = self.clock;
        let entry = self.entries.get_mut(key)?;
        entry.last_used = clock;
        Some(Arc::clone(&entry.documents))
    }

    fn insert(
        &mut self,
        key: String,
        fingerprint: SourceFingerprint,
        documents: Arc<Vec<SearchDocument>>,
    ) {
        if !fingerprint.is_reliable() {
            return;
        }

        let bytes = documents.iter().map(SearchDocument::heap_bytes).sum();
        if bytes > CACHE_BUDGET_BYTES {
            return;
        }

        self.remove(&key);
        self.clock = self.clock.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes);
        self.entries.insert(
            key,
            CachedSession {
                fingerprint,
                documents,
                bytes,
                last_used: self.clock,
            },
        );
        self.evict_to_budget();
    }

    fn retain(&mut self, active_keys: &HashSet<String>) {
        let stale = self
            .entries
            .keys()
            .filter(|key| !active_keys.contains(*key))
            .cloned()
            .collect::<Vec<_>>();
        for key in stale {
            self.remove(&key);
        }
    }

    fn remove(&mut self, key: &str) {
        if let Some(entry) = self.entries.remove(key) {
            self.bytes = self.bytes.saturating_sub(entry.bytes);
        }
    }

    fn evict_to_budget(&mut self) {
        while self.bytes > CACHE_BUDGET_BYTES {
            let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            self.remove(&oldest_key);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceFingerprint {
    last_active_at: Option<i64>,
    created_at: Option<i64>,
    modified_ms: Option<i64>,
    size: Option<u64>,
}

impl SourceFingerprint {
    fn is_reliable(&self) -> bool {
        self.last_active_at.is_some()
            || self.created_at.is_some()
            || self.modified_ms.is_some()
            || self.size.is_some()
    }
}

#[derive(Clone)]
struct SearchDocument {
    field: SessionSearchField,
    message_index: Option<usize>,
    role: Option<String>,
    kind: Option<SessionMessageKind>,
    ts: Option<i64>,
    text: String,
    normalized: String,
}

impl SearchDocument {
    fn metadata(field: SessionSearchField, text: Option<&str>) -> Option<Self> {
        let text = text?.trim();
        if text.is_empty() {
            return None;
        }
        Some(Self::new(field, None, None, None, None, text))
    }

    fn message(message_index: usize, message: &super::SessionMessage) -> Self {
        Self::new(
            SessionSearchField::Message,
            Some(message_index),
            Some(message.role.clone()),
            Some(message.kind.clone()),
            message.ts,
            &message.content,
        )
    }

    fn new(
        field: SessionSearchField,
        message_index: Option<usize>,
        role: Option<String>,
        kind: Option<SessionMessageKind>,
        ts: Option<i64>,
        text: &str,
    ) -> Self {
        let text = text.to_string();
        let normalized = normalize_text(&text);
        Self {
            field,
            message_index,
            role,
            kind,
            ts,
            text,
            normalized,
        }
    }

    fn heap_bytes(&self) -> usize {
        self.text.len()
            + self.normalized.len()
            + self.role.as_ref().map_or(0, String::len)
            + std::mem::size_of::<Self>()
    }

    fn matches(&self, terms: &[String]) -> bool {
        terms.iter().all(|term| self.normalized.contains(term))
    }
}

pub fn search_sessions_with_progress<F>(
    state: &SessionSearchState,
    request: SessionSearchRequest,
    hidden_provider_ids: &BTreeSet<String>,
    mut report_progress: F,
) -> SessionSearchResponse
where
    F: FnMut(SessionSearchProgress),
{
    if request.force_refresh {
        state.clear();
    }

    report_progress(SessionSearchProgress {
        request_id: request.request_id.clone(),
        phase: SessionSearchPhase::Scanning,
        indexed: 0,
        total: 0,
    });

    let sessions = super::scan_sessions_excluding(hidden_provider_ids);
    let active_keys = sessions
        .iter()
        .filter_map(cache_key)
        .collect::<HashSet<_>>();
    if let Ok(mut cache) = state.inner.lock() {
        cache.retain(&active_keys);
    }

    let candidates = sessions
        .into_iter()
        .filter(|session| session_matches_meta_filters(session, &request))
        .collect::<Vec<_>>();
    let total = candidates.len();
    report_progress(SessionSearchProgress {
        request_id: request.request_id.clone(),
        phase: SessionSearchPhase::Indexing,
        indexed: 0,
        total,
    });

    let terms = query_terms(&request.query);
    let needs_messages =
        !terms.is_empty() || !request.roles.is_empty() || !request.message_kinds.is_empty();
    let mut loaded = Vec::with_capacity(candidates.len());
    let mut failed_sessions: usize = 0;
    let mut indexed: usize = 0;

    for chunk in candidates.chunks(INDEX_WORKERS) {
        let results = std::thread::scope(|scope| {
            let handles = chunk
                .iter()
                .cloned()
                .map(|session| {
                    let state = state.clone();
                    scope.spawn(move || {
                        if needs_messages {
                            let documents = load_session_documents(&state, &session);
                            (session, Some(documents))
                        } else {
                            (session, None)
                        }
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join())
                .collect::<Vec<_>>()
        });

        for result in results {
            indexed = indexed.saturating_add(1);
            match result {
                Ok((session, documents)) => match documents {
                    Some(Ok(documents)) => loaded.push((session, Some(documents))),
                    Some(Err(_)) => {
                        failed_sessions = failed_sessions.saturating_add(1);
                        loaded.push((session, None));
                    }
                    None => loaded.push((session, None)),
                },
                Err(_) => failed_sessions = failed_sessions.saturating_add(1),
            }
            report_progress(SessionSearchProgress {
                request_id: request.request_id.clone(),
                phase: SessionSearchPhase::Indexing,
                indexed,
                total,
            });
        }
    }

    report_progress(SessionSearchProgress {
        request_id: request.request_id.clone(),
        phase: SessionSearchPhase::Searching,
        indexed,
        total,
    });

    let mut results = loaded
        .into_iter()
        .filter_map(|(session, documents)| {
            let message_documents = documents.as_ref().map(|documents| documents.as_slice());
            search_session_result(session, message_documents, &request, &terms)
        })
        .collect::<Vec<_>>();

    results.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| session_timestamp(&b.session).cmp(&session_timestamp(&a.session)))
            .then_with(|| a.session.session_id.cmp(&b.session.session_id))
    });

    let total_sessions = results.len();
    let total_matches = results.iter().map(|result| result.total_matches).sum();
    let truncated = total_sessions > RESULT_LIMIT;
    results.truncate(RESULT_LIMIT);

    SessionSearchResponse {
        results,
        total_sessions,
        total_matches,
        truncated,
        failed_sessions,
    }
}

fn load_session_documents(
    state: &SessionSearchState,
    session: &SessionMeta,
) -> Result<Arc<Vec<SearchDocument>>, String> {
    let Some(source_path) = session.source_path.as_deref() else {
        return Err("Session has no source path".to_string());
    };
    let key = format!(
        "{}\0{}\0{}",
        session.provider_id, session.session_id, source_path
    );
    let fingerprint = source_fingerprint(session, source_path);

    if let Ok(mut cache) = state.inner.lock() {
        if let Some(documents) = cache.get(&key, &fingerprint) {
            return Ok(documents);
        }
    }

    let messages = load_messages(&session.provider_id, source_path)?;
    let documents = Arc::new(
        messages
            .iter()
            .enumerate()
            .map(|(index, message)| SearchDocument::message(index, message))
            .collect::<Vec<_>>(),
    );

    if let Ok(mut cache) = state.inner.lock() {
        cache.insert(key, fingerprint, Arc::clone(&documents));
    }
    Ok(documents)
}

fn cache_key(session: &SessionMeta) -> Option<String> {
    Some(format!(
        "{}\0{}\0{}",
        session.provider_id,
        session.session_id,
        session.source_path.as_deref()?
    ))
}

fn source_fingerprint(session: &SessionMeta, source_path: &str) -> SourceFingerprint {
    let paths = source_fingerprint_paths(session, source_path);
    let mut modified_ms = None;
    let mut size: Option<u64> = None;

    for path in paths {
        let (path_modified, path_size) = stat_path(&path);
        modified_ms = max_option(modified_ms, path_modified);
        size = match (size, path_size) {
            (Some(left), Some(right)) => Some(left.saturating_add(right)),
            (None, Some(right)) => Some(right),
            (None, _) => None,
            (_, None) => None,
        };
    }

    SourceFingerprint {
        last_active_at: session.last_active_at,
        created_at: session.created_at,
        modified_ms,
        size,
    }
}

fn source_fingerprint_paths(session: &SessionMeta, source_path: &str) -> Vec<PathBuf> {
    if session.provider_id == "mcode" {
        return sqlite_fingerprint_paths(mcode::database_path());
    }

    if let Some((database_path, _)) = common::parse_sqlite_source(source_path) {
        return sqlite_fingerprint_paths(database_path);
    }

    if session.provider_id == "opencode" {
        if let Some(rest) = source_path.strip_prefix("sqlite:") {
            let suffix = format!(":{}", session.session_id);
            if let Some(database_path) = rest.strip_suffix(&suffix) {
                return sqlite_fingerprint_paths(PathBuf::from(database_path));
            }
        }
    }

    let path = PathBuf::from(source_path);
    path.exists().then_some(vec![path]).unwrap_or_default()
}

fn sqlite_fingerprint_paths(database_path: PathBuf) -> Vec<PathBuf> {
    let mut paths = vec![database_path.clone()];
    let wal = PathBuf::from(format!("{}-wal", database_path.display()));
    let shm = PathBuf::from(format!("{}-shm", database_path.display()));
    if wal.exists() {
        paths.push(wal);
    }
    if shm.exists() {
        paths.push(shm);
    }
    paths
}

fn stat_path(path: &Path) -> (Option<i64>, Option<u64>) {
    let Ok(metadata) = fs::metadata(path) else {
        return (None, None);
    };

    if metadata.is_file() {
        return (modified_ms(&metadata), Some(metadata.len()));
    }

    if !metadata.is_dir() {
        return (None, None);
    }

    (modified_ms(&metadata), Some(metadata.len()))
}

fn modified_ms(metadata: &fs::Metadata) -> Option<i64> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
}

fn max_option(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn session_matches_meta_filters(session: &SessionMeta, request: &SessionSearchRequest) -> bool {
    if !request.provider_ids.is_empty()
        && !request
            .provider_ids
            .iter()
            .any(|provider| provider == &session.provider_id)
    {
        return false;
    }

    if let Some(project_dir) = request
        .project_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let Some(session_project_dir) = session.project_dir.as_deref() else {
            return false;
        };
        if !session_project_dir
            .to_lowercase()
            .contains(&project_dir.to_lowercase())
        {
            return false;
        }
    }

    if request.active_from.is_some() || request.active_to.is_some() {
        let Some(timestamp) = session.last_active_at.or(session.created_at) else {
            return false;
        };
        if request
            .active_from
            .is_some_and(|active_from| timestamp < active_from)
        {
            return false;
        }
        if request
            .active_to
            .is_some_and(|active_to| timestamp > active_to)
        {
            return false;
        }
    }

    true
}

fn search_session_result(
    session: SessionMeta,
    message_documents: Option<&[SearchDocument]>,
    request: &SessionSearchRequest,
    terms: &[String],
) -> Option<SessionSearchResult> {
    let metadata_documents = metadata_documents(&session);
    let allowed_messages = message_documents
        .unwrap_or_default()
        .iter()
        .filter(|document| message_matches_filters(document, request));

    let has_query = !terms.is_empty();
    let has_message_filters = !request.roles.is_empty() || !request.message_kinds.is_empty();
    if !has_query && !has_message_filters {
        return Some(SessionSearchResult {
            session,
            score: 0,
            total_matches: 0,
            hits: Vec::new(),
        });
    }

    let mut total_matches = 0_usize;
    let mut hits = Vec::new();
    let mut score = 0_u32;
    let mut message_matches = 0_usize;
    let mut exact_message_match = false;
    let query_phrase = terms.join(" ");

    if has_query {
        for document in &metadata_documents {
            if !document.matches(terms) {
                continue;
            }
            total_matches = total_matches.saturating_add(1);
            score = score.saturating_add(metadata_score(document.field));
            push_hit(&mut hits, document, terms, has_query);
        }

        for document in allowed_messages {
            if !document.matches(terms) {
                continue;
            }
            total_matches = total_matches.saturating_add(1);
            message_matches = message_matches.saturating_add(1);
            exact_message_match |= document.normalized.contains(&query_phrase);
            push_hit(&mut hits, document, terms, has_query);
        }
    } else if has_message_filters {
        for document in message_documents
            .unwrap_or_default()
            .iter()
            .filter(|document| message_matches_filters(document, request))
        {
            total_matches = total_matches.saturating_add(1);
            message_matches = message_matches.saturating_add(1);
            push_hit(&mut hits, document, terms, false);
        }
    }

    if total_matches == 0 {
        return None;
    }

    let message_score = (message_matches.saturating_mul(10)
        + if exact_message_match { 20 } else { 0 })
    .min(100) as u32;
    score = score.saturating_add(message_score);

    Some(SessionSearchResult {
        session,
        score,
        total_matches,
        hits,
    })
}

fn metadata_documents(session: &SessionMeta) -> Vec<SearchDocument> {
    [
        (SessionSearchField::Title, session.title.as_deref()),
        (SessionSearchField::Summary, session.summary.as_deref()),
        (
            SessionSearchField::SessionId,
            Some(session.session_id.as_str()),
        ),
        (
            SessionSearchField::ProjectDir,
            session.project_dir.as_deref(),
        ),
        (
            SessionSearchField::SourcePath,
            session.source_path.as_deref(),
        ),
    ]
    .into_iter()
    .filter_map(|(field, text)| SearchDocument::metadata(field, text))
    .collect()
}

fn message_matches_filters(document: &SearchDocument, request: &SessionSearchRequest) -> bool {
    if !request.roles.is_empty() {
        let Some(role) = document.role.as_deref() else {
            return false;
        };
        if !request
            .roles
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(role))
        {
            return false;
        }
    }

    if !request.message_kinds.is_empty()
        && !document
            .kind
            .as_ref()
            .is_some_and(|kind| request.message_kinds.contains(kind))
    {
        return false;
    }

    true
}

fn metadata_score(field: SessionSearchField) -> u32 {
    match field {
        SessionSearchField::Title => 100,
        SessionSearchField::Summary => 60,
        SessionSearchField::SessionId => 50,
        SessionSearchField::ProjectDir => 40,
        SessionSearchField::SourcePath => 20,
        SessionSearchField::Message => 0,
    }
}

fn push_hit(
    hits: &mut Vec<SessionSearchHit>,
    document: &SearchDocument,
    terms: &[String],
    has_query: bool,
) {
    if hits.len() >= HIT_LIMIT {
        return;
    }
    hits.push(SessionSearchHit {
        field: document.field,
        message_index: document.message_index,
        role: document.role.clone(),
        kind: document.kind.clone(),
        ts: document.ts,
        snippet: if has_query {
            make_snippet(&document.text, terms)
        } else {
            make_snippet(&document.text, &[])
        },
    });
}

fn make_snippet(text: &str, terms: &[String]) -> String {
    let normalized = normalize_text(text);
    let match_byte = terms.iter().filter_map(|term| normalized.find(term)).min();
    let characters = text.chars().collect::<Vec<_>>();
    let match_char = match_byte
        .and_then(|byte_index| text.get(..byte_index))
        .map(|prefix| prefix.chars().count())
        .unwrap_or(0);
    let start = match_char.saturating_sub(SNIPPET_LEAD_CHARS);
    let end = (start + SNIPPET_MAX_CHARS).min(characters.len());
    let excerpt = characters[start..end].iter().collect::<String>();
    let collapsed = excerpt.split_whitespace().collect::<Vec<_>>().join(" ");
    let leading = if start > 0 { "…" } else { "" };
    let trailing = if end < characters.len() { "…" } else { "" };
    format!("{leading}{collapsed}{trailing}")
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(normalize_text)
        .filter(|term| !term.is_empty())
        .collect()
}

fn normalize_text(value: &str) -> String {
    value.to_lowercase()
}

fn session_timestamp(session: &SessionMeta) -> i64 {
    session.last_active_at.or(session.created_at).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_manager::SessionMessage;

    fn message(role: &str, kind: SessionMessageKind, content: &str) -> SearchDocument {
        SearchDocument::message(
            0,
            &SessionMessage {
                role: role.to_string(),
                content: content.to_string(),
                kind,
                tool_call_id: None,
                tool_name: None,
                ts: Some(1_700_000_000_000),
            },
        )
    }

    fn session() -> SessionMeta {
        SessionMeta {
            provider_id: "codex".to_string(),
            session_id: "session-1".to_string(),
            title: Some("Rust parser notes".to_string()),
            summary: Some("Investigating malformed input".to_string()),
            project_dir: Some("/workspace/session2md".to_string()),
            created_at: Some(1_700_000_000_000),
            last_active_at: Some(1_700_000_100_000),
            source_path: Some("/tmp/session.jsonl".to_string()),
            resume_command: None,
            can_delete: true,
        }
    }

    fn request(query: &str) -> SessionSearchRequest {
        SessionSearchRequest {
            request_id: "request".to_string(),
            query: query.to_string(),
            provider_ids: Vec::new(),
            project_dir: None,
            active_from: None,
            active_to: None,
            roles: Vec::new(),
            message_kinds: Vec::new(),
            force_refresh: false,
        }
    }

    #[test]
    fn query_terms_are_case_insensitive_and_require_all_terms() {
        let request = request("RUST parser");
        let terms = query_terms(&request.query);
        let documents = vec![message(
            "user",
            SessionMessageKind::Text,
            "A Rust parser test",
        )];
        let result = search_session_result(session(), Some(&documents), &request, &terms)
            .expect("session matches");
        assert_eq!(result.total_matches, 2);
        assert_eq!(result.hits.len(), 2);
    }

    #[test]
    fn role_and_kind_filters_restrict_message_hits() {
        let mut request = request("needle");
        request.message_kinds = vec![SessionMessageKind::ToolResult];
        let terms = query_terms(&request.query);
        let documents = vec![
            message("assistant", SessionMessageKind::Text, "needle in text"),
            message("tool", SessionMessageKind::ToolResult, "needle in tool"),
        ];
        let result = search_session_result(session(), Some(&documents), &request, &terms)
            .expect("tool result matches");
        assert_eq!(result.total_matches, 1);
        assert_eq!(result.hits[0].kind, Some(SessionMessageKind::ToolResult));
    }

    #[test]
    fn empty_query_with_message_filters_returns_sessions_with_matching_messages() {
        let mut request = request("");
        request.roles = vec!["user".to_string()];
        let documents = vec![
            message("assistant", SessionMessageKind::Text, "answer"),
            message("user", SessionMessageKind::Text, "question"),
        ];
        let result = search_session_result(session(), Some(&documents), &request, &[])
            .expect("user message exists");
        assert_eq!(result.total_matches, 1);
        assert_eq!(result.hits[0].role.as_deref(), Some("user"));
    }

    #[test]
    fn metadata_filters_apply_provider_path_and_time() {
        let session = session();
        let mut request = request("");
        request.provider_ids = vec!["codex".to_string()];
        request.project_dir = Some("SESSION2MD".to_string());
        request.active_from = Some(1_700_000_000_000);
        request.active_to = Some(1_700_000_200_000);
        assert!(session_matches_meta_filters(&session, &request));

        request.project_dir = Some("different".to_string());
        assert!(!session_matches_meta_filters(&session, &request));
    }

    #[test]
    fn metadata_only_search_returns_sessions_without_message_hits() {
        let request = request("");
        let result = search_session_result(session(), Some(&[]), &request, &[])
            .expect("session is returned for metadata-only filters");
        assert_eq!(result.total_matches, 0);
        assert!(result.hits.is_empty());
    }

    #[test]
    fn metadata_hits_survive_unreadable_message_sources() {
        let request = request("rust parser");
        let terms = query_terms(&request.query);
        let result = search_session_result(session(), None, &request, &terms)
            .expect("metadata match is returned without message documents");
        assert_eq!(result.total_matches, 1);
        assert_eq!(result.hits[0].field, SessionSearchField::Title);
    }

    #[test]
    fn snippets_include_match_context_and_ellipsis() {
        let text = format!("{}needle{}", "a".repeat(80), "b".repeat(200));
        let snippet = make_snippet(&text, &["needle".to_string()]);
        assert!(snippet.starts_with('…'));
        assert!(snippet.ends_with('…'));
        assert!(snippet.contains("needle"));
    }

    #[test]
    fn cache_reuses_matching_fingerprint_and_invalidates_changed_one() {
        let fingerprint = SourceFingerprint {
            last_active_at: Some(1),
            created_at: Some(1),
            modified_ms: Some(1),
            size: Some(1),
        };
        let changed = SourceFingerprint {
            size: Some(2),
            ..fingerprint.clone()
        };
        let documents = Arc::new(vec![message("user", SessionMessageKind::Text, "cached")]);
        let mut cache = SearchCache::default();
        cache.insert(
            "key".to_string(),
            fingerprint.clone(),
            Arc::clone(&documents),
        );

        assert!(cache.get("key", &fingerprint).is_some());
        assert!(cache.get("key", &changed).is_none());
        assert!(cache.get("key", &changed).is_none());
    }

    #[test]
    fn source_fingerprint_changes_when_file_size_changes() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("session.jsonl");
        fs::write(&path, "short").expect("write first fixture");
        let mut meta = session();
        meta.source_path = Some(path.display().to_string());
        let first = source_fingerprint(&meta, &path.display().to_string());

        fs::write(&path, "a much longer fixture").expect("write second fixture");
        let second = source_fingerprint(&meta, &path.display().to_string());

        assert_ne!(first, second);
    }
}
