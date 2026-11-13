//! Mutation handlers for the ChatStateActor.

use std::collections::HashMap;

use xai_grok_sampling_types::{
    ContentPart, ConversationItem, DanglingToolCallReason, SyntheticReason, SystemItem,
    dedup_duplicate_tool_results, repair_dangling_tool_calls_with, synthetic_dangling_result_text,
};

use super::ChatStateActor;
use super::request_builder::{HARD_CLEAR_PLACEHOLDER, append_prune_marker};
use crate::events::ChatStateEvent;
use crate::types::{ChatStateSnapshot, CompressFastOutcome, CompressFastSkip};

/// Static string label for tracing on `ConversationItem` (avoids pulling
/// the `Role` enum into the format string).
fn item_kind_str(item: &ConversationItem) -> &'static str {
    match item {
        ConversationItem::System(_) => "system",
        ConversationItem::User(_) => "user",
        ConversationItem::Assistant(_) => "assistant",
        ConversationItem::ToolResult(_) => "tool_result",
        ConversationItem::BackendToolCall(_) => "backend_tool_call",
        ConversationItem::Reasoning(_) => "reasoning",
    }
}

/// Derived-state matrix for in-place history rewrites: each kind picks turn-capture
/// handling and persistence flavor here. Item-count-changing rewrites use
/// [`ChatStateActor::replace_conversation`], which also reseeds token totals.
#[derive(Clone, Copy, Debug)]
pub(super) enum HistoryRewrite {
    /// Dedup / dangling-tool-call repair: may add or remove items ahead of
    /// an active capture's boundary → snapshot + rebase. Token totals
    /// untouched.
    IntegrityRepair,
    /// Old tool-result hard-clear: content shrinks, item count and ordering
    /// unchanged → capture offsets stay valid. Token totals untouched.
    RetainedPrune,
    /// Server-confirmed image strip: parts replaced in place
    /// (`strip_images_by_url`'s invariant), token totals untouched so the
    /// provider-reported total survives. Backup-gated, disk-acked persist.
    ImageStrip,
}

impl ChatStateActor {
    /// Apply `mutate` and drive the derived-state matrix for `kind`.
    /// Persists only when `mutate` reports a nonzero change count.
    /// Returns that count plus, for the strip flavor, the disk acknowledgement.
    pub(super) fn rewrite_history(
        &mut self,
        kind: HistoryRewrite,
        mutate: impl FnOnce(&mut Vec<ConversationItem>) -> usize,
    ) -> (
        usize,
        Option<tokio::sync::oneshot::Receiver<std::io::Result<()>>>,
    ) {
        let snapshots_capture = matches!(kind, HistoryRewrite::IntegrityRepair);
        if snapshots_capture {
            self.snapshot_turn_slice();
        }
        let changed = mutate(&mut self.state.conversation);
        let mut disk_ack = None;
        if changed > 0 {
            match kind {
                HistoryRewrite::IntegrityRepair | HistoryRewrite::RetainedPrune => {
                    self.persistence.replace_history(&self.state.conversation);
                }
                HistoryRewrite::ImageStrip => {
                    disk_ack = Some(
                        self.persistence
                            .replace_history_for_strip_and_ack(&self.state.conversation),
                    );
                }
            }
        }
        if snapshots_capture {
            self.rebase_turn_capture_offset();
        }
        (changed, disk_ack)
    }

    /// Repair dangling tool calls (assistant call IDs with no `ToolResult`) and persist.
    /// Idempotent. Only call at write boundaries where the previous turn is over.
    /// Do not call from read handlers — concurrent tool execution would look dangling.
    pub(super) fn ensure_conversation_integrity(&mut self) {
        self.ensure_conversation_integrity_with_reason(DanglingToolCallReason::UserCancelled);
    }

    /// Like [`Self::ensure_conversation_integrity`] but takes an explicit reason.
    pub(super) fn ensure_conversation_integrity_with_reason(
        &mut self,
        reason: DanglingToolCallReason,
    ) {
        self.ensure_conversation_integrity_answering(reason, HashMap::new());
    }

    fn ensure_conversation_integrity_answering(
        &mut self,
        reason: DanglingToolCallReason,
        mut answers: HashMap<String, String>,
    ) {
        self.rewrite_history(HistoryRewrite::IntegrityRepair, |conversation| {
            let deduped = dedup_duplicate_tool_results(conversation);
            if deduped > 0 {
                tracing::info!(
                    deduped_count = deduped,
                    "Removed duplicate tool results in conversation"
                );
            }
            let repaired = repair_dangling_tool_calls_with(conversation, |id, name| {
                answers
                    .remove(id)
                    .unwrap_or_else(|| synthetic_dangling_result_text(name, reason))
            });
            if repaired > 0 || deduped > 0 {
                tracing::info!(
                    repaired_count = repaired,
                    "Repaired dangling tool calls in conversation"
                );
            }
            repaired + deduped
        });
        if !answers.is_empty() {
            tracing::debug!(
                leftover = answers.len(),
                "answers for tool calls that already had a result were not written"
            );
        }
    }

    /// Repair dangling tool calls after a harness-initiated halt so the on-disk tail is clean.
    /// The next push repairs the same way lazily as a backstop.
    pub(super) fn repair_dangling_after_harness_halt(
        &mut self,
        class: &'static str,
        answers: HashMap<String, String>,
    ) {
        self.pop_stranded_continue_reminder();
        self.ensure_conversation_integrity_answering(
            DanglingToolCallReason::HarnessHalted { class },
            answers,
        );
    }

    /// Drop a trailing continue reminder whose continuation will never sample.
    /// Only where the turn is known dead — a live reminder must survive integrity repair.
    /// `IntegrityRepair` leaves token totals untouched, so the reminder's estimate lingers until reseed.
    pub(super) fn pop_stranded_continue_reminder(&mut self) {
        if !matches!(
            self.state.conversation.last(),
            Some(xai_grok_sampling_types::ConversationItem::User(u))
                if u.synthetic_reason
                    == xai_grok_sampling_types::SyntheticReason::LengthContinue
        ) {
            return;
        }
        self.rewrite_history(HistoryRewrite::IntegrityRepair, |conversation| {
            let mut stranded = 0;
            while matches!(
                conversation.last(),
                Some(xai_grok_sampling_types::ConversationItem::User(u))
                    if u.synthetic_reason
                        == xai_grok_sampling_types::SyntheticReason::LengthContinue
            ) {
                conversation.pop();
                stranded += 1;
            }
            tracing::info!(stranded, "Dropped stranded length-continue reminder");
            stranded
        });
    }

    /// Out-of-band history repair: run [`crate::compaction_utils::repair_history`] and persist.
    /// Unlike integrity repair, this also removes orphaned `ToolResult`s that brick sessions with 400s.
    /// `dry_run` only reports.
    pub(super) fn repair_history(
        &mut self,
        dry_run: bool,
    ) -> crate::compaction_utils::HistoryRepairReport {
        if dry_run {
            let mut copy = self.state.conversation.clone();
            return crate::compaction_utils::repair_history(&mut copy);
        }
        let mut items = std::mem::take(&mut self.state.conversation);
        let report = crate::compaction_utils::repair_history(&mut items);
        if report.changed() {
            tracing::warn!(
                duplicates_removed = report.duplicates_removed,
                stripped_tool_result_ids = ?report.stripped_tool_result_ids,
                synthetic_results_inserted = report.synthetic_results_inserted,
                "History repair modified the conversation"
            );
            // Full replace: persists atomically and re-bases token estimates.
            self.replace_conversation(items, false);
        } else {
            // Nothing changed — put the conversation back untouched.
            self.state.conversation = items;
        }
        report
    }

    /// Same URL-scoped strip as the request's, as [`HistoryRewrite::ImageStrip`].
    /// `None` when nothing matched, else occurrence count + disk ack.
    pub(super) fn strip_conversation_images(
        &mut self,
        urls: &[std::sync::Arc<str>],
    ) -> Option<(usize, tokio::sync::oneshot::Receiver<std::io::Result<()>>)> {
        let (stripped, disk_ack) =
            self.rewrite_history(HistoryRewrite::ImageStrip, |conversation| {
                let stripped = xai_grok_sampling_types::strip_images_by_url(conversation, urls);
                if stripped > 0 {
                    tracing::warn!(
                        stripped,
                        "stripped server-rejected image(s) from stored conversation"
                    );
                }
                stripped
            });
        disk_ack.map(|ack| (stripped, ack))
    }

    /// Make memory match the disk-authoritative switch for one generation.
    pub(super) fn converge_working_directory_switch(
        &mut self,
        generation: u64,
        authoritative: ConversationItem,
    ) {
        let existing = self
            .state
            .conversation
            .iter_mut()
            .find(|item| item.working_directory_switch_generation() == Some(generation));
        if let Some(existing) = existing {
            let old_tokens = super::state::estimate_item_tokens(existing);
            let new_tokens = super::state::estimate_item_tokens(&authoritative);
            self.state.estimated_tokens_since_model = if new_tokens >= old_tokens {
                self.state
                    .estimated_tokens_since_model
                    .saturating_add(new_tokens - old_tokens)
            } else {
                self.state
                    .estimated_tokens_since_model
                    .saturating_sub(old_tokens - new_tokens)
            };
            *existing = authoritative;
        } else {
            self.state.estimated_tokens_since_model +=
                super::state::estimate_item_tokens(&authoritative);
            self.state.conversation.push(authoritative);
        }
    }

    /// Push any conversation item (user, assistant, or tool result) and persist it.
    pub(super) fn push_message(&mut self, item: ConversationItem) {
        let count_in_delta = !matches!(item, ConversationItem::Assistant(_));
        if count_in_delta {
            let estimated_tokens = super::state::estimate_item_tokens(&item);
            self.state.estimated_tokens_since_model += estimated_tokens;
            tracing::debug!(
                item_kind = item_kind_str(&item),
                estimated_tokens_delta = estimated_tokens,
                estimated_total = self.state.total_tokens + self.state.estimated_tokens_since_model,
                model_reported_total = self.state.total_tokens,
                "ChatState: push_message updated estimated_tokens_since_model"
            );
        }
        self.persist_and_push_message(item);
    }

    /// Persist model output already included in the provider's usage total.
    pub(super) fn push_model_output(&mut self, item: ConversationItem) {
        self.persist_and_push_message(item);
    }

    /// Persist model output whose provider response omitted usage.
    pub(super) fn push_unreported_model_output(&mut self, item: ConversationItem) {
        self.state.estimated_tokens_since_model += super::state::estimate_item_tokens(&item);
        self.persist_and_push_message(item);
    }

    fn persist_and_push_message(&mut self, item: ConversationItem) {
        self.persistence.persist_message(&item);
        self.state.conversation.push(item);
    }

    /// Push a user message, repairing dangling tool calls first so cancel/crash tails stay consistent.
    /// Also runs [`prune_retained_conversation`] to hard-clear very old tool results in memory.
    pub(super) fn push_user_message(&mut self, item: ConversationItem) {
        self.push_user_message_with_repair_reason(item, DanglingToolCallReason::UserCancelled);
    }

    /// Like [`Self::push_user_message`] but takes an explicit repair reason.
    pub(super) fn push_user_message_with_repair_reason(
        &mut self,
        item: ConversationItem,
        reason: DanglingToolCallReason,
    ) {
        // A turn-starting or mid-turn user item landing on a trailing continue reminder
        // means the continuation is dead — the stranded reminder must not sit before the new instruction.
        // A live reminder is never trailing here: its continuation is still in flight.
        {
            use xai_grok_sampling_types::SyntheticReason as R;
            if matches!(
                &item,
                ConversationItem::User(u)
                    if u.synthetic_reason.starts_prompt_turn()
                        || matches!(
                            u.synthetic_reason,
                            R::AutoRecovery
                                | R::StopHookFeedback
                                | R::GoalSummary
                                | R::WorkingDirectorySwitch
                                | R::Interjection
                        )
            ) {
                self.pop_stranded_continue_reminder();
            }
        }
        self.ensure_conversation_integrity_with_reason(reason);
        let estimated_tokens = super::state::estimate_item_tokens(&item);
        self.state.estimated_tokens_since_model += estimated_tokens;
        tracing::debug!(
            item_kind = item_kind_str(&item),
            estimated_tokens_delta = estimated_tokens,
            estimated_total = self.state.total_tokens + self.state.estimated_tokens_since_model,
            model_reported_total = self.state.total_tokens,
            "ChatState: push_user_message updated estimated_tokens_since_model"
        );
        self.persistence.persist_message(&item);
        self.state.conversation.push(item);
        self.prune_retained_conversation();
    }

    /// Eagerly hard-clear tool results from very old turns in the retained conversation.
    /// Unlike API-copy pruning, this mutates `self.state.conversation` after every user turn.
    /// `updates.jsonl` is never touched, so replay stays intact; synthetic User items raise the age threshold.
    pub(super) fn prune_retained_conversation(&mut self) -> usize {
        if !self.pruning_config.enabled {
            return 0;
        }
        // LOCAL(cache-port): under `prune_mode = "append"` the retained
        // conversation must keep original tool-result bytes — the request-layer
        // markers reference them, so any in-place clear here would defeat the
        // mode (and `updates.jsonl` semantics stay untouched either way).
        if self.state.sampling_config.prune_mode == xai_grok_sampling_types::PruneMode::Append {
            return 0;
        }
        // Fast exit: not enough turns have elapsed for any hard-clear to apply.
        if self.state.prompt_index < self.pruning_config.hard_clear_age_turns {
            return 0;
        }

        let keep_turns = self.hard_clear_keep_turns(self.pruning_config.hard_clear_age_turns);
        let before_bytes = self.conversation_content_bytes();
        let cleared = self.clear_tool_results_older_than(keep_turns);

        if cleared > 0 {
            let after_bytes = self.conversation_content_bytes();
            tracing::debug!(
                hard_cleared = cleared,
                bytes_freed = before_bytes.saturating_sub(after_bytes),
                conversation_len = self.state.conversation.len(),
                "ChatState: in-memory tool-result prune"
            );
        }

        cleared
    }

    /// Number of most recent user turns whose tool results stay verbatim in the retained
    /// conversation. Synthetic User items are not real turns (they do not increment
    /// `prompt_index`), so their count is added back to keep the threshold measured in
    /// real turns.
    fn hard_clear_keep_turns(&self, keep_turns: usize) -> usize {
        let total_user_items = self
            .state
            .conversation
            .iter()
            .filter(|i| matches!(i, ConversationItem::User(_)))
            .count();
        let synthetic_count = total_user_items.saturating_sub(self.state.prompt_index);
        keep_turns.saturating_add(synthetic_count)
    }

    /// Hard-clear tool results outside the `keep_turns` most recent user turns.
    /// Persistence goes through `rewrite_history`, so a resumed session sees the
    /// cleared bodies. Callers gate on `prune_mode` themselves.
    fn clear_tool_results_older_than(&mut self, keep_turns: usize) -> usize {
        let (cleared, _) = self.rewrite_history(HistoryRewrite::RetainedPrune, |conversation| {
            clear_old_tool_results(conversation, keep_turns)
        });
        cleared
    }

    /// LOCAL(compress-fast): manual zero-model-call cleanup.
    ///
    /// Clears tool results outside the `keep_last_n_turns` most recent user turns, even
    /// when automatic pruning is disabled — the user asked for it explicitly. Under
    /// `prune_mode = "append"` each cleared result also gets the append-mode invalidation
    /// marker written into history, so the wire never carries a bare omission and the
    /// marker text is byte-stable across requests and resume. That rewrite costs one
    /// prefix-cache break at the cleared positions, which is what the manual action buys.
    pub(super) fn compress_fast(&mut self, keep_last_n_turns: usize) -> CompressFastOutcome {
        let append_mode =
            self.state.sampling_config.prune_mode == xai_grok_sampling_types::PruneMode::Append;

        let keep_turns = self.hard_clear_keep_turns(keep_last_n_turns);
        let before_bytes = self.conversation_content_bytes();
        let (cleared, _) = self.rewrite_history(HistoryRewrite::RetainedPrune, |conversation| {
            let cleared = clear_old_tool_results(conversation, keep_turns);
            if append_mode {
                annotate_cleared_results(conversation);
            }
            cleared
        });

        if cleared == 0 {
            return CompressFastOutcome {
                skipped: Some(CompressFastSkip::NothingToClear),
                ..Default::default()
            };
        }
        let bytes_freed = before_bytes.saturating_sub(self.conversation_content_bytes()) as u64;
        tracing::info!(
            cleared,
            bytes_freed,
            keep_last_n_turns,
            append_mode,
            "ChatState: compress-fast cleared old tool results"
        );
        CompressFastOutcome {
            cleared_tool_results: cleared,
            bytes_freed,
            skipped: None,
        }
    }

    /// Approximate byte footprint of all string content in the conversation.
    /// Used for before/after measurement logging when pruning runs.
    fn conversation_content_bytes(&self) -> usize {
        self.state
            .conversation
            .iter()
            .map(|item| match item {
                ConversationItem::System(s) => s.content.len(),
                ConversationItem::User(u) => u
                    .content
                    .iter()
                    .map(|p| match p {
                        ContentPart::Text { text } => text.len(),
                        ContentPart::Image { url } => url.len(),
                    })
                    .sum::<usize>(),
                ConversationItem::Assistant(a) => a.content.len(),
                ConversationItem::ToolResult(tr) => tr.content.len(),
                ConversationItem::BackendToolCall(b) => b.text_summary().len(),
                ConversationItem::Reasoning(r) => {
                    xai_grok_sampling_types::reasoning_item_text(r).len()
                        + r.encrypted_content.as_deref().map(str::len).unwrap_or(0)
                }
            })
            .sum()
    }

    /// Record accumulated token usage and emit an event.
    pub(super) fn record_token_usage(&mut self, total_tokens: u64) {
        self.state.estimated_tokens_since_model = 0;
        self.state.estimate_at_last_response =
            super::state::estimate_conversation_tokens(&self.state.conversation);
        self.state.total_tokens = total_tokens;
        self.send_event(ChatStateEvent::TokensUpdated { total_tokens });
    }

    /// Stash the per-turn `TokenUsage` from the most recent model response.
    /// No event is emitted — this slot is read on demand at `PromptResponse`
    /// construction time, not pushed to subscribers.
    pub(super) fn record_last_turn_usage(&mut self, usage: xai_grok_sampling_types::TokenUsage) {
        self.state.last_turn_usage = Some(usage);
    }

    pub(super) fn record_model_call_usage(
        &mut self,
        model_id: Option<String>,
        usage: &xai_grok_sampling_types::TokenUsage,
        api_duration_ms: Option<u64>,
        cost_usd_ticks: Option<i64>,
    ) {
        // LOCAL: remember which wire id the provider echoes for the configured
        // model. A terminal failure has no echo of its own and reuses it, so ✓
        // and ✗ stay on one ledger key even when the config-side spelling differs.
        if let Some(id) = model_id.as_deref().filter(|id| !id.is_empty()) {
            self.state.last_echoed_model = Some((
                self.state.sampling_config.model.clone(),
                id.to_owned(),
            ));
        }
        let model_key = match model_id.as_deref() {
            Some(id) if !id.is_empty() => id,
            _ => self.state.sampling_config.model.as_str(),
        }
        .to_owned();
        self.state
            .prompt_usage
            .get_or_insert_default()
            .record_main_loop_call(&model_key, usage, api_duration_ms, cost_usd_ticks);
        self.state.session_usage.record_main_loop_call(
            &model_key,
            usage,
            api_duration_ms,
            cost_usd_ticks,
        );
    }

    /// LOCAL: record one terminal model-call failure (endpoint-health counter; no token impact).
    /// Session ledger only — the status line reads cumulative session health, per-prompt bills stay token/cost shaped.
    pub(super) fn record_model_call_failure(&mut self, model_id: Option<String>) {
        let model_key = match model_id.as_deref() {
            Some(id) if !id.is_empty() => id.to_owned(),
            // LOCAL: book the failure under the id the provider echoed for the model
            // still configured; the config-side spelling would open a second,
            // success-less key that then shadows the model's real totals.
            _ => self
                .state
                .last_echoed_model
                .as_ref()
                .filter(|(configured, _)| *configured == self.state.sampling_config.model)
                .map(|(_, echoed)| echoed.clone())
                .unwrap_or_else(|| self.state.sampling_config.model.clone()),
        };
        self.state
            .session_usage
            .record_main_loop_failure(&model_key);
    }

    pub(super) fn record_subagent_usage(
        &mut self,
        by_model: &[(String, crate::usage::UsageTotals)],
        attribute_to_prompt: bool,
        incomplete: bool,
    ) {
        if by_model.is_empty() && !incomplete {
            return;
        }
        if attribute_to_prompt {
            self.state
                .prompt_usage
                .get_or_insert_default()
                .record_subagent(by_model, incomplete);
        }
        // The session ledger always folds, even when usage is not attributable to the open prompt.
        // Reporting that gap is the coordinator's sticky flag — never mark a different live prompt's ledger.
        self.state
            .session_usage
            .record_subagent(by_model, incomplete);
    }

    pub(super) fn mark_usage_incomplete(&mut self, prompt: bool, session: bool) {
        if prompt {
            self.state
                .prompt_usage
                .get_or_insert_default()
                .mark_incomplete();
        }
        if session {
            self.state.session_usage.mark_incomplete();
        }
    }

    pub(super) fn increment_prompt_index(&mut self) {
        self.state.prompt_usage = None;
        self.state.prompt_index += 1;
        self.send_event(ChatStateEvent::PromptIndexChanged {
            new_index: self.state.prompt_index,
        });
    }

    /// Reseed `total_tokens` after a conversation rewrite.
    /// Scales `base_estimate` by the provider-confirmed ratio, capped at the pre-rewrite total.
    pub(super) fn reseed_total_tokens(&self, base_estimate: u64) -> u64 {
        let pre_replace_total = self.state.total_tokens;
        let mut estimated_tokens =
            if pre_replace_total > 0 && self.state.estimate_at_last_response > 0 {
                let ratio = pre_replace_total as f64 / self.state.estimate_at_last_response as f64;
                (base_estimate as f64 * ratio).round() as u64
            } else {
                base_estimate
            };
        // Over-counting fires premature compaction; an under-count self-heals
        // on the next model response.
        if pre_replace_total > 0 {
            estimated_tokens = estimated_tokens.min(pre_replace_total);
        }
        estimated_tokens
    }

    /// Replace the entire conversation, persist, re-estimate `total_tokens`, and emit reset events.
    /// `is_compaction` only marks the turn capture; the reseed math is the same for every rewrite.
    pub(super) fn replace_conversation(
        &mut self,
        items: Vec<ConversationItem>,
        is_compaction: bool,
    ) {
        self.snapshot_turn_slice();
        if is_compaction && let Some(cap) = &mut self.state.turn_capture {
            cap.compaction_occurred = true;
        }
        // `harness_trace_buffer` / `harness_trace_turns` intentionally untouched:
        // the planner/verifier subagents ran, so their sealed trace turns survive
        // a conversation replace (same intent as the `TruncateToPromptIndex` arm).
        self.persistence.replace_history(&items);
        let base_estimate = super::state::estimate_conversation_tokens(&items);
        let estimated_tokens = self.reseed_total_tokens(base_estimate);
        self.state.conversation = items;
        self.state.estimated_tokens_since_model = 0;
        self.state.total_tokens = estimated_tokens;
        self.state.estimate_at_last_response = base_estimate;
        self.rebase_turn_capture_offset();
        self.send_event(ChatStateEvent::ConversationReset {
            new_len: self.state.conversation.len(),
        });
        self.send_event(ChatStateEvent::TokensUpdated {
            total_tokens: estimated_tokens,
        });
    }

    /// Atomically swap the leading `System` message with `prompt` (or insert one).
    /// Runs in the actor loop so it serializes with turn pushes — no lost-update on mid-turn reconnect.
    /// Clone, do not `mem::take`: `replace_conversation` snapshots the turn-capture tail first.
    pub(super) fn replace_system_head(&mut self, prompt: &str) -> bool {
        if let Some(ConversationItem::System(sys)) = self.state.conversation.first()
            && crate::conversation_util::canonical_system_prompt_eq(sys.content.as_ref(), prompt)
        {
            return false;
        }
        let mut conversation = self.state.conversation.clone();
        let changed =
            crate::conversation_util::replace_or_insert_system_head(&mut conversation, prompt);
        debug_assert!(changed, "head mismatch must produce a change");
        self.replace_conversation(conversation, false);
        changed
    }

    /// Restore all state fields from a snapshot.
    pub(super) fn restore_snapshot(&mut self, snap: ChatStateSnapshot) {
        self.snapshot_turn_slice();
        // Harness trace buffers are transient (not part of the snapshot) and
        // intentionally survive a restore — see `replace_conversation`.
        self.state.conversation = snap.conversation;
        self.rebase_turn_capture_offset();
        self.state.sampling_config = snap.sampling_config;
        self.state.prompt_index = snap.prompt_index;
        self.state.total_tokens = snap.total_tokens;
        self.state.estimated_tokens_since_model = 0;
        self.state.estimate_at_last_response = if snap.estimate_at_last_response > 0 {
            snap.estimate_at_last_response
        } else {
            super::state::estimate_conversation_tokens(&self.state.conversation)
        };
        self.state.agent_edited_paths = snap.agent_edited_paths;
        self.state.prompt_texts = snap.prompt_texts;
        self.state.stream_start_ms = snap.stream_start_ms;
        self.state.turn_start_ms = snap.turn_start_ms;
        self.state.last_compaction_prompt_index = snap.last_compaction_prompt_index;
        self.state.credentials = snap.credentials;
        // Drop abandoned prompt billing; session ledger is lifetime.
        self.state.prompt_usage = None;
    }

    /// If turn capture is active, append the current turn's tail items into
    /// `pre_replacement_messages` before an in-place mutation shifts or drops them.
    pub(super) fn snapshot_turn_slice(&mut self) {
        if let Some(cap) = &mut self.state.turn_capture {
            cap.pre_replacement_messages
                .extend_from_slice(Self::turn_tail(
                    &self.state.conversation,
                    cap.turn_start_offset,
                ));
        }
    }

    /// Re-base an active turn capture's start offset to the current conversation
    /// length after an in-place mutation, keeping the tail slice valid.
    pub(super) fn rebase_turn_capture_offset(&mut self) {
        if let Some(cap) = &mut self.state.turn_capture {
            cap.turn_start_offset = self.state.conversation.len();
        }
    }

    /// Fail-safe `conversation[offset..]` for turn capture.
    /// An out-of-range offset yields an empty slice — a raw index here SIGABRT-crashed a live CLI.
    /// Loud in dev via `debug_assert!`, with a prod breadcrumb via `error!`.
    pub(super) fn turn_tail(
        conversation: &[ConversationItem],
        offset: usize,
    ) -> &[ConversationItem] {
        debug_assert!(
            offset <= conversation.len(),
            "turn_start_offset {offset} > len {}",
            conversation.len()
        );
        conversation.get(offset..).unwrap_or_else(|| {
            tracing::error!(
                offset,
                len = conversation.len(),
                "turn-capture offset past conversation end; trace tail dropped"
            );
            &[]
        })
    }
}

/// Clear tool-result bodies outside the `keep_turns` most recent user turns.
/// Turn index runs backwards from 0 at the newest user item, so `keep_turns = 2`
/// leaves the two most recent turns verbatim. Returns how many bodies changed.
fn clear_old_tool_results(conversation: &mut [ConversationItem], keep_turns: usize) -> usize {
    let mut cleared = 0usize;
    let mut turn_from_end = 0usize;
    for item in conversation.iter_mut().rev() {
        if matches!(item, ConversationItem::User(_)) {
            // Results older than this user item belong to one turn further back:
            // the newest block is index 0, the next older block index 1, and so on.
            turn_from_end += 1;
            continue;
        }
        let ConversationItem::ToolResult(tr) = item else {
            continue;
        };
        if turn_from_end < keep_turns {
            continue;
        }
        if tr.content.as_ref() != HARD_CLEAR_PLACEHOLDER {
            tr.content = std::sync::Arc::<str>::from(HARD_CLEAR_PLACEHOLDER);
            cleared += 1;
        }
    }
    cleared
}

/// LOCAL(compress-fast): append-mode invalidation markers, written into history at clear
/// time so the wire never carries a bare omission. Uses the same call-id-keyed text as
/// automatic append pruning, and skips results that already carry their marker so running
/// the command twice does not stack notices.
fn annotate_cleared_results(conversation: &mut Vec<ConversationItem>) {
    let mut idx = 0;
    while idx < conversation.len() {
        let marker = match &conversation[idx] {
            ConversationItem::ToolResult(tr) if tr.content.as_ref() == HARD_CLEAR_PLACEHOLDER => {
                Some(append_prune_marker(&tr.tool_call_id))
            }
            _ => None,
        };
        let Some(text) = marker else {
            idx += 1;
            continue;
        };
        let already = matches!(
            conversation.get(idx + 1),
            Some(ConversationItem::System(s)) if s.content.as_ref() == text.as_str()
        );
        if !already {
            conversation.insert(
                idx + 1,
                ConversationItem::System(SystemItem {
                    content: std::sync::Arc::<str>::from(text),
                    synthetic_reason: SyntheticReason::InHistoryUpdate,
                }),
            );
        }
        idx += 2;
    }
}

#[cfg(test)]
mod compress_fast_tests {
    use super::*;

    fn bodies(conversation: &[ConversationItem]) -> Vec<String> {
        conversation
            .iter()
            .filter_map(|item| match item {
                ConversationItem::ToolResult(tr) => Some(tr.content.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn keeps_the_two_most_recent_turns() {
        let mut conversation = vec![
            ConversationItem::user("q1"),
            ConversationItem::tool_result("c1", "old body"),
            ConversationItem::user("q2"),
            ConversationItem::tool_result("c2", "recent body"),
            ConversationItem::user("q3"),
            ConversationItem::tool_result("c3", "current body"),
        ];
        assert_eq!(clear_old_tool_results(&mut conversation, 2), 1);
        assert_eq!(
            bodies(&conversation),
            vec![
                HARD_CLEAR_PLACEHOLDER.to_string(),
                "recent body".to_string(),
                "current body".to_string(),
            ]
        );
    }

    #[test]
    fn clearing_is_idempotent_and_a_wider_window_touches_nothing() {
        let mut conversation = vec![
            ConversationItem::user("q1"),
            ConversationItem::tool_result("c1", "old body"),
        ];
        assert_eq!(clear_old_tool_results(&mut conversation, 1), 0);
        assert_eq!(clear_old_tool_results(&mut conversation, 0), 1);
        assert_eq!(clear_old_tool_results(&mut conversation, 0), 0);
    }

    #[test]
    fn append_mode_markers_follow_each_cleared_result() {
        let mut conversation = vec![
            ConversationItem::user("q1"),
            ConversationItem::tool_result("c1", "old body"),
            ConversationItem::user("q2"),
            ConversationItem::tool_result("c2", "recent body"),
            ConversationItem::user("q3"),
        ];
        assert_eq!(clear_old_tool_results(&mut conversation, 2), 1);
        annotate_cleared_results(&mut conversation);

        // q1 / c1 / marker / q2 / c2 / q3
        assert_eq!(conversation.len(), 6);
        assert!(matches!(conversation[1], ConversationItem::ToolResult(_)));
        let ConversationItem::System(marker) = &conversation[2] else {
            panic!("marker must follow the cleared result");
        };
        assert_eq!(marker.content.as_ref(), append_prune_marker("c1").as_str());
        assert_eq!(marker.synthetic_reason, SyntheticReason::InHistoryUpdate);

        // Running again must not stack a second notice.
        let before = conversation.len();
        annotate_cleared_results(&mut conversation);
        assert_eq!(conversation.len(), before);
    }
}
