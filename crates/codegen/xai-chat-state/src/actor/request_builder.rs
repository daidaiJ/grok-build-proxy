//! ConversationRequest assembly — image compaction, pruning, repair, memory injection.

use xai_grok_sampling_types::{
    ApiBackend, ConversationItem, ConversationRequest, PruneMode, SyntheticReason, SystemItem,
    SystemPromptUpdate, ToolSpec, TraceContext,
};

use super::ChatStateActor;
use crate::events::ChatStateEvent;
use crate::image_budget::{
    ImageBudgetOutcome, apply_image_budget_with_limits, image_budget_limits,
};
use crate::types::PruningConfig;

/// Placeholder inserted when a tool result is hard-cleared.
/// `pub(super)` so `mutations.rs` can use the same string on the retained conversation.
pub(super) const HARD_CLEAR_PLACEHOLDER: &str = "[Tool result omitted — too old]";

/// Separator inserted between head and tail in soft-trimmed results.
const SOFT_TRIM_SEPARATOR: &str = "\n\n[…trimmed…]\n\n";

impl ChatStateActor {
    /// Build a `ConversationRequest` from current actor state (image eviction, prune, memory reminder).
    /// The command handler already ran integrity repair on the actor conversation before this clone.
    /// Do not re-run dangling/dedup repair on the clone — those would be O(n) no-ops.
    pub(super) fn build_conversation_request(
        &mut self,
        tool_definitions: Vec<ToolSpec>,
        memory_reminder: Option<String>,
        persist_memory_reminder: bool,
        trace: Option<Box<dyn TraceContext>>,
        conv_id: String,
        req_id: String,
    ) -> ConversationRequest {
        let mut memory_reminder = memory_reminder;
        if let Some(reminder) = memory_reminder.as_deref()
            && persist_memory_reminder
        {
            // A live in-place inject can prepend a `System` item, shifting indices
            // under an active capture; snapshot + rebase like the other mutators.
            self.snapshot_turn_slice();
            let injected = inject_memory_reminder(&mut self.state.conversation, reminder);
            if injected {
                self.persistence.replace_history(&self.state.conversation);
                memory_reminder = None;
            }
            self.rebase_turn_capture_offset();
        }
        let (trigger_bytes, reclaim_target_bytes) =
            image_budget_limits(self.state.sampling_config.max_request_bytes);
        let budgeted = apply_image_budget_with_limits(
            self.state.conversation.clone(),
            trigger_bytes,
            reclaim_target_bytes,
        );
        let ImageBudgetOutcome {
            body_bytes,
            body_bytes_after,
            inline_images,
            needs_image_compaction,
            evicted,
        } = budgeted.outcome;
        let mut items = budgeted.items;
        if inline_images > 0 {
            self.send_event(ChatStateEvent::ImageBudget {
                body_bytes,
                trigger_bytes,
                reclaim_target_bytes,
                inline_images,
                needs_image_compaction,
                evicted,
                body_bytes_after,
            });
        }
        items = self.prune_items_for_turn_request(items);
        if let Some(reminder) = memory_reminder {
            inject_memory_reminder(&mut items, &reminder);
        }
        self.apply_in_history_system_placement(&mut items);
        items = crate::compaction_utils::ModelRequestHistory::from_raw(items).into_items();

        // Step 4: Assemble request
        ConversationRequest {
            items,
            tools: tool_definitions,
            hosted_tools: vec![],
            tool_choice: None,
            model: Some(self.state.sampling_config.model.clone()),
            temperature: self.state.sampling_config.temperature,
            max_output_tokens: self.state.sampling_config.max_completion_tokens,
            top_p: self.state.sampling_config.top_p,
            x_grok_conv_id: Some(conv_id),
            x_grok_req_id: Some(req_id),
            x_grok_session_id: None,
            x_grok_turn_idx: None,
            x_grok_transient_retry: None,
            x_grok_agent_id: None,
            x_grok_deployment_id: None,
            x_grok_user_id: None,
            trace,
            traceparent: None,
            prompt_cache_key: None,
            reasoning_effort: self.state.sampling_config.reasoning_effort,
            json_schema: None,
            // LOCAL (vision bridge D9): the shell attaches the reactive
            // transcription hook after this build; chat-state never sets one.
            image_bridge: None,
            // Execute completed tool calls on a Length-truncated turn instead
            // of failing it; text-only salvage stays behind `CompletePartial`.
            length_policy: xai_grok_sampling_types::LengthPolicy::CompleteToolCalls,
        }
    }

    pub(super) fn prune_items_for_turn_request(
        &self,
        mut items: Vec<ConversationItem>,
    ) -> Vec<ConversationItem> {
        if should_prune(
            self.state.total_tokens,
            self.state.sampling_config.context_window,
        ) {
            if self.state.sampling_config.prune_mode == PruneMode::Append
                && self.state.sampling_config.api_backend != ApiBackend::Messages
            {
                prune_conversation_append(&mut items, &self.pruning_config);
            } else {
                prune_conversation(&mut items, &self.pruning_config);
            }
        }
        items
    }

    /// LOCAL(cache-port): dsh-style in-history system prompt placement.
    ///
    /// Under `system_prompt_update = in-history`, a changed system prompt is
    /// NOT written into `messages[0]`: the frozen head bytes are restored and
    /// the new snapshot is appended after the history tail (which is by
    /// construction the last user/tool-result turn). The endpoint reads the
    /// latest system message as effective, and the cached prefix survives the
    /// change — only the appended block is new bytes.
    ///
    /// Wire-layer derived state: the base never enters the journal; reset
    /// points (compaction reseed, model switch) clear `in_history_base_system`
    /// so the next build freezes the current head again.
    pub(super) fn apply_in_history_system_placement(&mut self, items: &mut Vec<ConversationItem>) {
        if self.state.sampling_config.system_prompt_update != SystemPromptUpdate::InHistory {
            return;
        }
        // Probe gate (dsh-port §3.2): the messages backend folds System items
        // into the top-level `system` param, so a mid-history snapshot never
        // reaches its position — fall back to the legacy in-place shape there.
        if self.state.sampling_config.api_backend == ApiBackend::Messages {
            return;
        }
        apply_in_history_placement(items, &mut self.state.in_history_wire);
    }
}

/// Pure core of [`ChatStateActor::apply_in_history_system_placement`].
///
/// `wire` is the in-history wire state (see [`InHistoryWireState`]): the
/// system prompt frozen at `messages[0]` plus the latest appended snapshot.
/// Under in-history placement, a changed system prompt does NOT rewrite
/// `messages[0]`: the frozen head bytes are restored and the new snapshot is
/// appended after the history tail (which is by construction the last
/// user/tool-result turn). The endpoint reads the latest system message as
/// effective, and the cached prefix survives the change — only the appended
/// block is new bytes. Previously appended snapshots stay in place, so the
/// wire stays append-only across successive changes.
///
/// Wire-layer derived state: `wire` never enters the journal; reset points
/// (compaction reseed, model switch) clear it so the next build freezes the
/// current head again.
fn apply_in_history_placement(
    items: &mut Vec<ConversationItem>,
    wire: &mut Option<super::state::InHistoryWireState>,
) {
    let Some(ConversationItem::System(current)) = items.first() else {
        // Head system vanished (systemless config): drop any stale state.
        *wire = None;
        return;
    };
    let current = current.content.clone();
    match wire {
        None => {
            // First build (or post-reset): freeze the current head as the
            // wire base; the request keeps its natural shape.
            *wire = Some(super::state::InHistoryWireState {
                head: current.clone(),
                last_snapshot: current,
            });
        }
        Some(st) if st.last_snapshot == current => {
            // Unchanged since the last snapshot — nothing to do.
        }
        Some(st) => {
            items[0] = ConversationItem::System(SystemItem {
                content: st.head.clone(),
                synthetic_reason: SyntheticReason::Primary,
            });
            items.push(ConversationItem::System(SystemItem {
                content: current.clone(),
                synthetic_reason: SyntheticReason::InHistoryUpdate,
            }));
            st.last_snapshot = current;
        }
    }
}

// ============================================================================
// Pruning (standalone functions, no actor state needed)
// ============================================================================

/// Check whether pruning should run based on context utilization.
///
/// Returns `true` when `total_tokens` exceeds 50% of `context_window`.
pub(crate) fn should_prune(total_tokens: u64, context_window: std::num::NonZeroU64) -> bool {
    total_tokens > context_window.get() / 2
}

/// Prune old, large tool results from the conversation in place.
/// Turn age is estimated by walking backward and counting `User` items.
pub(crate) fn prune_conversation(conversation: &mut [ConversationItem], config: &PruningConfig) {
    if !config.enabled {
        return;
    }

    let mut turn_from_end: usize = 0;
    let mut seen_first_user = false;

    for item in conversation.iter_mut().rev() {
        if matches!(item, ConversationItem::User(_)) {
            if seen_first_user {
                turn_from_end += 1;
            }
            seen_first_user = true;
            continue;
        }

        let ConversationItem::ToolResult(tool_result) = item else {
            continue;
        };

        // Never prune recent turns.
        if turn_from_end < config.keep_last_n_turns {
            continue;
        }

        // Hard clear: very old tool results → replace entirely.
        if turn_from_end >= config.hard_clear_age_turns {
            if tool_result.content.as_ref() != HARD_CLEAR_PLACEHOLDER {
                tool_result.content = std::sync::Arc::<str>::from(HARD_CLEAR_PLACEHOLDER);
            }
            continue;
        }

        // Soft trim: large tool results → keep head + tail.
        let content_len = tool_result.content.chars().count();
        if content_len > config.soft_trim_threshold {
            let head = safe_char_slice(&tool_result.content, 0, config.soft_trim_head);
            let tail = safe_char_slice_tail(&tool_result.content, config.soft_trim_tail);
            tool_result.content =
                std::sync::Arc::<str>::from(format!("{head}{SOFT_TRIM_SEPARATOR}{tail}"));
        }
    }
}

/// Stable suffix for append-mode invalidation markers.
/// Deliberately independent of the prune category (soft trim vs hard clear):
/// a result's category deepens with turn age, and a marker that changed with
/// it would rewrite wire bytes mid-history — the exact cliff this mode exists
/// to avoid. Text depends only on the tool call id, so once emitted a marker
/// is byte-stable for the rest of the session.
pub(crate) fn append_prune_marker(tool_call_id: &str) -> String {
    format!(
        "[Context management notice: the tool result above (call `{tool_call_id}`) has \
         been archived and its content superseded. Do not rely on or quote it; use \
         later turns, or ask to re-run the tool if those details matter.]"
    )
}

/// Append-mode pruning (LOCAL(cache-port) `prune_mode = "append"`): keep every
/// pruned tool result's original bytes and insert one invalidation marker item
/// after each. Marker text is call-id-keyed and category-independent, so the
/// emitted marker set only grows — the wire prefix stays append-only past the
/// 50% context line instead of taking the in-place-rewrite cliff.
pub(crate) fn prune_conversation_append(
    conversation: &mut Vec<ConversationItem>,
    config: &PruningConfig,
) {
    if !config.enabled {
        return;
    }

    // Collect marker texts for pruned results while walking, then insert after
    // the fact (reverse order keeps the recorded indices valid).
    let mut markers: Vec<(usize, String)> = Vec::new();
    let mut turn_from_end: usize = 0;
    let mut seen_first_user = false;
    for (idx, item) in conversation.iter().enumerate().rev() {
        if matches!(item, ConversationItem::User(_)) {
            if seen_first_user {
                turn_from_end += 1;
            }
            seen_first_user = true;
            continue;
        }
        let ConversationItem::ToolResult(tool_result) = item else {
            continue;
        };
        if turn_from_end < config.keep_last_n_turns {
            continue;
        }
        // Both arms prune; the content stays untouched in this mode.
        let hard = turn_from_end >= config.hard_clear_age_turns;
        let soft = !hard && tool_result.content.chars().count() > config.soft_trim_threshold;
        if hard || soft {
            markers.push((idx + 1, append_prune_marker(&tool_result.tool_call_id)));
        }
    }
    for (idx, text) in markers {
        conversation.insert(
            idx,
            ConversationItem::System(SystemItem {
                content: std::sync::Arc::<str>::from(text),
                synthetic_reason: SyntheticReason::InHistoryUpdate,
            }),
        );
    }
}

// ============================================================================
// Memory reminder injection
// ============================================================================

use crate::types::MEMORY_CONTEXT_OPEN_TAG;

/// Upsert a memory reminder into the conversation's system message.
/// Replaces a prior reminder section in-place, or prepends a `System` item if none exists.
/// Returns `true` when the conversation was changed.
pub(super) fn inject_memory_reminder(items: &mut Vec<ConversationItem>, reminder: &str) -> bool {
    let reminder = reminder.trim();
    if reminder.is_empty() {
        return false;
    }

    if let Some(ConversationItem::System(sys)) = items.first_mut() {
        upsert_memory_reminder_text(&mut sys.content, reminder)
    } else {
        items.insert(0, ConversationItem::system(reminder));
        true
    }
}

fn upsert_memory_reminder_text(system_prompt: &mut std::sync::Arc<str>, reminder: &str) -> bool {
    let existing_start = system_prompt.find(MEMORY_CONTEXT_OPEN_TAG).map(|idx| {
        system_prompt
            .get(..idx)
            .unwrap_or("")
            .trim_end_matches('\n')
            .len()
    });

    let updated: String = if let Some(prefix_len) = existing_start {
        let prefix = system_prompt
            .get(..prefix_len)
            .unwrap_or("")
            .trim_end_matches('\n');
        if prefix.is_empty() {
            reminder.to_string()
        } else {
            format!("{prefix}\n\n{reminder}")
        }
    } else if system_prompt.trim_end() == reminder {
        system_prompt.as_ref().to_owned()
    } else if system_prompt.is_empty() {
        reminder.to_string()
    } else {
        format!("{}\n\n{reminder}", system_prompt.trim_end_matches('\n'))
    };

    if system_prompt.as_ref() == updated.as_str() {
        false
    } else {
        *system_prompt = std::sync::Arc::<str>::from(updated);
        true
    }
}

// ============================================================================
// String helpers
// ============================================================================

fn safe_char_slice(s: &str, start: usize, count: usize) -> String {
    s.chars().skip(start).take(count).collect()
}

fn safe_char_slice_tail(s: &str, count: usize) -> String {
    let total = s.chars().count();
    if count >= total {
        return s.to_string();
    }
    s.chars().skip(total - count).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_prune_gating() {
        use std::num::NonZeroU64;
        let cw = NonZeroU64::new(10000).unwrap();
        assert!(!should_prune(1000, cw)); // 10%
        assert!(should_prune(6000, cw)); // 60%
        assert!(!should_prune(5000, cw)); // 50% exact (> not >=)
    }

    #[test]
    fn prune_disabled_is_noop() {
        let mut conv = vec![ConversationItem::tool_result("c1", "x".repeat(10_000))];
        let config = PruningConfig {
            enabled: false,
            ..Default::default()
        };
        prune_conversation(&mut conv, &config);
        let [ConversationItem::ToolResult(tr)] = conv.as_slice() else {
            panic!("expected one tool result: {conv:?}")
        };
        assert_eq!(tr.content.len(), 10_000);
    }

    #[test]
    fn inject_memory_into_existing_system() {
        let mut items = vec![
            ConversationItem::system("You are helpful."),
            ConversationItem::user("hi"),
        ];
        inject_memory_reminder(&mut items, "Remember: user likes rust");
        if let Some(ConversationItem::System(sys)) = items.first() {
            assert!(sys.content.contains("Remember: user likes rust"));
            assert!(sys.content.starts_with("You are helpful."));
        }
        assert_eq!(items.len(), 2); // no new item added
    }

    #[test]
    fn inject_memory_prepends_when_no_system() {
        let mut items = vec![ConversationItem::user("hi")];
        inject_memory_reminder(&mut items, "Remember: user likes rust");
        assert_eq!(items.len(), 2);
        assert!(matches!(items.first(), Some(ConversationItem::System(_))));
    }

    // ==== LOCAL(cache-port): in-history system placement ====

    fn system_text(items: &[ConversationItem], idx: usize) -> String {
        match items.get(idx) {
            Some(ConversationItem::System(s)) => s.content.to_string(),
            other => panic!("expected system at {idx}, got {other:?}"),
        }
    }

    #[test]
    fn in_history_first_build_freezes_head_without_append() {
        let mut items = vec![
            ConversationItem::system("You are helpful."),
            ConversationItem::user("hi"),
        ];
        let mut wire = None;
        apply_in_history_placement(&mut items, &mut wire);
        assert_eq!(items.len(), 2, "no appended snapshot on the first build");
        assert_eq!(system_text(&items, 0), "You are helpful.");
        let st = wire.expect("head frozen");
        assert_eq!(st.head.as_ref(), "You are helpful.");
        assert_eq!(st.last_snapshot.as_ref(), "You are helpful.");
    }

    #[test]
    fn in_history_change_restores_head_and_appends_snapshot() {
        let mut items = vec![
            ConversationItem::system("NEW prompt with rules B"),
            ConversationItem::user("q1"),
            ConversationItem::tool_result("c1", "r1"),
        ];
        let mut wire = Some(crate::actor::state::InHistoryWireState {
            head: std::sync::Arc::<str>::from("OLD prompt with rules A"),
            last_snapshot: std::sync::Arc::<str>::from("OLD prompt with rules A"),
        });
        apply_in_history_placement(&mut items, &mut wire);
        // Head restored to the frozen bytes; new snapshot appended at the tail.
        assert_eq!(system_text(&items, 0), "OLD prompt with rules A");
        assert_eq!(system_text(&items, 3), "NEW prompt with rules B");
        assert_eq!(items.len(), 4);
        let st = wire.unwrap();
        assert_eq!(st.head.as_ref(), "OLD prompt with rules A", "head stays frozen");
        assert_eq!(st.last_snapshot.as_ref(), "NEW prompt with rules B");
    }

    #[test]
    fn in_history_unchanged_head_is_byte_stable() {
        let mut items = vec![
            ConversationItem::system("same"),
            ConversationItem::user("q1"),
        ];
        let mut wire = Some(crate::actor::state::InHistoryWireState {
            head: std::sync::Arc::<str>::from("same"),
            last_snapshot: std::sync::Arc::<str>::from("same"),
        });
        let before = format!("{items:?}");
        apply_in_history_placement(&mut items, &mut wire);
        assert_eq!(format!("{items:?}"), before, "unchanged head must not touch items");
    }

    #[test]
    fn in_history_successive_changes_stay_append_only() {
        let mut wire = None;
        let mut items = vec![
            ConversationItem::system("P1"),
            ConversationItem::user("q1"),
        ];
        apply_in_history_placement(&mut items, &mut wire);

        // Turn 2: prompt changed to P2 — head restored, snapshot appended.
        items[0] = ConversationItem::system("P2");
        items.push(ConversationItem::user("q2"));
        apply_in_history_placement(&mut items, &mut wire);
        assert_eq!(system_text(&items, 0), "P1");
        assert_eq!(system_text(&items, 3), "P2");

        // Turn 3: prompt changed again to P3 — the turn-2 wire bytes (head P1
        // + appended P2 + q2) must be untouched so the cached prefix survives.
        items[0] = ConversationItem::system("P3");
        items.push(ConversationItem::user("q3"));
        apply_in_history_placement(&mut items, &mut wire);
        assert_eq!(system_text(&items, 0), "P1", "position 0 stays frozen forever");
        assert_eq!(system_text(&items, 3), "P2", "earlier snapshot stays in place");
        assert_eq!(system_text(&items, 5), "P3");
    }

    #[test]
    fn in_history_no_head_system_clears_state() {
        let mut items = vec![ConversationItem::user("q1")];
        let mut wire = Some(crate::actor::state::InHistoryWireState {
            head: std::sync::Arc::<str>::from("stale"),
            last_snapshot: std::sync::Arc::<str>::from("stale"),
        });
        apply_in_history_placement(&mut items, &mut wire);
        assert!(wire.is_none(), "stale state must drop when the head vanishes");
    }

    // ==== LOCAL(cache-port): append-mode tool-result pruning ====

    #[test]
    fn append_prune_keeps_original_bytes_and_inserts_marker() {
        // keep_last_n_turns = 1: `c1` (two newer users → age 1) is pruned,
        // `c2`/`c3` (age 0) are not.
        let config = PruningConfig {
            keep_last_n_turns: 1,
            ..Default::default()
        };
        let big = "x".repeat(10_000);
        let mut conv = vec![
            ConversationItem::user("q1"),
            ConversationItem::tool_result("c1", big.clone()),
            ConversationItem::user("q2"),
            ConversationItem::tool_result("c2", "mid".to_string()),
            ConversationItem::user("q3"),
            ConversationItem::tool_result("c3", "recent".to_string()),
        ];
        prune_conversation_append(&mut conv, &config);
        // Original bytes survive; a marker follows the pruned result only.
        let ids: Vec<String> = conv
            .iter()
            .filter_map(|i| match i {
                ConversationItem::ToolResult(tr) => Some(tr.content.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec![big, "mid".to_string(), "recent".to_string()]);
        match conv.get(2) {
            Some(ConversationItem::System(s)) => {
                assert!(s.content.contains("`c1`"));
                assert_eq!(s.synthetic_reason, SyntheticReason::InHistoryUpdate);
            }
            other => panic!("expected marker system after pruned result, got {other:?}"),
        }
        assert_eq!(conv.len(), 7);
    }

    #[test]
    fn append_prune_marker_text_is_category_independent() {
        // The same tool result must carry the same marker whether it is
        // soft-trim age or hard-clear age — otherwise aging would rewrite
        // wire bytes mid-history.
        let mk = |hard_clear_age_turns: usize| {
            let mut conv = vec![
                ConversationItem::user("q1"),
                ConversationItem::tool_result("c1", "y".repeat(10_000)),
                ConversationItem::user("q2"),
            ];
            let config = PruningConfig {
                keep_last_n_turns: 0,
                hard_clear_age_turns,
                ..Default::default()
            };
            prune_conversation_append(&mut conv, &config);
            conv.iter()
                .find_map(|i| match i {
                    ConversationItem::System(s) => Some(s.content.to_string()),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(mk(usize::MAX), mk(0), "marker text must not depend on prune category");
    }

    #[test]
    fn append_prune_marker_set_is_append_only_across_turns() {
        // Turn 1: c1 is soft-prune age → marker emitted. Each "build" prunes a
        // fresh clone of the journal (mirrors the request layer).
        let config = PruningConfig {
            keep_last_n_turns: 1,
            hard_clear_age_turns: 2,
            ..Default::default()
        };
        let journal1 = vec![
            ConversationItem::user("q1"),
            ConversationItem::tool_result("c1", "z".repeat(9_000)),
            ConversationItem::user("q2"),
            ConversationItem::tool_result("c2", "mid".to_string()),
            ConversationItem::user("q3"),
            ConversationItem::tool_result("c3", "recent".to_string()),
        ];
        let mut wire1 = journal1.clone();
        prune_conversation_append(&mut wire1, &config);
        let turn1_marker = wire1
            .iter()
            .find_map(|i| match i {
                ConversationItem::System(s) => Some(s.content.to_string()),
                _ => None,
            })
            .expect("turn 1 emits a marker for c1");

        // Turn 2: one more turn appended; c1 aged into the hard-clear arm.
        // The fresh build must emit the byte-identical marker at the same
        // position, and c1's original bytes must survive.
        let mut journal2 = journal1.clone();
        journal2.push(ConversationItem::user("q4"));
        journal2.push(ConversationItem::tool_result("c4", "newest".to_string()));
        let mut wire2 = journal2.clone();
        prune_conversation_append(&mut wire2, &config);

        let c1 = wire2.iter().find_map(|i| match i {
            ConversationItem::ToolResult(tr) if tr.tool_call_id == "c1" => {
                Some(tr.content.to_string())
            }
            _ => None,
        });
        assert_eq!(
            c1.as_deref(),
            Some("z".repeat(9_000).as_str()),
            "original bytes must survive aging in append mode"
        );
        // The whole turn-1 wire must be a byte-stable prefix of turn 2's —
        // nothing moved, nothing was rewritten, nothing was dropped.
        assert!(
            wire2.len() >= wire1.len()
                && wire1.iter().zip(wire2.iter()).all(|(a, b)| format!("{a:?}") == format!("{b:?}")),
            "turn-1 wire must be a byte-stable prefix of turn-2 wire"
        );
        assert!(
            wire2.iter().any(|i| matches!(i, ConversationItem::System(s) if s.content.to_string() == turn1_marker)),
            "the emitted marker must persist byte-identical across turns"
        );
    }
}
