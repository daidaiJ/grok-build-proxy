//! LOCAL(token-economy P1): long-horizon compaction relay anchor.
//!
//! Repeated compaction turns a summary into the next summary's input. The
//! original request, the user's later steering messages, and the task's
//! acceptance shape can erode a little on each pass. This module keeps a
//! small, deterministic anchor outside the summary:
//!
//! - `original_goal` is captured once and never rewritten;
//! - real user messages are appended, deduplicated, and capped;
//! - the anchor is rendered as a `CompactionMeta` user item so the next
//!   compaction reads it but the summarizer is not asked to paraphrase it.
//!
//! The anchor is intentionally small (at most a few thousand tokens). It
//! complements the detailed nine-section handoff summary; it does not replace
//! it.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use xai_grok_sampling_types::ConversationItem;

use crate::compaction_utils::extract_real_user_queries;

/// XML-ish wrapper so the anchor is easy to find and parse in history.
pub const RELAY_ANCHOR_BEGIN: &str = "<relay_anchor>";
pub const RELAY_ANCHOR_END: &str = "</relay_anchor>";

/// Keep the anchor bounded. The long handoff summary carries the detailed
/// state; the anchor only protects identity and user intent.
const MAX_USER_MESSAGES: usize = 24;
const MAX_MESSAGE_CHARS: usize = 4_000;
const MAX_TOTAL_CHARS: usize = 24_000;

/// Stable fields carried across compactions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayAnchor {
    /// Increments on every compaction that carries the anchor.
    pub generation: u64,
    /// First task-shaped user request. Captured once, never rewritten.
    pub original_goal: Option<String>,
    /// Verbatim real user messages, oldest first, deduplicated.
    pub user_messages: Vec<String>,
    /// Compaction counter at the time this anchor was written.
    pub compaction_count: u64,
    /// User messages appended by the extraction that produced this anchor.
    /// Transient telemetry; not serialized into history.
    #[serde(skip)]
    pub new_user_messages: usize,
}

/// Extract the next anchor from the pre-compaction conversation plus the
/// previous anchor. Existing fields win: the goal is immutable and user
/// messages are append-only.
pub fn extract_relay_anchor(
    conversation: &[ConversationItem],
    previous: Option<&RelayAnchor>,
    compaction_count: u64,
    goal_objective: Option<&str>,
) -> RelayAnchor {
    let mut user_messages = previous
        .map(|anchor| anchor.user_messages.clone())
        .unwrap_or_default();
    let mut seen: HashSet<String> = user_messages.iter().cloned().collect();
    let mut new_user_messages = 0usize;

    let goal_objective = goal_objective
        .map(str::trim)
        .filter(|goal| !goal.is_empty());
    if previous.is_none()
        && let Some(goal) = goal_objective
    {
        let goal = truncate_chars(goal, MAX_MESSAGE_CHARS);
        if seen.insert(goal.clone()) {
            user_messages.insert(0, goal);
        }
    }

    for query in extract_real_user_queries(conversation) {
        let query = query.trim();
        if query.is_empty() {
            continue;
        }
        let capped = truncate_chars(query, MAX_MESSAGE_CHARS);
        if seen.insert(capped.clone()) {
            user_messages.push(capped);
            new_user_messages += 1;
        }
    }

    let original_goal = previous
        .and_then(|anchor| anchor.original_goal.clone())
        .or_else(|| goal_objective.map(str::to_string))
        .or_else(|| {
            user_messages
                .iter()
                .find(|message| !message.trim_start().starts_with('/'))
                .cloned()
        })
        .or_else(|| user_messages.first().cloned());

    let user_messages = cap_user_messages(user_messages, original_goal.as_deref());
    RelayAnchor {
        generation: previous.map_or(1, |anchor| anchor.generation.saturating_add(1)),
        original_goal,
        user_messages,
        compaction_count,
        new_user_messages,
    }
}

/// Parse an anchor from history. Returns the first well-formed anchor.
pub fn parse_relay_anchor(conversation: &[ConversationItem]) -> Option<RelayAnchor> {
    conversation.iter().find_map(|item| {
        let text = item.text_content();
        let inner = text
            .trim()
            .strip_prefix(RELAY_ANCHOR_BEGIN)?
            .strip_suffix(RELAY_ANCHOR_END)?
            .trim();
        serde_json::from_str(inner).ok()
    })
}

/// Render the compact JSON payload carried in history.
pub fn render_relay_anchor(anchor: &RelayAnchor) -> String {
    let payload = serde_json::to_string(anchor).unwrap_or_else(|_| "{}".to_string());
    format!("{RELAY_ANCHOR_BEGIN}\n{payload}\n{RELAY_ANCHOR_END}")
}

/// Replace any existing anchor and insert the new one after the system message
/// and user-info prefix. Position stays stable across compactions when an
/// anchor already exists.
pub fn ensure_relay_anchor(conversation: &mut Vec<ConversationItem>, anchor: &RelayAnchor) {
    let existing_pos = conversation.iter().position(is_relay_anchor_item);
    conversation.retain(|item| !is_relay_anchor_item(item));
    let pos = existing_pos
        .unwrap_or_else(|| conversation.len().min(2))
        .min(conversation.len());
    conversation.insert(
        pos,
        ConversationItem::user_meta(render_relay_anchor(anchor)),
    );
}

fn is_relay_anchor_item(item: &ConversationItem) -> bool {
    item.text_content()
        .trim_start()
        .starts_with(RELAY_ANCHOR_BEGIN)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    text.chars().take(max_chars).collect()
}

fn cap_user_messages(messages: Vec<String>, original_goal: Option<&str>) -> Vec<String> {
    let total_chars: usize = messages.iter().map(|message| message.chars().count()).sum();
    if messages.len() <= MAX_USER_MESSAGES && total_chars <= MAX_TOTAL_CHARS {
        return messages;
    }

    let mut kept: Vec<String> = Vec::new();
    let mut kept_chars = 0usize;
    for message in messages.iter().rev() {
        if kept.len() + 1 >= MAX_USER_MESSAGES {
            break;
        }
        let chars = message.chars().count();
        if !kept.is_empty() && kept_chars + chars > MAX_TOTAL_CHARS {
            break;
        }
        kept_chars += chars;
        kept.push(message.clone());
    }
    kept.reverse();

    if let Some(goal) = original_goal {
        if kept.iter().any(|message| message.as_str() == goal) {
            kept.retain(|message| message.as_str() != goal);
        }
        kept.insert(0, goal.to_string());
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_sampling_types::{SyntheticReason, UserItem};

    fn real_user(text: &str) -> ConversationItem {
        ConversationItem::user(text)
    }

    fn synthetic_user(text: &str) -> ConversationItem {
        ConversationItem::User(UserItem {
            content: vec![xai_grok_sampling_types::ContentPart::Text { text: text.into() }],
            synthetic_reason: SyntheticReason::CompactionMeta,
            cwd_generation: None,
            prior_turn_interrupt: None,
            prompt_index: None,
        })
    }

    #[test]
    fn first_extraction_captures_goal_and_user_messages() {
        let conversation = vec![
            ConversationItem::system("sys"),
            synthetic_user("<user_info>prefix</user_info>"),
            real_user("build a long-running feature"),
            real_user("use the relay design"),
        ];
        let anchor = extract_relay_anchor(&conversation, None, 0, None);
        assert_eq!(anchor.generation, 1);
        assert_eq!(
            anchor.original_goal.as_deref(),
            Some("build a long-running feature")
        );
        assert_eq!(
            anchor.user_messages,
            vec!["build a long-running feature", "use the relay design"]
        );
        assert_eq!(anchor.new_user_messages, 2);
    }

    #[test]
    fn later_extraction_preserves_goal_and_appends_only_new_messages() {
        let previous = RelayAnchor {
            generation: 2,
            original_goal: Some("build a long-running feature".into()),
            user_messages: vec![
                "build a long-running feature".into(),
                "use the relay design".into(),
            ],
            compaction_count: 1,
            new_user_messages: 0,
        };
        let conversation = vec![
            real_user("build a long-running feature"),
            real_user("use the relay design"),
            real_user("now fix the drift"),
            synthetic_user(render_relay_anchor(&previous)),
        ];
        let anchor = extract_relay_anchor(&conversation, Some(&previous), 2, None);
        assert_eq!(anchor.generation, 3);
        assert_eq!(
            anchor.original_goal.as_deref(),
            Some("build a long-running feature")
        );
        assert_eq!(
            anchor.user_messages,
            vec![
                "build a long-running feature",
                "use the relay design",
                "now fix the drift"
            ]
        );
        assert_eq!(anchor.new_user_messages, 1);
    }

    #[test]
    fn goal_objective_wins_when_there_is_no_previous_anchor() {
        let conversation = vec![real_user("an earlier exploratory request")];
        let anchor = extract_relay_anchor(
            &conversation,
            None,
            0,
            Some("ship the long-horizon feature"),
        );
        assert_eq!(
            anchor.original_goal.as_deref(),
            Some("ship the long-horizon feature")
        );
        assert_eq!(
            anchor.user_messages.first().map(String::as_str),
            Some("ship the long-horizon feature")
        );
    }

    #[test]
    fn cap_keeps_goal_and_newest_messages() {
        let messages: Vec<String> = (0..40).map(|i| format!("message-{i}")).collect();
        let capped = cap_user_messages(messages, Some("message-0"));
        assert_eq!(capped.first().map(String::as_str), Some("message-0"));
        assert!(capped.len() <= MAX_USER_MESSAGES);
        assert!(capped.iter().any(|message| message == "message-39"));
        assert!(!capped.iter().any(|message| message == "message-1"));
    }

    #[test]
    fn render_parse_roundtrip() {
        let anchor = RelayAnchor {
            generation: 4,
            original_goal: Some("ship it".into()),
            user_messages: vec!["ship it".into(), "keep the API stable".into()],
            compaction_count: 3,
            new_user_messages: 0,
        };
        let item = synthetic_user(render_relay_anchor(&anchor));
        let parsed = parse_relay_anchor(&[item]).expect("anchor parses");
        assert_eq!(parsed, anchor);
    }

    #[test]
    fn ensure_replaces_anchor_in_place_and_is_idempotent() {
        let first = RelayAnchor {
            generation: 1,
            original_goal: Some("goal".into()),
            user_messages: vec!["goal".into()],
            compaction_count: 1,
            new_user_messages: 0,
        };
        let second = RelayAnchor {
            generation: 2,
            original_goal: Some("goal".into()),
            user_messages: vec!["goal".into(), "next".into()],
            compaction_count: 2,
            new_user_messages: 1,
        };
        let mut conversation = vec![
            ConversationItem::system("sys"),
            synthetic_user("<user_info>prefix</user_info>"),
            real_user("goal"),
            synthetic_user(render_relay_anchor(&first)),
        ];
        ensure_relay_anchor(&mut conversation, &first);
        let anchor_count = conversation
            .iter()
            .filter(|item| is_relay_anchor_item(item))
            .count();
        assert_eq!(anchor_count, 1);
        let pos = conversation.iter().position(is_relay_anchor_item).unwrap();

        ensure_relay_anchor(&mut conversation, &second);
        assert_eq!(
            conversation
                .iter()
                .filter(|item| is_relay_anchor_item(item))
                .count(),
            1
        );
        assert_eq!(
            conversation.iter().position(is_relay_anchor_item),
            Some(pos)
        );
        assert_eq!(
            parse_relay_anchor(&conversation).map(|a| a.generation),
            Some(2)
        );
    }
}
