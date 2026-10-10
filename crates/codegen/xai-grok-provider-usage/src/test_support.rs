//! 测试共享帮助：统一时刻的快照解析与断言。
//!
//! 每家解析器的夹具测试都经 `parse_response` 全链路（含状态码分层），而不是直接调
//! 内部函数，保证测试面与调用方一致。

use crate::types::{PlanUsageSnapshot, PlanUsageWindow, ProviderUsageError, UsageProviderId, UsageWindowKind};

pub(crate) const NOW_MS: u64 = 1_760_000_000_000;

pub(crate) fn parse_ok(provider: UsageProviderId, status: u16, body: &str) -> PlanUsageSnapshot {
    crate::parse_response(provider, status, body, NOW_MS).expect("parse ok")
}

pub(crate) fn parse_err(provider: UsageProviderId, status: u16, body: &str) -> ProviderUsageError {
    crate::parse_response(provider, status, body, NOW_MS).expect_err("parse err")
}

pub(crate) fn pct_of(snapshot: &PlanUsageSnapshot, kind: UsageWindowKind) -> Option<f64> {
    window_of(snapshot, kind).and_then(|w| w.used_pct)
}

pub(crate) fn window_of<'a>(
    snapshot: &'a PlanUsageSnapshot,
    kind: UsageWindowKind,
) -> Option<&'a PlanUsageWindow> {
    snapshot.windows.iter().find(|w| w.kind == kind)
}
