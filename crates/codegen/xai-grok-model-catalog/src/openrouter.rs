//! OpenRouter `/api/v1/models` 响应解析（参照 modelq `internal/api/client.go` 的结构）。
//!
//! `pricing` 为 per-token USD **十进制字符串**（`"0.000001"`）；空串 / 负数 / 非数
//! 视为未列价，`"0"` 是合法的免费牌价（`Some(0.0)`）。统一换算成 per-1M。

use crate::types::{CatalogEntry, CatalogError, CatalogSource, ModelCatalog, ModelPricing};
use serde::Deserialize;

/// per-token → per-1M。
pub fn per_million(per_token: f64) -> f64 {
    per_token * 1_000_000.0
}

/// 宽容取价：字符串 / 数字都收；空串、负数、非数 → `None`（`"0"` → `Some(0.0)` 免费牌价）。
pub fn price_from(value: serde_json::Value) -> Option<f64> {
    let n = match value {
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            s.parse::<f64>().ok()?
        }
        serde_json::Value::Number(n) => n.as_f64()?,
        _ => return None,
    };
    if n.is_nan() || n.is_infinite() || n < 0.0 {
        None
    } else {
        Some(n)
    }
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    #[serde(default)]
    data: Vec<OpenRouterModel>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    #[serde(default)]
    id: String,
    #[serde(default)]
    canonical_slug: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    pricing: serde_json::Value,
    /// `reasoning` 是否在支持参数表里（缺席 = 源未给，`None`）。
    #[serde(default)]
    supported_parameters: Vec<String>,
    /// LOCAL（目录思考位回填）：`reasoning` 能力对象（2026-10 起部分条目携带
    /// `supported_efforts` / `default_effort`）；旧载荷可能是布尔，按 Value 宽容取。
    #[serde(default)]
    reasoning: Option<serde_json::Value>,
    /// LOCAL（视觉桥 D8）：`architecture.input_modalities`（缺席 = 源未给，`None`）。
    #[serde(default)]
    architecture: Option<OpenRouterArchitecture>,
    #[serde(default)]
    top_provider: TopProvider,
}

#[derive(Debug, Default, Deserialize)]
struct OpenRouterArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct TopProvider {
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    max_completion_tokens: Option<u64>,
}

/// OpenRouter 目录 id 形如 `vendor/model`；provider 键固定 `openrouter`。
pub fn provider_id() -> &'static str {
    "openrouter"
}

fn model_id(value: Option<u64>) -> Option<u64> {
    value.filter(|v| *v > 0)
}

pub fn parse(body: &str) -> Result<ModelCatalog, CatalogError> {
    let err = |message: String| CatalogError {
        source: CatalogSource::OpenRouter,
        message,
    };
    let raw: OpenRouterResponse = serde_json::from_str(body).map_err(|e| err(e.to_string()))?;
    let entries = raw
        .data
        .into_iter()
        .filter(|m| !m.id.is_empty())
        .map(|m| {
            let empty = serde_json::Value::Null;
            let pricing = if m.pricing.is_object() {
                m.pricing
            } else {
                empty
            };
            let get = |key: &str| pricing.get(key).cloned().unwrap_or(serde_json::Value::Null);
            let reasoning = (!m.supported_parameters.is_empty())
                .then(|| m.supported_parameters.iter().any(|p| p == "reasoning"));
            // LOCAL（目录思考位回填）：`reasoning.supported_efforts` 是该模型的
            // 档位方言（如 z-ai/glm-5.3-flash → max/high/low）；对象缺席或数组
            // 缺席 = 源未给（None）。
            let reasoning_efforts = m
                .reasoning
                .as_ref()
                .and_then(|v| v.get("supported_efforts"))
                .and_then(|v| v.as_array())
                .and_then(|levels| {
                    crate::types::normalize_effort_levels(levels.iter().filter_map(|v| v.as_str()))
                });
            // LOCAL（视觉桥 D8）：input_modalities 列出 "image" 即识图；空列表 = 源未给（None）。
            let accepts_images = m.architecture.and_then(|a| {
                (!a.input_modalities.is_empty())
                    .then(|| a.input_modalities.iter().any(|m| m == "image"))
            });
            CatalogEntry {
                id: m.id,
                provider_id: provider_id().to_string(),
                source: CatalogSource::OpenRouter,
                name: m.name,
                canonical_id: m.canonical_slug.filter(|s| !s.is_empty()),
                reasoning,
                reasoning_efforts,
                accepts_images,
                pricing: ModelPricing {
                    input: price_from(get("prompt")).map(per_million),
                    output: price_from(get("completion")).map(per_million),
                    cache_read: price_from(get("input_cache_read")).map(per_million),
                    cache_write: price_from(get("input_cache_write")).map(per_million),
                },
                context_length: model_id(m.context_length).or(model_id(m.top_provider.context_length)),
                max_completion_tokens: model_id(m.top_provider.max_completion_tokens),
            }
        })
        .collect();
    Ok(ModelCatalog::new(entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::openrouter_fixture;

    #[test]
    fn parses_prices_context_and_canonical_slug() {
        let cat = parse(openrouter_fixture()).expect("fixture parses");
        assert_eq!(cat.len(), 5);

        let grok = cat
            .entries
            .iter()
            .find(|e| e.id == "x-ai/grok-4")
            .expect("grok entry");
        assert_eq!(grok.provider_id, "openrouter");
        assert_eq!(grok.source, CatalogSource::OpenRouter);
        assert_eq!(grok.name, "Grok 4");
        // 3e-6 per-token → 3.0 per-1M
        assert_eq!(grok.pricing.input, Some(3.0));
        assert_eq!(grok.pricing.output, Some(15.0));
        assert_eq!(grok.pricing.cache_read, Some(0.3));
        assert_eq!(grok.pricing.cache_write, None);
        assert_eq!(grok.context_length, Some(256_000));
        assert_eq!(grok.max_completion_tokens, Some(32_000));
        assert_eq!(grok.canonical_id.as_deref(), Some("x-ai/grok-4"));
        assert_eq!(grok.reasoning, Some(true));
        // LOCAL（视觉桥 D8）：input_modalities 含 image → Some(true)；
        // architecture 缺席 → None（未知）。
        assert_eq!(grok.accepts_images, Some(true));
        let chat = cat
            .entries
            .iter()
            .find(|e| e.id == "deepseek/deepseek-chat-v3.1")
            .expect("chat entry");
        assert_eq!(chat.reasoning, Some(false), "参数表存在但无 reasoning");
        assert_eq!(chat.accepts_images, None, "architecture 缺席 = 未知");
        let free = cat
            .entries
            .iter()
            .find(|e| e.id == "deepseek/deepseek-chat-v3.1:free")
            .expect("free entry");
        assert_eq!(free.reasoning, None, "字段缺席 = 未知");
    }

    #[test]
    fn free_model_prices_are_zero_not_missing() {
        let cat = parse(openrouter_fixture()).expect("fixture parses");
        let free = cat
            .entries
            .iter()
            .find(|e| e.id == "deepseek/deepseek-chat-v3.1:free")
            .expect("free entry");
        assert_eq!(free.pricing.input, Some(0.0));
        assert_eq!(free.pricing.output, Some(0.0));
        assert!(!free.pricing.is_unpriced());
    }

    #[test]
    fn negative_and_empty_prices_are_unknown() {
        let cat = parse(openrouter_fixture()).expect("fixture parses");
        let no_price = cat
            .entries
            .iter()
            .find(|e| e.id == "vendor/unpriced")
            .expect("unpriced entry");
        assert!(no_price.pricing.is_unpriced());
        assert_eq!(no_price.context_length, None);
    }

    #[test]
    fn numeric_price_fields_are_accepted() {
        let body = r#"{"data":[{"id":"a/b","name":"A B","pricing":{"prompt":0.000002,"completion":"0.000008"}}]}"#;
        let cat = parse(body).expect("parses");
        let e = &cat.entries[0];
        assert_eq!(e.pricing.input, Some(2.0));
        assert_eq!(e.pricing.output, Some(8.0));
    }

    #[test]
    fn reasoning_object_efforts_are_normalized() {
        let cat = parse(openrouter_fixture()).expect("fixture parses");
        let glm = cat
            .entries
            .iter()
            .find(|e| e.id == "zai/glm-5.3-flash")
            .expect("glm entry");
        // supported_efforts 原样 [max, high, low]，保源序。
        assert_eq!(
            glm.reasoning_efforts,
            Some(vec!["max".to_owned(), "high".to_owned(), "low".to_owned()])
        );

        // 大小写/空白归一 + 去重；布尔型 reasoning 容忍为无档位。
        let body = r#"{"data":[
            {"id":"a/b","reasoning":{"supported_efforts":[" High ","max","MAX",""]}},
            {"id":"c/d","reasoning":true},
            {"id":"e/f","reasoning":{"mandatory":false}}
        ]}"#;
        let cat = parse(body).expect("parses");
        assert_eq!(
            cat.entries.iter().find(|e| e.id == "a/b").unwrap().reasoning_efforts,
            Some(vec!["high".to_owned(), "max".to_owned()])
        );
        assert_eq!(
            cat.entries.iter().find(|e| e.id == "c/d").unwrap().reasoning_efforts,
            None,
            "布尔 reasoning 无档位信息"
        );
        assert_eq!(
            cat.entries.iter().find(|e| e.id == "e/f").unwrap().reasoning_efforts,
            None,
            "对象无 supported_efforts = 源未给"
        );
    }

    #[test]
    fn malformed_body_is_a_source_tagged_error() {
        let err = parse("not json").expect_err("error");
        assert_eq!(err.source, CatalogSource::OpenRouter);
        assert!(!err.message.is_empty());
    }

    #[test]
    fn entries_without_id_are_dropped() {
        let body = r#"{"data":[{"id":"","name":"x"},{"id":"a/b","name":"ok"}]}"#;
        let cat = parse(body).expect("parses");
        assert_eq!(cat.len(), 1);
        assert_eq!(cat.entries[0].id, "a/b");
    }
}
