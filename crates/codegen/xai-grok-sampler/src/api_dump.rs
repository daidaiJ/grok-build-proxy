//! LOCAL(api-dump): persist abnormal LLM API responses under `~/.grok/dump/`
//! so provider-compatibility failures can be summarized offline and traced back
//! to the exact request and response.
//!
//! One JSON file per abnormal exchange plus one line per file in
//! `<dir>/index.ndjson` (rotated at 16 MiB). Scripts read the index first
//! (`jq -c 'select(.status == 400)' index.ndjson`), then open the referenced
//! file for the full request/response bodies and SSE prefix.
//!
//! Excluded on purpose: HTTP 429 and rate-limit rejections, which are provider
//! policy rather than compatibility defects. Everything else that fails at the
//! transport, HTTP-status, stream-event, or decode layer is recorded.
//!
//! Dumps contain conversation content and tool output. The directory is
//! user-owned; headers and query parameters are redacted before writing.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, PoisonError, RwLock};

use chrono::{SecondsFormat, Utc};
use reqwest::header::HeaderMap;
use serde_json::{Map, Value, json};

use xai_grok_sampling_types::SamplingError;

const SCHEMA_VERSION: u32 = 1;
const INDEX_ROTATE_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const DEFAULT_MAX_STREAM_BYTES: usize = 256 * 1024;
const DEFAULT_MAX_FILES_PER_DAY: u32 = 200;

/// Process-wide dump settings, resolved from `[api_dump]` at config load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Master switch. Defaults to `false` here; the shell resolves the
    /// user-facing default (on) from config.
    pub enabled: bool,
    /// Root directory; the shell passes `$GROK_HOME/dump`.
    pub dir: PathBuf,
    /// Cap on the request body kept in one dump.
    pub max_body_bytes: usize,
    /// Cap on the SSE prefix kept for a streaming failure.
    pub max_stream_bytes: usize,
    /// Per-day file cap; `0` disables the cap.
    pub max_files_per_day: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            dir: default_dir(),
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            max_stream_bytes: DEFAULT_MAX_STREAM_BYTES,
            max_files_per_day: DEFAULT_MAX_FILES_PER_DAY,
        }
    }
}

fn default_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("GROK_API_DUMP_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home).join(".grok").join("dump")
}

fn env_disabled() -> bool {
    static DISABLED: LazyLock<bool> = LazyLock::new(|| {
        std::env::var("GROK_API_DUMP").is_ok_and(|v| {
            let v = v.trim();
            v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off")
        })
    });
    *DISABLED
}

static SETTINGS: LazyLock<RwLock<Settings>> = LazyLock::new(|| RwLock::new(Settings::default()));

#[cfg(test)]
thread_local! {
    /// Test-only override so a unit test can enable dumping on its own thread
    /// without other tests observing (or writing through) the global switch.
    static TEST_SETTINGS: std::cell::RefCell<Option<Settings>> =
        const { std::cell::RefCell::new(None) };
}

/// Install the resolved settings. Env overrides: `GROK_API_DUMP=0` forces the
/// feature off; `GROK_API_DUMP_DIR` replaces the directory.
pub fn configure(mut settings: Settings) {
    if env_disabled() {
        settings.enabled = false;
    }
    if let Ok(dir) = std::env::var("GROK_API_DUMP_DIR")
        && !dir.trim().is_empty()
    {
        settings.dir = PathBuf::from(dir);
    }
    settings.max_body_bytes = settings.max_body_bytes.max(1);
    settings.max_stream_bytes = settings.max_stream_bytes.max(1);
    *SETTINGS.write().unwrap_or_else(PoisonError::into_inner) = settings;
}

pub fn enabled() -> bool {
    !env_disabled() && current_settings().enabled
}

fn snapshot_settings() -> Settings {
    SETTINGS.read().map(|s| s.clone()).unwrap_or_default()
}

fn current_settings() -> Settings {
    #[cfg(test)]
    if let Some(settings) = TEST_SETTINGS.with(|cell| cell.borrow().clone()) {
        return settings;
    }
    snapshot_settings()
}

/// Test-only: enable dumping for the current thread without touching the
/// process-wide switch other tests in this binary share.
#[cfg(test)]
pub(crate) fn configure_for_test(settings: Settings) {
    TEST_SETTINGS.with(|cell| *cell.borrow_mut() = Some(settings));
}

/// Test-only: drop the thread-local override.
#[cfg(test)]
pub(crate) fn reset_for_test() {
    TEST_SETTINGS.with(|cell| *cell.borrow_mut() = None);
}

/// Identity of the call site, rendered into every dump and index line.
#[derive(Debug, Clone, Default)]
pub struct CallContext {
    pub backend: &'static str,
    pub mode: &'static str,
    pub model: String,
    pub session_id: String,
    pub req_id: String,
}

/// The outbound request as it went on the wire, with secrets redacted.
#[derive(Debug, Clone)]
pub struct RequestCapture {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// Uncompressed JSON body (what the caller built, not the zstd frame).
    pub body: Vec<u8>,
    pub body_truncated: bool,
    pub body_bytes: usize,
}

/// Capture an outbound request when dumping is enabled. `body` is the
/// uncompressed serialized payload.
pub fn capture_request(request: &reqwest::Request, body: &[u8]) -> Option<RequestCapture> {
    if !enabled() {
        return None;
    }
    let settings = current_settings();
    let body_bytes = body.len();
    let (body, body_truncated) = cap_bytes(body, settings.max_body_bytes);
    let headers = request
        .headers()
        .iter()
        .map(|(name, value)| {
            let rendered = if is_sensitive_name(name.as_str()) {
                "[REDACTED]".to_owned()
            } else {
                value.to_str().unwrap_or("[non-utf8]").to_owned()
            };
            (name.as_str().to_owned(), rendered)
        })
        .collect();
    Some(RequestCapture {
        method: request.method().as_str().to_owned(),
        url: redact_url(request.url()),
        headers,
        body,
        body_truncated,
        body_bytes,
    })
}

fn cap_bytes(bytes: &[u8], cap: usize) -> (Vec<u8>, bool) {
    if bytes.len() <= cap {
        return (bytes.to_vec(), false);
    }
    // Keep the prefix valid UTF-8 so the dump stays readable even for a
    // multibyte boundary; lossy conversion is enough for diagnostics.
    let cut = floor_char_boundary(bytes, cap);
    (bytes.get(..cut).unwrap_or_default().to_vec(), true)
}

fn floor_char_boundary(bytes: &[u8], cap: usize) -> usize {
    let mut end = cap.min(bytes.len());
    while end > 0 && std::str::from_utf8(bytes.get(..end).unwrap_or_default()).is_err() {
        end -= 1;
    }
    end
}

fn is_sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("authorization")
        || lower.contains("api-key")
        || lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("cookie")
        || lower == "key"
        || lower.ends_with("_key")
        || lower.ends_with("-key")
}

fn redact_url(url: &reqwest::Url) -> String {
    redact_url_str(url.as_str())
}

/// Redact secret-looking query parameters in a URL string. Applied at capture
/// time and again at write time, so a caller-built `RequestCapture` cannot leak
/// a key even if it skipped [`capture_request`].
fn redact_url_str(raw: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(raw) else {
        return raw.to_owned();
    };
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| {
            let value = if is_sensitive_name(&k) {
                "[REDACTED]".to_owned()
            } else {
                v.into_owned()
            };
            (k.into_owned(), value)
        })
        .collect();
    if !pairs.is_empty() {
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    url.to_string()
}

fn provider_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split('/').next().unwrap_or_default().to_owned()
}

/// Accumulates SSE frames for the dump of a streaming failure. Cheap when the
/// stream succeeds: the buffer is dropped with the request.
#[derive(Debug, Clone, Default)]
pub struct StreamRecorder {
    inner: Arc<Mutex<StreamBuf>>,
}

#[derive(Debug, Default)]
struct StreamBuf {
    text: String,
    truncated: bool,
    frames: u64,
    cap: usize,
}

impl StreamRecorder {
    fn new(cap: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(StreamBuf {
                cap,
                ..StreamBuf::default()
            })),
        }
    }

    /// Record one SSE frame. `event` is the SSE `event:` name (usually
    /// `message`); it is preserved when a gateway uses it for a non-standard
    /// event, which is exactly the compatibility signal worth keeping.
    pub fn push_frame(&self, event: &str, data: &str) {
        let mut buf = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        buf.frames += 1;
        if buf.truncated {
            return;
        }
        let frame = if event.is_empty() || event == "message" {
            format!("data: {data}\n\n")
        } else {
            format!("event: {event}\ndata: {data}\n\n")
        };
        if buf.text.len() + frame.len() > buf.cap {
            let remaining = buf.cap.saturating_sub(buf.text.len());
            let cut = floor_char_boundary(frame.as_bytes(), remaining);
            if let Some(prefix) = frame.get(..cut) {
                buf.text.push_str(prefix);
            }
            buf.truncated = true;
            return;
        }
        buf.text.push_str(&frame);
    }

    fn snapshot(&self) -> (String, u64, bool) {
        let buf = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        (buf.text.clone(), buf.frames, buf.truncated)
    }
}

/// One abnormal call, ready to render. Cloning shares the request capture and
/// the stream recorder, so the scan closures can own a copy.
#[derive(Debug, Clone)]
pub struct CallDump {
    capture: Arc<RequestCapture>,
    ctx: Arc<CallContext>,
    stream: StreamRecorder,
    max_body_bytes: usize,
}

impl CallDump {
    pub fn new(capture: RequestCapture, ctx: CallContext) -> Self {
        let settings = current_settings();
        Self {
            capture: Arc::new(capture),
            ctx: Arc::new(ctx),
            stream: StreamRecorder::new(settings.max_stream_bytes),
            max_body_bytes: settings.max_body_bytes,
        }
    }

    pub fn stream_recorder(&self) -> StreamRecorder {
        self.stream.clone()
    }

    /// Record a non-streaming failure (HTTP status, transport, or decode).
    pub fn note_failure(
        &self,
        status: Option<u16>,
        headers: Option<&HeaderMap>,
        body: Option<&[u8]>,
        err: &SamplingError,
    ) {
        self.record(status, headers, body.map(|b| (b, false)), None, err);
    }

    /// Record a streaming failure. The SSE prefix comes from the recorder.
    pub fn note_stream_failure(
        &self,
        status: Option<u16>,
        headers: Option<&HeaderMap>,
        recorder: &StreamRecorder,
        err: &SamplingError,
    ) {
        let (frames, frame_count, truncated) = recorder.snapshot();
        self.record(
            status,
            headers,
            None,
            Some((frames, frame_count, truncated)),
            err,
        );
    }

    fn record(
        &self,
        status: Option<u16>,
        headers: Option<&HeaderMap>,
        body: Option<(&[u8], bool)>,
        stream: Option<(String, u64, bool)>,
        err: &SamplingError,
    ) {
        if is_rate_limit(err) {
            return;
        }
        let Some(summary) = ErrorSummary::from_sampling_error(err) else {
            return;
        };
        let settings = current_settings();
        if !settings.enabled || env_disabled() {
            return;
        }
        let now = Utc::now();
        let date_dir = now.format("%Y-%m-%d").to_string();
        let stamp = now.format("%Y%m%dT%H%M%S%.3fZ").to_string();
        let iso = now.to_rfc3339_opts(SecondsFormat::Millis, true);
        let day_dir = settings.dir.join(&date_dir);
        if std::fs::create_dir_all(&day_dir).is_err() {
            return;
        }
        if !reserve_daily_slot(&date_dir, settings.max_files_per_day) {
            return;
        }
        let file_name = format!(
            "{stamp}-{kind}-{backend}-{mode}-{model}-{req}.json",
            kind = summary.kind,
            backend = self.ctx.backend,
            mode = self.ctx.mode,
            model = sanitize(&self.ctx.model, 36),
            req = sanitize(&self.ctx.req_id, 12),
        );
        let path = day_dir.join(&file_name);
        let relative = format!("{date_dir}/{file_name}");

        let mut request = Map::new();
        request.insert("method".into(), json!(self.capture.method));
        request.insert("url".into(), json!(redact_url_str(&self.capture.url)));
        request.insert("headers".into(), json!(header_map(&self.capture.headers)));
        let (request_body, request_truncated) = cap_bytes(&self.capture.body, self.max_body_bytes);
        request.insert(
            "body".into(),
            render_body(
                &request_body,
                request_truncated || self.capture.body_truncated,
            ),
        );
        request.insert("body_bytes".into(), json!(self.capture.body_bytes));
        request.insert(
            "body_truncated".into(),
            json!(request_truncated || self.capture.body_truncated),
        );

        let mut response = Map::new();
        response.insert("status".into(), json!(status));
        if let Some(headers) = headers {
            response.insert(
                "headers".into(),
                json!(header_map(&redact_headers(headers))),
            );
        }
        if let Some((bytes, _)) = body {
            let (kept, truncated) = cap_bytes(bytes, self.max_body_bytes);
            response.insert("body".into(), render_body(&kept, truncated));
            response.insert("body_bytes".into(), json!(bytes.len()));
            response.insert("body_truncated".into(), json!(truncated));
        }

        let mut doc = Map::new();
        doc.insert("schema_version".into(), json!(SCHEMA_VERSION));
        doc.insert("ts".into(), json!(iso));
        doc.insert("kind".into(), json!(summary.kind));
        doc.insert("backend".into(), json!(self.ctx.backend));
        doc.insert("mode".into(), json!(self.ctx.mode));
        doc.insert("model".into(), json!(self.ctx.model));
        doc.insert("provider".into(), json!(provider_of(&self.capture.url)));
        doc.insert("session_id".into(), json!(self.ctx.session_id));
        doc.insert("req_id".into(), json!(self.ctx.req_id));
        doc.insert("error".into(), summary.to_json());
        doc.insert("request".into(), Value::Object(request));
        doc.insert("response".into(), Value::Object(response));
        if let Some((frames, frame_count, truncated)) = stream.as_ref() {
            doc.insert(
                "stream".into(),
                json!({
                    "frames": frames,
                    "frame_count": frame_count,
                    "truncated": truncated,
                }),
            );
        }
        let rendered = serde_json::to_string_pretty(&Value::Object(doc)).unwrap_or_default();
        if std::fs::write(&path, rendered).is_err() {
            return;
        }
        let index = json!({
            "ts": iso,
            "kind": summary.kind,
            "status": status,
            "backend": self.ctx.backend,
            "mode": self.ctx.mode,
            "model": self.ctx.model,
            "provider": provider_of(&self.capture.url),
            "session_id": self.ctx.session_id,
            "req_id": self.ctx.req_id,
            "error_type": summary.error_type,
            "error_code": summary.error_code,
            "message": truncate(&summary.message, 400),
            "file": relative,
            "request_bytes": self.capture.body_bytes,
            "stream_truncated": stream.as_ref().map(|(_, _, t)| *t),
        });
        append_index(&settings.dir, &index.to_string());
    }
}

fn is_rate_limit(err: &SamplingError) -> bool {
    matches!(
        err,
        SamplingError::Api {
            status: reqwest::StatusCode::TOO_MANY_REQUESTS,
            ..
        }
    )
}

fn redact_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            let rendered = if is_sensitive_name(name.as_str()) {
                "[REDACTED]".to_owned()
            } else {
                value.to_str().unwrap_or("[non-utf8]").to_owned()
            };
            (name.as_str().to_owned(), rendered)
        })
        .collect()
}

fn header_map(headers: &[(String, String)]) -> Value {
    Value::Object(
        headers
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect(),
    )
}

fn render_body(bytes: &[u8], truncated: bool) -> Value {
    if !truncated && let Ok(value) = serde_json::from_slice::<Value>(bytes) {
        return value;
    }
    Value::String(String::from_utf8_lossy(bytes).into_owned())
}

fn sanitize(value: &str, cap: usize) -> String {
    let mut out = String::with_capacity(cap.min(value.len()));
    for ch in value.chars() {
        if out.len() >= cap {
            break;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "none".to_owned()
    } else {
        out
    }
}

fn truncate(value: &str, cap: usize) -> String {
    if value.chars().count() <= cap {
        return value.to_owned();
    }
    let mut out: String = value.chars().take(cap).collect();
    out.push('…');
    out
}

struct ErrorSummary {
    kind: &'static str,
    status: Option<u16>,
    error_type: Option<String>,
    error_code: Option<String>,
    message: String,
}

impl ErrorSummary {
    fn from_sampling_error(err: &SamplingError) -> Option<Self> {
        let summary = match err {
            SamplingError::Api {
                status,
                message,
                error_code,
                ..
            } => Self {
                kind: "http_error",
                status: Some(status.as_u16()),
                error_type: None,
                error_code: error_code.as_ref().map(|c| c.as_str().to_owned()),
                message: message.clone(),
            },
            SamplingError::Http(err) => Self {
                kind: "transport",
                status: None,
                error_type: None,
                error_code: None,
                message: err.to_string(),
            },
            SamplingError::EventStreamError(message) => Self {
                kind: "transport",
                status: None,
                error_type: None,
                error_code: None,
                message: message.clone(),
            },
            SamplingError::Serialization(err) => Self {
                kind: "decode",
                status: None,
                error_type: None,
                error_code: None,
                message: err.to_string(),
            },
            SamplingError::StreamError {
                error_type,
                message,
                code,
            } => Self {
                kind: "stream_error",
                status: None,
                error_type: Some(error_type.clone()),
                error_code: code.as_ref().map(|c| c.as_str().to_owned()),
                message: message.clone(),
            },
            SamplingError::IdleTimeout { elapsed_secs } => Self {
                kind: "idle_timeout",
                status: None,
                error_type: None,
                error_code: None,
                message: format!("inference idle timeout after {elapsed_secs}s"),
            },
            SamplingError::Auth { message, .. } => Self {
                kind: "auth",
                status: Some(401),
                error_type: None,
                error_code: None,
                message: message.clone(),
            },
            _ => return None,
        };
        Some(summary)
    }

    fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "status": self.status,
            "type": self.error_type,
            "code": self.error_code,
            "message": self.message,
        })
    }
}

static DAY_COUNTER: LazyLock<Mutex<(String, u32)>> =
    LazyLock::new(|| Mutex::new((String::new(), 0)));

fn reserve_daily_slot(date: &str, cap: u32) -> bool {
    if cap == 0 {
        return true;
    }
    let mut counter = DAY_COUNTER.lock().unwrap_or_else(PoisonError::into_inner);
    if counter.0 != date {
        *counter = (date.to_owned(), 0);
    }
    if counter.1 >= cap {
        return false;
    }
    counter.1 += 1;
    true
}

fn append_index(dir: &Path, line: &str) {
    let path = dir.join("index.ndjson");
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > INDEX_ROTATE_BYTES) {
        let _ = std::fs::rename(&path, dir.join("index.ndjson.1"));
    }
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file
            .write_all(line.as_bytes())
            .and_then(|_| file.write_all(b"\n"));
    }
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    use super::*;

    fn test_dir() -> PathBuf {
        std::env::temp_dir().join(format!("grok-api-dump-{}", uuid::Uuid::new_v4()))
    }

    fn api_error(status: StatusCode, message: &str) -> SamplingError {
        SamplingError::Api {
            status,
            message: message.to_owned(),
            model_metadata: None,
            retry_after_secs: None,
            should_retry: None,
            error_code: None,
        }
    }

    #[test]
    fn records_http_and_stream_failures_and_skips_rate_limits() {
        let dir = test_dir();
        configure_for_test(Settings {
            enabled: true,
            dir: dir.clone(),
            max_body_bytes: 64,
            max_stream_bytes: 512,
            max_files_per_day: 0,
        });
        let capture = RequestCapture {
            method: "POST".into(),
            url: "https://api.example.com/v1/chat/completions?api_key=sk-secret&x=1".into(),
            headers: vec![
                ("authorization".into(), "[REDACTED]".into()),
                ("x-grok-req-id".into(), "req-1".into()),
            ],
            body: br#"{"model":"deepseek/deepseek-v4.1-flash","messages":[]}"#.to_vec(),
            body_truncated: false,
            body_bytes: 57,
        };
        let ctx = CallContext {
            backend: "chat_completions",
            mode: "stream",
            model: "deepseek/deepseek-v4.1-flash".into(),
            session_id: "session-1".into(),
            req_id: "req-1".into(),
        };
        let dump = CallDump::new(capture, ctx);
        let headers = HeaderMap::new();

        dump.note_failure(
            Some(400),
            Some(&headers),
            Some(br#"{"error":{"message":"invalid_request_error: bad"}}"#),
            &api_error(StatusCode::BAD_REQUEST, "invalid_request_error: bad"),
        );
        // 429 must not produce a file.
        dump.note_failure(
            Some(429),
            Some(&headers),
            None,
            &api_error(StatusCode::TOO_MANY_REQUESTS, "rate limited"),
        );

        let recorder = dump.stream_recorder();
        recorder.push_frame("", r#"{"type":"keepalive","sequence_number":1}"#);
        recorder.push_frame("response.output_text.delta", r#"{"delta":"hi"}"#);
        dump.note_stream_failure(
            Some(200),
            Some(&headers),
            &recorder,
            &SamplingError::EventStreamError("error decoding response body".into()),
        );

        let day = Utc::now().format("%Y-%m-%d").to_string();
        let files: Vec<_> = std::fs::read_dir(dir.join(&day))
            .expect("day dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files.len(), 2, "429 must be skipped: {files:?}");

        let index = std::fs::read_to_string(dir.join("index.ndjson")).expect("index");
        let lines: Vec<&str> = index.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["kind"], "http_error");
        assert_eq!(first["status"], 400);
        assert_eq!(first["provider"], "api.example.com");
        let second: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(second["kind"], "transport");
        assert_eq!(second["stream_truncated"], false);

        let dump_file = dir
            .join(&day)
            .join(first["file"].as_str().unwrap().split_once('/').unwrap().1);
        let doc: Value =
            serde_json::from_str(&std::fs::read_to_string(dump_file).unwrap()).unwrap();
        assert_eq!(doc["request"]["headers"]["authorization"], "[REDACTED]");
        assert!(
            doc["request"]["url"]
                .as_str()
                .unwrap()
                .contains("api_key=%5BREDACTED%5D")
                || doc["request"]["url"]
                    .as_str()
                    .unwrap()
                    .contains("api_key=[REDACTED]"),
            "query secret must be redacted: {}",
            doc["request"]["url"]
        );
        assert_eq!(
            doc["response"]["body"]["error"]["message"],
            "invalid_request_error: bad"
        );
        assert_eq!(doc["error"]["kind"], "http_error");

        let stream_file = dir
            .join(&day)
            .join(second["file"].as_str().unwrap().split_once('/').unwrap().1);
        let stream_doc: Value =
            serde_json::from_str(&std::fs::read_to_string(stream_file).unwrap()).unwrap();
        let frames = stream_doc["stream"]["frames"].as_str().unwrap();
        assert!(
            frames.contains("event: response.output_text.delta"),
            "{frames}"
        );
        assert!(!stream_doc["stream"]["truncated"].as_bool().unwrap());

        TEST_SETTINGS.with(|cell| *cell.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stream_recorder_truncates_at_cap() {
        let recorder = StreamRecorder::new(24);
        recorder.push_frame("", "first-frame-payload");
        recorder.push_frame("", "second-frame-payload");
        let (text, frames, truncated) = recorder.snapshot();
        assert_eq!(frames, 2);
        assert!(truncated);
        assert!(text.len() <= 24);
        assert!(text.starts_with("data: first-frame"));
    }

    #[test]
    fn body_cap_keeps_valid_utf8() {
        let text = "错误响应体".repeat(32);
        let (kept, truncated) = cap_bytes(text.as_bytes(), 25);
        assert!(truncated);
        assert!(std::str::from_utf8(&kept).is_ok());
        assert!(kept.len() <= 25);
    }
}
