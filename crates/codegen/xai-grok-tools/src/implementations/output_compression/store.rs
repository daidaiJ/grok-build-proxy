//! File-backed CCR store. Hash is blake3 → 24 hex chars, matching headroom.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use super::runtime::{current_runtime, record_io_read, record_io_write};

pub(crate) fn compute_key(payload: &str) -> String {
    let hex = blake3::hash(payload.as_bytes()).to_hex();
    hex.as_str()[..24].to_string()
}

pub(crate) fn marker_for(hash: &str) -> String {
    format!("<<ccr:{hash}>>")
}

pub(crate) fn put(payload: &str) -> Option<String> {
    let rt = current_runtime();
    if !rt.ccr_enabled {
        return None;
    }
    let hash = compute_key(payload);
    let path = entry_path(&rt.ccr_dir, &hash);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let start = Instant::now();
    let result = std::fs::write(&path, payload.as_bytes());
    let elapsed = start.elapsed();
    record_io_write(elapsed, payload.len() as u64);
    match result {
        Ok(()) => Some(hash),
        Err(err) => {
            tracing::warn!(path = %path.display(), %err, "tool-output CCR put failed");
            None
        }
    }
}

pub(crate) fn get(hash: &str) -> Option<String> {
    let hash = hash.trim();
    if hash.is_empty() {
        return None;
    }
    let rt = current_runtime();
    let path = entry_path(&rt.ccr_dir, hash);
    let ttl = Duration::from_secs(rt.ccr_ttl_secs);
    let start = Instant::now();
    let meta = std::fs::metadata(&path).ok()?;
    if is_expired(&meta, ttl) {
        let _ = std::fs::remove_file(&path);
        record_io_read(start.elapsed(), 0);
        return None;
    }
    let data = std::fs::read_to_string(&path).ok()?;
    record_io_read(start.elapsed(), data.len() as u64);
    Some(data)
}

fn entry_path(dir: &Path, hash: &str) -> PathBuf {
    // Keep the filename inside [a-f0-9] so a model-supplied hash cannot escape the dir.
    let safe: String = hash
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(24)
        .collect();
    dir.join(safe)
}

fn is_expired(meta: &std::fs::Metadata, ttl: Duration) -> bool {
    let Ok(modified) = meta.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age > ttl)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::output_compression::runtime::{
        CompressionStrategiesSpec, ToolOutputCompressionRuntime, reset_for_tests,
    };

    fn with_store(test: impl Fn()) {
        let _guard = crate::implementations::output_compression::runtime::test_lock();
        let dir = tempfile::tempdir().unwrap();
        reset_for_tests(ToolOutputCompressionRuntime {
            enabled: true,
            scope: vec!["bash".into()],
            strategies: CompressionStrategiesSpec::Auto,
            min_input_tokens: 1,
            keep_tail_lines: 4,
            protect: vec!["exit_code".into(), "stderr".into()],
            ccr_enabled: true,
            ccr_ttl_secs: 3600,
            ccr_dir: dir.path().join("ccr"),
            stats_path: dir.path().join("stats.json"),
        });
        test();
    }

    #[test]
    fn put_get_roundtrip() {
        with_store(|| {
            let key = put("hello original").expect("put");
            assert_eq!(key.len(), 24);
            assert_eq!(get(&key).as_deref(), Some("hello original"));
        });
    }

    #[test]
    fn marker_format_is_pinned() {
        assert_eq!(marker_for("abc123"), "<<ccr:abc123>>");
    }

    #[test]
    fn rejects_path_escape_in_hash() {
        with_store(|| {
            assert!(get("../etc/passwd").is_none());
            assert!(get("zzzz").is_none());
        });
    }
}
