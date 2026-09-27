//! LOCAL: billing 百分比采样账本，周额度反推（[`crate::quota_estimate`]）的 pct 数据源。
//!
//! shell 每次成功拉取计费配置（每 turn 结束 / 打开 `/usage` / 轮询）追加一条 JSONL；
//! pager 反推与他机对账脚本读取。文件在 `grok_home/cache/billing-samples.jsonl`，
//! 超体积上限按保留窗口剪裁（同 [`crate::model_usage_ledger`] 的方案）。
//! `GROK_BILLING_SAMPLES=0` 关闭落盘。
//!
//! 采样去重：与文件内最后一条相比，pct 相同且间隔 < 60s 的重复拉取（同一 burst 的
//! 连续 turn 结束）跳过；pct 变化必然落盘，间隔拉长后的同 pct 也落盘（作为时间锚点，
//! 供反推选取跨度更大的样本对）。

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// 超过该体积触发按保留窗口重写。
const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// 采样保留窗口：只在体积超限触发剪裁时生效。
const RETAIN_MS: u64 = 35 * 24 * 60 * 60 * 1000;
/// 同 pct 去重窗口。
const DEDUP_WINDOW_MS: u64 = 60_000;

/// 进程内串行化 append + 剪裁（与账本同款）。
static SAMPLES_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 一次计费配置拉取的采样（用量百分比 + 周期锚点）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BillingSample {
    /// Unix 毫秒时间戳。
    pub ts_unix_ms: u64,
    /// 服务端下发的周/月用量百分比（实测整点量化）。
    pub pct: f64,
    /// 当前周期起点（RFC 3339）；反推按它切周期，跨周期样本不配对。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_start: Option<String>,
    /// 当前周期终点（RFC 3339）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_end: Option<String>,
    /// 周期类型（proto 枚举名，如 `USAGE_PERIOD_TYPE_WEEKLY`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_type: Option<String>,
    /// 套餐名（如 SuperGrok）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// 服务端返回的往期汇总条数（金额口径可用性信号，见 usage-quota-estimate-todo）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_len: Option<u32>,
}

impl BillingSample {
    pub fn now_unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

fn samples_path(grok_home: &Path) -> PathBuf {
    grok_home.join("cache").join("billing-samples.jsonl")
}

/// 追加一条采样；任何 IO 失败都静默忽略（采样账本不允许影响计费拉取主流程）。
pub fn append_sample(grok_home: &Path, sample: &BillingSample) {
    if std::env::var_os("GROK_BILLING_SAMPLES").is_some_and(|v| v == "0") {
        return;
    }
    let path = samples_path(grok_home);
    let _guard = SAMPLES_LOCK.lock();
    let existing = read_all(&path);
    if let Some(last) = existing.last() {
        if last.pct == sample.pct
            && sample.ts_unix_ms.saturating_sub(last.ts_unix_ms) < DEDUP_WINDOW_MS
        {
            return;
        }
    }
    let append = |p: &Path| -> std::io::Result<()> {
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
        writeln!(file, "{}", serde_json::to_string(sample).unwrap_or_default())
    };
    if append(&path).is_err() {
        return;
    }
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) <= MAX_FILE_BYTES {
        return;
    }
    let cutoff = sample.ts_unix_ms.saturating_sub(RETAIN_MS);
    let kept: Vec<String> = existing
        .iter()
        .chain(std::iter::once(sample))
        .filter(|s| s.ts_unix_ms >= cutoff)
        .filter_map(|s| serde_json::to_string(s).ok())
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

/// 读取全部采样（坏行跳过）。`grok_home` 为 `None` 或文件缺失返回空。
pub fn load_samples(grok_home: Option<&Path>) -> Vec<BillingSample> {
    let Some(home) = grok_home else {
        return Vec::new();
    };
    read_all(&samples_path(home))
}

fn read_all(path: &Path) -> Vec<BillingSample> {
    let Ok(content) = std::fs::read_to_string(path) else {
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

    fn sample(ts: u64, pct: f64) -> BillingSample {
        BillingSample {
            ts_unix_ms: ts,
            pct,
            period_start: Some("2026-09-22T23:50:39+00:00".into()),
            period_end: None,
            period_type: None,
            tier: Some("SuperGrok".into()),
            history_len: None,
        }
    }

    #[test]
    fn dedup_collapses_same_pct_burst_but_keeps_changes_and_later_anchors() {
        let dir = std::env::temp_dir().join(format!("billing-samples-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let t0 = BillingSample::now_unix_ms();
        append_sample(&dir, &sample(t0, 78.0));
        append_sample(&dir, &sample(t0 + 30_000, 78.0));
        assert_eq!(load_samples(Some(&dir)).len(), 1, "同 pct 且 30s 内去重");
        append_sample(&dir, &sample(t0 + 31_000, 79.0));
        assert_eq!(load_samples(Some(&dir)).len(), 2, "pct 变化必然落盘");
        append_sample(&dir, &sample(t0 + 31_000 + 61_000, 79.0));
        assert_eq!(load_samples(Some(&dir)).len(), 3, "超 60s 的同 pct 作为时间锚点保留");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn roundtrip_preserves_fields_and_prune_drops_stale() {
        let dir =
            std::env::temp_dir().join(format!("billing-samples-roundtrip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let now = BillingSample::now_unix_ms();
        let mut old = sample(now - RETAIN_MS - 1, 1.0);
        old.period_end = Some("2026-09-29T23:50:39+00:00".into());
        old.period_type = Some("USAGE_PERIOD_TYPE_WEEKLY".into());
        old.history_len = Some(0);
        append_sample(&dir, &old);
        append_sample(&dir, &sample(now, 2.0));
        let loaded = load_samples(Some(&dir));
        assert_eq!(loaded, vec![old, sample(now, 2.0)], "未超限不剪裁，字段保真");
        // 触发剪裁路径
        let path = samples_path(&dir);
        std::fs::write(&path, format!("{}\n", "x".repeat((MAX_FILE_BYTES + 1) as usize))).unwrap();
        append_sample(&dir, &sample(now + 1, 3.0));
        let kept = load_samples(Some(&dir));
        assert_eq!(kept.len(), 1, "垃圾行丢弃，旧样本剪掉");
        assert_eq!(kept[0].pct, 3.0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
