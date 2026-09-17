//! The `builtin` row: one segment per [`StatusLineItem`] the session asked for, each already cut to the columns it may use.

use std::time::Duration;

use xai_grok_pager_render::glyphs::{ballot_x, check_mark};
use xai_grok_status_line::{StatusLineContext, StatusLineItem};

use super::fit_columns;

pub const SEGMENT_SEPARATOR: &str = " │ ";

const CONTEXT_WARN_PCT: u8 = 80;

// Columns, not bytes: a byte budget halves a CJK or emoji name.
const CWD_COLS: usize = 40;
const MODEL_COLS: usize = 30;
const SESSION_NAME_COLS: usize = 40;

const MIN_DISPLAYED_COST_USD: f64 = 0.005;

/// Token counts in the compact `47k` / `1.2M` form the bundled status-line
/// script uses: one decimal, then trailing `0.` stripped.
fn fmt_tokens(n: u64) -> String {
    let (divisor, suffix) = if n >= 1_000_000 {
        (1_000_000.0, "M")
    } else if n >= 1000 {
        (1000.0, "k")
    } else {
        return n.to_string();
    };
    let scaled = format!("{:.1}", n as f64 / divisor);
    let scaled = scaled.strip_suffix('0').unwrap_or(&scaled);
    let scaled = scaled.strip_suffix('.').unwrap_or(scaled);
    format!("{scaled}{suffix}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentTone {
    Dim,
    Warn,
}

/// A `builtin` segment, already cut to the columns it may use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusSegment {
    // Not `pub`: a struct literal elsewhere would skip the control-character filter in [`Self::new`]
    // Read through [`Self::text`]
    pub(super) text: String,
    pub(super) tone: SegmentTone,
}

impl StatusSegment {
    fn toned(text: String, tone: SegmentTone) -> Self {
        Self::new(text, tone)
    }

    /// Read access for tests in other modules; the fields stay closed so a literal cannot skip [`Self::new`].
    #[cfg(test)]
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    fn dim(text: impl Into<String>) -> Self {
        Self::new(text, SegmentTone::Dim)
    }

    pub(crate) fn warn(text: impl Into<String>) -> Self {
        Self::new(text, SegmentTone::Warn)
    }

    /// Control characters are dropped here rather than at the painter.
    /// A segment carries the user's own text: a cwd, a model name, a config value they typed.
    /// Only [`SanitizedText`](super::SanitizedText) filters the path a script's output takes.
    fn new(text: impl Into<String>, tone: SegmentTone) -> Self {
        let text: String = text.into();
        Self {
            text: text.chars().filter(|c| !c.is_control()).collect(),
            tone,
        }
    }
}

#[must_use]
pub fn compose_builtin(
    ctx: &StatusLineContext,
    turn_elapsed: Option<Duration>,
    items: &[StatusLineItem],
) -> Vec<StatusSegment> {
    items
        .iter()
        .filter_map(|item| match item {
            StatusLineItem::Cwd => {
                let short = ctx.cwd.rsplit(['/', '\\']).find(|s| !s.is_empty())?;
                Some(StatusSegment::dim(fit_columns(short, CWD_COLS)))
            }
            StatusLineItem::Model => {
                let model = ctx
                    .model
                    .display_name
                    .as_deref()
                    .filter(|s| !s.is_empty())?;
                Some(StatusSegment::dim(fit_columns(model, MODEL_COLS)))
            }
            StatusLineItem::Context => {
                let window = &ctx.context_window;
                let pct = window.used_percentage?;
                let warn_at = window
                    .auto_compact_threshold_percent
                    .unwrap_or(CONTEXT_WARN_PCT);
                let tone = if pct >= warn_at {
                    SegmentTone::Warn
                } else {
                    SegmentTone::Dim
                };
                Some(StatusSegment::toned(format!("{pct}% ctx"), tone))
            }
            StatusLineItem::Cost => ctx
                .cost
                .total_cost_usd
                .filter(|usd| *usd >= MIN_DISPLAYED_COST_USD)
                .map(|usd| StatusSegment::dim(format!("${usd:.2}"))),
            StatusLineItem::TurnTimer => {
                let secs = turn_elapsed?.as_secs();
                let text = match secs {
                    0 => return None,
                    s if s < 60 => format!("{s}s"),
                    s => format!("{}m{:02}s", s / 60, s % 60),
                };
                Some(StatusSegment::dim(text))
            }
            StatusLineItem::SessionName => {
                let name = ctx.session_name.as_deref().filter(|s| !s.is_empty())?;
                Some(StatusSegment::dim(fit_columns(name, SESSION_NAME_COLS)))
            }
            // LOCAL: endpoint health, `✓ n`, turning amber and gaining `✗ n` on failures.
            StatusLineItem::ApiCalls => {
                let calls = ctx.api_calls?;
                let text = if calls.failed > 0 {
                    format!("{} {} {} {}", check_mark(), calls.succeeded, ballot_x(), calls.failed)
                } else {
                    format!("{} {}", check_mark(), calls.succeeded)
                };
                let tone = (calls.failed > 0).then_some(SegmentTone::Warn);
                Some(StatusSegment::toned(text, tone.unwrap_or(SegmentTone::Dim)))
            }
            // LOCAL: transient retry resubmissions, `↻ 3`; hidden while zero.
            StatusLineItem::ApiRetries => {
                let calls = ctx.api_calls?;
                (calls.retries > 0).then(|| StatusSegment::dim(format!("↻ {}", calls.retries)))
            }
            // LOCAL: uncached calls beyond the session's first, `miss 2`; hidden while zero.
            StatusLineItem::CacheMisses => {
                let calls = ctx.api_calls?;
                (calls.cache_misses > 0).then(|| StatusSegment::dim(format!("miss {}", calls.cache_misses)))
            }
            // LOCAL: cumulative session tokens, `in 47k out 3.2k`.
            StatusLineItem::Tokens => {
                let window = &ctx.context_window;
                let usage = window.session_usage;
                let total_in = window.session_input_tokens.or_else(|| {
                    usage.map(|u| {
                        u.input_tokens + u.cache_read_input_tokens + u.cache_creation_input_tokens
                    })
                })?;
                let out = usage
                    .map(|u| u.output_tokens)
                    .or(window.session_output_tokens)?;
                Some(StatusSegment::dim(format!(
                    "in {} out {}",
                    fmt_tokens(total_in),
                    fmt_tokens(out)
                )))
            }
            // LOCAL: cache-read share of the session's input tokens, `cache 95.7%`.
            StatusLineItem::Cache => {
                let window = &ctx.context_window;
                let usage = window.session_usage?;
                let total_in = window.session_input_tokens.unwrap_or(
                    usage.input_tokens + usage.cache_read_input_tokens + usage.cache_creation_input_tokens,
                );
                (total_in > 0).then(|| {
                    StatusSegment::dim(format!(
                        "cache {:.1}%",
                        100.0 * usage.cache_read_input_tokens as f64 / total_in as f64
                    ))
                })
            }
            // LOCAL: reasoning share of the session's output tokens, `think 28.1%`.
            StatusLineItem::Think => {
                let usage = ctx.context_window.session_usage?;
                (usage.output_tokens > 0).then(|| {
                    StatusSegment::dim(format!(
                        "think {:.1}%",
                        100.0 * usage.reasoning_tokens as f64 / usage.output_tokens as f64
                    ))
                })
            }
            // LOCAL: last turn's latency/throughput, `380ms ttft · 42.3 tok/s`.
            StatusLineItem::Perf => {
                let perf = ctx.perf.as_ref()?;
                let mut parts: Vec<String> = Vec::with_capacity(2);
                if let Some(ttft_ms) = perf.ttft_ms {
                    parts.push(format!("{ttft_ms}ms ttft"));
                }
                if let Some(tps) = perf.tps {
                    parts.push(format!("{tps:.1} tok/s"));
                }
                (!parts.is_empty()).then(|| StatusSegment::dim(parts.join(" · ")))
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "segments_tests.rs"]
mod tests;
