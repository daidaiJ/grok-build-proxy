//! models.dev `/api.json` 响应解析（参照 modelq `internal/modelsdev/modelsdev.go` 的结构）。
//!
//! 顶层为 `provider_id → provider`，provider 内 `models` 为 `model_id → model`；
//! `cost` 为 vendor 官方参考价（**per-1M USD** 浮点，字段可缺）；`limit` 持 token
//! 上限（注意 models.dev 语义：`input` 可低于 `context`）。

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
    limit: DevLimit,
    #[serde(default)]
    cost: DevCost,
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
        for (model_key, model) in provider.models {
            entries.push(CatalogEntry {
                id: model_key,
                provider_id: provider_id.clone(),
                source: CatalogSource::ModelsDev,
                name: model.name,
                canonical_id: model.canonical_model_id.filter(|s| !s.is_empty()),
                pricing: ModelPricing {
                    input: sane(model.cost.input),
                    output: sane(model.cost.output),
                    cache_read: sane(model.cost.cache_read),
                    cache_write: sane(model.cost.cache_write),
                },
                context_length: model.limit.context,
                max_completion_tokens: model.limit.output,
            });
        }
    }
    Ok(ModelCatalog { entries })
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
    fn missing_cost_fields_are_unknown() {
        let cat = parse(models_dev_fixture()).expect("fixture parses");
        let bare = cat
            .entries
            .iter()
            .find(|e| e.provider_id == "bare-provider")
            .expect("bare entry");
        assert!(bare.pricing.is_unpriced());
        assert_eq!(bare.context_length, None);
        assert_eq!(bare.max_completion_tokens, None);
    }

    #[test]
    fn malformed_body_is_a_source_tagged_error() {
        let err = parse("[]").expect_err("top level must be a map");
        assert_eq!(err.source, CatalogSource::ModelsDev);
    }
}
