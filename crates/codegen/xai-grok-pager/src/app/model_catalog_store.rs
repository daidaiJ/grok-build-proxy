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

/// 计算需要取数的 id：本进程没取过的（含空白 id 过滤 + 去重）；整体超过
/// [`REFRESH_AFTER_MS`] 时全部重取。返回空 = 无需请求。
pub fn ids_needing_fetch<'a>(model_ids: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let now = now_unix_ms();
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
}
