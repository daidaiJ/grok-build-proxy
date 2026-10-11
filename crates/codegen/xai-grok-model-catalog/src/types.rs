//! 目录条目与价格的数据模型（纯数据，无 IO）。

use serde::{Deserialize, Serialize};

/// 目录来源（拍板 4：OpenRouter 主、models.dev 兜底；取数按序尝试，首个成功者生效）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSource {
    /// OpenRouter `/api/v1/models` —— OpenRouter 路由模型的官方牌价（per-token USD 字符串）。
    OpenRouter,
    /// models.dev `/api.json` —— 供应商官方参考价（per-1M USD 浮点）。
    ModelsDev,
}

/// 模型价格，统一换算成 **USD per 1M tokens**；`None` = 目录未列该价。
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}

impl ModelPricing {
    /// 模型是否完全无价（四个价位都缺失）——渲染层据此显示 `–` 并从合计剔除。
    pub fn is_unpriced(&self) -> bool {
        self.input.is_none()
            && self.output.is_none()
            && self.cache_read.is_none()
            && self.cache_write.is_none()
    }
}

/// 一条目录条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// 目录内 id：OpenRouter 为 `vendor/model` 形态，models.dev 为 provider 内模型 id。
    pub id: String,
    /// 供应组分隔键（OpenRouter 条目固定 `openrouter`；models.dev 为其 provider id）。
    pub provider_id: String,
    pub source: CatalogSource,
    pub name: String,
    /// 规范 id（OpenRouter `canonical_slug` / models.dev `canonical_model_id`）。
    pub canonical_id: Option<String>,
    pub pricing: ModelPricing,
    pub context_length: Option<u64>,
    pub max_completion_tokens: Option<u64>,
}

/// 待执行请求（URL + 请求头）；HTTP 由调用方（shell 扩展层）执行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

/// 解析后的目录。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelCatalog {
    pub entries: Vec<CatalogEntry>,
}

impl ModelCatalog {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// 置信匹配结果（只含 [`crate::match_model`] 的置信档，fuzzy 子串噪声不参与）。
#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult<'a> {
    pub entry: &'a CatalogEntry,
    pub score: u8,
}

/// 解析错误（来源 + 原因）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError {
    pub source: CatalogSource,
    pub message: String,
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "catalog parse failed ({:?}): {}", self.source, self.message)
    }
}

impl std::error::Error for CatalogError {}
