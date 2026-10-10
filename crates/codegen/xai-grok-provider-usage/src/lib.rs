//! LOCAL: 供应商套餐用量纯函数层 —— 「推理同一把 SK 直查套餐窗口用量」的适配器。
//!
//! 设计与供应商口径见 `docs-local/usage/provider-quota-display-todo.md`（端点/响应
//! 结构/百分比语义依据 `docs-local/usage/quota-endpoints-and-credentials.md`，解析
//! 参照 cc-switch `coding_plan.rs` 与 yetone/magpie `planquota.go` 的实现与注释）。
//!
//! 职责边界：本 crate 只做「端点组装 + 请求头 + 响应 → [`PlanUsageSnapshot`]」的
//! 纯函数，HTTP 由调用方执行（shell 扩展层）。百分比语义在适配器内归一为
//! **已用** 0–100（opencode/GLM 原生已用、Command Code 由美元 used/cap 换算、
//! Kimi 由计数换算、MiniMax 剩余反转）。
//!
//! 多次尝试：GLM 的裸 key/Bearer 前缀分歧、MiniMax 的新旧两条端点路径，统一建模
//! 为「按序尝试的 [`PreparedRequest`] 列表」：解析返回
//! [`ProviderUsageError::TryNextAttempt`] 或 HTTP 404 且还有剩余尝试时，调用方换
//! 下一个请求重试。

mod parse;
mod providers;
mod requests;
mod types;

pub use requests::prepare_requests;
pub use types::{
    PlanUsageSnapshot, PlanUsageWindow, PreparedRequest, ProviderUsageError, UsageProviderId,
    UsageWindowKind,
};

/// 解析一次 HTTP 响应为快照。`now_ms` 是调用方时刻（快照的 `fetched_at_ms`）。
pub fn parse_response(
    provider: UsageProviderId,
    status: u16,
    body: &str,
    now_ms: u64,
) -> Result<PlanUsageSnapshot, ProviderUsageError> {
    parse::parse_response(provider, status, body, now_ms)
}

#[cfg(test)]
pub(crate) mod test_support;
