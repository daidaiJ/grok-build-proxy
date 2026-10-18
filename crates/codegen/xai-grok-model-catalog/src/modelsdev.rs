//! models.dev `/api.json` 响应解析（参照 modelq `internal/modelsdev/modelsdev.go` 的结构）。
//!
//! 顶层为 `provider_id → provider`，provider 内 `models` 为 `model_id → model`；
//! `cost` 为 vendor 官方参考价（**per-1M USD** 浮点，字段可缺）；`limit` 持 token
//! 上限（注意 models.dev 语义：`input` 可低于 `context`）。
//!
//! 拍板 2026-10-11：**只留原厂条目**——解析层按一方 provider 白名单过滤
//! （match_model 的 `FIRST_PARTY_PROVIDERS` 判定），第三方转售商
//! （302ai / kilo / nano-gpt 等两百余个网关 provider）整库不进目录；OpenRouter
//! 官方 API 为兜底源，不经此过滤。

use std::collections::BTreeMap;

use crate::types::{CatalogEntry, CatalogError, CatalogSource, ModelCatalog, ModelPricing};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ModelsDevProvider {
    #[serde(default)]
    id: String,
    #[serde(default)]
    models: BTreeMap<String, ModelsDevModel>,
}

#[derive(Debug, Deserialize)]
struct ModelsDevModel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    canonical_model_id: Option<String>,
    #[serde(default)]
    reasoning: Option<bool>,
    /// LOCAL（目录思考位回填）：`reasoning_options` 里的 effort 条目带档位方言
    /// （如 zai 原厂 glm-5.3 系 → low/high/max）；缺席/无 effort 条目 = None。
    #[serde(default)]
    reasoning_options: Vec<DevReasoningOption>,
    /// LOCAL（视觉桥 D8）：`modalities.input`（缺席 = 源未给，`None`）。
    #[serde(default)]
    modalities: Option<DevModalities>,
    #[serde(default)]
    limit: DevLimit,
    #[serde(default)]
    cost: DevCost,
}

#[derive(Debug, Default, Deserialize)]
struct DevReasoningOption {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    values: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DevModalities {
    #[serde(default)]
    input: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DevLimit {
    #[serde(default)]
    context: Option<u64>,
    #[serde(default)]
    output: Option<u64>,
}

/// vendor 官方参考价（per-1M USD）。
#[derive(Debug, Default, Deserialize)]
struct DevCost {
    #[serde(default)]
    input: Option<f64>,
    #[serde(default)]
    output: Option<f64>,
    #[serde(default)]
    cache_read: Option<f64>,
    #[serde(default)]
    cache_write: Option<f64>,
}

fn sane(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite() && *v >= 0.0)
}

pub fn parse(body: &str) -> Result<ModelCatalog, CatalogError> {
    let err = |message: String| CatalogError {
        source: CatalogSource::ModelsDev,
        message,
    };
    let raw: BTreeMap<String, ModelsDevProvider> =
        serde_json::from_str(body).map_err(|e| err(e.to_string()))?;
    let mut entries = Vec::new();
    for (provider_key, provider) in raw {
        let provider_id = if provider.id.is_empty() {
            provider_key
        } else {
            provider.id
        };
        // provider 级提前过滤：转售商整库跳过，模型条目不物化
        //（压低全量 JSON 解析的临时内存峰值，行为与文末 retain 一致）。
        if !crate::match_model::is_first_party_provider(&provider_id) {
            continue;
        }
        for (model_key, model) in provider.models {
            let input = sane(model.cost.input);
            let output = sane(model.cost.output);
            // 订阅 plan 类条目（coding-plan / token-plan）input/output 恒 0：
            // 订阅制无边际价，对 BYOK 成本统计是假 $0——整条归为无价（渲染层显示 –）。
            let (input, output, cache_read, cache_write) = match (input, output) {
                (Some(0.0), Some(0.0)) => (None, None, None, None),
                (i, o) => (i, o, sane(model.cost.cache_read), sane(model.cost.cache_write)),
            };
            // LOCAL（目录思考位回填）：首个 effort 型 reasoning_option 的 values。
            let reasoning_efforts = model
                .reasoning_options
                .iter()
                .find(|option| option.kind == "effort" && !option.values.is_empty())
                .and_then(|option| {
                    crate::types::normalize_effort_levels(option.values.iter().map(String::as_str))
                });
            entries.push(CatalogEntry {
                id: model_key,
                provider_id: provider_id.clone(),
                source: CatalogSource::ModelsDev,
                name: model.name,
                canonical_id: model.canonical_model_id.filter(|s| !s.is_empty()),
                reasoning: model.reasoning,
                reasoning_efforts,
                // LOCAL（视觉桥 D8）：modalities.input 列出 "image" 即识图；缺席 = None。
                accepts_images: model.modalities.and_then(|mo| {
                    (!mo.input.is_empty()).then(|| mo.input.iter().any(|m| m == "image"))
                }),
                pricing: ModelPricing {
                    input,
                    output,
                    cache_read,
                    cache_write,
                },
                context_length: model.limit.context,
                max_completion_tokens: model.limit.output,
            });
        }
    }
    // 只留原厂条目：第三方转售商 provider 整库不进目录（拍板 2026-10-11，
    // 上方 provider 级跳过后的不变式守卫）。
    entries.retain(|e| crate::match_model::is_first_party(e));
    Ok(ModelCatalog::new(entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::models_dev_fixture;

    #[test]
    fn parses_provider_models_limits_and_costs() {
        let cat = parse(models_dev_fixture()).expect("fixture parses");
        assert_eq!(cat.len(), 3);

        let glm = cat
            .entries
            .iter()
            .find(|e| e.id == "glm-5.3-flash" && e.provider_id == "zai")
            .expect("zai glm entry");
        assert_eq!(glm.source, CatalogSource::ModelsDev);
        assert_eq!(glm.name, "GLM-5.3-Flash");
        // models.dev 直接就是 per-1M。
        assert_eq!(glm.pricing.input, Some(0.15));
        assert_eq!(glm.pricing.output, Some(2.5));
        assert_eq!(glm.pricing.cache_read, Some(0.02));
        assert_eq!(glm.pricing.cache_write, None);
        assert_eq!(glm.context_length, Some(128_000));
        assert_eq!(glm.max_completion_tokens, Some(96_000));
        assert_eq!(glm.reasoning, Some(true));
        // LOCAL（视觉桥 D8）：modalities.input 仅 text → Some(false)；缺席 → None。
        assert_eq!(glm.accepts_images, Some(false));
        assert_eq!(
            cat.entries
                .iter()
                .find(|e| e.id == "bare-model")
                .expect("bare entry")
                .accepts_images,
            None
        );
    }

    #[test]
    fn canonical_model_id_is_kept() {
        let cat = parse(models_dev_fixture()).expect("fixture parses");
        let claude = cat
            .entries
            .iter()
            .find(|e| e.id == "claude-sonnet-4-5" && e.provider_id == "anthropic")
            .expect("anthropic entry");
        assert_eq!(
            claude.canonical_id.as_deref(),
            Some("anthropic/claude-sonnet-4-5")
        );
    }

    #[test]
    fn all_zero_plan_pricing_becomes_unpriced() {
        // 原厂 provider 内显式 0/0（免费档等）：无边际价，整条归为无价，
        // 避免 $0.00 假价参与成本统计与合计。
        let body = r#"{
          "deepseek": {
            "id": "deepseek",
            "models": {
              "glm-5.3-flash": {
                "name": "GLM-5.3-Flash (free)",
                "limit": { "context": 128000 },
                "cost": { "input": 0, "output": 0, "cache_read": 0, "cache_write": 0 }
              }
            }
          }
        }"#;
        let cat = parse(body).expect("parse");
        assert_eq!(cat.len(), 1);
        let entry = &cat.entries[0];
        assert!(entry.pricing.is_unpriced(), "全零价应归为无价");
        assert_eq!(entry.pricing.input, None);
        assert_eq!(entry.pricing.cache_read, None);
        // limit 与条目本身照常保留。
        assert_eq!(entry.context_length, Some(128_000));
    }

    #[test]
    fn non_first_party_providers_are_dropped() {
        // 拍板 2026-10-11：第三方转售商（302ai）与订阅 plan 条目整库不留，
        // 目录里只剩白名单内原厂。
        let body = r#"{
          "zai": { "id": "zai", "models": { "glm-5.3-flash": { "name": "GLM", "cost": { "input": 0.15, "output": 0.5 } } } },
          "302ai": { "id": "302ai", "models": { "glm-5.3-flash": { "name": "GLM (reseller)", "cost": { "input": 0.075, "output": 0.25 } } } },
          "zai-coding-plan": { "id": "zai-coding-plan", "models": { "glm-5.3-flash": { "name": "GLM (plan)", "cost": { "input": 0, "output": 0 } } } }
        }"#;
        let cat = parse(body).expect("parse");
        assert_eq!(cat.len(), 1, "只保留原厂条目");
        assert_eq!(cat.entries[0].provider_id, "zai");
        assert_eq!(cat.entries[0].pricing.input, Some(0.15));
    }

    #[test]
    fn missing_cost_fields_are_unknown() {
        let cat = parse(models_dev_fixture()).expect("fixture parses");
        let bare = cat
            .entries
            .iter()
            .find(|e| e.provider_id == "deepseek")
            .expect("bare entry");
        assert!(bare.pricing.is_unpriced());
        assert_eq!(bare.context_length, None);
        assert_eq!(bare.max_completion_tokens, None);
    }

    #[test]
    fn reasoning_options_effort_values_become_levels() {
        let cat = parse(models_dev_fixture()).expect("fixture parses");
        let glm = cat
            .entries
            .iter()
            .find(|e| e.id == "glm-5.3-flash" && e.provider_id == "zai")
            .expect("zai glm entry");
        assert_eq!(
            glm.reasoning_efforts,
            Some(vec!["low".to_owned(), "high".to_owned(), "max".to_owned()])
        );
        // toggle 型条目与缺席都不给档位。
        let claude = cat
            .entries
            .iter()
            .find(|e| e.id == "claude-sonnet-4-5")
            .expect("anthropic entry");
        assert_eq!(claude.reasoning_efforts, None);
        let body = r#"{
          "zai": { "id": "zai", "models": {
            "only-toggle": { "name": "T", "reasoning": true, "reasoning_options": [{ "type": "toggle" }] },
            "dirty-effort": { "name": "D", "reasoning_options": [{ "type": "effort", "values": ["High", "bogus-value"] }] }
          } }
        }"#;
        let cat = parse(body).expect("parse");
        assert_eq!(
            cat.entries.iter().find(|e| e.id == "only-toggle").unwrap().reasoning_efforts,
            None
        );
        assert_eq!(
            cat.entries
                .iter()
                .find(|e| e.id == "dirty-effort")
                .unwrap()
                .reasoning_efforts,
            Some(vec!["high".to_owned(), "bogus-value".to_owned()]),
            "未知值留给消费方按枚举过滤，解析层只归一化"
        );
    }

    #[test]
    fn malformed_body_is_a_source_tagged_error() {
        let err = parse("[]").expect_err("top level must be a map");
        assert_eq!(err.source, CatalogSource::ModelsDev);
    }
}
