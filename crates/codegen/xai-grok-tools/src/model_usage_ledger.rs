//! LOCAL: 按调用粒度的模型用量/性能账本，`/stats` 的 5h/天/周聚合数据源。
//!
//! shell 每次成功推理追加一条 JSONL（时间戳、model_id、输入/输出/缓存 token、
//! ttft、tps）；`/stats` 读取后按时间窗 × model 聚合出累计 token、平均缓存命中率
//! 和 ttft/tps 的 p50/p90。文件放在 `grok_home/cache/model-usage.jsonl`，超过体积
//! 上限时整体重写并丢弃超出保留窗口的旧样本。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// 聚合窗口：5 小时 / 一天 / 一周（毫秒）。
pub const WINDOW_5H_MS: u64 = 5 * 60 * 60 * 1000;
pub const WINDOW_DAY_MS: u64 = 24 * 60 * 60 * 1000;
pub const WINDOW_WEEK_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// 样本保留窗口：独立于报表窗口，只在文件体积超限触发剪裁时生效，避免账本无界增长。
const RETAIN_MS: u64 = 35 * 24 * 60 * 60 * 1000;

/// `/stats` 报表时间窗，同时充当模态窗口的标签页身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Window {
    FiveHours,
    Day,
    Week,
}

impl Window {
    /// 全部窗口，按显示顺序（也是标签页顺序）。
    pub const ALL: [Window; 3] = [Window::FiveHours, Window::Day, Window::Week];

    /// 窗口长度（毫秒）。
    pub fn len_ms(self) -> u64 {
        match self {
            Window::FiveHours => WINDOW_5H_MS,
            Window::Day => WINDOW_DAY_MS,
            Window::Week => WINDOW_WEEK_MS,
        }
    }

    /// 命令行参数名（`/stats <arg>`）。同时是渲染层的 i18n 键，故为 `'static`。
    pub fn arg(self) -> &'static str {
        match self {
            Window::FiveHours => "5h",
            Window::Day => "day",
            Window::Week => "week",
        }
    }

    /// 解析 `/stats` 参数；大小写不敏感，首尾空白忽略。
    pub fn from_arg(arg: &str) -> Option<Self> {
        let arg = arg.trim();
        Self::ALL.into_iter().find(|w| w.arg().eq_ignore_ascii_case(arg))
    }

    /// 窗口标题（i18n 表以英文原文为键）。
    pub fn label(self) -> &'static str {
        match self {
            Window::FiveHours => "Last 5h",
            Window::Day => "Last day",
            Window::Week => "Last week",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|w| *w == self).unwrap_or(0)
    }

    /// 越界索引回退到首个窗口（标签页点击与数字键都经这里）。
    pub fn from_index(i: usize) -> Self {
        *Self::ALL.get(i).unwrap_or(&Self::ALL[0])
    }
}

/// 超过该体积触发按保留窗口重写。
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// 进程内串行化 append + 剪裁，避免并发重写交错。
static LEDGER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 一次成功模型调用的用量/性能采样。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCallSample {
    /// Unix 毫秒时间戳。
    pub ts_unix_ms: u64,
    pub model_id: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_prompt_tokens: u64,
    pub cache_creation_tokens: u64,
    pub reasoning_tokens: u64,
    /// Time-to-first-token（流式首 token 延迟）；非流式或未测量时缺省。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttft_ms: Option<u64>,
    /// 解码吞吐（completion tokens / decode 秒），一位小数。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tps: Option<f64>,
    /// 整次调用耗时（含 TTFT），备用指标。
    #[serde(default)]
    pub duration_ms: u64,
}

impl ModelCallSample {
    pub fn now_unix_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

fn ledger_path(grok_home: &Path) -> PathBuf {
    grok_home.join("cache").join("model-usage.jsonl")
}

/// 追加一条样本到 `grok_home/cache/model-usage.jsonl`；体积超限时按保留窗口剪裁。
/// 任何 IO 失败都静默忽略（统计账本不允许影响推理主流程）。
pub fn append_sample(grok_home: &Path, sample: &ModelCallSample) {
    let path = ledger_path(grok_home);
    let _guard = LEDGER_LOCK.lock();
    let append = |s: &ModelCallSample, p: &Path| -> std::io::Result<()> {
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(p)?;
        writeln!(file, "{}", serde_json::to_string(s).unwrap_or_default())
    };
    if append(sample, &path).is_err() {
        return;
    }
    // 体积超限才整读重写（剪掉超出保留窗口的样本）。
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) <= MAX_FILE_BYTES {
        return;
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    let cutoff = sample.ts_unix_ms.saturating_sub(RETAIN_MS);
    let kept: Vec<String> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<ModelCallSample>(line).ok())
        .filter(|s| s.ts_unix_ms >= cutoff)
        .filter_map(|s| serde_json::to_string(&s).ok())
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

/// 读取账本全部样本（坏行跳过）。`grok_home` 为 `None` 或文件缺失返回空。
pub fn load_samples(grok_home: Option<&Path>) -> Vec<ModelCallSample> {
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

/// 单个 model 在一个窗口内的聚合结果。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModelUsageAggregate {
    pub model_id: String,
    pub calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub reasoning_tokens: u64,
    /// 平均缓存命中率 = Σcached_read / Σprompt（prompt 为 0 时为 0）。
    pub cache_hit_rate: f64,
    pub ttft_p50_ms: Option<u64>,
    pub ttft_p90_ms: Option<u64>,
    pub tps_p50: Option<f64>,
    pub tps_p90: Option<f64>,
}

/// token 数人性化：`823` / `45.2k` / `3.05m`。
pub fn fmt_tokens(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        let k = n as f64 / 1_000.0;
        if k < 100.0 {
            format!("{k:.1}k")
        } else {
            format!("{:.0}k", k)
        }
    } else {
        format!("{:.2}m", n as f64 / 1_000_000.0)
    }
}

/// 单个 ttft 显示串（毫秒，不带单位，与 [`fmt_ms_pair`] 同口径）：`900` / `n/a`。
/// 卡片把 p50/p90 拆成两个独立指标格，每格一个数值，故不再拼成 `900/1100`。
pub fn fmt_ms(v: Option<u64>, na: &str) -> String {
    match v {
        Some(ms) => ms.to_string(),
        None => na.to_string(),
    }
}

/// 单个吞吐（tokens/s）显示串：`50.0` / `n/a`。
pub fn fmt_tps(v: Option<f64>, na: &str) -> String {
    match v {
        Some(tps) => format!("{tps:.1}"),
        None => na.to_string(),
    }
}

/// 平均缓存命中率显示串：命中率衡量"有没有可复用的前缀"，单次调用必然冷启动、
/// 恒为 0%，直接显示 `0.0%` 会被读成"该 model 不支持缓存"，故单次调用返回 `na`。
pub fn fmt_hit_rate(calls: u64, rate: f64, na: &str) -> String {
    if calls > 1 {
        format!("{:.1}%", rate * 100.0)
    } else {
        na.to_string()
    }
}

/// 性能指标显示串：`900/1100` / `n/a`。文本报表一行内收纳 p50/p90 用，
/// 与 [`fmt_ms`] 同口径（数值部分完全一致）。
pub fn fmt_ms_pair(p50: Option<u64>, p90: Option<u64>, na: &str) -> String {
    match (p50, p90) {
        (Some(p50), Some(p90)) => format!("{p50}/{p90}"),
        _ => na.to_string(),
    }
}

/// 吞吐（tokens/s）显示串，与 [`fmt_ms_pair`] 同形。
pub fn fmt_tps_pair(p50: Option<f64>, p90: Option<f64>, na: &str) -> String {
    match (p50, p90) {
        (Some(p50), Some(p90)) => format!("{p50:.1}/{p90:.1}"),
        _ => na.to_string(),
    }
}

/// 把样本按 `window_ms` 时间窗过滤后按 model 聚合，按总 token 降序排列。
pub fn aggregate(
    samples: &[ModelCallSample],
    window_ms: u64,
    now_unix_ms: u64,
) -> Vec<ModelUsageAggregate> {
    let cutoff = now_unix_ms.saturating_sub(window_ms);
    let mut by_model: indexmap::IndexMap<String, Vec<&ModelCallSample>> = indexmap::IndexMap::new();
    for sample in samples {
        if sample.ts_unix_ms >= cutoff {
            by_model
                .entry(sample.model_id.clone())
                .or_default()
                .push(sample);
        }
    }
    let mut rows: Vec<ModelUsageAggregate> = by_model
        .into_iter()
        .map(|(model_id, group)| {
            let sum = |f: fn(&ModelCallSample) -> u64| group.iter().map(|s| f(s)).sum();
            let prompt = sum(|s| s.prompt_tokens);
            let cached = sum(|s| s.cached_prompt_tokens);
            let mut ttft: Vec<u64> =
                group.iter().filter_map(|s| s.ttft_ms).collect();
            ttft.sort_unstable();
            let mut tps: Vec<f64> = group.iter().filter_map(|s| s.tps).collect();
            tps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            ModelUsageAggregate {
                model_id,
                calls: group.len() as u64,
                prompt_tokens: prompt,
                completion_tokens: sum(|s| s.completion_tokens),
                cached_read_tokens: cached,
                cache_creation_tokens: sum(|s| s.cache_creation_tokens),
                reasoning_tokens: sum(|s| s.reasoning_tokens),
                cache_hit_rate: (cached as f64) / (prompt.max(1) as f64),
                ttft_p50_ms: percentile(&ttft, 0.50),
                ttft_p90_ms: percentile(&ttft, 0.90),
                tps_p50: percentile(&tps, 0.50),
                tps_p90: percentile(&tps, 0.90),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        let ta = a.prompt_tokens + a.completion_tokens;
        let tb = b.prompt_tokens + b.completion_tokens;
        tb.cmp(&ta).then_with(|| a.model_id.cmp(&b.model_id))
    });
    rows
}

/// 最近邻秩百分位：`sorted` 须已升序；空集返回 `None`。
fn percentile<T: Copy>(sorted: &[T], q: f64) -> Option<T> {
    if sorted.is_empty() {
        return None;
    }
    let rank = ((q * sorted.len() as f64).ceil() as usize)
        .clamp(1, sorted.len());
    Some(sorted[rank - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(ts_unix_ms: u64, model: &str, prompt: u64, cached: u64, ttft: u64, tps: f64) -> ModelCallSample {
        ModelCallSample {
            ts_unix_ms,
            model_id: model.to_string(),
            prompt_tokens: prompt,
            completion_tokens: 100,
            cached_prompt_tokens: cached,
            cache_creation_tokens: 0,
            reasoning_tokens: 0,
            ttft_ms: Some(ttft),
            tps: Some(tps),
            duration_ms: 1000,
        }
    }

    #[test]
    fn aggregate_filters_window_groups_by_model_and_sorts_by_tokens() {
        let now = 1_000_000_000_000;
        let samples = vec![
            sample(now - 1000, "m-a", 1000, 800, 900, 50.0),
            sample(now - WINDOW_WEEK_MS - 1000, "m-a", 999_999, 0, 5000, 10.0), // 周窗外
            sample(now - 2000, "m-b", 300, 0, 200, 90.0),
            sample(now - 3000, "m-a", 500, 500, 1100, 60.0),
        ];
        let rows = aggregate(&samples, WINDOW_WEEK_MS, now);
        assert_eq!(rows.len(), 2);
        // m-a 总量更大排前
        assert_eq!(rows[0].model_id, "m-a");
        assert_eq!(rows[0].calls, 2);
        assert_eq!(rows[0].prompt_tokens, 1500);
        assert_eq!(rows[0].completion_tokens, 200);
        assert_eq!(rows[0].cached_read_tokens, 1300);
        assert!((rows[0].cache_hit_rate - 1300.0 / 1500.0).abs() < 1e-9);
        // ttft 样本 [900, 1100] → p50=900, p90=1100；tps [50, 60] → p50=50, p90=60
        assert_eq!(rows[0].ttft_p50_ms, Some(900));
        assert_eq!(rows[0].ttft_p90_ms, Some(1100));
        assert_eq!(rows[0].tps_p50, Some(50.0));
        assert_eq!(rows[0].tps_p90, Some(60.0));
        assert_eq!(rows[1].model_id, "m-b");
        assert_eq!(rows[1].calls, 1);
        // 5h 窗更短，结果一致（样本都在 5h 内）
        let rows_5h = aggregate(&samples, WINDOW_5H_MS, now);
        assert_eq!(rows_5h.len(), 2);
    }

    #[test]
    fn percentile_nearest_rank() {
        let v = vec![10u64, 20, 30, 40];
        assert_eq!(percentile(&v, 0.5), Some(20));
        assert_eq!(percentile(&v, 0.9), Some(40));
        assert_eq!(percentile(&v, 0.0), Some(10));
        let one = vec![7u64];
        assert_eq!(percentile(&one, 0.9), Some(7));
        assert_eq!(percentile::<u64>(&[], 0.5), None);
    }

    #[test]
    fn fmt_tokens_boundaries() {
        assert_eq!(fmt_tokens(823), "823");
        assert_eq!(fmt_tokens(45_200), "45.2k");
        assert_eq!(fmt_tokens(999_999), "1000k");
        assert_eq!(fmt_tokens(3_050_000), "3.05m");
    }

    #[test]
    fn fmt_pairs_fall_back_to_na_when_either_percentile_is_missing() {
        assert_eq!(fmt_ms_pair(Some(900), Some(1100), "n/a"), "900/1100");
        assert_eq!(fmt_ms_pair(Some(900), None, "n/a"), "n/a");
        assert_eq!(fmt_ms_pair(None, Some(1100), "n/a"), "n/a");
        assert_eq!(fmt_tps_pair(Some(50.0), Some(60.0), "n/a"), "50.0/60.0");
        assert_eq!(fmt_tps_pair(None, Some(60.0), "n/a"), "n/a");
    }

    #[test]
    fn fmt_single_values_match_the_pair_form() {
        assert_eq!(fmt_ms(Some(4536), "n/a"), "4536");
        assert_eq!(fmt_ms(None, "n/a"), "n/a");
        assert_eq!(fmt_tps(Some(230.4), "n/a"), "230.4");
        assert_eq!(fmt_tps(Some(99.0), "n/a"), "99.0");
        assert_eq!(fmt_tps(None, "n/a"), "n/a");
    }

    #[test]
    fn hit_rate_needs_more_than_one_call_to_mean_anything() {
        assert_eq!(fmt_hit_rate(459, 0.986_396_761_835_524_3, "n/a"), "98.6%");
        assert_eq!(fmt_hit_rate(2, 0.0, "n/a"), "0.0%");
        // 单次调用是冷启动，0% 不代表"不支持缓存"
        assert_eq!(fmt_hit_rate(1, 0.0, "n/a"), "n/a");
    }

    #[test]
    fn window_arg_parsing_is_case_insensitive_and_rejects_unknown() {
        assert_eq!(Window::from_arg("5h"), Some(Window::FiveHours));
        assert_eq!(Window::from_arg("day"), Some(Window::Day));
        assert_eq!(Window::from_arg("week"), Some(Window::Week));
        assert_eq!(Window::from_arg("  DAY  "), Some(Window::Day));
        assert_eq!(Window::from_arg("month"), None);
        assert_eq!(Window::from_arg(""), None);
        assert_eq!(Window::ALL.map(Window::arg), ["5h", "day", "week"]);
    }

    #[test]
    fn window_index_roundtrips_and_clamps() {
        for (i, w) in Window::ALL.into_iter().enumerate() {
            assert_eq!(w.index(), i);
            assert_eq!(Window::from_index(i), w);
        }
        assert_eq!(Window::from_index(Window::ALL.len()), Window::FiveHours);
    }

    #[test]
    fn aggregate_empty_window_yields_no_rows() {
        let now = 1_000_000_000_000;
        let samples = vec![sample(now - WINDOW_WEEK_MS - 1, "m-a", 1, 0, 1, 1.0)];
        assert!(aggregate(&samples, WINDOW_WEEK_MS, now).is_empty());
    }

    #[test]
    fn aggregate_day_window_excludes_samples_at_or_before_the_cutoff() {
        let now = 1_000_000_000_000;
        let samples = vec![
            sample(now - WINDOW_DAY_MS + 1, "m-in", 1, 0, 1, 1.0),
            sample(now - WINDOW_DAY_MS - 1, "m-out", 1, 0, 1, 1.0),
        ];
        let rows = aggregate(&samples, WINDOW_DAY_MS, now);
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].model_id, "m-in");
    }

    #[test]
    fn append_and_load_roundtrip_with_pruning() {
        let dir = std::env::temp_dir().join(format!(
            "model-usage-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let now = ModelCallSample::now_unix_ms();
        // 一条远超保留窗口的旧样本 + 一条新样本
        append_sample(&dir, &sample(now - RETAIN_MS - 1, "m-old", 1, 0, 1, 1.0));
        append_sample(&dir, &sample(now, "m-new", 2, 1, 2, 2.0));
        assert_eq!(load_samples(Some(&dir)).len(), 2, "未超体积上限不剪裁");
        // 通过直接构造超限文件触发剪裁路径（垃圾行必须带换行，模拟真实损坏行）
        let path = ledger_path(&dir);
        std::fs::write(&path, format!("{}\n", "x".repeat((MAX_FILE_BYTES + 1) as usize))).unwrap();
        append_sample(&dir, &sample(now + 1, "m-new", 3, 1, 3, 3.0));
        let samples = load_samples(Some(&dir));
        assert_eq!(samples.len(), 1, "垃圾行被丢弃，剪裁后只剩新样本");
        assert!(samples.iter().all(|s| s.model_id == "m-new"));
        assert!(std::fs::metadata(&path).unwrap().len() <= MAX_FILE_BYTES);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
