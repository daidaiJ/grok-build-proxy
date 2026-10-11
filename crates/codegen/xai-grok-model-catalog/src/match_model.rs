//! 置信匹配：把用户侧模型 id（配置 slug 或上游回显 id）对到目录条目。
//!
//! 移植 modelq `internal/modelsdev/match.go` 的置信档（按分排序，平分时 OpenRouter
//! 主源优先，再按 provider/id 字典序保证确定性）。**fuzzy 前缀/子串命中视为噪声、
//! 不富化**（modelq 同款语义，拍板沿用）：只返回 ≥84 的置信档。
//!
//! | 分档 | 语义 |
//! |------|------|
//! | 100 | id 精确命中（含 `vendor/model` 全串） |
//! | 95  | 去掉 `:variant` 后精确命中（如 `:free`） |
//! | 86  | `vendor/model`：vendor 段对上 provider（含 `z-ai`→`zai` 别名折叠），model 段精确 |
//! | 85  | 规范 id（canonical）归一化后命中 |
//! | 84  | vendor 已对上、model 段分隔符归一化后命中（`.`/`_` 与 `-` 互折） |

use crate::types::{CatalogEntry, CatalogSource, MatchResult, ModelCatalog};

/// 置信门：低于该档的命中（vendor 前缀 / 子串噪声）一律不返回。
pub const CONFIDENT_MIN_SCORE: u8 = 84;

/// 归一化：小写 + `_`/`.` 折成 `-`（保留 `/`）。
pub fn norm_id(s: &str) -> String {
    s.trim().to_lowercase().replace(['_', '.'], "-")
}

/// 归一化后再去掉 `-`：把 `z-ai`/`zai`、`moonshot-ai`/`moonshotai` 这类 vendor 别名折到一起。
pub fn collapse(s: &str) -> String {
    norm_id(s).replace('-', "")
}

fn source_rank(source: CatalogSource) -> u8 {
    match source {
        CatalogSource::OpenRouter => 0,
        CatalogSource::ModelsDev => 1,
    }
}

/// 在目录里找 query 的最佳置信匹配；无置信命中返回 `None`。
pub fn match_model<'a>(catalog: &'a ModelCatalog, query: &str) -> Option<MatchResult<'a>> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    // `:variant`（`:free` / `:batch` / `:nitro` …）不是模型本体，剥离后参与各档匹配。
    let base = match q.split_once(':') {
        Some((b, _)) if !b.is_empty() => b,
        _ => q,
    };
    let bn = norm_id(base);

    let mut best: Option<(u8, &'a CatalogEntry)> = None;
    let mut consider = |score: u8, entry: &'a CatalogEntry| {
        let better = match &best {
            None => true,
            Some((s, e)) => {
                score > *s
                    || (score == *s
                        && (source_rank(entry.source), &entry.provider_id, &entry.id)
                            < (source_rank(e.source), &e.provider_id, &e.id))
            }
        };
        if better {
            best = Some((score, entry));
        }
    };

    // 100 / 95：id 精确命中（含 `:variant` 剥离）。
    for entry in &catalog.entries {
        if entry.id == q {
            consider(100, entry);
        } else if base != q && entry.id == base {
            consider(95, entry);
        }
    }

    // 86 / 85 / 84：vendor/model 拆段匹配 + canonical 反查。
    if let Some((vendor, model)) = base.split_once('/') {
        let vn = norm_id(vendor);
        let mn = norm_id(model);
        let vendor_matches = |provider_id: &str| {
            norm_id(provider_id) == vn || collapse(provider_id) == collapse(vendor)
        };
        for entry in &catalog.entries {
            if vendor_matches(&entry.provider_id) {
                if entry.id == model {
                    consider(86, entry);
                } else if norm_id(&entry.id) == mn {
                    consider(84, entry);
                }
            }
            if let Some(canonical) = &entry.canonical_id {
                if norm_id(canonical) == bn {
                    consider(85, entry);
                }
            }
        }
    } else {
        // 无 vendor 段：canonical 反查仍适用。
        for entry in &catalog.entries {
            if let Some(canonical) = &entry.canonical_id {
                if norm_id(canonical) == bn {
                    consider(85, entry);
                }
            }
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
}
