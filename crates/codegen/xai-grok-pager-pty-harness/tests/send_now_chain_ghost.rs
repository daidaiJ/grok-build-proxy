//! Send-now chain (typed Ctrl+Enter + force-sent queued row) must not ghost a bubble.

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore] // opt-in: real pager binary in a PTY (CI runs with --ignored)
async fn send_now_chain_leaves_no_ghost_bubble() {
    xai_grok_pager_pty_harness::scenarios::send_now_chain_ghost::assert_send_now_chain_leaves_no_ghost_bubble()
        .await
        .expect("send-now chain must not duplicate a user-prompt bubble");
}
