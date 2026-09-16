use super::*;

const DIR: &str = "/home/user/project";

fn context() -> StatusLineContext {
    let mut ctx = crate::app::status_line::test_context(DIR);
    ctx.session_name = Some("status_line work".into());
    ctx.model.display_name = Some("Grok Build".into());
    ctx.cost.total_cost_usd = Some(0.3745);
    ctx.context_window.used_percentage = Some(42);
    ctx.context_window.auto_compact_threshold_percent = Some(80);
    ctx
}

fn plain(ctx: &StatusLineContext, turn_elapsed: Option<Duration>) -> String {
    compose_builtin(ctx, turn_elapsed, StatusLineItem::ALL)
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(SEGMENT_SEPARATOR)
}

#[test]
fn composes_every_segment_in_order() {
    assert_eq!(
        plain(&context(), Some(Duration::from_secs(83))),
        "project │ Grok Build │ 42% ctx │ $0.37 │ 1m23s │ status_line work"
    );
}

#[test]
fn omits_segments_whose_data_is_missing_or_rounds_to_zero() {
    let mut ctx = context();
    ctx.cost.total_cost_usd = Some(0.004);
    assert_eq!(
        plain(&ctx, Some(Duration::from_millis(400))),
        "project │ Grok Build │ 42% ctx │ status_line work"
    );

    ctx.cost.total_cost_usd = Some(0.006);
    assert!(plain(&ctx, None).contains("$0.01"));

    ctx.context_window.used_percentage = None;
    assert!(compose_builtin(&ctx, None, &[StatusLineItem::Context]).is_empty());
}

#[test]
fn name_past_its_budget_is_cut_by_painted_columns() {
    let mut ctx = context();
    ctx.session_name = Some("辺".repeat(SESSION_NAME_COLS));
    let cut = &compose_builtin(&ctx, None, &[StatusLineItem::SessionName])[0].text;

    let width = super::super::painted_width(cut);
    assert!(
        cut.ends_with('…'),
        "a cut name has to say it was cut: {cut}"
    );
    assert!(
        width <= SESSION_NAME_COLS,
        "{width} columns overruns the {SESSION_NAME_COLS} the segment was given"
    );
    // The cut fills the budget to within one cluster; a byte or character cut would leave more unused
    assert!(
        width + 2 > SESSION_NAME_COLS,
        "{width} columns leaves more than a cluster of the budget unused"
    );
}

#[test]
fn cost_the_session_does_not_have_omits_its_segment() {
    let mut ctx = context();
    ctx.cost.total_cost_usd = None;
    assert!(compose_builtin(&ctx, None, &[StatusLineItem::Cost]).is_empty());
}

#[test]
fn context_segment_warns_near_compaction() {
    let mut ctx = context();
    let tone =
        |ctx: &StatusLineContext| compose_builtin(ctx, None, &[StatusLineItem::Context])[0].tone;

    ctx.context_window.used_percentage = Some(90);
    assert_eq!(tone(&ctx), SegmentTone::Warn);
    ctx.context_window.used_percentage = Some(50);
    assert_eq!(tone(&ctx), SegmentTone::Dim);

    ctx.context_window.auto_compact_threshold_percent = Some(65);
    ctx.context_window.used_percentage = Some(70);
    assert_eq!(tone(&ctx), SegmentTone::Warn);
}

fn api_calls(succeeded: u64, failed: u64) -> StatusLineContext {
    let mut ctx = context();
    ctx.api_calls = Some(xai_grok_status_line::StatusLineApiCalls { succeeded, failed });
    ctx
}

#[test]
fn api_calls_segment_counts_failures_and_warns() {
    let segment = |ctx: &StatusLineContext| compose_builtin(ctx, None, &[StatusLineItem::ApiCalls])[0].clone();

    let clean = segment(&api_calls(12, 0));
    assert_eq!(clean.text(), "✓ 12");
    assert_eq!(clean.tone, SegmentTone::Dim);

    let failing = segment(&api_calls(12, 3));
    assert_eq!(failing.text(), "✓ 12 ✗ 3");
    assert_eq!(failing.tone, SegmentTone::Warn);

    assert!(compose_builtin(&context(), None, &[StatusLineItem::ApiCalls]).is_empty());
}

#[test]
fn perf_segment_formats_whatever_the_snapshot_carries() {
    let segment = |ctx: &StatusLineContext| {
        compose_builtin(ctx, None, &[StatusLineItem::Perf])
            .first()
            .map(|s| s.text.clone())
    };

    let mut ctx = context();
    ctx.perf = Some(xai_grok_status_line::StatusLineTurnPerf {
        ttft_ms: Some(380),
        tps: Some(42.3),
        output_tokens: Some(915),
    });
    assert_eq!(segment(&ctx).as_deref(), Some("380ms ttft · 42.3 tok/s"));

    ctx.perf.as_mut().unwrap().ttft_ms = None;
    assert_eq!(segment(&ctx).as_deref(), Some("42.3 tok/s"));

    ctx.perf.as_mut().unwrap().ttft_ms = Some(380);
    ctx.perf.as_mut().unwrap().tps = None;
    assert_eq!(segment(&ctx).as_deref(), Some("380ms ttft"));

    ctx.perf = Some(xai_grok_status_line::StatusLineTurnPerf::default());
    assert!(segment(&ctx).is_none());
    assert!(compose_builtin(&context(), None, &[StatusLineItem::Perf]).is_empty());
}

fn session_usage(
    input: u64,
    cache_read: u64,
    cache_creation: u64,
    output: u64,
    reasoning: u64,
) -> xai_grok_status_line::StatusLineSessionUsage {
    xai_grok_status_line::StatusLineSessionUsage {
        input_tokens: input,
        cache_read_input_tokens: cache_read,
        cache_creation_input_tokens: cache_creation,
        output_tokens: output,
        reasoning_tokens: reasoning,
    }
}

#[test]
fn token_segments_mirror_the_bundled_status_line_script() {
    let mut ctx = context();
    ctx.context_window.session_input_tokens = Some(47_000);
    ctx.context_window.session_output_tokens = Some(3_200);
    ctx.context_window.session_usage = Some(session_usage(1_200, 45_000, 800, 3_200, 900));

    let text = |ctx: &StatusLineContext, item| {
        compose_builtin(ctx, None, &[item])[0].text.clone()
    };
    assert_eq!(text(&ctx, StatusLineItem::Tokens), "in 47k out 3.2k");
    assert_eq!(text(&ctx, StatusLineItem::Cache), "cache 95.7%");
    assert_eq!(text(&ctx, StatusLineItem::Think), "think 28.1%");

    // Without the window totals, `in` falls back to the disjoint usage sum.
    ctx.context_window.session_input_tokens = None;
    assert_eq!(text(&ctx, StatusLineItem::Tokens), "in 47k out 3.2k");

    // A scale past one mega gets the M suffix.
    ctx.context_window.session_usage = Some(session_usage(0, 1_260_000, 0, 0, 0));
    ctx.context_window.session_input_tokens = Some(1_260_000);
    assert_eq!(text(&ctx, StatusLineItem::Tokens), "in 1.3M out 0");
}

#[test]
fn a_fresh_session_shows_only_the_fields_it_has_data_for() {
    // The default item set over the fixture's bare context: only the model is
    // known before the first turn lands, so it is the only segment drawn.
    let default_config = xai_grok_status_line::StatusLineConfig::default();
    let items = match default_config.resolve() {
        Some(xai_grok_status_line::ResolvedStatusLine::Builtin { items }) => items,
        other => panic!("the default section draws a builtin row: {other:?}"),
    };
    assert_eq!(
        compose_builtin(&context(), None, items)
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(SEGMENT_SEPARATOR),
        "Grok Build"
    );
}

#[test]
fn the_default_row_keeps_the_model_anchored_to_the_left_edge() {
    let default_config = xai_grok_status_line::StatusLineConfig::default();
    let items = match default_config.resolve() {
        Some(xai_grok_status_line::ResolvedStatusLine::Builtin { items }) => items,
        other => panic!("the default section draws a builtin row: {other:?}"),
    };

    // Turn end: every metric exists, and the model still leads the row.
    let mut ctx = context();
    ctx.api_calls = Some(xai_grok_status_line::StatusLineApiCalls {
        succeeded: 3,
        failed: 0,
    });
    ctx.context_window.session_input_tokens = Some(47_000);
    ctx.context_window.session_output_tokens = Some(3_200);
    ctx.context_window.session_usage = Some(session_usage(1_200, 45_000, 800, 3_200, 900));
    ctx.perf = Some(xai_grok_status_line::StatusLineTurnPerf {
        ttft_ms: Some(750),
        tps: Some(55.2),
        output_tokens: None,
    });

    // Built through the same glyph helpers: which mark the console gets is an
    // environment decision, the segment ORDER is what this test pins.
    let expected = [
        "Grok Build".to_string(),
        format!("{} 3", check_mark()),
        "in 47k out 3.2k".to_string(),
        "cache 95.7%".to_string(),
        "think 28.1%".to_string(),
        "750ms ttft · 55.2 tok/s".to_string(),
    ]
    .join(SEGMENT_SEPARATOR);

    assert_eq!(
        compose_builtin(&ctx, None, items)
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(SEGMENT_SEPARATOR),
        expected
    );
}
