//! LOCAL: 模型目录进程级缓存（pager 侧）——`/stats` 成本列的取数触发与查询。
//!
//! 目录与账号无关（免鉴权公开端点），所以进程级单条目存储 + 全局在途标志即可；
//! TTL / 双源回退 / 单飞取数在 shell（`x.ai/modelCatalog`），这里只管「本进程
//! 要不要发请求」：有条目且未过期 → 不发；缺条目或超 [`REFRESH_AFTER_MS`] → 发。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use xai_grok_model_catalog::{UsageTokens, cost_usd};
use xai_grok_shell::extensions::model_catalog::{CatalogMatch, ModelCatalogResponse};
use xai_grok_tools::model_usage_ledger::ModelUsageAggregate;

use crate::slash::i18n::tr;

/// 请求的模型 id → 匹配条目（`None` = 已取过、目录无置信命中）。
pub type CatalogPrices = BTreeMap<String, Option<CatalogMatch>>;

/// 一个聚合行的窗口成本（USD）。模型无目录命中或关键价缺失 → `None`
/// （渲染 `–`、从合计剔除）；本进程未取过价（外层 `None`）同样 `None`。
pub fn aggregate_cost(pricing: &CatalogPrices, row: &ModelUsageAggregate) -> Option<f64> {
    let matched = pricing.get(&row.model_id)?.as_ref()?;
    cost_usd(
        &matched.pricing,
        &UsageTokens {
            prompt: row.prompt_tokens,
            completion: row.completion_tokens,
            cached_read: row.cached_read_tokens,
            cache_creation: row.cache_creation_tokens,
        },
    )
}

/// 进程内取数超过该年龄后再开面板会重新触发取数（shell 侧 TTL/stale 语义兜底）。
const REFRESH_AFTER_MS: u64 = 6 * 60 * 60 * 1000;

/// 无价（未命中 / 未取到 / 关键价缺失）时成本列的占位符。
pub const UNPRICED_LABEL: &str = "\u{2013}";

/// LOCAL: `/model` picker 行右侧的目录详情串（模态 / 上下文 / 思考 / 价格）。
///
/// `meta` = 本地 ACP `ModelInfo.meta`（`acceptsImages` / `totalContextTokens` /
/// `supportsReasoningEffort`，本地值优先）；`matched` = store 查询结果
/// （外层 `None` = 本进程尚未取到，内层 `None` = 已知无命中，恒显 [`UNPRICED_LABEL`]）。
/// 未知段逐段省略；全未知返回空串，调用方回退行 `description`。
pub fn picker_detail_label(
    meta: Option<&serde_json::Map<String, serde_json::Value>>,
    matched: Option<Option<&CatalogMatch>>,
) -> String {
    let mut tokens: Vec<String> = Vec::with_capacity(4);
    let modality = meta
        .and_then(|m| m.get("acceptsImages"))
        .and_then(|v| v.as_bool())
        .or_else(|| matched.and_then(|m| m.and_then(|m| m.accepts_images)));
    if let Some(accepts) = modality {
        tokens.push(if accepts { tr("vision") } else { tr("text") }.to_string());
    }
    let context = meta
        .and_then(|m| m.get("totalContextTokens"))
        .and_then(|v| v.as_u64())
        .or_else(|| matched.and_then(|m| m.and_then(|m| m.context_length)));
    if let Some(n) = context {
        tokens.push(fmt_tokens(n));
    }
    let reasoning = meta
        .and_then(|m| m.get("supportsReasoningEffort"))
        .and_then(|v| v.as_bool())
        .or_else(|| matched.and_then(|m| m.and_then(|m| m.reasoning)));
    if reasoning == Some(true) {
        tokens.push(tr("reasoning").to_string());
    }
    let price = match matched {
        Some(Some(m)) => {
            let input = m.pricing.input.map(fmt_price);
            let output = m.pricing.output.map(fmt_price);
            match (input, output) {
                (Some(i), Some(o)) => Some(format!("{i}/{o}")),
                (Some(i), None) => Some(format!("{i}/{UNPRICED_LABEL}")),
                (None, Some(o)) => Some(format!("{UNPRICED_LABEL}/{o}")),
                (None, None) => Some(UNPRICED_LABEL.to_string()),
            }
        }
        // 已知无命中（负缓存）与 /stats 成本列同口径：显式 `–`
        Some(None) => Some(UNPRICED_LABEL.to_string()),
        // 本进程还没取到价：先不出价格段，等回包后下一帧自然补上
        None => None,
    };
    if let Some(p) = price {
        tokens.push(p);
    }
    tokens.join(" ")
}

/// Compact per-1M USD price: up to 2 decimals, trailing zeros trimmed ($0.15 / $3 / $12.5).
fn fmt_price(v: f64) -> String {
    let s = format!("{v:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string();
    format!("${s}")
}

/// Compact token count: 999 → `999`, 256_000 → `256k`, 1_048_576 → `1M`.
fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        let s = format!("{:.1}", n as f64 / 1_000_000.0)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
        format!("{s}M")
    } else if n >= 1_000 {
        format!("{}k", (n as f64 / 1_000.0).round() as u64)
    } else {
        n.to_string()
    }
}

#[derive(Default)]
struct Inner {
    entries: CatalogPrices,
    fetched_at_ms: u64,
}

fn store() -> &'static Mutex<Option<Inner>> {
    static STORE: Mutex<Option<Inner>> = Mutex::new(None);
    &STORE
}

static LOADING: AtomicBool = AtomicBool::new(false);

/// 是否已有取数在途（面板打开路径的单发守卫）。
pub fn is_loading() -> bool {
    LOADING.load(Ordering::SeqCst)
}

/// 标记取数在途（在推送 Effect 前调用；回包 [`install`] 时清除）。
pub fn begin_loading() {
    LOADING.store(true, Ordering::SeqCst);
}

/// 安装回包：条目逐条并入（增量，保旧），取数时刻取较新者；清除在途标志。
pub fn install(response: &ModelCatalogResponse) {
    if let Ok(mut guard) = store().lock() {
        let inner = guard.get_or_insert_with(Inner::default);
        for (id, matched) in &response.entries {
            inner.entries.insert(id.clone(), matched.clone());
        }
        inner.fetched_at_ms = inner.fetched_at_ms.max(response.fetched_at_ms);
    }
    LOADING.store(false, Ordering::SeqCst);
}

/// 查一个模型 id 的目录价。外层 `None` = 本进程还没取过（渲染层显示 `–`），
/// 内层 `None` = 取过但无置信命中（同样显示 `–`，但不再触发取数）。
pub fn get(model_id: &str) -> Option<Option<CatalogMatch>> {
    let guard = store().lock().ok()?;
    guard.as_ref()?.entries.get(model_id).cloned()
}

/// 整份快照（打开面板时喂给模态状态；调用方拿到的是克隆）。
pub fn snapshot() -> CatalogPrices {
    store()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|inner| inner.entries.clone()))
        .unwrap_or_default()
}

/// 模型列表到达时的一次性目录预取（lifecycle/load 的模型安装点调用）。
/// dashboard `/model` 下拉、会话内下拉与 ArgPicker 共享本进程 store；负缓存 +
/// 单飞 + 6h 新鲜度把门，重复调用零成本。
pub fn prefetch_effect(
    models: &crate::acp::model_state::ModelState,
) -> Vec<crate::app::actions::Effect> {
    let ids: Vec<String> = models.available.keys().map(|k| k.0.to_string()).collect();
    let needed = ids_needing_fetch(ids.iter().map(String::as_str));
    if needed.is_empty() || is_loading() {
        return vec![];
    }
    begin_loading();
    vec![crate::app::actions::Effect::FetchModelCatalog { model_ids: needed }]
}

/// 计算需要取数的 id：本进程没取过的（含空白 id 过滤 + 去重）；整体超过
/// [`REFRESH_AFTER_MS`] 时全部重取。返回空 = 无需请求。
pub fn ids_needing_fetch<'a>(model_ids: impl IntoIterator<Item = &'a str>) -> Vec<String> {    let now = now_unix_ms();
    let guard = store().lock().ok();
    let inner = guard.as_deref().and_then(Option::as_ref);
    let stale = match inner {
        Some(inner) => now.saturating_sub(inner.fetched_at_ms) > REFRESH_AFTER_MS,
        None => true,
    };
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for id in model_ids {
        let id = id.trim();
        if id.is_empty() || !seen.insert(id.to_string()) {
            continue;
        }
        let known = inner.is_some_and(|inner| inner.entries.contains_key(id));
        if stale || !known {
            out.push(id.to_string());
        }
    }
    out
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(ids: &[(&str, bool)], at: u64) -> ModelCatalogResponse {
        let mut entries = BTreeMap::new();
        for (id, matched) in ids {
            entries.insert(
                id.to_string(),
                matched.then(|| CatalogMatch {
                    source: xai_grok_model_catalog::CatalogSource::OpenRouter,
                    model_id: "x-ai/grok-4".into(),
                    provider_id: "openrouter".into(),
                    name: "Grok 4".into(),
                    match_score: 100,
                    pricing: xai_grok_model_catalog::ModelPricing {
                        input: Some(3.0),
                        output: Some(15.0),
                        cache_read: Some(0.3),
                        cache_write: None,
                    },
                    context_length: Some(256_000),
                    max_completion_tokens: Some(32_000),
                    reasoning: None,
                accepts_images: None,
                }),
            );
        }
        ModelCatalogResponse {
            entries,
            fetched_at_ms: at,
            cache: "fetch".into(),
            error: None,
        }
    }

    /// 测试间共享进程级 store：这里只用单一用例内先 reset 的行为断言，
    /// 不依赖其他用例留下的状态。
    #[test]
    fn install_get_and_fetch_trigger_lifecycle() {
        // 时间戳用「相对当前时刻」的新鲜值：staleness 判定以真实墙钟为基准
        let now = now_unix_ms();
        let at1 = now - 120_000;
        // 冷 store：任何 id 都需要取数
        assert_eq!(
            ids_needing_fetch(["x-ai/grok-4", "glm-5.3-flash"]),
            vec!["x-ai/grok-4".to_string(), "glm-5.3-flash".to_string()]
        );
        assert!(!is_loading());
        begin_loading();
        assert!(is_loading());

        install(&response(
            &[("x-ai/grok-4", true), ("glm-5.3-flash", false)],
            at1,
        ));
        assert!(!is_loading(), "回包清除在途标志");
        assert!(get("x-ai/grok-4").is_some_and(|m| m.is_some()), "命中条目");
        assert!(get("glm-5.3-flash").is_some_and(|m| m.is_none()), "无匹配 = 已知空");
        assert!(get("never-asked").is_none(), "未请求过的 id 无记录");
        assert_eq!(snapshot().len(), 2);

        // 已知 id 不再触发；新 id 触发
        assert_eq!(ids_needing_fetch(["x-ai/grok-4"]), Vec::<String>::new());
        assert_eq!(
            ids_needing_fetch(["new-model"]),
            vec!["new-model".to_string()]
        );
        // 去重 + 空白过滤
        assert_eq!(
            ids_needing_fetch(["new-model", "new-model", "  ", "x-ai/grok-4"]),
            vec!["new-model".to_string()]
        );

        // 增量并入：再次回包不丢旧条目（时间戳较新，取较新者）
        install(&response(&[("another", true)], now - 60_000));
        assert!(get("x-ai/grok-4").is_some_and(|m| m.is_some()));
        assert!(get("another").is_some_and(|m| m.is_some()));
    }

    fn detail_fixture() -> CatalogMatch {
        CatalogMatch {
            source: xai_grok_model_catalog::CatalogSource::OpenRouter,
            model_id: "x-ai/grok-4".into(),
            provider_id: "openrouter".into(),
            name: "Grok 4".into(),
            match_score: 100,
            pricing: xai_grok_model_catalog::ModelPricing {
                input: Some(3.0),
                output: Some(15.0),
                cache_read: Some(0.3),
                cache_write: None,
            },
            context_length: Some(256_000),
            max_completion_tokens: Some(32_000),
            reasoning: Some(true),
            accepts_images: Some(true),
        }
    }

    fn meta_json(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn picker_label_composes_all_known_tokens() {
        let matched = detail_fixture();
        let meta = meta_json(&[
            ("acceptsImages", serde_json::Value::Bool(false)),
            ("totalContextTokens", serde_json::Value::Number(256_000.into())),
            (
                "supportsReasoningEffort",
                serde_json::Value::Bool(true),
            ),
        ]);
        assert_eq!(
            picker_detail_label(Some(&meta), Some(Some(&matched))),
            "text 256k reasoning $3/$15"
        );
    }

    #[test]
    fn picker_label_local_meta_wins_over_catalog() {
        let matched = detail_fixture();
        // 本地上下文与目录不一致：本地为准（设计拍板 3）
        let meta = meta_json(&[(
            "totalContextTokens",
            serde_json::Value::Number(1_048_576.into()),
        )]);
        assert_eq!(
            picker_detail_label(Some(&meta), Some(Some(&matched))),
            "vision 1M reasoning $3/$15"
        );
    }

    #[test]
    fn picker_label_falls_back_to_catalog_without_meta() {
        let matched = detail_fixture();
        assert_eq!(
            picker_detail_label(None, Some(Some(&matched))),
            "vision 256k reasoning $3/$15"
        );
    }

    #[test]
    fn picker_label_renders_known_no_match_as_placeholder() {
        // 负缓存：与 /stats 成本列同口径，显式 `–`
        assert_eq!(picker_detail_label(None, Some(None)), UNPRICED_LABEL);
    }

    #[test]
    fn picker_label_omits_price_until_fetched() {
        let meta = meta_json(&[("acceptsImages", serde_json::Value::Bool(true))]);
        assert_eq!(picker_detail_label(Some(&meta), None), "vision");
    }

    #[test]
    fn picker_label_empty_when_nothing_known() {
        assert_eq!(picker_detail_label(None, None), "");
    }

    #[test]
    fn picker_label_price_sides_render_independently() {
        let mut matched = detail_fixture();
        matched.pricing.input = None;
        assert_eq!(
            picker_detail_label(None, Some(Some(&matched))),
            format!("vision 256k reasoning {UNPRICED_LABEL}/$15")
        );
        matched.pricing.output = Some(0.5);
        matched.reasoning = None;
        matched.accepts_images = None;
        matched.context_length = None;
        assert_eq!(
            picker_detail_label(None, Some(Some(&matched))),
            format!("{UNPRICED_LABEL}/$0.5")
        );
    }

    #[test]
    fn fmt_tokens_compacts_k_and_m() {
        assert_eq!(fmt_tokens(999), "999");
        assert_eq!(fmt_tokens(256_000), "256k");
        assert_eq!(fmt_tokens(131_072), "131k");
        assert_eq!(fmt_tokens(1_048_576), "1M");
        assert_eq!(fmt_tokens(2_000_000), "2M");
    }
}
