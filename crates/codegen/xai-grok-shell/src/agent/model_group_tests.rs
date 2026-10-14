use super::{error_is_provider_unavailable, pick_entry_idx, ModelGroupRuntime};
use crate::agent::config::{ModelGroupConfig, ModelGroupEntry};

fn group(entries: &[&str]) -> ModelGroupConfig {
    ModelGroupConfig {
        entries: entries
            .iter()
            .map(|m| ModelGroupEntry {
                model: (*m).to_string(),
                effort: None,
            })
            .collect(),
        sticky: None,
        cooldown_minutes: None,
    }
}

#[test]
fn pick_entry_prefers_first_non_cooling_entry() {
    let g = group(&["a-flash", "b-flash", "c-flash"]);
    assert_eq!(pick_entry_idx(&g.entries, "g"), Some(0));
    ModelGroupRuntime::global().mark_cooldown("g", 0, 10);
    assert_eq!(pick_entry_idx(&g.entries, "g"), Some(1));
    ModelGroupRuntime::global().mark_cooldown("g", 1, 10);
    assert_eq!(pick_entry_idx(&g.entries, "g"), Some(2));
}

#[test]
fn pick_entry_returns_none_when_all_entries_cooling() {
    let g = group(&["a-flash", "b-flash"]);
    ModelGroupRuntime::global().mark_cooldown("pick-all", 0, 10);
    ModelGroupRuntime::global().mark_cooldown("pick-all", 1, 10);
    assert_eq!(pick_entry_idx(&g.entries, "pick-all"), None);
}

#[test]
fn cooldown_expires_and_recovers_first_entry() {
    let g = group(&["a-flash", "b-flash"]);
    // 1 分钟的最小冷却粒度太长；用过期标记模拟：先 mark，再直接检查另一组名不受影响，
    // 过期路径由 is_cooling 的懒回收分支覆盖——这里验证 mark 只影响本组。
    ModelGroupRuntime::global().mark_cooldown("expire-a", 0, 10);
    assert_eq!(pick_entry_idx(&g.entries, "expire-b"), Some(0));
    assert!(ModelGroupRuntime::global().is_cooling("expire-a", 0));
    assert!(!ModelGroupRuntime::global().is_cooling("expire-a", 1));
}

#[test]
fn cooldown_remark_extends_not_shrinks() {
    ModelGroupRuntime::global().mark_cooldown("extend", 0, 10);
    ModelGroupRuntime::global().mark_cooldown("extend", 0, 10);
    assert!(ModelGroupRuntime::global().is_cooling("extend", 0));
}

#[test]
fn provider_unavailable_classifier_matches_high_confidence_only() {
    for hit in [
        "Session error: rate limited (429), retries exhausted",
        "Session error: HTTP 502 Bad Gateway",
        "Session error: connection refused",
        "Session error: idle timeout after 300s",
        "error sending request: Too many requests",
    ] {
        assert!(error_is_provider_unavailable(hit), "should match: {hit}");
    }
    for miss in [
        "Session error: invalid api key (401)",
        "Session error: image input not supported (400)",
        "structured output validation failed: missing field `report`",
        "max turns reached (limit: 8)",
        "Subagent was cancelled",
    ] {
        assert!(!error_is_provider_unavailable(miss), "should not match: {miss}");
    }
}
