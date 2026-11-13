//! LOCAL(token-economy): prompt-cache break detection.
//!
//! Every main-turn sample feeds [`observe`], which compares it with the previous
//! request of the same session. A high hit rate that suddenly collapses is either an
//! explainable prefix change (model switch, history rewrite) or an unexplained cliff;
//! either way it is recorded so `/stats` and `unified.jsonl` can correlate it with a
//! compaction, a tool-set change, or a provider-side cache eviction.
//!
//! Records live in `grok_home/cache/cache-breaks.jsonl` and never affect a request.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

/// Previous request must have been well cached for a drop to be meaningful.
pub const MIN_PREV_HIT_RATE: f64 = 0.9;
/// Absolute cached-token collapse that counts even when the prompt grew.
pub const MIN_CACHED_DROP_TOKENS: u64 = 20_000;
/// Hit-rate collapse that counts even below the absolute threshold.
pub const MIN_HIT_RATE_DROP: f64 = 0.30;

const MAX_FILE_BYTES: u64 = 1 << 20;
const RETAIN_MS: u64 = 30 * 24 * 60 * 60 * 1000;
const MAX_TRACKED_SESSIONS: usize = 512;

/// The cache-relevant numbers of one request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheTurn {
    pub prompt_tokens: u64,
    pub cached_tokens: u64,
}

impl CacheTurn {
    pub fn hit_rate(self) -> f64 {
        if self.prompt_tokens == 0 {
            0.0
        } else {
            self.cached_tokens as f64 / self.prompt_tokens as f64
        }
    }
}

/// What the comparison says about the cliff. Only [`Self::Unknown`] is a real surprise;
/// the other two name a prefix change the caller can confirm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheBreakReason {
    ModelSwitch,
    HistoryRewrite,
    Unknown,
}

impl CacheBreakReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ModelSwitch => "model_switch",
            Self::HistoryRewrite => "history_rewrite",
            Self::Unknown => "unknown",
        }
    }
}

/// One recorded cliff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheBreakRecord {
    pub ts_unix_ms: u64,
    pub session_id: String,
    pub model_id: String,
    pub prompt_tokens: u64,
    pub cached_before: u64,
    pub cached_after: u64,
    pub hit_rate_before: f64,
    pub hit_rate_after: f64,
    pub reason: CacheBreakReason,
}

impl CacheBreakRecord {
    /// Tokens this request paid uncached; the first-order cost of the cliff.
    pub fn uncached_tokens(&self) -> u64 {
        self.prompt_tokens.saturating_sub(self.cached_after)
    }
}

/// Pure comparison: `None` when the two requests are not a cliff.
/// `prev_model` is the model of the previous request, so a switch is classified before
/// the prompt-size heuristic.
pub fn evaluate(
    prev: CacheTurn,
    curr: CacheTurn,
    prev_model: &str,
    model: &str,
) -> Option<CacheBreakReason> {
    if prev.prompt_tokens == 0 || prev.hit_rate() < MIN_PREV_HIT_RATE {
        return None;
    }
    let cached_drop = prev.cached_tokens.saturating_sub(curr.cached_tokens);
    let rate_drop = prev.hit_rate() - curr.hit_rate();
    if cached_drop < MIN_CACHED_DROP_TOKENS && rate_drop < MIN_HIT_RATE_DROP {
        return None;
    }
    if prev_model != model {
        return Some(CacheBreakReason::ModelSwitch);
    }
    if curr.prompt_tokens < prev.prompt_tokens {
        return Some(CacheBreakReason::HistoryRewrite);
    }
    Some(CacheBreakReason::Unknown)
}

struct LastTurn {
    turn: CacheTurn,
    model: String,
}

static LAST_TURN: LazyLock<Mutex<HashMap<String, LastTurn>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static FILE_LOCK: Mutex<()> = Mutex::new(());

/// Feed one main-turn request. Returns a record (already appended to disk) when the
/// request broke the session's cache. Process-local state; a restart starts cold, which
/// is correct because the first request of a resumed session is also cold.
pub fn observe(
    session_id: &str,
    model_id: &str,
    prompt_tokens: u64,
    cached_tokens: u64,
    now_unix_ms: u64,
) -> Option<CacheBreakRecord> {
    let curr = CacheTurn {
        prompt_tokens,
        cached_tokens,
    };
    let prev = {
        let mut map = LAST_TURN.lock().ok()?;
        if map.len() > MAX_TRACKED_SESSIONS {
            map.clear();
        }
        map.insert(
            session_id.to_string(),
            LastTurn {
                turn: curr,
                model: model_id.to_string(),
            },
        )
        .map(|last| (last.turn, last.model))
    };
    let (prev_turn, prev_model) = prev?;
    let reason = evaluate(prev_turn, curr, &prev_model, model_id)?;
    let record = CacheBreakRecord {
        ts_unix_ms: now_unix_ms,
        session_id: session_id.to_string(),
        model_id: model_id.to_string(),
        prompt_tokens,
        cached_before: prev_turn.cached_tokens,
        cached_after: cached_tokens,
        hit_rate_before: prev_turn.hit_rate(),
        hit_rate_after: curr.hit_rate(),
        reason,
    };
    append(&xai_grok_config::grok_home(), &record);
    Some(record)
}

fn cache_break_path(grok_home: &Path) -> PathBuf {
    grok_home.join("cache").join("cache-breaks.jsonl")
}

/// Append one record; IO failures are silent (observability must not affect a turn).
pub fn append(grok_home: &Path, record: &CacheBreakRecord) {
    let path = cache_break_path(grok_home);
    let _guard = FILE_LOCK.lock();
    let write = |p: &Path| -> std::io::Result<()> {
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
        writeln!(file, "{}", serde_json::to_string(record).unwrap_or_default())
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
        .filter_map(|line| serde_json::from_str::<CacheBreakRecord>(line).ok())
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

/// Read recorded cliffs (bad lines skipped). Missing file or home returns empty.
pub fn load(grok_home: Option<&Path>) -> Vec<CacheBreakRecord> {
    let Some(home) = grok_home else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(cache_break_path(home)) else {
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

    fn turn(prompt: u64, cached: u64) -> CacheTurn {
        CacheTurn {
            prompt_tokens: prompt,
            cached_tokens: cached,
        }
    }

    #[test]
    fn no_break_on_small_drop() {
        let prev = turn(100_000, 98_000);
        let curr = turn(100_000, 90_000);
        assert_eq!(evaluate(prev, curr, "m", "m"), None);
    }

    #[test]
    fn absolute_cached_drop_triggers() {
        let prev = turn(100_000, 98_000);
        let curr = turn(120_000, 70_000);
        assert_eq!(
            evaluate(prev, curr, "m", "m"),
            Some(CacheBreakReason::Unknown)
        );
    }

    #[test]
    fn hit_rate_collapse_triggers_on_a_smaller_prompt() {
        let prev = turn(100_000, 95_000);
        let curr = turn(50_000, 40_000);
        assert_eq!(
            evaluate(prev, curr, "m", "m"),
            Some(CacheBreakReason::HistoryRewrite)
        );
    }

    #[test]
    fn low_previous_hit_rate_is_not_a_break() {
        let prev = turn(100_000, 50_000);
        let curr = turn(100_000, 0);
        assert_eq!(evaluate(prev, curr, "m", "m"), None);
    }

    #[test]
    fn model_switch_is_classified_before_history_rewrite() {
        let prev = turn(100_000, 95_000);
        let curr = turn(90_000, 0);
        assert_eq!(
            evaluate(prev, curr, "old-model", "new-model"),
            Some(CacheBreakReason::ModelSwitch)
        );
    }

    #[test]
    fn first_request_of_a_session_never_triggers() {
        let first = turn(0, 0);
        assert_eq!(evaluate(first, turn(50_000, 0), "m", "m"), None);
    }

    #[test]
    fn record_serializes_camel_case_and_counts_uncached_tokens() {
        let record = CacheBreakRecord {
            ts_unix_ms: 1,
            session_id: "s".into(),
            model_id: "m".into(),
            prompt_tokens: 120_000,
            cached_before: 98_000,
            cached_after: 10_000,
            hit_rate_before: 0.98,
            hit_rate_after: 0.08,
            reason: CacheBreakReason::Unknown,
        };
        assert_eq!(record.uncached_tokens(), 110_000);
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"sessionId\":\"s\""), "{json}");
        assert!(json.contains("\"reason\":\"unknown\""), "{json}");
    }
}
