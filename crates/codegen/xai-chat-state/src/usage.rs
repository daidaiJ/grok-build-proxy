//! Per-prompt and per-session billing ledgers (not serialized).
//!
//! `total_tokens()` is input + output: Responses wire `total` is live context
//! length. Compaction and other side calls never call `record_main_loop_call`.
//!
//! # Completeness ownership
//!
//! Wire incomplete is the OR of these stores (each has a distinct role):
//!
//! - **`UsageLedger.incomplete`** — durable on the bill snapshot. Set by nested
//!   subagent incomplete fold, drain timeout, true apply-miss, and
//!   `mark_usage_incomplete`. Monotonic for a ledger instance.
//! - **Sticky (`subagent_usage_not_applied` on the coordinator)** — pin-scoped
//!   **report** signal (session-only attribution or apply-miss report). Not a
//!   second token sink; does not stain ledgers by itself.
//! - **Foreground live IDs** — fold may still land; freeze drains ≤120s or fails
//!   closed. Cancel skips multi-second drain (actor-loop safety).
//! - **Background live** — never waits; prompt report incomplete immediately;
//!   spend still folds into the session ledger at completion (no session-ledger
//!   incomplete).
//!
//! Freeze and cancel share one outcome policy: ledger marks only on fail-closed;
//! sticky and background_live are report-level only.
//!
//! Projection (`PromptUsage`) never invents tokens; it only ORs completeness
//! and scrubs costs when partial or incomplete.

use indexmap::IndexMap;
use xai_grok_sampling_types::TokenUsage;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub reasoning_tokens: u64,
    pub model_calls: u64,
    /// LOCAL: terminal model-call failures (retries exhausted or non-retryable), for status-line endpoint-health display.
    pub failed_model_calls: u64,
    /// LOCAL: calls whose response reported zero cache-read tokens, i.e. the
    /// prompt prefix was not served from the provider cache. Status-line
    /// display subtracts the session's first call, which can never hit.
    pub cache_miss_calls: u64,
    /// LOCAL: calls whose cache read dropped below
    /// [`CACHE_BREAK_DROP_RATIO`] of the previous main-loop call's cache read
    /// for the same model — the signature of a broken prompt prefix (rewritten
    /// history, model/effort switch, eviction). Ported from kimi-code's
    /// `cache-hint-controller.ts`. Surfaced by the status line / notifications;
    /// never affects billing.
    pub cache_break_calls: u64,
    pub api_duration_ms: u64,
    /// USD ticks (1e10 per USD). Absent when no call reported cost.
    pub cost_usd_ticks: Option<i64>,
    pub cost_missing_calls: u64,
}

/// LOCAL: minimum cache read (tokens) a previous call must have reported before
/// a drop counts as a break. Below this the prompt is too small for provider
/// caches to be meaningful, so every call would look like a break.
const CACHE_BREAK_MIN_PREV_READ_TOKENS: u64 = 512;

/// Whether a cache read of `read` tokens, following a same-model call that read
/// `prev_read`, indicates the prompt prefix stopped being served from cache.
pub fn is_cache_break(prev_read: u64, read: u64) -> bool {
    prev_read >= CACHE_BREAK_MIN_PREV_READ_TOKENS && read < prev_read - prev_read / 20
}

impl UsageTotals {
    fn from_call(
        usage: &TokenUsage,
        api_duration_ms: Option<u64>,
        cost_usd_ticks: Option<i64>,
    ) -> Self {
        let cost_usd_ticks = xai_grok_sampling_types::reported_cost_ticks(cost_usd_ticks);
        Self {
            input_tokens: u64::from(usage.prompt_tokens),
            output_tokens: u64::from(usage.completion_tokens),
            cached_read_tokens: u64::from(usage.cached_prompt_tokens),
            cache_creation_tokens: u64::from(usage.cache_creation_prompt_tokens),
            reasoning_tokens: u64::from(usage.reasoning_tokens),
            model_calls: 1,
            failed_model_calls: 0,
            cache_miss_calls: u64::from(usage.cached_prompt_tokens == 0),
            cache_break_calls: 0,
            api_duration_ms: api_duration_ms.unwrap_or(0),
            cost_usd_ticks,
            cost_missing_calls: u64::from(cost_usd_ticks.is_none()),
        }
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.saturating_add(self.output_tokens)
    }

    pub fn cost_is_partial(&self) -> bool {
        self.cost_usd_ticks.is_some() && self.cost_missing_calls > 0
    }

    fn fold_totals(&mut self, other: &UsageTotals) {
        let Self {
            input_tokens,
            output_tokens,
            cached_read_tokens,
            cache_creation_tokens,
            reasoning_tokens,
            model_calls,
            failed_model_calls,
            cache_miss_calls,
            cache_break_calls,
            api_duration_ms,
            cost_usd_ticks,
            cost_missing_calls,
        } = other;
        self.input_tokens = self.input_tokens.saturating_add(*input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(*output_tokens);
        self.cached_read_tokens = self.cached_read_tokens.saturating_add(*cached_read_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(*cache_creation_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(*reasoning_tokens);
        self.model_calls = self.model_calls.saturating_add(*model_calls);
        self.failed_model_calls = self.failed_model_calls.saturating_add(*failed_model_calls);
        self.cache_miss_calls = self.cache_miss_calls.saturating_add(*cache_miss_calls);
        self.cache_break_calls = self.cache_break_calls.saturating_add(*cache_break_calls);
        self.api_duration_ms = self.api_duration_ms.saturating_add(*api_duration_ms);
        self.cost_missing_calls = self.cost_missing_calls.saturating_add(*cost_missing_calls);
        self.cost_usd_ticks = merge_cost_ticks(self.cost_usd_ticks, *cost_usd_ticks);
    }
}

fn merge_cost_ticks(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0).saturating_add(b.unwrap_or(0))),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageLedger {
    pub totals: UsageTotals,
    pub by_model: IndexMap<String, UsageTotals>,
    /// Main-agent loop rounds for `num_turns` (subagents excluded).
    pub main_loop_model_calls: u64,
    /// Bill may under-count (drain timeout, nested subagent incomplete, apply failure).
    pub incomplete: bool,
    /// LOCAL: last main-loop cache read per model, for the consecutive-call
    /// break detector. Session-scoped state, never serialized or billed.
    last_cache_read_by_model: IndexMap<String, u64>,
}

impl UsageLedger {
    /// Fold one main-agent-loop model call. This is the only writer of
    /// `main_loop_model_calls` (the wire `numTurns`); side calls such as
    /// compaction must not use it.
    pub fn record_main_loop_call(
        &mut self,
        model_id: &str,
        usage: &TokenUsage,
        api_duration_ms: Option<u64>,
        cost_usd_ticks: Option<i64>,
    ) {
        let read = u64::from(usage.cached_prompt_tokens);
        let prev_read = self.last_cache_read_by_model.get(model_id).copied();
        self.last_cache_read_by_model.insert(model_id.to_owned(), read);
        let cache_break_calls = u64::from(matches!(prev_read, Some(prev) if is_cache_break(prev, read)));
        let call = UsageTotals {
            cache_break_calls,
            ..UsageTotals::from_call(usage, api_duration_ms, cost_usd_ticks)
        };
        self.main_loop_model_calls = self.main_loop_model_calls.saturating_add(1);
        self.fold_entry(model_id, &call);
    }

    /// LOCAL: fold one terminal model-call failure (retries exhausted or non-retryable).
    /// Counts the endpoint-health signal only; no token or cost impact.
    pub fn record_main_loop_failure(&mut self, model_id: &str) {
        let call = UsageTotals {
            failed_model_calls: 1,
            ..UsageTotals::default()
        };
        self.fold_entry(model_id, &call);
    }

    /// Fold subagent usage without incrementing `main_loop_model_calls`.
    pub fn record_subagent(&mut self, by_model: &[(String, UsageTotals)], incomplete: bool) {
        for (model_id, totals) in by_model {
            self.fold_entry(model_id, totals);
        }
        if incomplete {
            self.incomplete = true;
        }
    }

    pub fn mark_incomplete(&mut self) {
        self.incomplete = true;
    }

    fn fold_entry(&mut self, model_id: &str, totals: &UsageTotals) {
        self.totals.fold_totals(totals);
        self.by_model
            .entry(model_id.to_owned())
            .or_default()
            .fold_totals(totals);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tu(prompt: u32, completion: u32) -> TokenUsage {
        TokenUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: 999_999,
            reasoning_tokens: 0,
            cached_prompt_tokens: 0,
            cache_creation_prompt_tokens: 0,
        }
    }

    #[test]
    fn ledger_sums_partial_subagent_and_zero_cost() {
        let mut ledger = UsageLedger::default();
        ledger.record_main_loop_call("m", &tu(1, 1), None, Some(0));
        assert_eq!(ledger.totals.cost_usd_ticks, None);
        assert_eq!(ledger.totals.cost_missing_calls, 1);

        ledger.record_main_loop_call("a", &tu(100, 10), Some(100), None);
        ledger.record_main_loop_call("a", &tu(50, 5), Some(50), Some(70));
        assert_eq!(ledger.totals.cost_usd_ticks, Some(70));
        assert!(ledger.totals.cost_is_partial());
        assert_eq!(ledger.main_loop_model_calls, 3);

        ledger.record_subagent(
            &[(
                "b".into(),
                UsageTotals {
                    input_tokens: 5,
                    model_calls: 1,
                    ..Default::default()
                },
            )],
            false,
        );
        assert_eq!(ledger.by_model["b"].input_tokens, 5);
        assert_eq!(ledger.main_loop_model_calls, 3);
        assert_eq!(ledger.totals.model_calls, 4);
        assert!(!ledger.incomplete);

        ledger.record_subagent(&[], true);
        assert!(ledger.incomplete);
    }

    #[test]
    fn cache_break_detection_thresholds() {
        // Below the minimum previous read: never a break.
        assert!(!is_cache_break(100, 0));
        assert!(!is_cache_break(0, 0));
        // prev=1000: 95% threshold at 950 — exactly 95% is not a break.
        assert!(!is_cache_break(1000, 950));
        assert!(is_cache_break(1000, 949));
        // Growing reads are never breaks.
        assert!(!is_cache_break(1000, 1200));
        assert!(!is_cache_break(1000, 1000));
    }

    #[test]
    fn cache_break_counts_dropped_reads_per_model() {
        let mut usage = tu(1000, 10);
        usage.cached_prompt_tokens = 1000;
        let mut dropped = tu(1000, 10);
        dropped.cached_prompt_tokens = 400;

        let mut ledger = UsageLedger::default();
        ledger.record_main_loop_call("m", &usage, None, None);
        assert_eq!(ledger.totals.cache_break_calls, 0, "first call has no predecessor");
        ledger.record_main_loop_call("m", &dropped, None, None);
        assert_eq!(ledger.totals.cache_break_calls, 1);

        // A model switch does not break the new model's first call...
        ledger.record_main_loop_call("other", &dropped, None, None);
        assert_eq!(ledger.totals.cache_break_calls, 1);
        // ...and the original model recovers on its next call.
        ledger.record_main_loop_call("m", &usage, None, None);
        assert_eq!(ledger.totals.cache_break_calls, 1);
    }

    #[test]
    fn cache_miss_counts_uncached_calls_only() {
        let mut ledger = UsageLedger::default();
        let mut miss = tu(100, 10);
        miss.cached_prompt_tokens = 0;
        let mut hit = tu(100, 10);
        hit.cached_prompt_tokens = 90;
        ledger.record_main_loop_call("m", &miss, None, None);
        ledger.record_main_loop_call("m", &hit, None, None);
        ledger.record_main_loop_call("m", &miss, None, None);
        ledger.record_main_loop_failure("m");
        assert_eq!(ledger.totals.model_calls, 3);
        assert_eq!(ledger.totals.cache_miss_calls, 2);
    }
}
