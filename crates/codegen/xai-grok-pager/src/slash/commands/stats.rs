//! `/stats` — 按模型聚合的用量/性能报表（LOCAL）+ 工具输出压缩账本。
//!
//! 数据源：`grok_home/cache/model-usage.jsonl`（shell 每次成功推理追加一条采样，
//! 见 [`xai_grok_tools::model_usage_ledger`]）。按 5h / 一周 / 一月三个时间窗
//! 分 model id 聚合：累计 token（输入/输出/缓存读/缓存写）、平均缓存命中率、
//! TTFT 与 TPS 的 p50/p90。

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};
use crate::slash::i18n::tr;
use xai_grok_tools::implementations::output_compression::{
    format_stats_report, is_enabled, stats_for_display,
};
use xai_grok_tools::model_usage_ledger::{
    self, ModelCallSample, ModelUsageAggregate, WINDOW_5H_MS, WINDOW_MONTH_MS, WINDOW_WEEK_MS,
};

pub struct StatsCommand;

impl SlashCommand for StatsCommand {
    slash_meta! {
        name: "stats",
        description: "Show model usage stats and compression savings",
        usage: "/stats",
        takes_args: false,
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        let home = xai_grok_config::grok_home();
        let samples = model_usage_ledger::load_samples(Some(&home));
        let now = ModelCallSample::now_unix_ms();
        let mut report = format_usage_report(&samples, now);
        report.push('\n');
        let stats = stats_for_display(Some(&home));
        report.push_str(&format_stats_report(&stats, is_enabled()));
        CommandResult::Message(report)
    }
}

/// 渲染按时间窗 × model 聚合的用量报表（纯函数，便于测试）。
pub(crate) fn format_usage_report(samples: &[ModelCallSample], now_unix_ms: u64) -> String {
    let mut lines = vec![tr("Model usage (aggregated by model)").to_string()];
    for (window_ms, label) in [
        (WINDOW_5H_MS, tr("last 5h")),
        (WINDOW_WEEK_MS, tr("last week")),
        (WINDOW_MONTH_MS, tr("last month")),
    ] {
        lines.push(format!("{label}:"));
        let rows = model_usage_ledger::aggregate(samples, window_ms, now_unix_ms);
        if rows.is_empty() {
            lines.push(format!("  {}", tr("no calls in this window")));
            continue;
        }
        for row in &rows {
            lines.push(format_usage_row(row));
        }
    }
    lines.join("\n")
}

/// 单个 model 在单个窗口内的两行摘要。
fn format_usage_row(row: &ModelUsageAggregate) -> String {
    let mut token_parts = vec![
        format!("{} {}", tr("input"), fmt_tokens(row.prompt_tokens)),
        format!("{} {}", tr("output"), fmt_tokens(row.completion_tokens)),
        format!(
            "{} {}",
            tr("cache read"),
            fmt_tokens(row.cached_read_tokens)
        ),
        format!(
            "{} {}",
            tr("cache write"),
            fmt_tokens(row.cache_creation_tokens)
        ),
    ];
    if row.reasoning_tokens > 0 {
        token_parts.push(format!("{} {}", tr("reasoning"), fmt_tokens(row.reasoning_tokens)));
    }
    let perf_parts = [
        match (row.ttft_p50_ms, row.ttft_p90_ms) {
            (Some(p50), Some(p90)) => format!("ttft p50/p90 {p50}/{p90}ms"),
            _ => format!("ttft p50/p90 {}", tr("n/a")),
        },
        match (row.tps_p50, row.tps_p90) {
            (Some(p50), Some(p90)) => format!("tps p50/p90 {p50:.1}/{p90:.1}"),
            _ => format!("tps p50/p90 {}", tr("n/a")),
        },
    ];
    format!(
        "  {} · {} {}\n    {} · {} {} · {}",
        row.model_id,
        row.calls,
        tr("calls"),
        token_parts.join(" · "),
        tr("cache hit"),
        format!("{:.1}%", row.cache_hit_rate * 100.0),
        perf_parts.join(" · "),
    )
}

/// token 数人性化：`823` / `45.2k` / `3.05m`。
fn fmt_tokens(n: u64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    static DEFAULT_BUNDLE_STATE: BundleState = BundleState {
        has_cache: false,
        version: String::new(),
        personas: Vec::new(),
        roles: Vec::new(),
        agents: Vec::new(),
        skills: Vec::new(),
        persona_details: Vec::new(),
        role_details: Vec::new(),
    };

    fn exec_ctx(models: &ModelState) -> CommandExecCtx<'_> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &DEFAULT_BUNDLE_STATE,
            screen_mode: crate::app::ScreenMode::Minimal,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        }
    }

    #[test]
    fn reports_disabled_state_by_default() {
        let models = ModelState::default();
        let mut ctx = exec_ctx(&models);
        match StatsCommand.run(&mut ctx, "") {
            CommandResult::Message(text) => {
                assert!(text.contains("Tool-output compression"));
                assert!(
                    text.contains("enabled: no") || text.contains("enabled: yes"),
                    "{text}"
                );
            }
            other => panic!("expected message, got {other:?}"),
        }
    }

    fn sample(ts: u64, model: &str, prompt: u64, cached: u64, ttft: u64, tps: f64) -> ModelCallSample {
        ModelCallSample {
            ts_unix_ms: ts,
            model_id: model.to_string(),
            prompt_tokens: prompt,
            completion_tokens: 500,
            cached_prompt_tokens: cached,
            cache_creation_tokens: 200,
            reasoning_tokens: 10,
            ttft_ms: Some(ttft),
            tps: Some(tps),
            duration_ms: 2000,
        }
    }

    #[test]
    fn usage_report_aggregates_windows_and_models() {
        let now = 1_000_000_000_000;
        let samples = vec![
            sample(now - 1000, "m-a", 1000, 800, 900, 50.0),
            sample(now - 2000, "m-a", 500, 500, 1100, 60.0),
            sample(now - WINDOW_WEEK_MS - 1000, "m-b", 777, 0, 100, 99.0),
        ];
        let report = format_usage_report(&samples, now);
        // 三个窗口标题齐全
        assert!(report.contains("Model usage (aggregated by model)"));
        assert!(report.contains("last 5h"));
        assert!(report.contains("last week"));
        assert!(report.contains("last month"));
        // 5h 窗只有 m-a 两笔
        assert!(report.contains("m-a · 2 calls"));
        assert!(report.contains("input 1.5k"));
        assert!(report.contains("output 1.0k"));
        assert!(report.contains("cache read 1.3k"));
        assert!(report.contains("cache write 400"));
        assert!(report.contains("cache hit 86.7%"));
        assert!(report.contains("ttft p50/p90 900/1100ms"));
        assert!(report.contains("tps p50/p90 50.0/60.0"));
        // 月窗包含 m-b（旧样本落在月窗内）；周窗不含
        assert!(report.contains("m-b · 1 call"), "月窗应含旧样本 {report}");
        // 周窗段落只出现在 5h 与 month 之间，且 m-b 不在其中
        let week_start = report.find("last week:").unwrap();
        let month_start = report.find("last month:").unwrap();
        assert!(!report[week_start..month_start].contains("m-b"));
        // 各窗无样本时给出占位
        let empty = format_usage_report(&[], now);
        assert_eq!(empty.matches("no calls in this window").count(), 3);
    }

    #[test]
    fn fmt_tokens_boundaries() {
        assert_eq!(fmt_tokens(823), "823");
        assert_eq!(fmt_tokens(45_200), "45.2k");
        assert_eq!(fmt_tokens(999_999), "1000k");
        assert_eq!(fmt_tokens(3_050_000), "3.05m");
    }
}
