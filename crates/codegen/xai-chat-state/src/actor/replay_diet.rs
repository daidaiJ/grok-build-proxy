//! LOCAL(token-economy P1): in-turn replay diet.
//!
//! Long turns replay every earlier reasoning item and tool result on each model
//! call. Age-based pruning only fires across user turns, so a 100-call turn can
//! keep growing inside one turn. This module plans one deterministic rewrite
//! when the turn crosses a call/context budget:
//!
//! - reasoning: keep the current and most recent prompt turns, drop older items;
//! - tool results: keep the newest N results plus every small result, archive
//!   older large bodies into a short stub that keeps the call id and tool name.
//!
//! The rewrite is a pure function so the actor only owns triggering and
//! persistence. It is idempotent: stubs are recognizable and skipped on the
//! next plan.

use std::collections::{HashMap, HashSet};

use xai_grok_sampling_types::ConversationItem;

use super::state::estimate_item_tokens;

/// Marker opening every archived tool-result stub. Recognition is prefix-based
/// so a later format tweak can still detect stubs written by this version.
pub const REPLAY_DIET_STUB_PREFIX: &str = "[Archived earlier tool result:";

/// Thresholds for one in-turn diet.
#[derive(Debug, Clone, Copy)]
pub struct ReplayDietPolicy {
    /// Master switch (`GROK_REPLAY_DIET=0` disables).
    pub enabled: bool,
    /// Minimum model calls in the current turn before the diet may run.
    pub min_turn_calls: u32,
    /// Minimum estimated prompt tokens before the diet may run.
    pub min_prompt_tokens: u64,
    /// Minimum estimated tokens the plan must reclaim to be worth the cache break.
    pub min_reclaim_tokens: u64,
    /// Newest tool results kept verbatim regardless of size.
    pub keep_recent_tool_results: usize,
    /// Prompt turns whose reasoning stays verbatim (current turn included).
    pub keep_recent_user_turns: usize,
    /// Results at or below this size stay verbatim.
    pub small_result_bytes: usize,
    /// Only results above this size are stubbed.
    pub stub_min_bytes: usize,
    /// Maximum characters of a result's first line kept in the stub.
    pub stub_first_line_chars: usize,
}

impl Default for ReplayDietPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            min_turn_calls: 20,
            min_prompt_tokens: 200_000,
            min_reclaim_tokens: 64_000,
            keep_recent_tool_results: 10,
            keep_recent_user_turns: 2,
            small_result_bytes: 1024,
            stub_min_bytes: 2048,
            stub_first_line_chars: 120,
        }
    }
}

impl ReplayDietPolicy {
    /// Read the rollout switch and optional threshold overrides from the
    /// environment. Parsing stays here so the actor does not carry a config
    /// dependency for a LOCAL experiment.
    pub fn from_env() -> Self {
        let mut policy = Self::default();
        if matches!(
            std::env::var("GROK_REPLAY_DIET").ok().as_deref(),
            Some("0" | "off" | "false" | "no")
        ) {
            policy.enabled = false;
        }
        if let Ok(raw) = std::env::var("GROK_REPLAY_DIET_MIN_CALLS")
            && let Ok(value) = raw.parse()
        {
            policy.min_turn_calls = value;
        }
        if let Ok(raw) = std::env::var("GROK_REPLAY_DIET_MIN_PROMPT_TOKENS")
            && let Ok(value) = raw.parse()
        {
            policy.min_prompt_tokens = value;
        }
        if let Ok(raw) = std::env::var("GROK_REPLAY_DIET_MIN_RECLAIM_TOKENS")
            && let Ok(value) = raw.parse()
        {
            policy.min_reclaim_tokens = value;
        }
        policy
    }
}

/// LOCAL(token-economy P1): cold-window microcompaction policy.
///
/// Runs the same deterministic replay-diet plan once a session has been idle
/// past the gateway prompt-cache TTL. There is no model call; the cold window
/// is what keeps the rewrite from paying a prefix-cache miss. The floor and
/// reclaim minimum are separate from the in-turn policy: the in-turn diet must
/// justify a hot-cache break, the cold-window diet only needs a worthwhile
/// rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColdWindowDietPolicy {
    pub enabled: bool,
    pub min_prompt_tokens: u64,
    pub min_reclaim_tokens: u64,
}

impl Default for ColdWindowDietPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            min_prompt_tokens: 60_000,
            min_reclaim_tokens: 8_000,
        }
    }
}

/// Result of one cold-window diet attempt. Carries the plan estimate on both
/// arms so the session log can explain a skip as well as a rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColdWindowDietOutcome {
    Applied {
        outcome: ReplayDietOutcome,
        prompt_tokens_before: u64,
        prompt_tokens_after: u64,
        est_reclaim_tokens: u64,
    },
    Skipped {
        reason: ReplayDietSkip,
        est_reclaim_tokens: u64,
    },
}

/// Why a turn was left alone. Logged at debug level by the actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayDietSkip {
    Disabled,
    BackendUnsupported,
    NotEnoughCalls,
    ContextBelowThreshold,
    RecentlyCompacted,
    AlreadyAppliedThisTurn,
    PromptChanged,
    NothingCleanable,
    BelowReclaimThreshold,
}

impl ReplayDietSkip {
    /// Stable snake_case label for unified-log reasons.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::BackendUnsupported => "backend_unsupported",
            Self::NotEnoughCalls => "not_enough_calls",
            Self::ContextBelowThreshold => "context_below_threshold",
            Self::RecentlyCompacted => "recently_compacted",
            Self::AlreadyAppliedThisTurn => "already_applied_this_turn",
            Self::PromptChanged => "prompt_changed",
            Self::NothingCleanable => "nothing_cleanable",
            Self::BelowReclaimThreshold => "below_reclaim_threshold",
        }
    }
}

/// Per-turn application mark. A turn gets at most one diet: a second rewrite
/// would pay another prefix-cache break for less reclaimed context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayDietMark {
    pub prompt_index: usize,
    pub turn_calls: u32,
}

/// One tool result selected for stubbing, with the tool name resolved from the
/// assistant call so the stub stays self-describing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubTarget {
    pub index: usize,
    pub tool_name: String,
}

/// Indices into the conversation plus the estimated reclaim used for gating.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayDietPlan {
    pub remove_reasoning: Vec<usize>,
    pub stub_tool_results: Vec<StubTarget>,
    pub est_reclaim_tokens: u64,
}

impl ReplayDietPlan {
    pub fn is_empty(&self) -> bool {
        self.remove_reasoning.is_empty() && self.stub_tool_results.is_empty()
    }
}

/// Result of applying a plan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReplayDietOutcome {
    pub removed_reasoning: usize,
    pub stubbed_tool_results: usize,
    pub bytes_freed: u64,
    pub est_tokens_freed: u64,
}

/// Gate ordering matters: cheap checks first, and "already applied" before
/// planning so a long turn does not rescan history on every call.
#[allow(clippy::too_many_arguments)]
pub fn replay_diet_gate(
    policy: ReplayDietPolicy,
    backend_supported: bool,
    turn_calls: u32,
    prompt_tokens: u64,
    prompt_index: usize,
    last_compaction_prompt_index: Option<usize>,
    last_applied: Option<ReplayDietMark>,
) -> Result<(), ReplayDietSkip> {
    if !policy.enabled {
        return Err(ReplayDietSkip::Disabled);
    }
    if !backend_supported {
        return Err(ReplayDietSkip::BackendUnsupported);
    }
    if turn_calls < policy.min_turn_calls {
        return Err(ReplayDietSkip::NotEnoughCalls);
    }
    if prompt_tokens < policy.min_prompt_tokens {
        return Err(ReplayDietSkip::ContextBelowThreshold);
    }
    if last_compaction_prompt_index == Some(prompt_index) {
        return Err(ReplayDietSkip::RecentlyCompacted);
    }
    if last_applied.is_some_and(|mark| mark.prompt_index == prompt_index) {
        return Err(ReplayDietSkip::AlreadyAppliedThisTurn);
    }
    Ok(())
}

/// Build the diet plan. Pure: same conversation and policy produce the same
/// indices, which is what keeps successive requests byte-stable.
pub fn plan_replay_diet(
    conversation: &[ConversationItem],
    policy: ReplayDietPolicy,
) -> ReplayDietPlan {
    let mut plan = ReplayDietPlan::default();
    plan.remove_reasoning = plan_reasoning_removal(conversation, policy);
    plan.stub_tool_results = plan_tool_result_stubs(conversation, policy);
    plan.est_reclaim_tokens = estimate_plan_reclaim(conversation, &plan, policy);
    plan
}

/// Drop reasoning older than the start of the `keep_recent_user_turns`-th most
/// recent prompt turn. The current turn is always inside that window.
fn plan_reasoning_removal(
    conversation: &[ConversationItem],
    policy: ReplayDietPolicy,
) -> Vec<usize> {
    let keep = policy.keep_recent_user_turns.max(1);
    let turn_starts: Vec<usize> = conversation
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| match item {
            ConversationItem::User(user) if user.synthetic_reason.starts_prompt_turn() => Some(idx),
            _ => None,
        })
        .collect();
    if turn_starts.len() < keep {
        return Vec::new();
    }
    // Ascending starts: walking back `keep` items lands on the oldest turn
    // that stays verbatim. Everything before it is older replay.
    let cutoff = turn_starts
        .iter()
        .rev()
        .take(keep)
        .last()
        .copied()
        .unwrap_or(0);
    conversation
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            (idx < cutoff && matches!(item, ConversationItem::Reasoning(_))).then_some(idx)
        })
        .collect()
}

/// Keep the newest N results and every small result; stub older large ones.
/// Results carrying images stay verbatim: the text stub would orphan pixels.
fn plan_tool_result_stubs(
    conversation: &[ConversationItem],
    policy: ReplayDietPolicy,
) -> Vec<StubTarget> {
    let tool_names = tool_names_by_call_id(conversation);
    let mut seen = 0usize;
    let mut targets = Vec::new();
    for (idx, item) in conversation.iter().enumerate().rev() {
        let ConversationItem::ToolResult(result) = item else {
            continue;
        };
        seen += 1;
        if seen <= policy.keep_recent_tool_results {
            continue;
        }
        if !result.images.is_empty() {
            continue;
        }
        let bytes = result.content.len();
        if bytes <= policy.small_result_bytes || bytes < policy.stub_min_bytes {
            continue;
        }
        if is_already_archived(result.content.as_ref()) {
            continue;
        }
        let tool_name = tool_names
            .get(result.tool_call_id.as_str())
            .cloned()
            .unwrap_or_else(|| "tool".to_string());
        let stub = build_stub(
            result.tool_call_id.as_ref(),
            &tool_name,
            result.content.as_ref(),
            policy.stub_first_line_chars,
        );
        // Keep a real margin: a stub that barely shrinks the result pays a
        // cache break for almost nothing.
        if stub.len() + 64 >= bytes {
            continue;
        }
        targets.push(StubTarget {
            index: idx,
            tool_name,
        });
    }
    // Reverse iteration finds the newest first; report oldest-first so plans
    // are stable and match conversation order.
    targets.sort_by_key(|target| target.index);
    targets
}

fn estimate_plan_reclaim(
    conversation: &[ConversationItem],
    plan: &ReplayDietPlan,
    policy: ReplayDietPolicy,
) -> u64 {
    let mut tokens = 0u64;
    for idx in &plan.remove_reasoning {
        if let Some(item) = conversation.get(*idx) {
            tokens = tokens.saturating_add(estimate_item_tokens(item));
        }
    }
    for target in &plan.stub_tool_results {
        let Some(ConversationItem::ToolResult(result)) = conversation.get(target.index) else {
            continue;
        };
        let stub = build_stub(
            result.tool_call_id.as_ref(),
            &target.tool_name,
            result.content.as_ref(),
            policy.stub_first_line_chars,
        );
        let before = estimate_item_tokens(&ConversationItem::ToolResult(result.clone()));
        let after = (stub.len() as u64) / xai_token_estimation::BYTES_PER_TOKEN;
        tokens = tokens.saturating_add(before.saturating_sub(after));
    }
    tokens
}

/// Apply a plan in one pass. Indices refer to the input conversation; the
/// rebuild keeps removal and stubbing from invalidating each other.
pub fn apply_replay_diet(
    conversation: &mut Vec<ConversationItem>,
    plan: &ReplayDietPlan,
    policy: ReplayDietPolicy,
) -> ReplayDietOutcome {
    if plan.is_empty() {
        return ReplayDietOutcome::default();
    }
    let remove: HashSet<usize> = plan.remove_reasoning.iter().copied().collect();
    let stub: HashMap<usize, &StubTarget> = plan
        .stub_tool_results
        .iter()
        .map(|target| (target.index, target))
        .collect();
    let mut outcome = ReplayDietOutcome::default();
    let mut kept = Vec::with_capacity(conversation.len());
    for (idx, mut item) in std::mem::take(conversation).into_iter().enumerate() {
        if remove.contains(&idx) && matches!(item, ConversationItem::Reasoning(_)) {
            outcome.bytes_freed = outcome
                .bytes_freed
                .saturating_add(reasoning_bytes(&item) as u64);
            outcome.est_tokens_freed = outcome
                .est_tokens_freed
                .saturating_add(estimate_item_tokens(&item));
            outcome.removed_reasoning += 1;
            continue;
        }
        if let Some(target) = stub.get(&idx)
            && let ConversationItem::ToolResult(result) = &item
        {
            let before_bytes = result.content.len();
            let text = build_stub(
                result.tool_call_id.as_ref(),
                &target.tool_name,
                result.content.as_ref(),
                policy.stub_first_line_chars,
            );
            if text.len() < before_bytes {
                let before_tokens = estimate_item_tokens(&item);
                let stub_len = text.len();
                if let ConversationItem::ToolResult(result) = &mut item {
                    result.content = std::sync::Arc::<str>::from(text);
                }
                let after_tokens = estimate_item_tokens(&item);
                outcome.bytes_freed = outcome
                    .bytes_freed
                    .saturating_add((before_bytes - stub_len) as u64);
                outcome.est_tokens_freed = outcome
                    .est_tokens_freed
                    .saturating_add(before_tokens.saturating_sub(after_tokens));
                outcome.stubbed_tool_results += 1;
            }
        }
        kept.push(item);
    }
    *conversation = kept;
    outcome
}

fn tool_names_by_call_id(conversation: &[ConversationItem]) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for item in conversation {
        if let ConversationItem::Assistant(assistant) = item {
            for call in &assistant.tool_calls {
                names.insert(call.id.to_string(), call.name.clone());
            }
        }
    }
    names
}

fn is_already_archived(content: &str) -> bool {
    content.starts_with(REPLAY_DIET_STUB_PREFIX)
        || content == super::request_builder::HARD_CLEAR_PLACEHOLDER
}

fn reasoning_bytes(item: &ConversationItem) -> usize {
    match item {
        ConversationItem::Reasoning(reasoning) => {
            xai_grok_sampling_types::reasoning_item_text(reasoning).len()
                + reasoning
                    .encrypted_content
                    .as_deref()
                    .map(str::len)
                    .unwrap_or(0)
        }
        _ => 0,
    }
}

fn build_stub(
    tool_call_id: &str,
    tool_name: &str,
    content: &str,
    first_line_chars: usize,
) -> String {
    let first_line = content.lines().next().unwrap_or("").trim();
    let first_line = truncate_chars(first_line, first_line_chars);
    let mut stub = format!(
        "{REPLAY_DIET_STUB_PREFIX} call `{tool_call_id}` ({tool_name}), original {} bytes.",
        content.len()
    );
    if !first_line.is_empty() {
        stub.push_str(" First line: ");
        stub.push_str(&first_line);
    }
    stub.push_str(" Re-run the tool with narrower arguments if these details matter.]");
    stub
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_sampling_types::{AssistantItem, ConversationItem, synthesized_reasoning_item};

    fn user(text: &str) -> ConversationItem {
        ConversationItem::user(text)
    }

    fn reasoning(text: &str) -> ConversationItem {
        ConversationItem::Reasoning(synthesized_reasoning_item(text))
    }

    fn assistant_call(id: &str, name: &str) -> ConversationItem {
        assistant_calls(&[(id, name)])
    }

    fn assistant_calls(calls: &[(&str, &str)]) -> ConversationItem {
        ConversationItem::Assistant(AssistantItem {
            content: "".into(),
            tool_calls: calls
                .iter()
                .map(|(id, name)| xai_grok_sampling_types::ToolCall {
                    id: (*id).into(),
                    name: (*name).to_string(),
                    arguments: "{\"path\":\"a.rs\"}".into(),
                })
                .collect(),
            model_id: None,
            model_fingerprint: None,
            reasoning_effort: None,
        })
    }

    fn result(id: &str, bytes: usize) -> ConversationItem {
        ConversationItem::tool_result(id, "x".repeat(bytes))
    }

    fn policy() -> ReplayDietPolicy {
        ReplayDietPolicy {
            min_prompt_tokens: 0,
            min_reclaim_tokens: 0,
            min_turn_calls: 0,
            ..Default::default()
        }
    }

    #[test]
    fn keeps_current_and_previous_turn_reasoning() {
        let conversation = vec![
            ConversationItem::system("sys"),
            user("q1"),
            reasoning("old-1"),
            assistant_call("c1", "read_file"),
            result("c1", 3000),
            user("q2"),
            reasoning("prev-2"),
            assistant_call("c2", "read_file"),
            result("c2", 3000),
            user("q3"),
            reasoning("current-3"),
            assistant_call("c3", "read_file"),
            result("c3", 3000),
        ];
        let plan = plan_replay_diet(&conversation, policy());
        assert_eq!(plan.remove_reasoning, vec![2]);
    }

    #[test]
    fn stubs_old_large_results_and_keeps_recent_small_and_images() {
        let calls: Vec<(String, &str)> = (0..12)
            .map(|i| (format!("c{i}"), "run_terminal_command"))
            .collect();
        let call_refs: Vec<(&str, &str)> = calls
            .iter()
            .map(|(id, name)| (id.as_str(), *name))
            .collect();
        let mut conversation = vec![user("q1"), assistant_calls(&call_refs)];
        for i in 0..12 {
            conversation.push(result(&format!("c{i}"), 3000));
        }
        conversation.push(ConversationItem::tool_result("small", "ok"));
        let mut large_with_image = ConversationItem::tool_result("img", "y".repeat(4000));
        if let ConversationItem::ToolResult(result) = &mut large_with_image {
            result
                .images
                .push(xai_grok_sampling_types::ContentPart::Image {
                    url: "data:image/png;base64,AAAA".into(),
                });
        }
        conversation.push(large_with_image);

        let plan = plan_replay_diet(&conversation, policy());
        let stubbed: Vec<_> = plan
            .stub_tool_results
            .iter()
            .map(|target| target.index)
            .collect();
        // Newest 10 results stay: c4..c11, small, image. c0..c3 are old.
        assert_eq!(stubbed, vec![2, 3, 4, 5]);
        assert!(
            plan.stub_tool_results
                .iter()
                .all(|t| t.tool_name == "run_terminal_command")
        );
    }

    #[test]
    fn apply_is_idempotent_and_reports_reclaim() {
        let conversation = vec![
            user("q1"),
            reasoning("old"),
            assistant_call("c1", "read_file"),
            result("c1", 5000),
            user("q2"),
            reasoning("current"),
            assistant_call("c2", "read_file"),
            result("c2", 5000),
        ];
        let policy = ReplayDietPolicy {
            keep_recent_tool_results: 1,
            keep_recent_user_turns: 1,
            ..policy()
        };
        let plan = plan_replay_diet(&conversation, policy);
        assert!(plan.est_reclaim_tokens > 0);

        let mut items = conversation.clone();
        let first = apply_replay_diet(&mut items, &plan, policy);
        assert_eq!(first.removed_reasoning, 1);
        assert_eq!(first.stubbed_tool_results, 1);
        assert!(first.bytes_freed > 0);

        let second_plan = plan_replay_diet(&items, policy);
        assert!(second_plan.is_empty());
        let second = apply_replay_diet(&mut items, &second_plan, policy);
        assert_eq!(second, ReplayDietOutcome::default());
    }

    #[test]
    fn stub_keeps_call_id_tool_name_and_size() {
        let content = "line one is useful\nsecond line\n".to_string() + &"z".repeat(4096);
        let stub = build_stub("call_123", "run_terminal_command", &content, 120);
        assert!(stub.starts_with(REPLAY_DIET_STUB_PREFIX));
        assert!(stub.contains("call_123"));
        assert!(stub.contains("run_terminal_command"));
        assert!(stub.contains(&format!("{} bytes", content.len())));
        assert!(stub.contains("line one is useful"));
        assert!(stub.len() < 400);
    }

    #[test]
    fn current_turn_reasoning_survives_even_with_tiny_keep_window() {
        let conversation = vec![
            user("q1"),
            reasoning("old"),
            assistant_call("c1", "read_file"),
            result("c1", 5000),
            user("q2"),
            reasoning("current"),
            assistant_call("c2", "read_file"),
            result("c2", 5000),
        ];
        let plan = plan_replay_diet(
            &conversation,
            ReplayDietPolicy {
                keep_recent_user_turns: 1,
                ..policy()
            },
        );
        assert_eq!(plan.remove_reasoning, vec![1]);
        assert!(!plan.remove_reasoning.contains(&5));
    }

    #[test]
    fn gate_enforces_budget_and_one_shot_per_turn() {
        let policy = ReplayDietPolicy::default();
        assert_eq!(
            replay_diet_gate(policy, true, 0, 0, 0, None, None),
            Err(ReplayDietSkip::NotEnoughCalls)
        );
        assert_eq!(
            replay_diet_gate(policy, true, 20, 100, 0, None, None),
            Err(ReplayDietSkip::ContextBelowThreshold)
        );
        assert_eq!(
            replay_diet_gate(policy, true, 20, 200_000, 3, Some(3), None),
            Err(ReplayDietSkip::RecentlyCompacted)
        );
        assert_eq!(
            replay_diet_gate(
                policy,
                true,
                20,
                200_000,
                3,
                None,
                Some(ReplayDietMark {
                    prompt_index: 3,
                    turn_calls: 20
                })
            ),
            Err(ReplayDietSkip::AlreadyAppliedThisTurn)
        );
        assert_eq!(
            replay_diet_gate(policy, true, 20, 200_000, 3, None, None),
            Ok(())
        );
    }
}
