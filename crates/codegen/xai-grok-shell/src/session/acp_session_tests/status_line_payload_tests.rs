use super::support::create_test_actor;
use serde_json::Value;
use std::sync::atomic::Ordering;
use xai_grok_sampling_types::{ConversationItem, ConversationResponse, TokenUsage};

/// A completed call with `completion` output tokens; usage present so the
/// status line's perf snapshot has an output-token count to divide by.
fn response_with_usage(completion: u32) -> ConversationResponse {
    ConversationResponse {
        items: vec![ConversationItem::assistant("ok")],
        stop_reason: None,
        usage: Some(TokenUsage {
            prompt_tokens: 4_000,
            completion_tokens: completion,
            total_tokens: 4_000 + completion,
            reasoning_tokens: 0,
            cached_prompt_tokens: 0,
            cache_creation_prompt_tokens: 0,
        }),
        cost_usd_ticks: None,
        message_chunks_emitted: 1,
        doom_loop_signals: Vec::new(),
        stop_message: None,
        message_id: None,
        raw_stop_reason: None,
        stop_sequence: None,
    }
}

// Only the string arm can fire on a built payload: every wire `Option` is marked `skip_serializing_if`
fn assert_no_placeholders(value: &Value, path: &str) {
    match value {
        Value::String(text) => assert!(!text.is_empty(), "{path} is empty, not omitted"),
        Value::Null => panic!("{path} is null; omit the field instead"),
        Value::Object(fields) => {
            for (key, child) in fields {
                assert_no_placeholders(child, &format!("{path}.{key}"));
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                assert_no_placeholders(child, &format!("{path}[{i}]"));
            }
        }
        Value::Bool(_) | Value::Number(_) => {}
    }
}

#[tokio::test]
async fn the_payload_carries_real_values_or_no_field_at_all() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(50_000, 100_000, 85, gateway_tx, persistence_tx).await;

            let ctx = actor.build_status_context().await;
            // The payload promises two of its fields are copies of other fields
            // Both are built from one source today, so this fails the day one of them is sourced separately and the promise quietly stops holding
            assert_eq!(ctx.cwd, ctx.workspace.current_dir);
            if let Some(worktree) = &ctx.worktree {
                assert_eq!(worktree.branch, ctx.workspace.branch);
            }

            let value = serde_json::to_value(ctx).unwrap();
            assert_no_placeholders(&value, "payload");
            assert_eq!(value["schema_version"], 1);
        })
        .await;
}

#[tokio::test]
async fn the_perf_snapshot_prefers_the_sampler_sample() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(50_000, 100_000, 85, gateway_tx, persistence_tx).await;

            actor
                .signals_handle()
                .record_inference_metrics(xai_grok_sampler::InferenceLatencyStats {
                    time_to_first_token_ms: Some(380),
                    time_to_last_byte_ms: 2_900,
                    chunk_count: 20,
                    itl_intervals_ms: vec![120; 19],
                    itl_p50_ms: Some(120),
                    itl_p99_ms: Some(120),
                    itl_max_ms: Some(120),
                    itl_mean_ms: Some(120),
                    attempts: 1,
                });
            actor.last_turn_api_duration_ms.store(3_000, Ordering::Relaxed);
            actor.record_response_token_usage(&response_with_usage(50), Some(3_000));

            let perf = actor.build_turn_perf().await.expect("perf snapshot");
            assert_eq!(perf.ttft_ms, Some(380), "sampler sample wins");
            // Stream window: 3000ms API − 380ms TTFT over 50 output tokens.
            assert_eq!(perf.tps, Some(19.1));
        })
        .await;
}

#[tokio::test]
async fn a_buffered_call_falls_back_to_the_shell_measured_ttft() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(50_000, 100_000, 85, gateway_tx, persistence_tx).await;

            // Buffered delivery: the sampler saw no stream (0ms artifact skipped),
            // the shell measured the first output 5.9s into the 6.2s call.
            {
                let mut perf = actor.shell_turn_perf.lock();
                perf.attempt_first_output_ms = 5_900;
                perf.last_ttft_ms = 5_900;
            }
            actor.last_turn_api_duration_ms.store(6_200, Ordering::Relaxed);
            actor.record_response_token_usage(&response_with_usage(50), Some(6_200));

            let perf = actor.build_turn_perf().await.expect("perf snapshot");
            assert_eq!(perf.ttft_ms, Some(5_900), "shell fallback fills in");
            // Whole-call window: 6200ms over 50 output tokens.
            assert_eq!(perf.tps, Some(8.1));
        })
        .await;
}

#[tokio::test]
async fn a_call_with_neither_sample_nor_shell_output_reads_no_perf() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (gateway_tx, _gateway_rx) = tokio::sync::mpsc::unbounded_channel();
            let (persistence_tx, _persistence_rx) = tokio::sync::mpsc::unbounded_channel();
            let actor = create_test_actor(50_000, 100_000, 85, gateway_tx, persistence_tx).await;

            actor.record_response_token_usage(&response_with_usage(50), Some(3_000));

            let perf = actor.build_turn_perf().await.expect("perf snapshot");
            assert_eq!(perf.ttft_ms, None);
            assert_eq!(perf.tps, None);
        })
        .await;
}
