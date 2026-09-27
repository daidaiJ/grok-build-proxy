//! LOCAL: 周额度反推内核 —— 用「本机 xAI 直连 token 消耗 ÷ 同期 Δpct」反推账号周池
//! 额度的下界。数学与验收标准见 `docs-local/usage-quota-estimate-todo.md`。
//!
//! 核心不变量：**结果永远是下界**。其他设备/网页/App 的消耗同样推动 pct 但不进本机
//! 账本，所以 Q̂ = Δtokens_本机 / Δpct × 100 ≤ 真实额度 Q；多设备场景下各机各算、
//! 取最大值。服务端 pct 实测整点量化，Δpct < 2 点的样本对噪声占比过大，直接弃用。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::billing_samples::BillingSample;
use crate::model_usage_ledger::ModelCallSample;

/// 反推所需的最少当前周期采样数。
pub const MIN_SAMPLES: usize = 3;
/// 样本对参与反推的最小 Δpct（整点量化下的噪声门）。
pub const MIN_PAIR_DELTA_PCT: f64 = 2.0;
/// Δpct 达到该值视为"紧"下界（相对误差小），否则"松"。
pub const TIGHT_DELTA_PCT: f64 = 10.0;

/// 反推结果。`weekly_tokens_lower_bound` 仅为下界（多设备语义：各机取 max）。
#[derive(Debug, Clone, PartialEq)]
pub struct QuotaEstimate {
    pub status: EstimateStatus,
    /// 反推的周额度下界（token 数 / 周期）。
    pub weekly_tokens_lower_bound: Option<f64>,
    /// 最佳样本对窗口（本机 token 计入的起止，毫秒）。
    pub basis_from_ms: Option<u64>,
    pub basis_to_ms: Option<u64>,
    /// 最佳样本对的 Δpct（百分点）。
    pub basis_delta_pct: Option<f64>,
    /// 参与反推的当前周期采样数（供 UI 显示数据新鲜度）。
    pub samples_in_period: usize,
    /// 当前周期的服务端类型（`USAGE_PERIOD_TYPE_WEEKLY` 等，取自最后一条采样）。
    pub period_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstimateStatus {
    /// Δpct ≥ [`TIGHT_DELTA_PCT`]，下界相对误差小。
    Tight,
    /// Δpct 达门槛但偏小，下界偏松。
    Loose,
    /// 无法反推，原因见 [`NoDataReason`]。
    NoData(NoDataReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoDataReason {
    /// 采样不足（当前周期 < [`MIN_SAMPLES`] 条、无两个不同 pct、或无达门槛样本对）。
    InsufficientSamples,
    /// 达门槛窗口内本机无 xAI 直连消耗（多设备场景下本机份额为 0 时必然出现）。
    NoLocalUsage,
}

/// 反推周额度下界。
///
/// `samples` 只用与**最后一条**同 `period_start` 的采样（pct 周期切换清零，跨周期
/// 配对无意义）；`rows` 是本机模型调用账本，只有 `is_xai_direct` 为真的行计入分子
/// （第三方网关流量不消耗 xAI 配额）。
pub fn estimate(
    samples: &[BillingSample],
    rows: &[ModelCallSample],
    is_xai_direct: &dyn Fn(&str) -> bool,
) -> QuotaEstimate {
    let insufficient = |n: usize| no_data(n, NoDataReason::InsufficientSamples);
    let mut sorted_samples: Vec<&BillingSample> = samples.iter().collect();
    sorted_samples.sort_by_key(|s| s.ts_unix_ms);
    let Some(last) = sorted_samples.last() else {
        return insufficient(0);
    };
    let period = last.period_start.clone();
    let kept: Vec<&BillingSample> = sorted_samples
        .iter()
        .copied()
        .filter(|s| s.period_start == period)
        .collect();
    let n = kept.len();
    if n < MIN_SAMPLES {
        return insufficient(n);
    }
    let distinct = kept
        .iter()
        .map(|s| s.pct.to_bits())
        .collect::<HashSet<_>>()
        .len();
    if distinct < 2 {
        return insufficient(n);
    }

    // xAI 直连 token 前缀和（按 ts 排序），窗口求和走二分。
    let mut events: Vec<(u64, u64)> = rows
        .iter()
        .filter(|r| is_xai_direct(&r.model_id))
        .map(|r| (r.ts_unix_ms, r.prompt_tokens.saturating_add(r.completion_tokens)))
        .collect();
    events.sort_by_key(|(ts, _)| *ts);
    let prefix: Vec<u64> = events
        .iter()
        .scan(0u64, |acc, (_, t)| {
            *acc += t;
            Some(*acc)
        })
        .collect();
    let tokens_between = |a: u64, b: u64| -> u64 {
        let lo = events.partition_point(|(ts, _)| *ts < a);
        let hi = events.partition_point(|(ts, _)| *ts <= b);
        if hi <= lo {
            return 0;
        }
        prefix[hi - 1] - if lo == 0 { 0 } else { prefix[lo - 1] }
    };

    // 所有达门槛样本对里取最大 Q̂（最紧下界；单调性保证其合法性）。
    let mut best: Option<(f64, u64, u64, f64)> = None; // (Q̂, from, to, Δpct)
    for (i, a) in kept.iter().enumerate() {
        for b in &kept[i + 1..] {
            let delta = b.pct - a.pct;
            if delta < MIN_PAIR_DELTA_PCT {
                continue;
            }
            let tokens = tokens_between(a.ts_unix_ms, b.ts_unix_ms);
            let q = tokens as f64 / delta * 100.0;
            if best.is_none_or(|(bq, ..)| q > bq) {
                best = Some((q, a.ts_unix_ms, b.ts_unix_ms, delta));
            }
        }
    }
    let Some((q, from, to, delta)) = best else {
        return insufficient(n);
    };
    if q <= 0.0 {
        return no_data(n, NoDataReason::NoLocalUsage);
    }
    let status = if delta >= TIGHT_DELTA_PCT {
        EstimateStatus::Tight
    } else {
        EstimateStatus::Loose
    };
    QuotaEstimate {
        status,
        weekly_tokens_lower_bound: Some(q),
        basis_from_ms: Some(from),
        basis_to_ms: Some(to),
        basis_delta_pct: Some(delta),
        samples_in_period: n,
        period_type: last.period_type.clone(),
    }
}

fn no_data(samples_in_period: usize, reason: NoDataReason) -> QuotaEstimate {
    QuotaEstimate {
        status: EstimateStatus::NoData(reason),
        weekly_tokens_lower_bound: None,
        basis_from_ms: None,
        basis_to_ms: None,
        basis_delta_pct: None,
        samples_in_period,
        period_type: None,
    }
}

/// 从 config.toml 提取 `[model.<id>]` 块的 `base_url`（缺省 key → `None` = 官方默认后端）。
///
/// 有意不走 managed overlay 合并：overlay 不允许改写 `base_url`（见
/// `xai-grok-config/src/config_override.rs` 的 mtls 重定向防护），磁盘文件即权威。
/// 解析不了的头部按"非本模型"处理——漏配方向是安全的（把该模型当第三方 = 低估下界）。
pub fn load_model_base_urls(grok_home: &Path) -> HashMap<String, Option<String>> {
    match std::fs::read_to_string(grok_home.join("config.toml")) {
        Ok(text) => parse_model_base_urls(&text),
        Err(_) => HashMap::new(),
    }
}

fn parse_model_base_urls(text: &str) -> HashMap<String, Option<String>> {
    let mut out: HashMap<String, Option<String>> = HashMap::new();
    let mut current: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            current = parse_model_header(line);
            if let Some(id) = &current {
                out.entry(id.clone()).or_insert(None);
            }
            continue;
        }
        if let (Some(id), Some(rest)) = (&current, line.strip_prefix("base_url")) {
            if let Some(value) = rest.trim_start().strip_prefix('=').map(str::trim) {
                if let Some(url) = unquote(value) {
                    out.insert(id.clone(), Some(url.to_string()));
                }
            }
        }
    }
    out
}

/// 匹配 `[model."id"]` / `[model.id]`；`[models]`、`[model."x".sub]` 等一律不采信。
fn parse_model_header(line: &str) -> Option<String> {
    let inner = line.trim_start_matches('[').trim_end_matches(']').trim();
    let rest = inner.strip_prefix("model")?.strip_prefix('.')?.trim();
    if rest.is_empty() {
        return None;
    }
    if let Some(inner) = rest.strip_prefix('"') {
        let inner = inner.strip_suffix('"')?;
        return (!inner.is_empty() && !inner.contains('"')).then(|| inner.to_string());
    }
    if let Some(inner) = rest.strip_prefix('\'') {
        let inner = inner.strip_suffix('\'')?;
        return (!inner.is_empty() && !inner.contains('\'')).then(|| inner.to_string());
    }
    // 未加引号的裸 id：含 `.` 说明是子表头，拒绝（漏配 = 低估，安全侧）。
    (!rest.contains('.')).then(|| rest.to_string())
}

fn unquote(value: &str) -> Option<&str> {
    if let Some(rest) = value.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some(&rest[..end]);
    }
    if let Some(rest) = value.strip_prefix('\'') {
        let end = rest.find('\'')?;
        return Some(&rest[..end]);
    }
    None
}

/// 官方默认后端判定：无 `base_url`（缺省配置）或 host 为 `api.x.ai` / `*.x.ai`。
pub fn is_xai_direct(base_url: Option<&str>) -> bool {
    let Some(url) = base_url else {
        return true;
    };
    let host = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = host.split('/').next().unwrap_or(host);
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = host.split(':').next().unwrap_or(host);
    let host = host.to_ascii_lowercase();
    host == "api.x.ai" || host.ends_with(".x.ai")
}

/// 由 config 模型表构造账本行的直连判定闭包（账本里查不到的 model id = 默认后端）。
pub fn xai_direct_predicate(
    models: &HashMap<String, Option<String>>,
) -> impl Fn(&str) -> bool + '_ {
    move |model_id: &str| is_xai_direct(models.get(model_id).and_then(|o| o.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pct_sample(ts: u64, pct: f64, period: &str) -> BillingSample {
        BillingSample {
            ts_unix_ms: ts,
            pct,
            period_start: Some(period.to_string()),
            period_end: None,
            period_type: None,
            tier: None,
            history_len: None,
        }
    }

    fn row(ts: u64, model: &str, prompt: u64, completion: u64) -> ModelCallSample {
        ModelCallSample {
            ts_unix_ms: ts,
            model_id: model.to_string(),
            prompt_tokens: prompt,
            completion_tokens: completion,
            cached_prompt_tokens: 0,
            cache_creation_tokens: 0,
            reasoning_tokens: 0,
            ttft_ms: None,
            tps: None,
            duration_ms: 0,
        }
    }

    const P: &str = "2026-09-22T23:50:39+00:00";
    const XAI: &str = "grok-4.6";
    const THIRD: &str = "deepseek/deepseek-v4.1-flash";
    fn all_xai(model_id: &str) -> bool {
        model_id.starts_with("grok-")
    }

    #[test]
    fn lower_bound_holds_under_multi_device_dilution() {
        // 真实额度 Q = 每 1% 对应 10_000 tokens；本机只贡献 100，其余 900 是其他设备烧的。
        let t = 1_000_000_000_000u64;
        let samples = vec![
            pct_sample(t, 0.0, P),
            pct_sample(t + 1_000, 1.0, P),
            pct_sample(t + 2_000, 2.0, P),
        ];
        let rows = vec![row(t + 1_500, XAI, 90, 10)];
        let est = estimate(&samples, &rows, &all_xai);
        let q = est.weekly_tokens_lower_bound.unwrap();
        assert!(
            q <= 10_000.0,
            "下界必须 ≤ 真实额度（其他设备消耗稀释本机口径）: {q}"
        );
        assert_eq!(est.status, EstimateStatus::Loose, "Δpct=2 < 10 → 松");
        assert_eq!(est.basis_delta_pct, Some(2.0));
        assert_eq!(est.samples_in_period, 3);
    }

    #[test]
    fn large_delta_marks_tight_and_uses_best_pair() {
        let t = 1_000_000_000_000u64;
        let samples = vec![
            pct_sample(t, 1.0, P),
            pct_sample(t + 1_000, 2.0, P),
            pct_sample(t + 2_000, 20.0, P),
        ];
        // 可选对：(1→2) Δ=1 弃；(2→20) 400 tokens → Q̂=2_222；(1→20) 900 tokens →
        // Q̂=4_737（最大，胜出）。Δ=19 ≥ 10 → Tight。
        let rows = vec![
            row(t + 500, XAI, 500, 0),
            row(t + 1_500, XAI, 400, 0),
        ];
        let est = estimate(&samples, &rows, &all_xai);
        assert_eq!(est.status, EstimateStatus::Tight);
        assert_eq!(est.basis_delta_pct, Some(19.0));
        assert_eq!(est.weekly_tokens_lower_bound, Some(900.0 / 19.0 * 100.0));
        assert_eq!(est.basis_from_ms, Some(t));
    }

    #[test]
    fn third_party_tokens_never_enter_numerator() {
        let t = 1_000_000_000_000u64;
        let samples = vec![
            pct_sample(t, 0.0, P),
            pct_sample(t + 1_000, 10.0, P),
            pct_sample(t + 2_000, 20.0, P),
        ];
        let rows = vec![row(t + 1_500, THIRD, 500_000_000, 1_000)];
        let est = estimate(&samples, &rows, &all_xai);
        assert_eq!(
            est.status,
            EstimateStatus::NoData(NoDataReason::NoLocalUsage),
            "第三方网关流量不消耗 xAI 配额，纯第三方窗口必须报 NoLocalUsage"
        );
    }

    #[test]
    fn quantization_gate_rejects_single_point_deltas() {
        let t = 1_000_000_000_000u64;
        let samples = vec![
            pct_sample(t, 1.0, P),
            pct_sample(t + 1_000, 2.0, P),
            pct_sample(t + 2_000, 2.0, P),
        ];
        let rows = vec![row(t + 1_500, XAI, 999_999, 0)];
        let est = estimate(&samples, &rows, &all_xai);
        assert_eq!(
            est.status,
            EstimateStatus::NoData(NoDataReason::InsufficientSamples)
        );
    }

    #[test]
    fn cross_period_samples_are_dropped() {
        let t = 1_000_000_000_000u64;
        let old = "2026-09-15T23:50:39+00:00";
        // 旧周期 3 条 + 新周期 2 条：当前周期 = 最后一条所属，样本数 2 < 3 → 不足。
        let samples = vec![
            pct_sample(t, 10.0, old),
            pct_sample(t + 1_000, 20.0, old),
            pct_sample(t + 2_000, 26.0, old),
            pct_sample(t + 3_000, 1.0, P),
            pct_sample(t + 4_000, 5.0, P),
        ];
        let rows = vec![row(t + 3_500, XAI, 50_000, 0)];
        let est = estimate(&samples, &rows, &all_xai);
        assert_eq!(est.samples_in_period, 2);
        assert_eq!(
            est.status,
            EstimateStatus::NoData(NoDataReason::InsufficientSamples)
        );
    }

    #[test]
    fn window_boundaries_are_inclusive_and_outside_rows_ignored() {
        let t = 1_000_000_000_000u64;
        let samples = vec![
            pct_sample(t, 0.0, P),
            pct_sample(t + 1_000, 5.0, P),
            pct_sample(t + 2_000, 10.0, P),
        ];
        // 窗外行用大数值制造区分度：若 t-1 / t+2001 的行被错误计入，最佳 Q̂ 会跳到
        // 64_000 / 102_000，断言立即转红。
        let rows = vec![
            row(t - 1, XAI, 3_000, 0),     // 首采样之前，不计
            row(t, XAI, 100, 0),           // 恰在 (0→5) 起点，计入
            row(t + 1_000, XAI, 100, 0),   // 同时是 (0→5) 终点与 (5→10) 起点，计入
            row(t + 1_001, XAI, 1_000, 0), // (5→10) 窗内（窗延伸到最后采样点），计入
            row(t + 2_001, XAI, 5_000, 0), // 末采样之后，不计
        ];
        let est = estimate(&samples, &rows, &all_xai);
        // (0→5) 200 → 4_000；(5→10) 1_100 → 22_000；(0→10) 1_200 → 12_000；最佳 22_000
        assert_eq!(est.weekly_tokens_lower_bound, Some(22_000.0));
        assert_eq!(est.basis_from_ms, Some(t + 1_000));
        assert_eq!(est.basis_to_ms, Some(t + 2_000));
    }

    #[test]
    fn empty_and_tiny_inputs_report_insufficient() {
        let est = estimate(&[], &[], &all_xai);
        assert_eq!(est.samples_in_period, 0);
        assert_eq!(
            est.status,
            EstimateStatus::NoData(NoDataReason::InsufficientSamples)
        );
        let t = 1_000_000_000_000u64;
        let est = estimate(&[pct_sample(t, 1.0, P)], &[], &all_xai);
        assert_eq!(
            est.status,
            EstimateStatus::NoData(NoDataReason::InsufficientSamples)
        );
    }

    #[test]
    fn config_parse_extracts_quoted_and_bare_ids_and_rejects_lookalikes() {
        let text = r#"
[ui]
fork_secondary_model = "grok-4.6"

[models]
default = "glm-5.3-flash"

[model."deepseek/deepseek-v4.1-flash"]
base_url = "https://api.commandcode.ai/provider/v1" # gateway
name = "DeepSeek"

[model.ark-flash]
base_url = 'https://ark.cn-beijing.volces.com/api/coding/v3'

[model.local-default]
name = "no base_url"

[model."x".sub]
base_url = "https://evil.example"

[models]
base_url = "https://ignored.example"
"#;
        let m = parse_model_base_urls(text);
        assert_eq!(
            m.get("deepseek/deepseek-v4.1-flash").unwrap().as_deref(),
            Some("https://api.commandcode.ai/provider/v1")
        );
        assert_eq!(
            m.get("ark-flash").unwrap().as_deref(),
            Some("https://ark.cn-beijing.volces.com/api/coding/v3")
        );
        assert_eq!(m.get("local-default"), Some(&None), "缺 base_url = 默认后端");
        assert!(!m.contains_key("x"), "子表头不采信");
        assert!(!m.contains_key("models"), "[models] 不是 [model.<id>]");
        assert!(!is_xai_direct(Some("https://api.commandcode.ai/provider/v1")));
        assert!(is_xai_direct(None));
        assert!(is_xai_direct(Some("https://api.x.ai/v1")));
        assert!(is_xai_direct(Some("https://API.X.AI:443/v1")));
        assert!(is_xai_direct(Some("https://user@console.x.ai/v1")));
        assert!(!is_xai_direct(Some("https://evil.example/x.ai")));
        let pred = xai_direct_predicate(&m);
        assert!(pred("grok-4.6"), "不在 config 的 model id = 默认后端");
        assert!(!pred("deepseek/deepseek-v4.1-flash"));
        assert!(pred("local-default"));
    }
}
