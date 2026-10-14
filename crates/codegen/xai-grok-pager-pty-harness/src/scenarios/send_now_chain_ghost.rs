//! Send-now chains must not leave a ghost user-prompt bubble.
//!
//! Reproduction for the live-reported display bug: after a Ctrl+Enter send-now
//! (typed text or force-sent queued row), the transcript occasionally shows the
//! previous prompt's bubble again below the new prompt. The model's context
//! stays clean — the duplicate exists only in the live scrollback.
//!
//! ```bash
//! cargo test -p xai-grok-pager-pty-harness --test send_now_chain_ghost -- --ignored --nocapture
//! ```

use std::time::Duration;

use anyhow::{Context, Result, bail};

use super::wait_for_welcome;
use crate::{ContentController, PtyHarness, pager_binary};

const DEFAULT_ROWS: u16 = 50;
const DEFAULT_COLS: u16 = 120;
/// Ctrl+Enter under the kitty keyboard protocol the pager pushes at startup.
const CTRL_ENTER: &[u8] = b"\x1b[13;5u";

fn slow_turn_text(sentinel: &str) -> String {
    let mut s = String::from(sentinel);
    for i in 0..30 {
        s.push_str(&format!(" streaming{i}"));
    }
    s
}

fn screen_count(screen: &str, needle: &str) -> usize {
    screen.matches(needle).count()
}

/// The exact mixed flow from the reporter's sessions: queue a row mid-turn via
/// Enter, send separately-typed text via Ctrl+Enter (cancel-and-send), then
/// force-send the queued row via Ctrl+Enter on the empty composer.
pub async fn assert_send_now_chain_leaves_no_ghost_bubble() -> Result<()> {
    let content = ContentController::start()
        .await
        .context("start ContentController")?;
    let mut turn_a = content
        .expect_agent_turn_blocked("turn A", slow_turn_text("TURN_A_OUTPUT"));
    let mut turn_b = content
        .expect_agent_turn_blocked("turn B", slow_turn_text("TURN_B_OUTPUT"));
    let mut turn_c = content
        .expect_agent_turn_blocked("turn C", slow_turn_text("TURN_C_OUTPUT"));

    content
        .server()
        .set_settings(serde_json::json!({ "allow_access": true }));

    let binary = pager_binary().context("resolve pager binary")?;
    let mut harness = PtyHarness::spawn_with_content_env_in_dir(
        &binary,
        DEFAULT_ROWS,
        DEFAULT_COLS,
        &content,
        &[],
        &[],
        Some(content.home()),
    )
    .context("spawn pager")?;

    wait_for_welcome(&mut harness).await?;

    // Turn A: idle submit.
    harness.inject_keys(b"go\r").context("submit turn A")?;
    harness
        .wait_for_text("TURN_A_OUTPUT", Duration::from_secs(30))
        .context("turn A streaming")?;
    tokio::time::timeout(Duration::from_secs(10), turn_a.wait_blocked())
        .await
        .context("turn A barrier timeout")?;

    // Queue a follow-up row mid-turn (plain Enter): shows in the queue pane only.
    harness
        .inject_keys(b"BBBB second\r")
        .context("queue follow-up row")?;
    harness
        .wait_for_text("BBBB second", Duration::from_secs(10))
        .context("queued row visible")?;

    // Type different text and send-now it via Ctrl+Enter: cancels turn A and
    // runs the typed text next.
    harness
        .inject_keys(b"CCCC third")
        .context("type send-now text")?;
    harness.inject_keys(CTRL_ENTER).context("Ctrl+Enter send-now")?;
    turn_a.release();
    harness
        .wait_for_text("\u{276F} CCCC third", Duration::from_secs(15))
        .context("send-now bubble chrome")?;
    harness
        .wait_for_text("TURN_B_OUTPUT", Duration::from_secs(40))
        .context("turn B reply")?;
    tokio::time::timeout(Duration::from_secs(10), turn_b.wait_blocked())
        .await
        .context("turn B barrier timeout")?;

    // Empty-composer Ctrl+Enter force-sends the top queued row: cancels turn B
    // and runs the row.
    harness.inject_keys(CTRL_ENTER).context("force-send queue row")?;
    harness
        .wait_for_text("\u{276F} BBBB second", Duration::from_secs(15))
        .context("force-sent row bubble chrome")?;
    harness
        .wait_for_text("TURN_C_OUTPUT", Duration::from_secs(40))
        .context("turn C reply")?;
    tokio::time::timeout(Duration::from_secs(10), turn_c.wait_blocked())
        .await
        .context("turn C barrier timeout")?;

    turn_b.release();
    turn_c.release();
    tokio::time::timeout(Duration::from_secs(15), turn_c.wait_satisfied())
        .await
        .context("turn C completion timeout")?;
    // Give late broadcasts/echoes a moment to (mis)paint before asserting.
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let screen = harness.screen_contents();
    for (needle, expected) in [
        ("\u{276F} CCCC third", 1),
        ("\u{276F} BBBB second", 1),
        ("\u{276F} go", 1),
    ] {
        let got = screen_count(&screen, needle);
        if got != expected {
            bail!("bubble `{needle}` rendered {got} times (expected {expected})\n{screen}");
        }
    }
    // The queued-row pane representation must not linger as a transcript bubble,
    // and the queue hint must be gone once every row has run.
    if screen.contains("Queued ·") {
        bail!("stale queue hint visible after send-now\n{screen}");
    }
    if harness.contains_text("Turn cancelled by user") {
        bail!("send-now cancel must stay silent\n{screen}");
    }
    if harness.contains_text("panicked") {
        bail!("pager panicked\n{screen}");
    }

    // The model must see each prompt exactly once (context stays clean).
    fn collect_user_texts(body: &serde_json::Value, out: &mut Vec<String>) {
        let Some(items) = body["messages"]
            .as_array()
            .or_else(|| body["input"].as_array())
        else {
            return;
        };
        for m in items {
            if m["role"] == "user" {
                let text = match &m["content"] {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                out.push(text);
            }
        }
    }
    let mut user_texts: Vec<String> = Vec::new();
    for body in content.request_bodies() {
        collect_user_texts(&body, &mut user_texts);
    }
    for needle in ["go", "CCCC third", "BBBB second"] {
        let got = user_texts.iter().filter(|t| t.contains(needle)).count();
        if got != 1 {
            bail!("wire saw `{needle}` {got} times (expected 1)\n{user_texts:#?}");
        }
    }

    harness.quit().context("clean quit")?;
    Ok(())
}
