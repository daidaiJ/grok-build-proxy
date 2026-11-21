//! LOCAL (experimental) idle-time auto-compact for `SessionActor`.
//!
//! Fires once per completed turn when the session settles idle: if the context
//! usage is already past the auto-compact threshold, run the same compaction
//! the inline pre-sampling check would run — but now, off the critical path, so
//! the user's next prompt does not pay the summarization latency.
//!
//! The same task also covers a second case: the turn ended while background or
//! monitor tasks are still running. Just before the gateway cache TTL (~5 min)
//! the next wake-up is about to pay a cold read; compacting the idle context
//! first can shrink that cold read. This path is gated on an explicit token-
//! benefit estimate (future-call savings minus the compaction call and the
//! cache miss), fires at most once per cooldown/growth window, and logs every
//! skip reason.
//!
//! Opt-in for the threshold path via `[features] idle_auto_compact = true` (or
//! `GROK_IDLE_AUTO_COMPACT`); the wait path is on by default and can be disabled
//! with `[compaction] idle_wait_compact = false` /
//! `GROK_IDLE_WAIT_COMPACT=0`. The inline pre-sampling check stays as the
//! backstop and is suppressed while an idle compact is in flight
//! ([`CompactionConfig::idle_compact_in_flight`]).
//!
//! Concurrency: the spawned task re-checks the shared idle predicate after a
//! short grace period, and keeps polling while the compact runs — a user prompt
//! (or any turn start) aborts the in-flight compact by dropping its future, the
//! same cancellation semantics a user stop gets.

use super::*;

/// How long to wait after turn end before firing, so a user already typing a
/// follow-up does not kick off a summarization request they are about to make
/// redundant.
const IDLE_COMPACT_GRACE: std::time::Duration = std::time::Duration::from_secs(3);
/// Poll interval for both the grace wait and the busy-abort while compacting.
const IDLE_COMPACT_BUSY_POLL: std::time::Duration = std::time::Duration::from_millis(250);
/// Token-equivalent buffer for the cache miss a rewritten prefix causes. The
/// wait runs near the TTL, so the cache may still be warm; 75% of the prefix
/// is a conservative stand-in for losing the cache-read discount.
const IDLE_WAIT_CACHE_MISS_PERCENT: u64 = 75;

/// Result of the wait-path token gate. `benefit_tokens` is the replay saved
/// across the expected future calls; `cost_tokens` is the compaction call
/// (full prefix read) plus the summary write plus the cache-miss buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IdleWaitCompactPlan {
    pub pre_tokens: u64,
    pub post_tokens: u64,
    pub expected_future_calls: u64,
    pub benefit_tokens: u64,
    pub cost_tokens: u64,
    pub net_tokens: i64,
}

/// Decide whether an idle-wait compact is worth one model call.
///
/// The next `N` calls are assumed cold (the wait reached the cache TTL):
/// without compaction they read `pre` each; with it the compaction call reads
/// `pre` once, writes `post`, and the next `N` calls read `post`.
pub(crate) fn plan_idle_wait_compact(
    pre_tokens: u64,
    last_post_tokens: Option<u64>,
    expected_future_calls: u64,
    min_prompt_tokens: u64,
    min_net_tokens: u64,
) -> Result<IdleWaitCompactPlan, &'static str> {
    if pre_tokens < min_prompt_tokens {
        return Err("below_min_prompt_tokens");
    }
    if expected_future_calls == 0 {
        return Err("no_expected_future_calls");
    }
    let post_tokens = last_post_tokens
        .filter(|post| *post > 0 && *post < pre_tokens)
        .unwrap_or_else(|| (pre_tokens / 10).clamp(4_000, 24_000));
    if post_tokens >= pre_tokens {
        return Err("no_reclaimable_context");
    }
    let benefit_tokens = (pre_tokens - post_tokens).saturating_mul(expected_future_calls);
    let cache_miss_penalty = pre_tokens.saturating_mul(IDLE_WAIT_CACHE_MISS_PERCENT) / 100;
    let cost_tokens = pre_tokens
        .saturating_add(post_tokens)
        .saturating_add(cache_miss_penalty);
    let net_tokens = benefit_tokens as i64 - cost_tokens as i64;
    if net_tokens < min_net_tokens as i64 {
        return Err("negative_net_benefit");
    }
    Ok(IdleWaitCompactPlan {
        pre_tokens,
        post_tokens,
        expected_future_calls,
        benefit_tokens,
        cost_tokens,
        net_tokens,
    })
}

impl SessionActor {
    /// Entry point spawned from the run loop at the post-turn idle settle point.
    /// `enabled` is resolved by the caller (feature flag + env), keeping this
    /// body testable without touching process env.
    pub(crate) async fn maybe_run_idle_auto_compact(self: Arc<Self>, enabled: bool) {
        let wait = crate::util::config::resolve_idle_wait_compact_settings();
        Self::maybe_run_idle_auto_compact_with_wait(self, enabled, wait, IDLE_COMPACT_GRACE).await
    }

    /// Testable core for the existing threshold path; the wait path is off so
    /// callers keep the pre-wait behavior.
    pub(crate) async fn maybe_run_idle_auto_compact_inner(
        self: Arc<Self>,
        enabled: bool,
        grace: std::time::Duration,
    ) {
        Self::maybe_run_idle_auto_compact_with_wait(
            self,
            enabled,
            crate::util::config::IdleWaitCompactSettings::disabled(),
            grace,
        )
        .await
    }

    async fn maybe_run_idle_auto_compact_with_wait(
        self: Arc<Self>,
        threshold_enabled: bool,
        wait: crate::util::config::IdleWaitCompactSettings,
        grace: std::time::Duration,
    ) {
        if (!threshold_enabled && !wait.enabled) || self.startup_hints.is_subagent {
            return;
        }
        if self.compaction.is_suppressed() {
            return;
        }
        let started = tokio::time::Instant::now();
        // The in-flight slot is claimed only when a compact is about to run, so
        // the inline pre-sampling check stays available while this task waits.
        let busy_now = {
            let state = self.state.lock().await;
            !is_session_idle_for_injection(&state)
        };
        if busy_now {
            return;
        }
        // Grace wait: a queued follow-up outranks a speculative compact.
        if self.wait_for_busy(grace).await {
            xai_grok_telemetry::unified_log::info(
                "shell.idle_compact.aborted",
                Some(self.session_info.id.0.as_ref()),
                Some(serde_json::json!({ "reason": "session_busy_during_grace" })),
            );
            return;
        }
        if threshold_enabled {
            let count_before = self
                .compaction
                .count
                .load(std::sync::atomic::Ordering::Relaxed);
            let len_before = self.chat_state_handle.get_conversation_len().await;
            if let Some(trigger_info) = self.check_auto_compact_needed_inner().await {
                if !self.try_claim_idle_compact() {
                    return;
                }
                if !self.is_idle_now().await
                    || self.compaction.is_suppressed()
                    || self
                        .compaction
                        .count
                        .load(std::sync::atomic::Ordering::Relaxed)
                        != count_before
                    || self.chat_state_handle.get_conversation_len().await != len_before
                {
                    self.clear_idle_compact_in_flight();
                    return;
                }
                xai_grok_telemetry::unified_log::info(
                    "shell.idle_compact.fired",
                    Some(self.session_info.id.0.as_ref()),
                    Some(serde_json::json!({
                        "tokens_used": trigger_info.tokens_used,
                        "context_window": trigger_info.context_window,
                        "percentage": trigger_info.percentage,
                    })),
                );
                self.run_idle_compact_select(trigger_info).await;
                return;
            }
        }
        if wait.enabled {
            self.run_idle_wait_compact(wait, started).await;
        }
    }

    /// Wait for the cache TTL, then compact only when the token gate says the
    /// next wake-up will save more than the call plus the cache miss.
    async fn run_idle_wait_compact(
        self: Arc<Self>,
        settings: crate::util::config::IdleWaitCompactSettings,
        started: tokio::time::Instant,
    ) {
        let count_at_start = self
            .compaction
            .count
            .load(std::sync::atomic::Ordering::Relaxed);
        let len_at_start = self.chat_state_handle.get_conversation_len().await;
        let outstanding = self.outstanding_wait_tasks().await;
        if outstanding == 0 {
            self.log_idle_wait_skip("no_outstanding_tasks", 0, None, 0);
            return;
        }
        let remaining = settings.after.saturating_sub(started.elapsed());
        if self.wait_for_busy(remaining).await {
            self.log_idle_wait_skip("session_busy_during_wait", 0, None, outstanding);
            return;
        }
        // A turn that started and ended between poll ticks still changes the
        // conversation; do not compact a context this task did not observe.
        if self
            .compaction
            .count
            .load(std::sync::atomic::Ordering::Relaxed)
            != count_at_start
            || self.chat_state_handle.get_conversation_len().await != len_at_start
        {
            self.log_idle_wait_skip("conversation_changed_during_wait", 0, None, outstanding);
            return;
        }
        let outstanding = self.outstanding_wait_tasks().await;
        if outstanding == 0 {
            self.log_idle_wait_skip("tasks_finished_during_wait", 0, None, 0);
            return;
        }
        let idle_now = {
            let state = self.state.lock().await;
            is_session_idle_for_injection(&state)
        };
        if !idle_now {
            self.log_idle_wait_skip("session_busy_at_fire", 0, None, outstanding);
            return;
        }
        if self
            .memory
            .is_flushing
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.log_idle_wait_skip("memory_flush_in_flight", 0, None, outstanding);
            return;
        }
        // Over the normal auto-compact line: the inline path would fire on the
        // next request anyway, so do it now while the session is idle.
        if let Some(trigger_info) = self.check_auto_compact_needed_inner().await {
            if !self.try_claim_idle_compact() {
                return;
            }
            if !self.is_idle_now().await
                || self.compaction.is_suppressed()
                || self
                    .compaction
                    .count
                    .load(std::sync::atomic::Ordering::Relaxed)
                    != count_at_start
                || self.chat_state_handle.get_conversation_len().await != len_at_start
            {
                self.clear_idle_compact_in_flight();
                return;
            }
            xai_grok_telemetry::unified_log::info(
                "shell.idle_wait_compact.fired",
                Some(self.session_info.id.0.as_ref()),
                Some(serde_json::json!({
                    "reason": "over_threshold",
                    "tokens_used": trigger_info.tokens_used,
                    "context_window": trigger_info.context_window,
                    "percentage": trigger_info.percentage,
                    "outstanding_tasks": outstanding,
                })),
            );
            self.run_idle_compact_select(trigger_info).await;
            return;
        }
        let pre_tokens = self.chat_state_handle.get_estimated_total_tokens().await;
        let Some(context_window) = self
            .chat_state_handle
            .get_sampling_config()
            .await
            .map(|config| config.context_window.get())
        else {
            self.log_idle_wait_skip("no_context_window", pre_tokens, None, outstanding);
            return;
        };
        if let Some(last) = self.compaction.last_idle_wait_compact_at.get() {
            if last.elapsed() < settings.cooldown {
                self.log_idle_wait_skip("cooldown", pre_tokens, None, outstanding);
                return;
            }
            let last_tokens = self.compaction.last_idle_wait_compact_tokens.get();
            if pre_tokens < last_tokens.saturating_add(settings.min_growth_tokens) {
                self.log_idle_wait_skip("insufficient_growth", pre_tokens, None, outstanding);
                return;
            }
        }
        let last_post = self.compaction.last_compact_post_tokens.get();
        let last_post = (last_post > 0).then_some(last_post);
        let plan = match plan_idle_wait_compact(
            pre_tokens,
            last_post,
            settings.expected_future_calls,
            settings.min_prompt_tokens,
            settings.min_net_tokens,
        ) {
            Ok(plan) => plan,
            Err(reason) => {
                self.log_idle_wait_skip(reason, pre_tokens, last_post, outstanding);
                return;
            }
        };
        let trigger_info = crate::session::acp_session::compaction::AutoCompactTriggerInfo {
            tokens_used: pre_tokens,
            context_window,
            percentage: xai_token_estimation::usage_percentage_u8(pre_tokens, context_window),
            reason_override: Some("Idle wait: compacting before the next wake-up"),
        };
        if !self.try_claim_idle_compact() {
            self.log_idle_wait_skip("compact_in_flight", pre_tokens, last_post, outstanding);
            return;
        }
        if !self.is_idle_now().await
            || self.compaction.is_suppressed()
            || self
                .compaction
                .count
                .load(std::sync::atomic::Ordering::Relaxed)
                != count_at_start
            || self.chat_state_handle.get_conversation_len().await != len_at_start
        {
            self.clear_idle_compact_in_flight();
            self.log_idle_wait_skip(
                "conversation_changed_before_fire",
                pre_tokens,
                last_post,
                outstanding,
            );
            return;
        }
        xai_grok_telemetry::unified_log::info(
            "shell.idle_wait_compact.fired",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "reason": "positive_net_benefit",
                "pre_tokens": plan.pre_tokens,
                "post_tokens_est": plan.post_tokens,
                "expected_future_calls": plan.expected_future_calls,
                "benefit_tokens": plan.benefit_tokens,
                "cost_tokens": plan.cost_tokens,
                "net_tokens": plan.net_tokens,
                "outstanding_tasks": outstanding,
                "idle_secs": started.elapsed().as_secs(),
            })),
        );
        let succeeded = tokio::select! {
            result = self.run_compact_only(trigger_info, false) => {
                if let Err(e) = result {
                    tracing::warn!(error = %e, "idle-wait auto-compaction failed");
                    false
                } else {
                    true
                }
            }
            _ = self.wait_until_busy() => {
                self.emit_idle_compact_cancelled().await;
                false
            }
        };
        if succeeded {
            self.compaction
                .last_idle_wait_compact_at
                .set(Some(std::time::Instant::now()));
            self.compaction
                .last_idle_wait_compact_tokens
                .set(pre_tokens);
        }
        self.clear_idle_compact_in_flight();
    }

    async fn run_idle_compact_select(
        self: &Arc<Self>,
        trigger_info: crate::session::acp_session::compaction::AutoCompactTriggerInfo,
    ) {
        // Run the compact, but abort it the moment the session stops being idle
        // (user prompt promoted, synthetic turn, finalization). Dropping the
        // future unwinds the cancel scope; nothing partial is applied.
        tokio::select! {
            result = self.run_compact_only(trigger_info, false) => {
                self.clear_idle_compact_in_flight();
                if let Err(e) = result {
                    tracing::warn!(error = %e, "idle auto-compaction failed");
                }
            }
            _ = self.wait_until_busy() => {
                self.clear_idle_compact_in_flight();
                self.emit_idle_compact_cancelled().await;
            }
        }
    }

    async fn outstanding_wait_tasks(&self) -> usize {
        self.tool_bridge_handle()
            .list_background_tasks()
            .await
            .into_iter()
            .filter(xai_grok_tools::computer::types::TaskSnapshot::is_outstanding)
            .count()
    }

    fn log_idle_wait_skip(
        &self,
        reason: &str,
        pre_tokens: u64,
        last_post_tokens: Option<u64>,
        outstanding_tasks: usize,
    ) {
        xai_grok_telemetry::unified_log::info(
            "shell.idle_wait_compact.skipped",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "reason": reason,
                "pre_tokens": pre_tokens,
                "last_post_tokens": last_post_tokens,
                "outstanding_tasks": outstanding_tasks,
            })),
        );
    }

    /// Poll `state_is_busy` for up to `timeout`; `true` when the session turned
    /// busy within the window.
    async fn wait_for_busy(&self, timeout: std::time::Duration) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            {
                let state = self.state.lock().await;
                if state_is_busy(&state) {
                    return true;
                }
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return false;
            }
            tokio::time::sleep((deadline - now).min(IDLE_COMPACT_BUSY_POLL)).await;
        }
    }

    /// Poll until the session is no longer idle (never returns false).
    async fn wait_until_busy(&self) {
        loop {
            {
                let state = self.state.lock().await;
                if state_is_busy(&state) {
                    return;
                }
            }
            tokio::time::sleep(IDLE_COMPACT_BUSY_POLL).await;
        }
    }

    fn try_claim_idle_compact(&self) -> bool {
        self.compaction
            .idle_compact_in_flight
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
    }

    async fn is_idle_now(&self) -> bool {
        let state = self.state.lock().await;
        is_session_idle_for_injection(&state)
    }

    fn clear_idle_compact_in_flight(&self) {
        self.compaction
            .idle_compact_in_flight
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// The compact future was dropped because the session turned busy; clear the
    /// compaction activity the `AutoCompactStarted` notice armed. The pager does
    /// not learn the difference between "cancelled by user stop" and "cancelled
    /// by a new prompt" — both are user-initiated activity.
    async fn emit_idle_compact_cancelled(&self) {
        use crate::extensions::notification::{
            AutoCompactCancelReason, SessionUpdate as XaiSessionUpdate,
        };
        self.send_xai_notification(XaiSessionUpdate::AutoCompactCancelled {
            reason: AutoCompactCancelReason::UserCancelled,
        })
        .await;
    }
}

#[cfg(test)]
mod idle_wait_plan_tests {
    use super::*;

    #[test]
    fn large_context_with_future_calls_is_positive() {
        let plan = plan_idle_wait_compact(200_000, Some(10_000), 4, 100_000, 20_000)
            .expect("large context should pass");
        assert_eq!(plan.post_tokens, 10_000);
        assert_eq!(plan.benefit_tokens, 760_000);
        assert_eq!(plan.cost_tokens, 360_000);
        assert_eq!(plan.net_tokens, 400_000);
    }

    #[test]
    fn one_future_call_does_not_repay_the_compaction() {
        assert_eq!(
            plan_idle_wait_compact(200_000, Some(10_000), 1, 100_000, 20_000),
            Err("negative_net_benefit")
        );
    }

    #[test]
    fn below_prompt_floor_is_skipped() {
        assert_eq!(
            plan_idle_wait_compact(50_000, Some(5_000), 8, 100_000, 20_000),
            Err("below_min_prompt_tokens")
        );
    }

    #[test]
    fn missing_post_estimate_uses_conservative_fallback() {
        let plan = plan_idle_wait_compact(200_000, None, 4, 100_000, 20_000)
            .expect("fallback estimate should pass");
        assert_eq!(plan.post_tokens, 20_000);
    }

    #[test]
    fn stale_post_estimate_is_ignored() {
        let plan = plan_idle_wait_compact(100_000, Some(150_000), 4, 100_000, 20_000)
            .expect("stale post estimate should fall back");
        assert_eq!(plan.post_tokens, 10_000);
    }
}
