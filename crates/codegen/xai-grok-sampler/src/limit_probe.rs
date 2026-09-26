//! LOCAL(limit-probe): passive evidence log for reverse-inferring provider-side
//! prompt-cache TTL and RPM/TPM rate limits from long-term agent traffic
//! (method: `docs-local/limit-inference.md`).
//!
//! Best-effort JSON lines, one per physical HTTP response ("req"), per non-2xx
//! error body ("err") and per chat-backend usage terminal ("usage"). All
//! failures are silently ignored — the probe must never affect sampling.
//!
//! - File: `<USERPROFILE|HOME>/.grok/limit-probe/records.ndjson`
//!   (override with `GROK_LIMIT_PROBE_FILE`), rotated at 32 MiB to `.1`.
//! - Disable with `GROK_LIMIT_PROBE=0`.
//!
//! Prefix identity (`tools_hash` / `msgs_hash`) is a 64-bit FNV-1a over the
//! serialized request fragments — stable within this binary's lifetime, which
//! is all the offline analysis needs (joins happen per log file).

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::json;

use xai_grok_sampling_types::types::{ChatCompletionRequest, Usage};
use xai_grok_sampling_types::MessagesRequestWrapper;

use crate::client::extract_retry_after;

const ROTATE_CAP_BYTES: u64 = 32 * 1024 * 1024;
const RL_VALUE_CAP_CHARS: usize = 80;
const ERR_BODY_CAP_CHARS: usize = 500;

static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("GROK_LIMIT_PROBE").is_ok_and(|v| v != "0"));
static LOG_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
    if let Ok(path) = std::env::var("GROK_LIMIT_PROBE_FILE") {
        return PathBuf::from(path);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home)
        .join(".grok")
        .join("limit-probe")
        .join("records.ndjson")
});
static WRITE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn enabled() -> bool {
    *ENABLED
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn append_line(line: &str) {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let path = &*LOG_PATH;
    if let Ok(meta) = std::fs::metadata(path)
        && meta.len() > ROTATE_CAP_BYTES
        && let Some(rotated) = path.to_str().map(|p| format!("{p}.1"))
    {
        let _ = std::fs::rename(path, rotated);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes()).and_then(|_| file.write_all(b"\n"));
    }
}

fn truncate(value: &str, cap: usize) -> String {
    value.chars().take(cap).collect()
}

// =============================================================================
// Request identity (prefix hashes)
// =============================================================================

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv_update(mut hash: u64, data: &[u8]) -> u64 {
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Length-prefixed FNV-1a chain over the parts, rendered as 16 hex chars.
/// Length prefixes keep `(a, b)` and `(ab,)` collisions apart.
fn hash_parts(parts: &[&[u8]]) -> String {
    let mut hash = FNV_OFFSET;
    for part in parts {
        hash = fnv_update(hash, &(part.len() as u64).to_le_bytes());
        hash = fnv_update(hash, part);
    }
    format!("{hash:016x}")
}

fn hash_serialized<T: Serialize + ?Sized>(value: &T) -> Option<String> {
    serde_json::to_vec(value)
        .ok()
        .map(|bytes| hash_parts(&[&bytes]))
}

/// Cached prefix identity of one outgoing request: which tools block and which
/// message prefix it carries. Cross-backend comparability is not a goal; the
/// analysis only joins lines produced by the same backend shape.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequestIdentity {
    pub tools_hash: Option<String>,
    pub msgs_hash: Option<String>,
    pub msgs_count: Option<u32>,
}

impl RequestIdentity {
    pub fn from_chat(request: &ChatCompletionRequest) -> Self {
        Self {
            tools_hash: request.tools.as_ref().and_then(hash_serialized),
            msgs_hash: hash_serialized(&request.messages),
            msgs_count: u32::try_from(request.messages.len()).ok(),
        }
    }

    /// Responses API: identity from the post-processed JSON body (`input` + `tools`).
    pub fn from_json_body(body: &serde_json::Value) -> Self {
        let tools_hash = body.get("tools").and_then(hash_serialized);
        let (msgs_hash, msgs_count) = match body.get("input") {
            Some(input) => (
                hash_serialized(input),
                input.as_array().and_then(|a| u32::try_from(a.len()).ok()),
            ),
            None => (None, None),
        };
        Self {
            tools_hash,
            msgs_hash,
            msgs_count,
        }
    }

    pub fn from_messages(request: &MessagesRequestWrapper) -> Self {
        let system = request
            .inner
            .system
            .as_ref()
            .and_then(|s| serde_json::to_vec(s).ok());
        let messages = serde_json::to_vec(&request.inner.messages).ok();
        let msgs_hash = match (system.as_deref(), messages.as_deref()) {
            (Some(system), Some(messages)) => Some(hash_parts(&[system, messages])),
            (Some(system), None) => Some(hash_parts(&[system])),
            (None, Some(messages)) => Some(hash_parts(&[messages])),
            (None, None) => None,
        };
        Self {
            tools_hash: request.inner.tools.as_ref().and_then(hash_serialized),
            msgs_hash,
            msgs_count: u32::try_from(request.inner.messages.len()).ok(),
        }
    }
}

// =============================================================================
// Notes
// =============================================================================

/// Identity of one call site; `None` when the probe is disabled so callers can
/// hand the same `Option` down to response/error/usage notes without re-gating.
#[derive(Debug, Clone)]
pub struct CallNote {
    pub backend: &'static str,
    pub mode: &'static str,
    pub model: String,
    pub session_id: String,
    pub req_id: String,
    pub attempt: Option<String>,
    pub identity: RequestIdentity,
}

impl CallNote {
    fn new(
        backend: &'static str,
        mode: &'static str,
        model: &str,
        session_id: &str,
        req_id: &str,
        attempt: Option<&str>,
    ) -> Option<Self> {
        if !enabled() {
            return None;
        }
        Some(Self {
            backend,
            mode,
            model: model.to_owned(),
            session_id: session_id.to_owned(),
            req_id: req_id.to_owned(),
            attempt: attempt.map(str::to_owned),
            identity: RequestIdentity::default(),
        })
    }

    /// Chat Completions request. `mode`: `"stream"` or `"nonstream"`.
    pub fn chat(
        mode: &'static str,
        model: &str,
        session_id: &str,
        req_id: &str,
        attempt: Option<&str>,
        request: &ChatCompletionRequest,
    ) -> Option<Self> {
        let mut note = Self::new("chat", mode, model, session_id, req_id, attempt)?;
        note.identity = RequestIdentity::from_chat(request);
        Some(note)
    }

    /// Responses API request from the post-processed JSON body.
    pub fn json_body(
        mode: &'static str,
        model: &str,
        session_id: &str,
        req_id: &str,
        attempt: Option<&str>,
        body: &serde_json::Value,
    ) -> Option<Self> {
        let mut note = Self::new("responses", mode, model, session_id, req_id, attempt)?;
        note.identity = RequestIdentity::from_json_body(body);
        Some(note)
    }

    /// Anthropic Messages API request.
    pub fn messages(
        mode: &'static str,
        model: &str,
        session_id: &str,
        req_id: &str,
        attempt: Option<&str>,
        request: &MessagesRequestWrapper,
    ) -> Option<Self> {
        let mut note = Self::new("messages", mode, model, session_id, req_id, attempt)?;
        note.identity = RequestIdentity::from_messages(request);
        Some(note)
    }
}

fn render_req(note: &CallNote, status: u16, headers: &reqwest::header::HeaderMap) -> serde_json::Value {
    let mut rl = serde_json::Map::new();
    for (name, value) in headers.iter() {
        let lower = name.as_str().to_ascii_lowercase();
        if lower.contains("ratelimit") {
            let rendered = value.to_str().map_or("[non-utf8]".to_owned(), |v| {
                truncate(v, RL_VALUE_CAP_CHARS)
            });
            rl.insert(lower, json!(rendered));
        }
    }
    json!({
        "kind": "req",
        "ts_ms": now_ms(),
        "backend": note.backend,
        "mode": note.mode,
        "model": note.model,
        "session_id": note.session_id,
        "req_id": note.req_id,
        "attempt": note.attempt,
        "status": status,
        "retry_after": extract_retry_after(headers),
        "rl": rl,
        "tools_hash": note.identity.tools_hash,
        "msgs_hash": note.identity.msgs_hash,
        "msgs_count": note.identity.msgs_count,
    })
}

/// One line per physical HTTP response: status, rate-limit headers, retry-after
/// and the request's prefix identity. Retry attempts share `req_id` and differ
/// in `attempt`, so the 429-sliding-window analysis can attribute each attempt.
pub fn note_response(note: &CallNote, status: u16, headers: &reqwest::header::HeaderMap) {
    append_line(&render_req(note, status, headers).to_string());
}

/// Non-2xx body preview, so the offline pass can separate rate-limit 429s from
/// quota-exhaustion and other error families without re-probing.
pub fn note_error_body(note: &CallNote, status: u16, body: &[u8]) {
    let preview = truncate(&String::from_utf8_lossy(body), ERR_BODY_CAP_CHARS);
    let record = json!({
        "kind": "err",
        "ts_ms": now_ms(),
        "backend": note.backend,
        "model": note.model,
        "session_id": note.session_id,
        "req_id": note.req_id,
        "status": status,
        "body": preview,
    });
    append_line(&record.to_string());
}

/// Terminal chat usage: the cached-token signal the TTL analysis needs, stored
/// next to the same prefix identity as the matching "req" line.
pub fn note_usage(
    note: &CallNote,
    prompt_tokens: u32,
    cached_prompt_tokens: u32,
    completion_tokens: u32,
    reasoning_tokens: u32,
) {
    let record = json!({
        "kind": "usage",
        "ts_ms": now_ms(),
        "backend": note.backend,
        "mode": note.mode,
        "model": note.model,
        "session_id": note.session_id,
        "req_id": note.req_id,
        "prompt_tokens": prompt_tokens,
        "cached_prompt_tokens": cached_prompt_tokens,
        "completion_tokens": completion_tokens,
        "reasoning_tokens": reasoning_tokens,
        "tools_hash": note.identity.tools_hash,
        "msgs_hash": note.identity.msgs_hash,
        "msgs_count": note.identity.msgs_count,
    });
    append_line(&record.to_string());
}

/// `(prompt, cached, completion, reasoning)` from the Chat Completions wire
/// usage, same fold order as `TokenUsage::from` (DeepSeek flat fields max-in).
pub fn chat_usage_parts(usage: &Usage) -> (u32, u32, u32, u32) {
    let cached = usage
        .prompt_tokens_details
        .as_ref()
        .map_or(0, |d| d.cached_tokens)
        .max(usage.prompt_cache_hit_tokens.unwrap_or(0));
    let reasoning = usage
        .completion_tokens_details
        .as_ref()
        .map_or(0, |d| d.reasoning_tokens);
    (
        usage.prompt_tokens,
        cached,
        usage.completion_tokens,
        reasoning,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_parts_is_deterministic_and_length_separated() {
        let a = hash_parts(&[b"ab", b"c"]);
        let b = hash_parts(&[b"ab", b"c"]);
        let c = hash_parts(&[b"abc"]);
        let d = hash_parts(&[b"a", b"bc"]);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn identity_from_chat_hashes_tools_and_messages() {
        let parse = |json: serde_json::Value| -> ChatCompletionRequest {
            serde_json::from_value(json).expect("chat request should deserialize")
        };
        let request = parse(serde_json::json!({
            "model": "m",
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user", "content": "hi"},
            ],
        }));
        let identity = RequestIdentity::from_chat(&request);
        assert_eq!(identity.msgs_count, Some(2));
        assert!(identity.msgs_hash.is_some());
        assert!(identity.tools_hash.is_none());
        // Same content -> same hash; grown tail -> different hash.
        let mut grown = request.clone();
        grown.messages.push(
            serde_json::from_value(serde_json::json!({"role": "user", "content": "more"}))
                .expect("message should deserialize"),
        );
        assert_ne!(
            identity.msgs_hash,
            RequestIdentity::from_chat(&grown).msgs_hash
        );
        // A changed tools block gets its own hash.
        let mut with_tools = request.clone();
        with_tools.tools = Some(vec![serde_json::from_value(serde_json::json!({
            "type": "function",
            "function": {"name": "f", "parameters": {}},
        }))
        .expect("tool should deserialize")]);
        let with_tools_identity = RequestIdentity::from_chat(&with_tools);
        assert!(with_tools_identity.tools_hash.is_some());
        assert_eq!(with_tools_identity.msgs_hash, identity.msgs_hash);
    }

    #[test]
    fn identity_from_json_body_reads_input_and_tools() {
        let body = serde_json::json!({
            "model": "m",
            "input": [{"role": "user", "content": "hi"}],
            "tools": [{"type": "function", "name": "f"}],
        });
        let identity = RequestIdentity::from_json_body(&body);
        assert_eq!(identity.msgs_count, Some(1));
        assert!(identity.tools_hash.is_some());
        assert!(RequestIdentity::from_json_body(&serde_json::json!({})).msgs_hash.is_none());
    }

    #[test]
    fn render_req_collects_only_ratelimit_headers() {
        let note = CallNote {
            backend: "chat",
            mode: "stream",
            model: "m".into(),
            session_id: "s".into(),
            req_id: "r".into(),
            attempt: Some("1".into()),
            identity: RequestIdentity {
                tools_hash: Some("t".into()),
                msgs_hash: None,
                msgs_count: Some(3),
            },
        };
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-ratelimit-remaining-requests", "57".parse().unwrap());
        headers.insert("anthropic-ratelimit-tokens-limit", "80000".parse().unwrap());
        headers.insert("x-grok-internal", "secret".parse().unwrap());
        headers.insert("retry-after", "7".parse().unwrap());
        let record = render_req(&note, 429, &headers);
        assert_eq!(record["kind"], "req");
        assert_eq!(record["status"], 429);
        assert_eq!(record["retry_after"], 7);
        assert_eq!(record["msgs_count"], 3);
        assert_eq!(record["attempt"], "1");
        let rl = record["rl"].as_object().unwrap();
        assert_eq!(rl.len(), 2);
        assert!(rl.contains_key("x-ratelimit-remaining-requests"));
        assert!(rl.contains_key("anthropic-ratelimit-tokens-limit"));
    }

    #[test]
    fn truncate_caps_by_chars() {
        assert_eq!(truncate("abcdef", 3), "abc");
        assert_eq!(truncate("短短短", 2), "短短");
    }
}
