//! LOCAL: client-side loop-detect ledger for `/stats`.
//!
//! Every nudge or stop threshold writes one JSONL row under
//! `grok_home/cache/loop-detect.jsonl`. Aborting the turn is a separate switch
//! ([`set_abort_override`] / `[features] loop_detect_abort`); recording never
//! depends on that switch.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const MAX_FILE_BYTES: u64 = 1 << 20;
const RETAIN_MS: u64 = 30 * 24 * 60 * 60 * 1000;

const OVERRIDE_UNSET: u8 = 0;
const OVERRIDE_ABORT: u8 = 1;
const OVERRIDE_RECORD: u8 = 2;

static ABORT_OVERRIDE: AtomicU8 = AtomicU8::new(OVERRIDE_UNSET);
static FILE_LOCK: Mutex<()> = Mutex::new(());

/// Nudge (reminder threshold) or hard-stop threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopDetectPhase {
    Nudge,
    Stop,
}

impl LoopDetectPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Nudge => "nudge",
            Self::Stop => "stop",
        }
    }
}

/// One recorded loop-detect crossing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoopDetectRecord {
    pub ts_unix_ms: u64,
    pub session_id: String,
    pub model_id: String,
    /// `identical_tool_calls` / `alternating_tool_calls` / `content_streak` / `thinking_streak`.
    pub kind: String,
    pub phase: LoopDetectPhase,
    pub run_len: u32,
    /// Whether this crossing actually aborted the turn.
    pub aborted: bool,
}

/// Process-level override: `Some(true)` abort, `Some(false)` record-only, `None` follow config/env.
pub fn abort_override() -> Option<bool> {
    match ABORT_OVERRIDE.load(Ordering::Relaxed) {
        OVERRIDE_ABORT => Some(true),
        OVERRIDE_RECORD => Some(false),
        _ => None,
    }
}

/// `/loop-detect abort|record`. Cleared by [`clear_abort_override`].
pub fn set_abort_override(abort: bool) {
    ABORT_OVERRIDE.store(
        if abort {
            OVERRIDE_ABORT
        } else {
            OVERRIDE_RECORD
        },
        Ordering::Relaxed,
    );
}

/// `/loop-detect default` — fall back to config/env.
pub fn clear_abort_override() {
    ABORT_OVERRIDE.store(OVERRIDE_UNSET, Ordering::Relaxed);
}

fn ledger_path(grok_home: &Path) -> PathBuf {
    grok_home.join("cache").join("loop-detect.jsonl")
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Append one crossing. IO failures are silent.
pub fn record(
    grok_home: &Path,
    session_id: &str,
    model_id: &str,
    kind: &str,
    phase: LoopDetectPhase,
    run_len: u32,
    aborted: bool,
) {
    let record = LoopDetectRecord {
        ts_unix_ms: now_unix_ms(),
        session_id: session_id.to_string(),
        model_id: model_id.to_string(),
        kind: kind.to_string(),
        phase,
        run_len,
        aborted,
    };
    append(grok_home, &record);
}

fn append(grok_home: &Path, record: &LoopDetectRecord) {
    let path = ledger_path(grok_home);
    let _guard = FILE_LOCK.lock();
    let write = |p: &Path| -> std::io::Result<()> {
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)?;
        writeln!(
            file,
            "{}",
            serde_json::to_string(record).unwrap_or_default()
        )
    };
    if write(&path).is_err() {
        return;
    }
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) <= MAX_FILE_BYTES {
        return;
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    let cutoff = record.ts_unix_ms.saturating_sub(RETAIN_MS);
    let kept: Vec<String> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<LoopDetectRecord>(line).ok())
        .filter(|r| r.ts_unix_ms >= cutoff)
        .filter_map(|r| serde_json::to_string(&r).ok())
        .collect();
    let mut tmp = path.clone();
    tmp.set_extension("jsonl.tmp");
    if let Ok(mut out) = std::fs::File::create(&tmp) {
        let ok = kept
            .iter()
            .try_for_each(|line| writeln!(out, "{line}"))
            .is_ok();
        drop(out);
        if ok {
            let _ = std::fs::rename(&tmp, &path);
        } else {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

/// Read recorded crossings (bad lines skipped).
pub fn load(grok_home: Option<&Path>) -> Vec<LoopDetectRecord> {
    let Some(home) = grok_home else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(ledger_path(home)) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_round_trip() {
        clear_abort_override();
        assert_eq!(abort_override(), None);
        set_abort_override(false);
        assert_eq!(abort_override(), Some(false));
        set_abort_override(true);
        assert_eq!(abort_override(), Some(true));
        clear_abort_override();
        assert_eq!(abort_override(), None);
    }

    #[test]
    fn append_and_load_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        record(
            dir.path(),
            "sid",
            "model-a",
            "content_streak",
            LoopDetectPhase::Stop,
            5,
            false,
        );
        let rows = load(Some(dir.path()));
        let row = rows.first().expect("one row");
        assert_eq!(rows.len(), 1);
        assert_eq!(row.kind, "content_streak");
        assert_eq!(row.phase, LoopDetectPhase::Stop);
        assert!(!row.aborted);
        assert_eq!(row.run_len, 5);
    }
}
