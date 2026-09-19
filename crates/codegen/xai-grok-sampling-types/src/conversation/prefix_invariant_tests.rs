//! Byte-level prefix-cache invariants for the wire request construction paths.
//!
//! Server/implicit prompt caching matches the serialized request prefix, so the
//! whole construction chain — `ConversationRequest` → wire request — must be
//! deterministic: the same logical state always serializes to identical bytes,
//! and request N+1 extends request N without mutating it. These tests are the
//! tripwire for nondeterminism creeping into the prefix path (HashMap iteration
//! order, timestamps, ambient state) and for normalization passes (user-turn
//! merging, cache breakpoints) that would silently rewrite already-sent turns.
//!
//! The construction functions are pure by design; keep them that way.

use super::*;

fn sample_items(turns: usize) -> Vec<ConversationItem> {
    let mut items = vec![
        ConversationItem::system("You are a helpful assistant."),
        ConversationItem::user("Fix the bug in src/main.rs"),
        // A compaction-shaped consecutive-user run: summary carrier + reminder.
        ConversationItem::user_meta("Earlier context: the parser was rewritten."),
        ConversationItem::system_reminder("<system-reminder>plan mode is active</system-reminder>"),
    ];
    for n in 0..turns {
        let id = format!("call_{n}");
        items.push(ConversationItem::Reasoning(synthesized_reasoning_item(
            format!("thinking about step {n}"),
        )));
        items.push(ConversationItem::Assistant(AssistantItem {
            content: String::new().into(),
            tool_calls: vec![ToolCall {
                id: id.as_str().into(),
                name: "read_file".to_string(),
                arguments: format!(r#"{{"path": "src/step_{n}.rs"}}"#).into(),
            }],
            model_id: None,
            model_fingerprint: None,
            reasoning_effort: None,
        }));
        items.push(ConversationItem::tool_result(id, format!("contents {n}")));
        items.push(ConversationItem::user(format!("next step {n}")));
    }
    items
}

fn sample_request(turns: usize) -> ConversationRequest {
    ConversationRequest::from_items(sample_items(turns)).with_model("test-model")
}

fn chat_completions_bytes(req: &ConversationRequest) -> Vec<u8> {
    let wire: crate::types::ChatCompletionRequest = req.clone().into();
    serde_json::to_vec(&wire).unwrap()
}

fn messages_bytes(req: &ConversationRequest) -> Vec<u8> {
    serde_json::to_vec(&build_messages_request(req)).unwrap()
}

/// The same logical conversation, built twice from independently constructed
/// items, must serialize byte-identically on every protocol. This is what lets
/// the provider cache serve turn N+1 from turn N's prefix.
#[test]
fn chat_completions_construction_is_byte_deterministic() {
    let a = chat_completions_bytes(&sample_request(2));
    let b = chat_completions_bytes(&sample_request(2));
    assert_eq!(a, b, "identical logical requests must be byte-equal");
}

#[test]
fn messages_construction_is_byte_deterministic() {
    let a = messages_bytes(&sample_request(2));
    let b = messages_bytes(&sample_request(2));
    assert_eq!(a, b, "identical logical requests must be byte-equal");
}

/// Appending a turn must not change the message list of everything before it —
/// on the Chat Completions wire, where the user-turn merge post-pass could in
/// principle rewrite earlier messages. Compared element-wise because a JSON
/// array of different length cannot be byte-prefix-stable (the closing bracket).
#[test]
fn chat_completions_prefix_stable_across_turns() {
    let messages = |req: &ConversationRequest| {
        let wire: crate::types::ChatCompletionRequest = req.clone().into();
        serde_json::to_value(&wire.messages).unwrap()
    };
    let base = messages(&sample_request(2));
    let extended = messages(&sample_request(3));
    let (serde_json::Value::Array(base), serde_json::Value::Array(extended)) = (base, extended)
    else {
        panic!("messages serialize to an array");
    };
    assert!(extended.len() > base.len(), "extended request must be longer");
    assert_eq!(
        &extended[..base.len()],
        base.as_slice(),
        "request N+1's message list must extend request N's element-for-element"
    );
}

/// Messages-path prefix stability, modulo the rolling `cache_control`
/// breakpoint markers: `apply_cache_breakpoints` intentionally moves the
/// tip marker onto each new turn (that is how the provider cache entry
/// extends), so the markers differ across turns while the marked content
/// stays identical.
#[test]
fn messages_prefix_stable_across_turns_modulo_breakpoint_markers() {
    fn strip(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(map) => {
                map.remove("cache_control");
                map.values_mut().for_each(strip);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let messages = |req: &ConversationRequest| {
        let mut value = serde_json::to_value(&build_messages_request(req).messages).unwrap();
        strip(&mut value);
        value
    };
    let base = messages(&sample_request(2));
    let extended = messages(&sample_request(3));
    let (serde_json::Value::Array(base), serde_json::Value::Array(extended)) = (base, extended)
    else {
        panic!("messages serialize to an array");
    };
    assert!(extended.len() > base.len(), "extended request must be longer");
    assert_eq!(
        &extended[..base.len()],
        base.as_slice(),
        "request N+1's message list must extend request N's element-for-element \
         (modulo rolling cache_control markers)"
    );
}

/// Responses-path byte determinism (prefix stability across turns has its own
/// dedicated suite in `test_support.rs` / `responses_tests.rs`).
#[test]
fn responses_construction_is_byte_deterministic() {
    let to_bytes = |req: &ConversationRequest| {
        let cr: crate::rs::CreateResponse = req.into();
        serde_json::to_vec(&cr).unwrap()
    };
    assert_eq!(
        to_bytes(&sample_request(2)),
        to_bytes(&sample_request(2)),
        "identical logical requests must be byte-equal"
    );
}

/// Ported from mimo-code's `fork-prefix-invariant.test.ts` invariant statement:
/// "any future change to system/tools/messages construction that introduces a
/// caller-conditional branch breaks this assertion." A fork shares the parent's
/// context and appends its own tail, so the parent's serialized message list
/// must be an element-wise prefix of the fork's — that is what lets the fork
/// inherit the parent's provider cache entry instead of re-billing the prompt.
/// (Upstream's service-layer test is skipped for flakiness; the invariant lives
/// at the conversion layer here, which is where a conditional branch would bite.)
#[test]
fn fork_request_extends_parent_prefix() {
    let parent = sample_request(2);

    let mut fork_items = sample_items(2);
    // Fork spawn injects a one-shot context snapshot after the shared prefix.
    fork_items.push(ConversationItem::user_meta(
        "You are a subagent. The history above is reference material, not your own.",
    ));
    fork_items.push(ConversationItem::user("Summarize the failure mode"));
    let fork = ConversationRequest::from_items(fork_items).with_model("test-model");

    let chat_messages = |r: &ConversationRequest| {
        let w: crate::types::ChatCompletionRequest = r.clone().into();
        serde_json::to_value(&w.messages).unwrap()
    };
    let anthropic_messages = |r: &ConversationRequest| {
        serde_json::to_value(&build_messages_request(r).messages).unwrap()
    };
    let wires: [&dyn Fn(&ConversationRequest) -> serde_json::Value; 2] =
        [&chat_messages, &anthropic_messages];
    for to_value in wires {
        let (serde_json::Value::Array(parent_msgs), serde_json::Value::Array(fork_msgs)) =
            (to_value(&parent), to_value(&fork))
        else {
            panic!("messages serialize to an array");
        };
        // The fork tail is user-role, so the user-turn merge absorbs it into
        // the parent's final user message rather than appending new turns.
        // The guarantee that survives: everything before that final shared
        // user turn is element-for-element identical, which is what lets the
        // fork hit the parent's provider cache entry up to the last turn.
        assert!(fork_msgs.len() >= parent_msgs.len() - 1);
        assert_eq!(
            &fork_msgs[..parent_msgs.len() - 1],
            &parent_msgs[..parent_msgs.len() - 1],
            "fork must preserve the parent's serialized prefix up to the last user turn"
        );
        let merged_last = serde_json::to_string(&fork_msgs.last().unwrap()).unwrap();
        assert!(
            merged_last.contains("next step 1")
                && merged_last.contains("reference material, not your own.")
                && merged_last.contains("Summarize the failure mode"),
            "fork's final turn carries the parent's final user content plus the fork tail: {merged_last}"
        );
    }
}
