//! 目录条目与价格的数据模型（纯数据，无 IO）。

use std::collections::HashMap;

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
    /// 是否支持思考/推理（OpenRouter `supported_parameters` 含 `reasoning` /
    /// models.dev `reasoning` 布尔）；`None` = 源未给该字段。
    #[serde(default)]
    pub reasoning: Option<bool>,
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

/// 解析后的目录。构建时一次性预建匹配索引（[`ModelCatalog::new`]），
/// 查询走哈希候选取回（见 [`crate::match_model`]），避免每次全库线性扫描；
/// 索引不参与 serde（磁盘缓存反序列化后按条目重建）。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ModelCatalog {
    /// 只能经 [`Self::new`] 整体注入（索引与条目强一致，外部直接改条目会使
    /// 索引失效）；crate 内只读访问。serde 序列化 = 条目数组（反序列化走
    /// 手写 impl 重建索引）。
    pub(crate) entries: Vec<CatalogEntry>,
    #[serde(skip)]
    index: CatalogIndex,
}

impl ModelCatalog {
    pub fn new(entries: Vec<CatalogEntry>) -> Self {
        let mut catalog = Self {
            entries,
            index: CatalogIndex::default(),
        };
        catalog.rebuild_index();
        catalog
    }

    /// 依据当前条目重建匹配索引（幂等；条目只能经 [`Self::new`] 注入）。
    fn rebuild_index(&mut self) {
        let mut index = CatalogIndex::default();
        for (pos, entry) in self.entries.iter().enumerate() {
            let pos = u32::try_from(pos).expect("catalog entry count overflows u32");
            index.by_id.entry(entry.id.clone()).or_default().push(pos);
            if let Some(canonical) = &entry.canonical_id {
                if !canonical.is_empty() {
                    index
                        .by_canonical
                        .entry(crate::match_model::norm_id(canonical))
                        .or_default()
                        .push(pos);
                }
            }
            index
                .by_vendor
                .entry(crate::match_model::collapse(&entry.provider_id))
                .or_default()
                .push(pos);
            // model 段键：`/` 后半段（无 `/` 即整 id）；同时插剥部署戳的备选键，
            // 两侧（查询/条目）任一侧带戳都能命中（stamp_tolerant_eq 的索引化）。
            let segment = entry
                .id
                .rsplit_once('/')
                .map_or(entry.id.as_str(), |(_, m)| m);
            let norm = crate::match_model::norm_id(segment);
            index.by_segment.entry(norm.clone()).or_default().push(pos);
            let stripped = crate::match_model::strip_trailing_stamp(&norm);
            if stripped != norm {
                index.by_segment.entry(stripped.to_string()).or_default().push(pos);
            }
        }
        self.index = index;
    }

    pub(crate) fn index(&self) -> &CatalogIndex {
        &self.index
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl<'de> Deserialize<'de> for ModelCatalog {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        // 与 derive Serialize 同形（`{"entries": [...]}`，index 已 skip），读入后重建索引。
        #[derive(serde::Deserialize)]
        struct Raw {
            entries: Vec<CatalogEntry>,
        }
        Ok(Self::new(Raw::deserialize(deserializer)?.entries))
    }
}

/// 匹配倒排索引：`fetch`/磁盘缓存加载时构建一次，查询按哈希取回候选下标，
/// 平局裁定（score → authority → provider/id 字典序）在候选集上进行。
/// 全部键为归一化形态（`norm_id` / `collapse`），见各字段注释。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogIndex {
    /// 原样完整 id（含 `:variant`，大小写敏感）→ 条目下标（100/95 档）。
    by_id: HashMap<String, Vec<u32>>,
    /// 归一化 canonical id → 下标（85 档）。
    by_canonical: HashMap<String, Vec<u32>>,
    /// `collapse(provider_id)` → 下标（vendor 段识别 86/84 档）。
    by_vendor: HashMap<String, Vec<u32>>,
    /// 归一化 model 段（最后一个 `/` 之后；无 `/` 条目为整 id）→ 下标
    /// （84 档：裸 id 查询 / 未知 vendor 剥壳共用，`:variant` 条目键自带后缀、
    /// 不会被裸查询命中）。
    by_segment: HashMap<String, Vec<u32>>,
}

impl CatalogIndex {
    pub(crate) fn by_id_candidates(&self, id: &str) -> &[u32] {
        self.by_id.get(id).map_or(&[], |v| v)
    }

    pub(crate) fn by_canonical_candidates(&self, norm: &str) -> &[u32] {
        self.by_canonical.get(norm).map_or(&[], |v| v)
    }

    pub(crate) fn by_vendor_candidates(&self, vendor: &str) -> &[u32] {
        self.by_vendor
            .get(&crate::match_model::collapse(vendor))
            .map_or(&[], |v| v)
    }

    /// vendor 段是否对上某个 provider（collapse 别名折叠，`z-ai` → `zai`）。
    pub(crate) fn vendor_known(&self, vendor: &str) -> bool {
        self.by_vendor.contains_key(&crate::match_model::collapse(vendor))
    }

    /// model 段候选：查归一化键 + 剥部署戳键（任一侧带戳都命中，stamp_tolerant_eq
    /// 的索引化；同条目可能两键都返回，`consider` 幂等）。
    pub(crate) fn by_segment_candidates<'s>(
        &'s self,
        model: &str,
    ) -> impl Iterator<Item = &'s u32> + 's {
        let norm = crate::match_model::norm_id(model);
        let stripped = crate::match_model::strip_trailing_stamp(&norm).to_string();
        let exact = self.by_segment.get(&norm);
        let tolerance = if stripped == norm {
            None
        } else {
            self.by_segment.get(&stripped)
        };
        exact
            .into_iter()
            .flatten()
            .chain(tolerance.into_iter().flatten())
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
