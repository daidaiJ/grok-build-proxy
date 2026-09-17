//! Process-wide runtime + persisted stats ledger.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::util::grok_home;

static ENABLED: AtomicBool = AtomicBool::new(false);
static CCR_ENABLED: AtomicBool = AtomicBool::new(false);
static RUNTIME: OnceLock<Mutex<ToolOutputCompressionRuntime>> = OnceLock::new();
static STATS: OnceLock<Mutex<ToolOutputCompressionStats>> = OnceLock::new();
static STATS_PATH: OnceLock<Mutex<PathBuf>> = OnceLock::new();

fn stats_path_lock() -> &'static Mutex<PathBuf> {
    STATS_PATH.get_or_init(|| Mutex::new(PathBuf::new()))
}

fn runtime_lock() -> &'static Mutex<ToolOutputCompressionRuntime> {
    RUNTIME.get_or_init(|| Mutex::new(ToolOutputCompressionRuntime::disabled()))
}

fn stats_lock() -> &'static Mutex<ToolOutputCompressionStats> {
    STATS.get_or_init(|| Mutex::new(ToolOutputCompressionStats::default()))
}

/// User-facing `[tool_output_compression]` table. Missing keys stay at the
/// documented defaults; `enabled` defaults to false.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolOutputCompressionConfig {
    pub enabled: bool,
    pub scope: Vec<String>,
    pub strategies: CompressionStrategiesSpec,
    pub min_input_tokens: usize,
    pub keep_tail_lines: usize,
    pub protect: Vec<String>,
    pub ccr: CcrConfig,
}

impl Default for ToolOutputCompressionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scope: default_scope(),
            strategies: CompressionStrategiesSpec::Auto,
            min_input_tokens: 500,
            keep_tail_lines: 20,
            protect: default_protect(),
            ccr: CcrConfig::default(),
        }
    }
}

fn default_scope() -> Vec<String> {
    vec!["bash".into(), "mcp".into()]
}

fn default_protect() -> Vec<String> {
    vec!["exit_code".into(), "stderr".into()]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CcrConfig {
    pub enabled: bool,
    pub ttl_secs: u64,
}

impl Default for CcrConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_secs: 3600,
        }
    }
}

/// `strategies = "auto"` or `strategies = ["json", "logs", "search", "diff"]`.
#[derive(Debug, Clone, PartialEq)]
pub enum CompressionStrategiesSpec {
    Auto,
    List(Vec<String>),
}

impl Default for CompressionStrategiesSpec {
    fn default() -> Self {
        Self::Auto
    }
}

impl Serialize for CompressionStrategiesSpec {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::List(list) => list.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for CompressionStrategiesSpec {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StrategiesVisitor;
        impl<'de> serde::de::Visitor<'de> for StrategiesVisitor {
            type Value = CompressionStrategiesSpec;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("\"auto\" or an array of strategy names")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                if v.eq_ignore_ascii_case("auto") {
                    Ok(CompressionStrategiesSpec::Auto)
                } else {
                    Ok(CompressionStrategiesSpec::List(vec![v.to_owned()]))
                }
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut list = Vec::new();
                while let Some(item) = seq.next_element::<String>()? {
                    list.push(item);
                }
                Ok(CompressionStrategiesSpec::List(list))
            }
        }
        deserializer.deserialize_any(StrategiesVisitor)
    }
}

#[derive(Debug, Clone)]
pub struct ToolOutputCompressionRuntime {
    pub enabled: bool,
    pub scope: Vec<String>,
    pub strategies: CompressionStrategiesSpec,
    pub min_input_tokens: usize,
    pub keep_tail_lines: usize,
    pub protect: Vec<String>,
    pub ccr_enabled: bool,
    pub ccr_ttl_secs: u64,
    pub ccr_dir: PathBuf,
    pub stats_path: PathBuf,
}

impl ToolOutputCompressionRuntime {
    pub fn disabled() -> Self {
        Self::from_config(&ToolOutputCompressionConfig::default())
    }

    pub fn from_config(cfg: &ToolOutputCompressionConfig) -> Self {
        let home = grok_home();
        let cache = home.join("cache");
        Self {
            enabled: cfg.enabled,
            scope: if cfg.scope.is_empty() {
                default_scope()
            } else {
                cfg.scope.clone()
            },
            strategies: cfg.strategies.clone(),
            min_input_tokens: cfg.min_input_tokens.max(1),
            keep_tail_lines: cfg.keep_tail_lines.max(1),
            protect: if cfg.protect.is_empty() {
                default_protect()
            } else {
                cfg.protect.clone()
            },
            ccr_enabled: cfg.enabled && cfg.ccr.enabled,
            ccr_ttl_secs: if cfg.ccr.ttl_secs == 0 {
                3600
            } else {
                cfg.ccr.ttl_secs
            },
            ccr_dir: cache.join("tool-output-ccr"),
            stats_path: cache.join("tool-output-compression-stats.json"),
        }
    }

    pub fn strategy_allowed(&self, name: &str) -> bool {
        match &self.strategies {
            CompressionStrategiesSpec::Auto => true,
            CompressionStrategiesSpec::List(list) if list.is_empty() => true,
            CompressionStrategiesSpec::List(list) => {
                list.iter().any(|s| s.eq_ignore_ascii_case(name))
            }
        }
    }
}

/// Compression config pinned at toolset finalize; not updated mid-session.
#[derive(Debug, Clone)]
pub struct SessionCompressionPolicy(pub ToolOutputCompressionRuntime);

crate::register_resource!("local", "ToolOutputCompression", SessionCompressionPolicy);

/// Cumulative compression ledger. `saved_tokens` is the positive side;
/// `expanded_tokens` + `retrieved_tokens` are the negative side.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolOutputCompressionStats {
    pub compressed_calls: u64,
    pub skipped_calls: u64,
    pub no_win_calls: u64,
    pub retrieve_calls: u64,
    pub original_tokens: u64,
    pub saved_tokens: u64,
    pub expanded_tokens: u64,
    pub retrieved_tokens: u64,
    pub io_write_ops: u64,
    pub io_read_ops: u64,
    pub io_write_ns: u64,
    pub io_read_ns: u64,
    pub io_write_bytes: u64,
    pub io_read_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at_unix: Option<u64>,
}

impl ToolOutputCompressionStats {
    pub fn net_tokens(&self) -> i64 {
        self.saved_tokens as i64 - self.expanded_tokens as i64 - self.retrieved_tokens as i64
    }

    pub fn saved_ratio(&self) -> f64 {
        if self.original_tokens == 0 {
            0.0
        } else {
            self.saved_tokens as f64 / self.original_tokens as f64
        }
    }

    pub fn net_ratio(&self) -> f64 {
        if self.original_tokens == 0 {
            0.0
        } else {
            self.net_tokens() as f64 / self.original_tokens as f64
        }
    }

    pub fn io_write_avg_ms(&self) -> f64 {
        avg_ms(self.io_write_ns, self.io_write_ops)
    }

    pub fn io_read_avg_ms(&self) -> f64 {
        avg_ms(self.io_read_ns, self.io_read_ops)
    }

    pub fn io_total_ms(&self) -> f64 {
        (self.io_write_ns.saturating_add(self.io_read_ns)) as f64 / 1_000_000.0
    }

    #[allow(dead_code)]
    fn fold(&mut self, other: &Self) {
        self.compressed_calls = self.compressed_calls.saturating_add(other.compressed_calls);
        self.skipped_calls = self.skipped_calls.saturating_add(other.skipped_calls);
        self.no_win_calls = self.no_win_calls.saturating_add(other.no_win_calls);
        self.retrieve_calls = self.retrieve_calls.saturating_add(other.retrieve_calls);
        self.original_tokens = self.original_tokens.saturating_add(other.original_tokens);
        self.saved_tokens = self.saved_tokens.saturating_add(other.saved_tokens);
        self.expanded_tokens = self.expanded_tokens.saturating_add(other.expanded_tokens);
        self.retrieved_tokens = self.retrieved_tokens.saturating_add(other.retrieved_tokens);
        self.io_write_ops = self.io_write_ops.saturating_add(other.io_write_ops);
        self.io_read_ops = self.io_read_ops.saturating_add(other.io_read_ops);
        self.io_write_ns = self.io_write_ns.saturating_add(other.io_write_ns);
        self.io_read_ns = self.io_read_ns.saturating_add(other.io_read_ns);
        self.io_write_bytes = self.io_write_bytes.saturating_add(other.io_write_bytes);
        self.io_read_bytes = self.io_read_bytes.saturating_add(other.io_read_bytes);
    }

    fn is_empty(&self) -> bool {
        self.compressed_calls == 0
            && self.skipped_calls == 0
            && self.retrieve_calls == 0
            && self.original_tokens == 0
    }
}

fn avg_ms(ns: u64, ops: u64) -> f64 {
    if ops == 0 {
        0.0
    } else {
        (ns as f64 / ops as f64) / 1_000_000.0
    }
}

pub fn set_runtime(cfg: &ToolOutputCompressionConfig) {
    let rt = ToolOutputCompressionRuntime::from_config(cfg);
    ENABLED.store(rt.enabled, Ordering::Relaxed);
    CCR_ENABLED.store(rt.ccr_enabled, Ordering::Relaxed);
    if let Ok(mut guard) = runtime_lock().lock() {
        *guard = rt.clone();
    }
    if let Ok(mut path) = stats_path_lock().lock() {
        *path = rt.stats_path.clone();
    }
    if rt.enabled {
        load_persisted_into_memory(&rt.stats_path);
    }
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn ccr_is_enabled() -> bool {
    CCR_ENABLED.load(Ordering::Relaxed)
}

pub fn current_runtime_for_session() -> ToolOutputCompressionRuntime {
    current_runtime()
}

pub(crate) fn current_runtime() -> ToolOutputCompressionRuntime {
    runtime_lock()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| ToolOutputCompressionRuntime::disabled())
}

pub fn snapshot_stats() -> ToolOutputCompressionStats {
    stats_lock().lock().map(|g| g.clone()).unwrap_or_default()
}

/// Read the on-disk ledger under `grok_home` without requiring `set_runtime`.
/// Used by `grok stats` / `/stats` in processes that have not loaded agent config.
pub fn load_persisted_stats(grok_home: &Path) -> Option<ToolOutputCompressionStats> {
    let path = grok_home
        .join("cache")
        .join("tool-output-compression-stats.json");
    read_stats_file(&path)
}

/// Live process totals merged with the persisted file (persisted wins on empty live).
pub fn stats_for_display(grok_home: Option<&Path>) -> ToolOutputCompressionStats {
    let live = snapshot_stats();
    if !live.is_empty() {
        return live;
    }
    grok_home.and_then(load_persisted_stats).unwrap_or(live)
}

pub(crate) fn record_skip() {
    if let Ok(mut g) = stats_lock().lock() {
        g.skipped_calls = g.skipped_calls.saturating_add(1);
    }
}

pub(crate) fn record_no_win(original_tokens: u64, expanded_tokens: u64) {
    if let Ok(mut g) = stats_lock().lock() {
        g.no_win_calls = g.no_win_calls.saturating_add(1);
        g.original_tokens = g.original_tokens.saturating_add(original_tokens);
        g.expanded_tokens = g.expanded_tokens.saturating_add(expanded_tokens);
        persist_locked(&mut g);
    }
}

pub(crate) fn record_win(original_tokens: u64, saved_tokens: u64) {
    if let Ok(mut g) = stats_lock().lock() {
        g.compressed_calls = g.compressed_calls.saturating_add(1);
        g.original_tokens = g.original_tokens.saturating_add(original_tokens);
        g.saved_tokens = g.saved_tokens.saturating_add(saved_tokens);
        persist_locked(&mut g);
    }
}

pub(crate) fn record_retrieve(tokens: u64) {
    if let Ok(mut g) = stats_lock().lock() {
        g.retrieve_calls = g.retrieve_calls.saturating_add(1);
        g.retrieved_tokens = g.retrieved_tokens.saturating_add(tokens);
        persist_locked(&mut g);
    }
}

pub(crate) fn record_io_write(duration: Duration, bytes: u64) {
    if let Ok(mut g) = stats_lock().lock() {
        g.io_write_ops = g.io_write_ops.saturating_add(1);
        g.io_write_ns = g.io_write_ns.saturating_add(duration.as_nanos() as u64);
        g.io_write_bytes = g.io_write_bytes.saturating_add(bytes);
        persist_locked(&mut g);
    }
}

pub(crate) fn record_io_read(duration: Duration, bytes: u64) {
    if let Ok(mut g) = stats_lock().lock() {
        g.io_read_ops = g.io_read_ops.saturating_add(1);
        g.io_read_ns = g.io_read_ns.saturating_add(duration.as_nanos() as u64);
        g.io_read_bytes = g.io_read_bytes.saturating_add(bytes);
        persist_locked(&mut g);
    }
}

fn persist_locked(stats: &mut ToolOutputCompressionStats) {
    stats.updated_at_unix = Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    );
    let path = stats_path_lock().lock().ok().map(|p| p.clone());
    let Some(path) = path.filter(|p| !p.as_os_str().is_empty()) else {
        return;
    };
    write_stats_file(&path, stats);
}

fn load_persisted_into_memory(path: &Path) {
    let Some(file) = read_stats_file(path) else {
        return;
    };
    if let Ok(mut g) = stats_lock().lock() {
        if g.is_empty() {
            *g = file;
        }
    }
}

fn read_stats_file(path: &Path) -> Option<ToolOutputCompressionStats> {
    let data = std::fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

fn write_stats_file(path: &Path, stats: &ToolOutputCompressionStats) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(json) = serde_json::to_vec_pretty(stats) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Human report used by `/stats` and `grok stats`.
pub fn format_stats_report(stats: &ToolOutputCompressionStats, enabled: bool) -> String {
    let mut lines = vec!["Tool-output compression (experimental)".to_string()];
    if !enabled {
        lines.push("  enabled: no  (set [tool_output_compression] enabled = true)".into());
        if stats.is_empty() {
            return lines.join("\n");
        }
        lines.push("  (showing persisted totals from when it was last on)".into());
    } else {
        lines.push("  enabled: yes".into());
    }
    if stats.is_empty() {
        lines.push("  no compressed outputs yet".into());
        return lines.join("\n");
    }

    let net = stats.net_tokens();
    let net_sign = if net > 0 { "+" } else { "" };
    lines.push(format!(
        "  calls: {} compressed, {} no-win, {} skipped, {} retrieves",
        stats.compressed_calls, stats.no_win_calls, stats.skipped_calls, stats.retrieve_calls
    ));
    lines.push(format!("  tokens original: {}", stats.original_tokens));
    lines.push(format!(
        "  positive: saved {} tokens ({:.1}% of original)",
        stats.saved_tokens,
        stats.saved_ratio() * 100.0
    ));
    lines.push(format!(
        "  negative: expanded {} tokens; retrieves brought back {} tokens",
        stats.expanded_tokens, stats.retrieved_tokens
    ));
    lines.push(format!(
        "  net: {net_sign}{net} tokens ({:.1}% of original)",
        stats.net_ratio() * 100.0
    ));
    lines.push(format!(
        "  extra I/O: {} writes ({:.2} ms total, avg {:.2} ms, {} bytes), {} reads ({:.2} ms total, avg {:.2} ms, {} bytes)",
        stats.io_write_ops,
        ns_to_ms(stats.io_write_ns),
        stats.io_write_avg_ms(),
        stats.io_write_bytes,
        stats.io_read_ops,
        ns_to_ms(stats.io_read_ns),
        stats.io_read_avg_ms(),
        stats.io_read_bytes
    ));
    lines.push(format!("  extra I/O total: {:.2} ms", stats.io_total_ms()));
    lines.join("\n")
}

fn ns_to_ms(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
pub(crate) fn reset_for_tests(rt: ToolOutputCompressionRuntime) {
    ENABLED.store(rt.enabled, Ordering::Relaxed);
    CCR_ENABLED.store(rt.ccr_enabled, Ordering::Relaxed);
    if let Ok(mut g) = runtime_lock().lock() {
        *g = rt.clone();
    }
    if let Ok(mut path) = stats_path_lock().lock() {
        *path = rt.stats_path;
    }
    if let Ok(mut g) = stats_lock().lock() {
        *g = ToolOutputCompressionStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        let cfg = ToolOutputCompressionConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.scope, ["bash", "mcp"]);
        assert_eq!(cfg.min_input_tokens, 500);
    }

    #[test]
    fn net_tokens_subtracts_negative_and_retrieves() {
        let stats = ToolOutputCompressionStats {
            saved_tokens: 100,
            expanded_tokens: 10,
            retrieved_tokens: 20,
            original_tokens: 200,
            ..Default::default()
        };
        assert_eq!(stats.net_tokens(), 70);
        assert!((stats.net_ratio() - 0.35).abs() < 1e-9);
    }

    #[test]
    fn format_report_mentions_positive_negative_and_io() {
        let stats = ToolOutputCompressionStats {
            compressed_calls: 3,
            skipped_calls: 1,
            no_win_calls: 1,
            retrieve_calls: 1,
            original_tokens: 1000,
            saved_tokens: 400,
            expanded_tokens: 50,
            retrieved_tokens: 100,
            io_write_ops: 3,
            io_read_ops: 1,
            io_write_ns: 3_000_000,
            io_read_ns: 1_000_000,
            io_write_bytes: 900,
            io_read_bytes: 200,
            updated_at_unix: Some(1),
        };
        let text = format_stats_report(&stats, true);
        assert!(text.contains("positive: saved 400"));
        assert!(text.contains("negative: expanded 50"));
        assert!(text.contains("retrieves brought back 100"));
        assert!(text.contains("extra I/O"));
        assert!(text.contains("net: +250"));
    }
}
