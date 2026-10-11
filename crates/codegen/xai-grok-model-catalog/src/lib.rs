//! LOCAL: 模型目录纯函数层 —— models.dev / OpenRouter 双源取数、解析与置信匹配。
//!
//! 拍板口径（`docs-local/provider-onboarding-api-cost-todo.md` 六轮 + 2026-10-11 修订）：
//! 价格目录 TTL 1 天起步（可配，缓存面在 shell）；**只留原厂定价**——models.dev
//! 解析层按一方 provider 白名单过滤（`match_model::FIRST_PARTY_PROVIDERS`：主流大厂
//! 原厂 + OpenRouter 官方牌价镜像），第三方转售商条目整库不留；OpenRouter 官方 API
//! 为兜底源（`vendor/model` 形态 id 的权威）。models.dev cost = vendor 官方参考价
//! （per-1M USD），OpenRouter pricing = 官方牌价（per-token USD 字符串）。
//! 来源优先级：models.dev 主、OpenRouter 兜底 —— 调用方按 [`prepare_requests`]
//! 顺序尝试，首个「HTTP 200 且 [`parse_response`] 成功」的来源生效。
//!
//! 职责边界：本 crate 只做「端点组装 + 响应 → [`ModelCatalog`] + 置信匹配 +
//! 成本计算」的纯函数，HTTP 与 TTL/磁盘缓存由调用方执行（shell 扩展层），与
//! `xai-grok-provider-usage` 同一分层。匹配语义参照用户自有 modelq
//! （`D:/CODE/ai/openrouter-cli` `internal/modelsdev/match.go`）的置信档：
//! exact → variant 剥离 → vendor 别名 → canonical → 归一化；fuzzy 子串为噪声。

mod cost;
mod match_model;
mod modelsdev;
mod openrouter;
mod requests;
mod types;

pub use cost::{cost_usd, UsageTokens};
pub use match_model::{match_model, norm_id, collapse, CONFIDENT_MIN_SCORE};
pub use modelsdev::parse as parse_models_dev;
pub use openrouter::parse as parse_openrouter;
pub use requests::prepare_requests;
pub use types::{
    CatalogEntry, CatalogError, CatalogSource, MatchResult, ModelCatalog, ModelPricing,
    PreparedRequest,
};

/// 解析一个目录来源的响应体。来源与请求配对（[`prepare_requests`] 的序号）。
pub fn parse_response(source: CatalogSource, body: &str) -> Result<ModelCatalog, CatalogError> {
    match source {
        CatalogSource::OpenRouter => openrouter::parse(body),
        CatalogSource::ModelsDev => modelsdev::parse(body),
    }
}

#[cfg(test)]
pub(crate) mod test_support;
