//! Human tables for `grok stats`. Aligned columns, one section per bucket.

use std::borrow::Cow;
use std::io::Write;

use super::{BucketRow, SessionRow, StatsReport, Totals};
use xai_grok_tools::implementations::output_compression::{
    format_stats_report, is_enabled as compression_enabled, stats_for_display,
};

/// Session ids show as their first 8 columns; `sessions` names the rest.
const ID_COLS: usize = 8;
const PROJECT_COLS: usize = 24;
const WHEN_COLS: usize = 16;
const KEY_COLS: usize = 10;
const MODEL_COLS: usize = 32;

pub(super) fn print_report(report: &StatsReport, limit: usize, out: &mut impl Write) {
    if report.sessions.is_empty() && report.days.is_empty() {
        let _ = writeln!(out, "No usage recorded.");
        return;
    }
    print_sessions(report, limit, out);
    print_buckets("By day", &report.days, out);
    print_buckets("By week (ISO)", &report.weeks, out);
    print_models(&report.models, out);
    print_compression(report, out);
}

fn print_compression(report: &StatsReport, out: &mut impl Write) {
    let Some(json) = report.tool_output_compression.as_ref() else {
        return;
    };
    let stats = stats_for_display(None);
    let _ = writeln!(
        out,
        "{}",
        format_stats_report(&stats, json.enabled || compression_enabled())
    );
    let _ = writeln!(out);
}

fn print_sessions(report: &StatsReport, limit: usize, out: &mut impl Write) {
    let _ = writeln!(
        out,
        "{:<idw$} {:<projw$} {:<whenw$} {:>6} {:>9} {:>9} {:>9} {:>9} {:>10}  {}",
        "SESSION", "PROJECT", "LAST ACTIVITY", "TURNS", "INPUT", "OUTPUT", "CACHED", "REASON", "COST", "MODEL",
        idw = ID_COLS,
        projw = PROJECT_COLS,
        whenw = WHEN_COLS,
    );
    let shown: &[SessionRow] = if limit == 0 {
        &report.sessions
    } else {
        &report.sessions[..report.sessions.len().min(limit)]
    };
    for row in shown {
        let _ = writeln!(
            out,
            "{:<idw$} {:<projw$} {:<whenw$} {:>6} {:>9} {:>9} {:>9} {:>9} {:>10}  {}",
            short_id(&row.session_id),
            row.project.as_deref().unwrap_or("-"),
            row.last_activity.as_deref().unwrap_or("-"),
            row.totals.turns,
            row.totals.input_tokens,
            row.totals.output_tokens,
            row.totals.cached_read_tokens,
            row.totals.reasoning_tokens,
            cost_cell(&row.totals),
            row.primary_model.as_deref().unwrap_or("-"),
            idw = ID_COLS,
            projw = PROJECT_COLS,
            whenw = WHEN_COLS,
        );
    }
    let total = Totals::sum_of(report.sessions.iter().map(|row| &row.totals));
    let _ = writeln!(
        out,
        "{:<idw$} {:<projw$} {:<whenw$} {:>6} {:>9} {:>9} {:>9} {:>9} {:>10}",
        "TOTAL",
        format!("{} sessions", report.sessions.len()),
        "",
        total.turns,
        total.input_tokens,
        total.output_tokens,
        total.cached_read_tokens,
        total.reasoning_tokens,
        cost_cell(&total),
        idw = ID_COLS,
        projw = PROJECT_COLS,
        whenw = WHEN_COLS,
    );
    let hidden = report.sessions.len() - shown.len();
    if hidden > 0 {
        let _ = writeln!(out, "… {hidden} more sessions (--limit 0 shows every one)");
    }
    let _ = writeln!(out);
}

fn print_buckets(title: &str, rows: &[BucketRow], out: &mut impl Write) {
    let _ = writeln!(out, "{title}");
    let _ = writeln!(
        out,
        "{:<keyw$} {:>8} {:>6} {:>9} {:>9} {:>10}",
        "BUCKET", "SESSIONS", "TURNS", "INPUT", "OUTPUT", "COST",
        keyw = KEY_COLS,
    );
    for row in rows {
        let _ = writeln!(
            out,
            "{:<keyw$} {:>8} {:>6} {:>9} {:>9} {:>10}",
            row.key,
            row.sessions,
            row.totals.turns,
            row.totals.input_tokens,
            row.totals.output_tokens,
            cost_cell(&row.totals),
            keyw = KEY_COLS,
        );
    }
    let _ = writeln!(out);
}

/// The whole window split by model id, busiest first. Each row's own per-turn
/// model split is in `--json`; the table shows just the rollup.
fn print_models(rows: &[super::ModelRow], out: &mut impl Write) {
    if rows.is_empty() {
        return;
    }
    let _ = writeln!(out, "By model");
    let _ = writeln!(
        out,
        "{:<modelw$} {:>6} {:>7} {:>9} {:>9} {:>10}",
        "MODEL ID", "TURNS", "CALLS", "INPUT", "OUTPUT", "COST",
        modelw = MODEL_COLS,
    );
    for row in rows {
        let _ = writeln!(
            out,
            "{:<modelw$} {:>6} {:>7} {:>9} {:>9} {:>10}",
            fit(&row.model_id, MODEL_COLS),
            row.totals.turns,
            row.totals.model_calls,
            row.totals.input_tokens,
            row.totals.output_tokens,
            cost_cell(&row.totals),
            modelw = MODEL_COLS,
        );
    }
    let _ = writeln!(out);
}

fn fit(text: &str, cols: usize) -> Cow<'_, str> {
    if text.chars().count() <= cols {
        return Cow::Borrowed(text);
    }
    let mut kept: String = text.chars().take(cols.saturating_sub(1)).collect();
    kept.push('…');
    Cow::Owned(kept)
}

fn short_id(session_id: &str) -> &str {
    session_id.get(..ID_COLS).unwrap_or(session_id)
}

/// `+` says the sum under-counts: some turns reported no cost.
fn cost_cell(totals: &Totals) -> String {
    match totals.cost_usd {
        None => "-".into(),
        Some(usd) => format!("${:.2}{}", usd, if totals.cost_partial { "+" } else { "" }),
    }
}
