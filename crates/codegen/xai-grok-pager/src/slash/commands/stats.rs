//! LOCAL: `/stats` — 按模型聚合的用量/性能报表 + 工具输出压缩账本。
//!
//! 数据源：`grok_home/cache/model-usage.jsonl`（shell 每次成功推理追加一条采样，
//! 见 [`xai_grok_tools::model_usage_ledger`]）。全屏模式下由
//! [`crate::views::stats_modal`] 以窗口呈现（时间窗标签页 + 压缩账本标签页），
//! 极简模式保持回滚缓冲里的纯文本输出。
//!
//! 参数：`/stats [5h|day|week]` 选择打开时定位的时间窗；不带参数打开第一个窗。
//! 文本路径渲染全部窗口（与改造前的「全窗口」输出一致）。

use crate::app::actions::Action;
use crate::slash::command::{
    AppCtx, ArgItem, CommandExecCtx, CommandResult, SlashCommand, slash_meta,
};
use crate::slash::i18n::tr;
use xai_grok_tools::implementations::output_compression::{
    format_stats_report, is_enabled, stats_for_display,
};
use xai_grok_tools::model_usage_ledger::{
    self, ModelCallSample, ModelUsageAggregate, Window, fmt_hit_rate, fmt_ms_pair, fmt_tokens,
    fmt_tps_pair,
};

pub struct StatsCommand;

impl SlashCommand for StatsCommand {
    slash_meta! {
        name: "stats",
        description: "Show model usage stats and compression savings",
        usage: "/stats [5h|day|week]",
        takes_args: true,
    }

    fn suggest_args(&self, _ctx: &AppCtx, _args_query: &str) -> Option<Vec<ArgItem>> {
        Some(window_arg_items())
    }

    fn run(&self, ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let arg = args.trim();
        let tab = match arg {
            "" => None,
            _ => match Window::from_arg(arg) {
                Some(window) => Some(window),
                None => {
                    return CommandResult::Error(format!(
                        "Unknown argument: {arg}. Use /stats [5h|day|week]"
                    ));
                }
            },
        };
        // 极简模式没有浮动窗口，保持回滚缓冲里的文本报表
        if ctx.screen_mode.is_minimal() {
            return CommandResult::Message(format_stats_text());
        }
        CommandResult::Action(Action::ShowStats { window: tab })
    }
}

/// `/stats` 参数补全项（描述走 i18n 表）。
pub(crate) fn window_arg_items() -> Vec<ArgItem> {
    Window::ALL
        .into_iter()
        .map(|w| {
            let description = match w {
                Window::FiveHours => tr("Last 5 hours"),
                Window::Day => tr("Last 24 hours"),
                Window::Week => tr("Last 7 days"),
            };
            ArgItem {
                display: w.arg().into(),
                match_text: w.arg().into(),
                insert_text: w.arg().into(),
                description: description.into(),
            }
        })
        .collect()
}

/// 极简模式的文本报表：全部时间窗 + 压缩账本（纯函数，便于测试）。
pub(crate) fn format_stats_text() -> String {
    let home = xai_grok_config::grok_home();
    let samples = model_usage_ledger::load_samples(Some(&home));
    let now = ModelCallSample::now_unix_ms();
    let mut report = format_usage_report(&samples, now, &Window::ALL);
    report.push('\n');
    let stats = stats_for_display(Some(&home));
    report.push_str(&format_stats_report(&stats, is_enabled()));
    report
}

/// 渲染按时间窗 × model 聚合的用量报表（纯函数，便于测试）。
pub(crate) fn format_usage_report(
    samples: &[ModelCallSample],
    now_unix_ms: u64,
    windows: &[Window],
) -> String {
    let mut lines = vec![tr("Model usage (aggregated by model)").to_string()];
    for &window in windows {
        lines.push(format!("{}:", tr(window.label())));
        let rows = model_usage_ledger::aggregate(samples, window.len_ms(), now_unix_ms);
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
        token_parts.push(format!(
            "{} {}",
            tr("reasoning"),
            fmt_tokens(row.reasoning_tokens)
        ));
    }
    let perf_parts = [
        format!(
            "ttft p50/p90 {}",
            fmt_ms_pair(row.ttft_p50_ms, row.ttft_p90_ms, tr("n/a"))
        ),
        format!(
            "tps p50/p90 {}",
            fmt_tps_pair(row.tps_p50, row.tps_p90, tr("n/a"))
        ),
    ];
    format!(
        "  {} · {} {}\n    {} · {} {} · {}",
        row.model_id,
        row.calls,
        tr("calls"),
        token_parts.join(" · "),
        tr("cache hit"),
        fmt_hit_rate(row.calls, row.cache_hit_rate, tr("n/a")),
        perf_parts.join(" · "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::ScreenMode;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;
    use xai_grok_tools::model_usage_ledger::{WINDOW_5H_MS, WINDOW_DAY_MS, WINDOW_WEEK_MS};

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

    fn exec_ctx(models: &ModelState, screen_mode: ScreenMode) -> CommandExecCtx<'_> {
        CommandExecCtx {
            models,
            session_id: None,
            bundle_state: &DEFAULT_BUNDLE_STATE,
            screen_mode,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        }
    }

    fn sample(
        ts: u64,
        model: &str,
        prompt: u64,
        cached: u64,
        ttft: u64,
        tps: f64,
    ) -> ModelCallSample {
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
    fn bare_stats_in_fullscreen_opens_modal_without_preset_window() {
        let models = ModelState::default();
        let mut ctx = exec_ctx(&models, ScreenMode::Fullscreen);
        assert!(
            matches!(
                StatsCommand.run(&mut ctx, ""),
                CommandResult::Action(Action::ShowStats { window: None })
            ),
            "裸 /stats 应打开窗口且不预设时间窗"
        );
    }

    #[test]
    fn window_argument_selects_the_opening_tab() {
        let models = ModelState::default();
        for (arg, window) in [
            ("5h", Window::FiveHours),
            ("day", Window::Day),
            ("week", Window::Week),
            ("  DAY  ", Window::Day),
        ] {
            let mut ctx = exec_ctx(&models, ScreenMode::Fullscreen);
            assert!(
                matches!(
                    StatsCommand.run(&mut ctx, arg),
                    CommandResult::Action(Action::ShowStats { window: Some(w) }) if w == window
                ),
                "arg={arg:?}"
            );
        }
    }

    #[test]
    fn unknown_argument_is_an_error_without_panicking() {
        let models = ModelState::default();
        let mut ctx = exec_ctx(&models, ScreenMode::Fullscreen);
        match StatsCommand.run(&mut ctx, "month") {
            CommandResult::Error(msg) => {
                assert!(msg.contains("Unknown argument: month"), "{msg}");
                assert!(msg.contains("/stats [5h|day|week]"), "{msg}");
            }
            other => panic!("expected error, got {other:?}"),
        }
    }

    #[test]
    fn minimal_mode_keeps_the_scrollback_text_report() {
        let models = ModelState::default();
        let mut ctx = exec_ctx(&models, ScreenMode::Minimal);
        match StatsCommand.run(&mut ctx, "") {
            CommandResult::Message(text) => {
                assert!(text.contains("Tool-output compression"), "{text}");
                // 不带模块过滤时账本为空，只断言标题段
                assert!(text.contains("Model usage (aggregated by model)"), "{text}");
            }
            other => panic!("expected message, got {other:?}"),
        }
    }

    #[test]
    fn suggest_args_lists_the_three_windows() {
        let ctx = AppCtx {
            models: &ModelState::default(),
            cwd: std::path::Path::new("."),
            has_session_announcements: false,
            billing_surface_visible: true,
            usage_command_visible: true,
            workflows_available: false,
            saved_workflows: &[],
            workflow_runs: &[],
            screen_mode: ScreenMode::Fullscreen,
            current_title: None,
        };
        let items = StatsCommand.suggest_args(&ctx, "").expect("suggestions");
        let inserted: Vec<&str> = items.iter().map(|i| i.insert_text.as_str()).collect();
        assert_eq!(inserted, ["5h", "day", "week"]);
    }

    #[test]
    fn default_report_covers_every_window_and_day_boundary() {
        let now = 1_000_000_000_000;
        let samples = vec![
            sample(now - 1000, "m-a", 1000, 800, 900, 50.0),
            sample(now - 2000, "m-a", 500, 500, 1100, 60.0),
            // 5h 窗外、日窗内
            sample(now - WINDOW_5H_MS - 1000, "m-b", 777, 0, 100, 99.0),
            // 日窗外、周窗内
            sample(now - WINDOW_DAY_MS - 1000, "m-c", 1, 0, 1, 1.0),
            // 周窗外
            sample(now - WINDOW_WEEK_MS - 1000, "m-d", 1, 0, 1, 1.0),
        ];
        let report = format_usage_report(&samples, now, &Window::ALL);
        assert!(report.contains("Model usage (aggregated by model)"));
        assert!(report.contains("Last 5h:"));
        assert!(report.contains("Last day:"));
        assert!(report.contains("Last week:"));
        assert!(!report.contains("Last month"), "月窗已移除: {report}");
        // 5h 窗只有 m-a 两笔
        assert!(report.contains("m-a · 2 calls"));
        assert!(report.contains("input 1.5k"));
        assert!(report.contains("output 1.0k"));
        assert!(report.contains("cache read 1.3k"));
        assert!(report.contains("cache write 400"));
        assert!(report.contains("cache hit 86.7%"));
        // m-b / m-c 各只有一次调用：冷启动算不出命中率，显示 n/a 而不是 0.0%
        assert!(report.contains("cache hit n/a"), "{report}");
        assert!(report.contains("ttft p50/p90 900/1100"));
        assert!(report.contains("tps p50/p90 50.0/60.0"));
        // 分节定位：5h 窗不含 m-b，日窗含 m-b 不含 m-c，周窗含 m-c 不含 m-d
        let day_start = report.find("Last day:").unwrap();
        let week_start = report.find("Last week:").unwrap();
        assert!(
            !report[..day_start].contains("m-b"),
            "5h 窗不该含 m-b: {report}"
        );
        assert!(report[day_start..week_start].contains("m-b"), "{report}");
        assert!(
            !report[day_start..week_start].contains("m-c"),
            "日窗不该含 m-c: {report}"
        );
        assert!(report[week_start..].contains("m-c"), "{report}");
        assert!(!report.contains("m-d"), "周窗外的样本不该出现: {report}");
    }

    #[test]
    fn single_window_report_contains_only_that_section() {
        let now = 1_000_000_000_000;
        let samples = vec![sample(now - 1000, "m-a", 1000, 800, 900, 50.0)];
        let report = format_usage_report(&samples, now, &[Window::Day]);
        assert!(report.contains("Last day:"), "{report}");
        assert!(!report.contains("Last 5h:"), "{report}");
        assert!(!report.contains("Last week:"), "{report}");
    }

    #[test]
    fn empty_windows_get_a_placeholder_per_window() {
        let now = 1_000_000_000_000;
        let report = format_usage_report(&[], now, &Window::ALL);
        assert_eq!(report.matches("no calls in this window").count(), 3);
        let single = format_usage_report(&[], now, &[Window::Week]);
        assert_eq!(single.matches("no calls in this window").count(), 1);
    }
}
