//! 置信匹配：把用户侧模型 id（配置 slug 或上游回显 id）对到目录条目。
//!
//! 移植 modelq `internal/modelsdev/match.go` 的置信档（按分排序，平分时按
//! 「来源 → 一方原厂 → provider/id 字典序」保证确定性）。**fuzzy 前缀/子串命中
//! 视为噪声、不富化**（modelq 同款语义，拍板沿用）：只返回 ≥84 的置信档。
//! 候选集来自 [`ModelCatalog`] 预建的哈希倒排索引（`types::CatalogIndex`，
//! fetch 时构建一次），匹配本身不再做全库线性扫描。
//!
//! | 分档 | 语义 |
//! |------|------|
//! | 100 | id 精确命中（含 `vendor/model` 全串） |
//! | 95  | 去掉 `:variant` 后精确命中（如 `:free`） |
//! | 86  | `vendor/model`：vendor 段对上 provider（含 `z-ai`→`zai` 别名折叠），model 段精确 |
//! | 85  | 规范 id（canonical）归一化后命中 |
//! | 84  | model 段归一化命中（vendor 已对上 / vendor 不识别剥壳取后半段 / 纯 id 分隔符变体 / 裸 id 对 `vendor/model` 条目按 model 段命中），部署戳后缀容忍（尾部 `-20260826` 等 6–8 位日期） |

use crate::types::{CatalogEntry, CatalogSource, MatchResult, ModelCatalog};

/// 置信门：低于该档的命中（vendor 前缀 / 子串噪声）一律不返回。
pub const CONFIDENT_MIN_SCORE: u8 = 84;

/// 一方（原厂）provider 白名单：models.dev 解析层**只保留**名单内 provider
///（拍板 2026-10-11：只查主流大厂原厂定价，第三方转售商条目不留——小厂模型
/// 少人用，不值得为它们留匹配面）。按 [`collapse`] 归一化后精确比对。
///
/// 刻意**不收**各厂 coding-plan / token-plan 订阅条目：订阅制无边际价
///（models.dev 上价格恒 0，见 [`crate::modelsdev`] 的零价归无价防护），进名单会让
/// 订阅 0 价盖过真实牌价。models.dev 的 `openrouter` 镜像 = OpenRouter 官方牌价，
/// 是 `vendor/model` 形态 id 的权威锚点，一并收录；OpenRouter 官方 API 整库为兜底源
///（[`CatalogSource::OpenRouter`]，免名单判定）。
const FIRST_PARTY_PROVIDERS: &[&str] = &[
    // 国外：gpt / claude / gemini / grok / muse
    "openai",
    "anthropic",
    "google",
    "google-vertex",
    "xai",
    "meta",
    // 国内：glm / deepseek / kimi / qwen / mimo / doubao / step / hy(混元)
    "zai",
    "zhipuai",
    "deepseek",
    "moonshotai",
    "moonshotai-cn",
    "alibaba",
    "alibaba-cn",
    "xiaomi",
    "volcengine",
    "stepfun",
    "stepfun-ai",
    "tencent-coding-plan",
    "tencent-token-plan",
    "tencent-tokenhub",
    // models.dev 内嵌的 OpenRouter 官方牌价镜像
    "openrouter",
];

/// 条目是否出自一方原厂 provider（OpenRouter 官方源整库视为一方）。
///
/// models.dev 解析层用同一判定做**条目过滤**（拍板 2026-10-11：只留原厂定价，
/// 第三方转售商条目不进目录），匹配层再用作平局裁定，两层共用一份名单。
pub(crate) fn is_first_party(entry: &CatalogEntry) -> bool {
    match entry.source {
        CatalogSource::OpenRouter => true,
        CatalogSource::ModelsDev => is_first_party_provider(&entry.provider_id),
    }
}

/// provider 归一化后是否一方原厂（models.dev 解析层在 provider 循环级提前跳过
/// 转售商，转售商模型条目不物化，压低 5.4MB 级响应的解析内存峰值）。
pub(crate) fn is_first_party_provider(provider_id: &str) -> bool {
    let provider = collapse(provider_id);
    FIRST_PARTY_PROVIDERS.iter().any(|p| collapse(p) == provider)
}

/// 原厂自证：条目 canonical 的 vendor 段与自家 provider id 一致
/// （如 deepseek 自家条目 canonical = `deepseek/...`，而 alibaba-cn 托管的同名
/// 条目 canonical vendor 段是 `deepseek`）——同价档平局时自家条目优先。
fn canonical_vendor_self(entry: &CatalogEntry) -> bool {
    entry
        .canonical_id
        .as_deref()
        .and_then(|c| c.split_once('/'))
        .is_some_and(|(vendor, _)| collapse(vendor) == collapse(&entry.provider_id))
}

/// 平局裁定键：来源（OpenRouter 官方 API 优先）→ 一方原厂 → 原厂内 canonical
/// 自证（deepseek 自家 0.15/0.6 优先于 alibaba-cn 托管的 0.14/0.28，output 差 2×）
/// → provider/id 字典序。
fn authority(entry: &CatalogEntry) -> (u8, u8, u8) {
    match entry.source {
        CatalogSource::OpenRouter => (0, 0, 0),
        CatalogSource::ModelsDev => (
            1,
            u8::from(!is_first_party(entry)),
            u8::from(!(is_first_party(entry) && canonical_vendor_self(entry))),
        ),
    }
}

/// 归一化：小写 + `_`/`.` 折成 `-`（保留 `/`）。
pub fn norm_id(s: &str) -> String {
    s.trim().to_lowercase().replace(['_', '.'], "-")
}

/// 剥离尾部部署戳（6–8 位纯数字段，如 `-20260826`）；4 位以下的日期型型号
///（`step-3.5-flash-2603`、`deepseek-v3-0324`）是独立型号，不动。
pub(crate) fn strip_trailing_stamp(s: &str) -> &str {
    if let Some(pos) = s.rfind('-') {
        let tail = &s[pos + 1..];
        let len = tail.len();
        if (6..=8).contains(&len) && tail.bytes().all(|b| b.is_ascii_digit()) {
            return &s[..pos];
        }
    }
    s
}

/// 归一化 + 部署戳容忍相等（任一侧带戳）。
fn stamp_tolerant_eq(a: &str, b: &str) -> bool {
    let (an, bn) = (norm_id(a), norm_id(b));
    an == bn
        || strip_trailing_stamp(&an) == bn
        || an == strip_trailing_stamp(&bn)
        || strip_trailing_stamp(&an) == strip_trailing_stamp(&bn)
}

/// 归一化后再去掉 `-`：把 `z-ai`/`zai`、`moonshot-ai`/`moonshotai` 这类 vendor 别名折到一起。
pub fn collapse(s: &str) -> String {
    norm_id(s).replace('-', "")
}

/// 在目录里找 query 的最佳置信匹配；无置信命中返回 `None`。
/// 候选取自 [`ModelCatalog`] 预建索引（`by_id` / `by_canonical` / `by_vendor` /
/// `by_segment`），命中比较与平局裁定语义与线性扫描版一致。
pub fn match_model<'a>(catalog: &'a ModelCatalog, query: &str) -> Option<MatchResult<'a>> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    let index = catalog.index();

    let mut best: Option<(u8, &'a CatalogEntry)> = None;
    let mut consider = |score: u8, entry: &'a CatalogEntry| {
        let better = match &best {
            None => true,
            Some((s, e)) => {
                score > *s
                    || (score == *s
                        && (authority(entry), &entry.provider_id, &entry.id)
                            < (authority(e), &e.provider_id, &e.id))
            }
        };
        if better {
            best = Some((score, entry));
        }
    };

    // `:variant`（`:free` / `:batch` / `:nitro` …）不是模型本体，剥离后参与各档匹配。
    let base = match q.split_once(':') {
        Some((b, _)) if !b.is_empty() => b,
        _ => q,
    };
    let bn = norm_id(base);

    // 100 / 95：id 精确命中（原样大小写，含 `:variant` 剥离）。
    for pos in index.by_id_candidates(q) {
        consider(100, &catalog.entries[*pos as usize]);
    }
    if base != q {
        for pos in index.by_id_candidates(base) {
            consider(95, &catalog.entries[*pos as usize]);
        }
    }

    // 86 / 85 / 84：vendor/model 拆段匹配 + canonical 反查。
    if let Some((vendor, model)) = base.split_once('/') {
        // canonical 反查与 vendor 识别无关，先做（85）。
        for pos in index.by_canonical_candidates(&bn) {
            consider(85, &catalog.entries[*pos as usize]);
        }
        // vendor 段别名折叠（z-ai → zai）经 by_vendor 的 collapse 键完成。
        for pos in index.by_vendor_candidates(vendor) {
            let entry = &catalog.entries[*pos as usize];
            if entry.id == model {
                consider(86, entry);
            } else if stamp_tolerant_eq(&entry.id, model) {
                consider(84, entry);
            }
        }
        if !index.vendor_known(vendor) {
            // vendor 段不识别（网关自有前缀，如 `cline-pass/glm-5.3-flash`）：
            // 剥壳取 `xx/` 后半段命中原厂条目——转售 id 也按原厂价披露
            // （拍板 2026-10-11：只留原厂定价）。
            for pos in index.by_segment_candidates(model) {
                consider(84, &catalog.entries[*pos as usize]);
            }
        }
    } else {
        // 无 vendor 段：canonical 反查仍适用；model 段容忍命中覆盖两类目录形态——
        // models.dev 裸 id（segment = 整 id）与 OpenRouter 兜底整库 `vendor/model`
        //（裸网关 id 按段命中，v1.0.51 活回合：主源瞬时失败静默回退时裸 id 全灭）。
        // `:variant` 条目的段键自带后缀，不会被裸查询命中（`:free` 零价不冒充牌价）。
        for pos in index.by_canonical_candidates(&bn) {
            consider(85, &catalog.entries[*pos as usize]);
        }
        for pos in index.by_segment_candidates(base) {
            consider(84, &catalog.entries[*pos as usize]);
        }
    }

    best.and_then(|(score, entry)| {
        (score >= CONFIDENT_MIN_SCORE).then_some(MatchResult { entry, score })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_catalog;

    fn top(query: &str) -> (String, CatalogSource, u8) {
        let cat = test_catalog();
        let m = match_model(&cat, query).unwrap_or_else(|| panic!("no match for {query}"));
        (m.entry.id.clone(), m.entry.source, m.score)
    }

    #[test]
    fn exact_openrouter_style_id_wins() {
        assert_eq!(
            top("x-ai/grok-4"),
            ("x-ai/grok-4".into(), CatalogSource::OpenRouter, 100)
        );
    }

    #[test]
    fn bare_exact_id_hits_provider_entry() {
        assert_eq!(
            top("glm-5.3-flash"),
            ("glm-5.3-flash".into(), CatalogSource::ModelsDev, 100)
        );
    }

    #[test]
    fn variant_suffix_strips_to_base_entry() {
        assert_eq!(
            top("deepseek/deepseek-chat-v3.1:batch"),
            ("deepseek/deepseek-chat-v3.1".into(), CatalogSource::OpenRouter, 95)
        );
        // variant 本体也是精确命中
        assert_eq!(top("deepseek/deepseek-chat-v3.1:free").2, 100);
    }

    #[test]
    fn vendor_alias_with_separator_folding() {
        assert_eq!(
            top("z-ai/glm-5.3-flash"),
            ("glm-5.3-flash".into(), CatalogSource::ModelsDev, 86)
        );
    }

    #[test]
    fn canonical_match_beats_normalized_vendor_match() {
        // canonical 归一化命中（85）优先于 vendor 段归一化命中（84）。
        let (id, source, score) = top("anthropic/claude_sonnet_4_5");
        assert_eq!((id.as_str(), source, score), ("claude-sonnet-4-5", CatalogSource::ModelsDev, 85));
    }

    #[test]
    fn normalized_model_segment_under_matched_vendor() {
        // vendor 段别名折叠命中（z-ai → zai），model 段仅归一化后相等 → 84
        // （zai 的 glm 条目无 canonical，不会先被 85 拦走）。
        let (id, source, score) = top("z-ai/glm_5.3_flash");
        assert_eq!(
            (id.as_str(), source, score),
            ("glm-5.3-flash", CatalogSource::ModelsDev, 84)
        );
    }

    #[test]
    fn tie_prefers_openrouter_source_then_provider_order() {
        // OpenRouter 有 `zai/glm-5.3-flash`（100），models.dev vendor 路径给 86 → 主源赢。
        let (id, source, score) = top("zai/glm-5.3-flash");
        assert_eq!((id.as_str(), source, score), ("zai/glm-5.3-flash", CatalogSource::OpenRouter, 100));
    }

    #[test]
    fn unknown_model_has_no_confident_match() {
        let cat = test_catalog();
        assert!(match_model(&cat, "totally-made-up-model").is_none());
    }

    #[test]
    fn fuzzy_substring_is_noise_and_never_matches() {
        let cat = test_catalog();
        assert!(match_model(&cat, "glm").is_none(), "前缀子串不富化");
        assert!(match_model(&cat, "grok").is_none());
    }

    #[test]
    fn empty_and_whitespace_queries_match_nothing() {
        let cat = test_catalog();
        assert!(match_model(&cat, "").is_none());
        assert!(match_model(&cat, "   ").is_none());
    }

    #[test]
    fn norm_and_collapse_helpers() {
        assert_eq!(norm_id("GLM_5.3_Flash"), "glm-5-3-flash");
        assert_eq!(collapse("Z-AI"), "zai");
        assert_eq!(collapse("moonshot-ai"), "moonshotai");
    }

    fn dev_entry(provider: &str, id: &str, input: f64) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            provider_id: provider.to_string(),
            source: CatalogSource::ModelsDev,
            name: format!("name-of-{id}"),
            canonical_id: None,
            reasoning: None,
            accepts_images: None,
            pricing: crate::types::ModelPricing {
                input: Some(input),
                output: Some(input * 3.0),
                cache_read: None,
                cache_write: None,
            },
            context_length: Some(128_000),
            max_completion_tokens: None,
        }
    }

    /// OpenRouter 源条目（`vendor/model` 形态 id；provider 键固定 openrouter）。
    fn or_entry(id: &str, input: f64) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            provider_id: "openrouter".to_string(),
            source: CatalogSource::OpenRouter,
            name: format!("name-of-{id}"),
            canonical_id: None,
            reasoning: None,
            accepts_images: None,
            pricing: crate::types::ModelPricing {
                input: Some(input),
                output: Some(input * 3.0),
                cache_read: None,
                cache_write: None,
            },
            context_length: Some(128_000),
            max_completion_tokens: None,
        }
    }

    #[test]
    fn first_party_provider_wins_reseller_tie() {
        // 裸 id 在 models.dev 挂在几十个 provider 下：一方原厂（zai 官方价）优先于
        // 字典序更靠前的转售商（302ai 半价）。
        let cat = ModelCatalog::new(vec![
            dev_entry("302ai", "glm-5.3-flash", 0.075),
            dev_entry("zai", "glm-5.3-flash", 0.15),
        ]);
        let m = match_model(&cat, "glm-5.3-flash").expect("match");
        assert_eq!(m.entry.provider_id, "zai");
        assert_eq!(m.entry.pricing.input, Some(0.15));
    }

    #[test]
    fn openrouter_mirror_provider_counts_as_first_party() {
        // models.dev 内嵌 openrouter 镜像 = 官方牌价，优先于网关转售条目。
        let cat = ModelCatalog::new(vec![
            dev_entry("kilo", "deepseek/deepseek-v4.1-flash", 0.4),
            dev_entry("openrouter", "deepseek/deepseek-v4.1-flash", 0.3),
        ]);
        let m = match_model(&cat, "deepseek/deepseek-v4.1-flash").expect("match");
        assert_eq!(m.entry.provider_id, "openrouter");
        assert_eq!(m.entry.pricing.input, Some(0.3));
    }

    #[test]
    fn canonical_vendor_self_breaks_first_party_ties() {
        // deepseek 自家条目（canonical vendor 段 = 自家 id）优先于 alibaba-cn 托管的
        // 同 id 条目——两者价格差 2×，字典序会错选 alibaba-cn。
        let mut official = dev_entry("deepseek", "deepseek-v4-flash", 0.15);
        official.canonical_id = Some("deepseek/deepseek-v4.1-flash".into());
        let mut hosted = dev_entry("alibaba-cn", "deepseek-v4-flash", 0.14);
        hosted.canonical_id = Some("deepseek/deepseek-v4-flash".into());
        let cat = ModelCatalog::new(vec![hosted, official]);
        let m = match_model(&cat, "deepseek-v4-flash").expect("match");
        assert_eq!(m.entry.provider_id, "deepseek");
        assert_eq!(m.entry.pricing.input, Some(0.15));
    }

    #[test]
    fn unknown_vendor_strips_to_official_model_segment() {
        // 网关自有前缀（cline-pass/xxx）不识别：剥壳按 model 段命中原厂条目。
        let cat = ModelCatalog::new(vec![dev_entry("zai", "glm-5.3-flash", 0.15)]);
        let m = match_model(&cat, "cline-pass/glm-5.3-flash").expect("match");
        assert_eq!(m.entry.provider_id, "zai");
        assert_eq!(m.score, 84);

        // vendor 段可识别时走原档位，剥壳逻辑不介入。
        let m2 = match_model(&cat, "zai/glm-5.3-flash").expect("match");
        assert_eq!(m2.score, 86);
    }

    #[test]
    fn bare_query_matches_vendor_prefixed_model_segment() {
        // OpenRouter 兜底整库 `vendor/model`（v1.0.51 活回合：models.dev 主源瞬时
        // 失败静默回退，裸网关 id 全灭）：裸 id 按 model 段命中；`:variant` 条目
        // 段键自带后缀不参与，近似型号（flashx）不冒充。
        let cat = ModelCatalog::new(vec![
            or_entry("zai/glm-5.3-flash:batch", 0.15),
            or_entry("zai/glm-5.3-flash", 0.15),
            or_entry("zai/glm-5.3-flashx", 0.2),
        ]);
        let m = match_model(&cat, "glm-5.3-flash").expect("match");
        assert_eq!(m.entry.id, "zai/glm-5.3-flash");
        assert_eq!(m.entry.source, CatalogSource::OpenRouter);
        assert_eq!(m.score, 84);
    }

    #[test]
    fn bare_models_dev_entry_still_beats_vendor_segment_match() {
        // models.dev 主源在场时裸 id 走 100 精确档，OpenRouter 段命中（84）只兜底。
        let cat = ModelCatalog::new(vec![
            or_entry("zai/glm-5.3-flash", 0.15),
            dev_entry("zai", "glm-5.3-flash", 0.15),
        ]);
        let m = match_model(&cat, "glm-5.3-flash").expect("match");
        assert_eq!(m.entry.source, CatalogSource::ModelsDev);
        assert_eq!(m.score, 100);
    }

    #[test]
    fn bare_query_stamped_vendor_entry_is_tolerated() {
        // 条目带部署戳（`vendor/model-20260826`）：裸查询剥戳后按段命中。
        let cat = ModelCatalog::new(vec![or_entry("zai/glm-5.3-flash-20260826", 0.15)]);
        let m = match_model(&cat, "glm-5.3-flash").expect("match");
        assert_eq!(m.entry.id, "zai/glm-5.3-flash-20260826");
        assert_eq!(m.score, 84);
    }

    #[test]
    fn unknown_vendor_never_invents_a_match() {
        // 剥壳也命不中就如实返回 None，不冒充匹配。
        let cat = ModelCatalog::new(vec![dev_entry("zai", "glm-5.3-flash", 0.15)]);
        assert!(match_model(&cat, "cline-pass/totally-other-model").is_none());
    }

    #[test]
    fn bare_normalized_variant_hits() {
        // 纯 id 的分隔符变体（`_`/`.` vs `-`）按 84 档命中。
        let cat = ModelCatalog::new(vec![dev_entry("zai", "glm-5.3-flash", 0.15)]);
        let m = match_model(&cat, "glm_5.3_flash").expect("match");
        assert_eq!(m.score, 84);
        let m2 = match_model(&cat, "GLM.5.3-FLASH").expect("match");
        assert_eq!(m2.score, 84);
    }

    #[test]
    fn deployment_stamp_suffix_is_tolerated() {
        // 网关回显带部署戳的 id（尾部 6–8 位日期）：剥戳后同模型同价。
        let cat = ModelCatalog::new(vec![dev_entry("zai", "glm-5.3-flash", 0.15)]);
        let m = match_model(&cat, "z-ai/glm-5.3-flash-20260826").expect("match");
        assert_eq!(m.score, 84);
        let m2 = match_model(&cat, "glm-5.3-flash-20260826").expect("match");
        assert_eq!(m2.score, 84);

        // 4 位以下日期型后缀是独立型号（step-3.5-flash-2603），不剥。
        let mut step = dev_entry("stepfun", "step-3.5-flash", 0.1);
        step.canonical_id = Some("stepfun/step-3.5-flash".into());
        let cat2 = ModelCatalog::new(vec![step]);
        assert!(match_model(&cat2, "step-3.5-flash-2603").is_none());
    }
}
