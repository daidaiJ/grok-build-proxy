//! LOCAL (experimental) idle-time auto-compact for `SessionActor`.
//!
//! Fires once per completed turn when the session settles idle: if the context
//! usage is already past the auto-compact threshold, run the same compaction
//! the inline pre-sampling check would run — but now, off the critical path, so
//! the user's next prompt does not pay the summarization latency.
//!
//! Opt-in via `[features] idle_auto_compact = true` (or `GROK_IDLE_AUTO_COMPACT`);
//! default off. The inline pre-sampling check stays as the backstop and is
//! suppressed while an idle compact is in flight
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

impl SessionActor {
    /// Entry point spawned from the run loop at the post-turn idle settle point.
    /// `enabled` is resolved by the caller (feature flag + env), keeping this
    /// body testable without touching process env.
    pub(crate) async fn maybe_run_idle_auto_compact(self: Arc<Self>, enabled: bool) {
        Self::maybe_run_idle_auto_compact_inner(self, enabled, IDLE_COMPACT_GRACE).await
    }

    /// Testable core: `grace` is injectable so tests don't wait out the
    /// production debounce.
    pub(crate) async fn maybe_run_idle_auto_compact_inner(
        self: Arc<Self>,
        enabled: bool,
        grace: std::time::Duration,
    ) {
        if !enabled || self.startup_hints.is_subagent {
            return;
        }
        if self.compaction.is_suppressed() {
            return;
        }
        self.compaction
            .idle_compact_in_flight
            .store(true, std::sync::atomic::Ordering::Relaxed);
        // Claim the slot first, then verify we are actually idle; if not, release.
        let busy_now = {
            let state = self.state.lock().await;
            !is_session_idle_for_injection(&state)
        };
        if busy_now {
            self.clear_idle_compact_in_flight();
            return;
        }
        // Grace wait: a queued follow-up outranks a speculative compact.
        if self.wait_for_busy(grace).await {
            xai_grok_telemetry::unified_log::info(
                "shell.idle_compact.aborted",
                Some(self.session_info.id.0.as_ref()),
                Some(serde_json::json!({ "reason": "session_busy_during_grace" })),
            );
            self.clear_idle_compact_in_flight();
            return;
        }
        let Some(trigger_info) = self.check_auto_compact_needed_inner().await else {
            xai_grok_telemetry::unified_log::debug(
                "shell.idle_compact.skipped",
                Some(self.session_info.id.0.as_ref()),
                Some(serde_json::json!({ "reason": "below_threshold_or_gated" })),
            );
            self.clear_idle_compact_in_flight();
            return;
        };
        xai_grok_telemetry::unified_log::info(
            "shell.idle_compact.fired",
            Some(self.session_info.id.0.as_ref()),
            Some(serde_json::json!({
                "tokens_used": trigger_info.tokens_used,
                "context_window": trigger_info.context_window,
                "percentage": trigger_info.percentage,
            })),
        );
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
        use crate::extensions::notification::{AutoCompactCancelReason, SessionUpdate as XaiSessionUpdate};
        self.send_xai_notification(XaiSessionUpdate::AutoCompactCancelled {
            reason: AutoCompactCancelReason::UserCancelled,
        })
        .await;
    }
}
