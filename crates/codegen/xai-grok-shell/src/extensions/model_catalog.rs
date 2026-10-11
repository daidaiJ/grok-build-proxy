//! LOCAL: 模型目录扩展（`x.ai/modelCatalog`）—— models.dev / OpenRouter 双源取数、
//! TTL 缓存 + `~/.grok/prices.cache` 磁盘缓存、置信匹配（`xai-grok-model-catalog`
//! 纯函数层）。
//!
//! 拍板口径（`docs-local/provider-onboarding-api-cost-todo.md`）：价格目录 TTL
//! **1 天起步**（`[model_catalog] cache_minutes` 缺省 1440，可配）；只留官方价格
//! （一方原厂优先，见 match_model 白名单）。目录与账号无关 → 进程级单条目缓存 +
//! 全局单飞锁；价格数据变动极少，落盘 `prices.cache`，TTL 内重启/新进程免请求，
//! 取数失败回退旧目录（进程内 → 磁盘，宁旧勿空，与 provider usage 同语义）。
//!
//! 匹配语义见纯 crate：置信档（exact/variant/vendor/canonical/normalized），
//! fuzzy 子串噪声不富化。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as TokioMutex;

use agent_client_protocol as acp;
use xai_grok_config::grok_home;
use xai_grok_model_catalog::{
    CatalogEntry, CatalogSource, ModelCatalog, ModelPricing, PreparedRequest, match_model,
    parse_response, prepare_requests,
};

use crate::agent::MvpAgent;
use super::{ExtResult, to_raw_response};

/// 单条目录响应的请求超时（目录响应数百 KB 级，源站尾部延迟留足余量）。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// 缺省目录缓存 1 天（拍板：价格一天甚至多天更新一次就够）。
const DEFAULT_CACHE_MINUTES: u64 = 1440;
/// `cache_minutes` 配置边界：下限给调试留口子，上限两周。
const CACHE_MINUTES_RANGE: std::ops::RangeInclusive<u64> = 5..=20_160;

/// 一条匹配结果（目录条目 + 置信分）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogMatch {
    pub source: CatalogSource,
    /// 目录条目 id（OpenRouter `vendor/model` 或 models.dev provider 内 id）。
    pub model_id: String,
    pub provider_id: String,
    pub name: String,
    pub match_score: u8,
    pub pricing: ModelPricing,
    pub context_length: Option<u64>,
    pub max_completion_tokens: Option<u64>,
}

impl CatalogMatch {
    fn from_match(entry: &CatalogEntry, match_score: u8) -> Self {
        Self {
            source: entry.source,
            model_id: entry.id.clone(),
            provider_id: entry.provider_id.clone(),
            name: entry.name.clone(),
            match_score,
            pricing: entry.pricing,
            context_length: entry.context_length,
            max_completion_tokens: entry.max_completion_tokens,
        }
    }
}

/// `x.ai/modelCatalog` 回包：请求的模型 id → 匹配条目（`null` = 无置信命中）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelCatalogResponse {
    pub entries: BTreeMap<String, Option<CatalogMatch>>,
    /// 目录取数时刻（缓存命中 / stale 回退时为取数当时）。
    pub fetched_at_ms: u64,
    /// `hit` = 进程内 TTL 缓存命中；`disk` = `prices.cache` TTL 内复用（免请求）；
    /// `fetch` = 本次取数；`stale` = 取数失败回退旧目录（进程内或磁盘）；
    /// `error` = 无目录可用（条目全 null）。
    pub cache: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 进程级目录缓存（单条目：目录与账号无关）。
#[derive(Clone)]
struct Cached {
    catalog: ModelCatalog,
    at_ms: u64,
    at: Instant,
}

fn cache() -> &'static Mutex<Option<Cached>> {
    static CACHE: OnceLock<Mutex<Option<Cached>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// 全局单飞锁：并发请求共用一把锁，后来者等锁后复检缓存。
fn inflight_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(TokioMutex::default)
}

/// 缓存 TTL（配置 `[model_catalog] cache_minutes`，缺省 1440，clamp 5–20160）。
fn cache_ttl(agent: &MvpAgent) -> Duration {
    let minutes = agent
        .cfg
        .borrow()
        .model_catalog
        .cache_minutes
        .unwrap_or(DEFAULT_CACHE_MINUTES)
        .clamp(*CACHE_MINUTES_RANGE.start(), *CACHE_MINUTES_RANGE.end());
    Duration::from_secs(minutes * 60)
}

fn cache_get(ttl: Option<Duration>) -> Option<Cached> {
    let cached = cache().lock().ok()?.as_ref()?.clone();
    if let Some(ttl) = ttl {
        if cached.at.elapsed() >= ttl {
            return None;
        }
    }
    Some(cached)
}

fn cache_put(catalog: ModelCatalog, at_ms: u64) {
    if let Ok(mut slot) = cache().lock() {
        *slot = Some(Cached {
            catalog,
            at_ms,
            at: Instant::now(),
        });
    }
}

// --- 磁盘缓存 `~/.grok/prices.cache`（拍板 2026-10-11：价格数据变动极少，
// 落盘跨进程复用；TTL 内免请求，取数失败时宁旧勿空回退旧值） ---

const DISK_CACHE_FILE: &str = "prices.cache";
const DISK_CACHE_VERSION: u32 = 1;
/// 读上限：models.dev 全量原始 JSON 5.4MB，条目化后更小；超限视为损坏。
const DISK_CACHE_READ_CAP: u64 = 32 * 1024 * 1024;
/// tmp 残留清扫龄期（write_atomic 的 best-effort 清扫参数）。
const DISK_TMP_SWEEP_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Serialize, Deserialize)]
struct DiskCache {
    version: u32,
    fetched_at_ms: u64,
    catalog: ModelCatalog,
}

fn disk_cache_path() -> PathBuf {
    grok_home().join(DISK_CACHE_FILE)
}

/// 读取磁盘缓存；版本不符 / 解析失败一律按缺失处理。
fn disk_cache_load(path: &Path) -> Option<(ModelCatalog, u64)> {
    let bytes = crate::agent::remote_config::cache_file::read_capped(path, DISK_CACHE_READ_CAP)?;
    match serde_json::from_slice::<DiskCache>(&bytes) {
        Ok(parsed) if parsed.version == DISK_CACHE_VERSION => {
            Some((parsed.catalog, parsed.fetched_at_ms))
        }
        Ok(_) => {
            tracing::debug!(path = %path.display(), "model catalog: disk cache version mismatch");
            None
        }
        Err(e) => {
            tracing::debug!(path = %path.display(), error = %e, "model catalog: disk cache parse failed");
            None
        }
    }
}

/// 原子落盘（tmp + rename，与 models_cache.json 同原语）；失败仅记日志。
fn disk_cache_store(path: &Path, catalog: &ModelCatalog, at_ms: u64) {
    let payload = DiskCache {
        version: DISK_CACHE_VERSION,
        fetched_at_ms: at_ms,
        catalog: catalog.clone(),
    };
    match serde_json::to_vec(&payload) {
        Ok(bytes) => {
            crate::agent::remote_config::cache_file::write_atomic(
                path,
                DISK_TMP_SWEEP_TTL,
                &bytes,
                false,
            );
        }
        Err(e) => {
            tracing::debug!(error = %e, "model catalog: disk cache serialize failed");
        }
    }
}

/// 磁盘取数时刻是否仍在 TTL 内（时钟回拨时 saturating_sub 为 0，按新处理）。
fn disk_fresh(at_ms: u64, ttl: Duration) -> bool {
    now_unix_ms().saturating_sub(at_ms) < ttl.as_millis() as u64
}

/// 直连（缺省）或跟随进程级出口代理（`[network] proxy_hosts` / `GROK_PROXY_HOSTS`
/// 显式收录的目录 host 会走隧道，与采样路径的 allowlist 语义一致）。
fn http_client() -> reqwest::Client {
    let configure = |builder: reqwest::ClientBuilder| builder.tcp_nodelay(true);
    xai_grok_extra_ca::build_reqwest_client(configure).unwrap_or_else(|_| reqwest::Client::new())
}

/// 按序尝试目录源：models.dev 主、OpenRouter 兜底（拍板 4，2026-10-11 修订——
/// models.dev 裸 id + openrouter 镜像双覆盖，见 `prepare_requests`）。传输层失败、
/// 非 200、解析失败都换下一个；全部失败返回拼接错误。
async fn fetch_catalog(client: &reqwest::Client) -> Result<(ModelCatalog, u64), String> {
    let mut last: Option<String> = None;
    for (source, req) in prepare_requests() {
        match fetch_one(client, source, &req).await {
            Ok(catalog) => return Ok((catalog, now_unix_ms())),
            Err(e) => {
                tracing::debug!(source = ?source, url = %req.url, error = %e, "model catalog: source failed");
                last = Some(e);
            }
        }
    }
    Err(match last {
        Some(e) => e,
        None => "no catalog sources configured".to_string(),
    })
}

async fn fetch_one(
    client: &reqwest::Client,
    source: CatalogSource,
    req: &PreparedRequest,
) -> Result<ModelCatalog, String> {
    let mut builder = client.get(&req.url).timeout(REQUEST_TIMEOUT);
    for (key, value) in &req.headers {
        builder = builder.header(key, value);
    }
    let response = builder
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = response.status().as_u16();
    // body 读取失败单独报因，绝不吞成空串（v1.0.47 用量取数同款教训）。
    let body = response
        .text()
        .await
        .map_err(|e| format!("response body read failed (HTTP {status}): {e}"))?;
    if status != 200 {
        return Err(format!("HTTP {status}"));
    }
    parse_response(source, &body).map_err(|e| e.to_string())
}

/// 匹配请求 id 并组装回包（纯函数，便于测试）。
fn response_for(
    catalog: &ModelCatalog,
    fetched_at_ms: u64,
    cache_kind: &str,
    requested: &BTreeSet<String>,
    error: Option<String>,
) -> ModelCatalogResponse {
    let entries = requested
        .iter()
        .map(|id| {
            let matched = match_model(catalog, id).map(|m| CatalogMatch::from_match(m.entry, m.score));
            (id.clone(), matched)
        })
        .collect();
    ModelCatalogResponse {
        entries,
        fetched_at_ms,
        cache: cache_kind.to_string(),
        error,
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let params: serde_json::Value = serde_json::from_str(args.params.get())
        .map_err(|e| acp::Error::invalid_params().data(e.to_string()))?;
    let requested: BTreeSet<String> = params
        .get("models")
        .and_then(|v| v.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let ttl = cache_ttl(agent);
    if let Some(cached) = cache_get(Some(ttl)) {
        tracing::debug!("model catalog: cache hit");
        return to_raw_response(&response_for(
            &cached.catalog,
            cached.at_ms,
            "hit",
            &requested,
            None,
        ));
    }

    // 单飞：等全局锁；等锁期间前一个请求多半已写入缓存，复检直接复用。
    let _guard = inflight_lock().lock().await;
    if let Some(cached) = cache_get(Some(ttl)) {
        tracing::debug!("model catalog: cache hit after single-flight wait");
        return to_raw_response(&response_for(
            &cached.catalog,
            cached.at_ms,
            "hit",
            &requested,
            None,
        ));
    }

    // 磁盘缓存 TTL 内直接复用：重启 / 新进程免 5MB 级请求。命中后并入进程缓存。
    let disk_path = disk_cache_path();
    let disk = disk_cache_load(&disk_path);
    if let Some((catalog, at_ms)) = disk.as_ref().filter(|(_, at)| disk_fresh(*at, ttl)) {
        tracing::debug!("model catalog: disk cache hit");
        cache_put(catalog.clone(), *at_ms);
        return to_raw_response(&response_for(
            catalog, *at_ms, "disk", &requested, None,
        ));
    }

    let client = http_client();
    match fetch_catalog(&client).await {
        Ok((catalog, at_ms)) => {
            xai_grok_telemetry::unified_log::info(
                "model catalog: fetched",
                None,
                Some(serde_json::json!({ "entries": catalog.len() })),
            );
            cache_put(catalog.clone(), at_ms);
            disk_cache_store(&disk_path, &catalog, at_ms);
            to_raw_response(&response_for(
                &catalog, at_ms, "fetch", &requested, None,
            ))
        }
        Err(e) => {
            xai_grok_telemetry::unified_log::warn(
                "model catalog: fetch failed",
                None,
                Some(serde_json::json!({ "error": e })),
            );
            // 价格面宁旧勿空：取数失败回退旧目录（进程内过期值 → 磁盘任意龄期）。
            if let Some(cached) = cache_get(None) {
                return to_raw_response(&response_for(
                    &cached.catalog,
                    cached.at_ms,
                    "stale",
                    &requested,
                    Some(e),
                ));
            }
            if let Some((catalog, at_ms)) = disk.or_else(|| disk_cache_load(&disk_path)) {
                cache_put(catalog.clone(), at_ms);
                return to_raw_response(&response_for(
                    &catalog,
                    at_ms,
                    "stale",
                    &requested,
                    Some(e),
                ));
            }
            to_raw_response(&response_for(
                &ModelCatalog::default(),
                now_unix_ms(),
                "error",
                &requested,
                Some(e),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, provider: &str, source: CatalogSource, input: Option<f64>) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            provider_id: provider.to_string(),
            source,
            name: format!("name-of-{id}"),
            canonical_id: None,
            pricing: ModelPricing {
                input,
                output: input.map(|v| v * 5.0),
                cache_read: None,
                cache_write: None,
            },
            context_length: Some(128_000),
            max_completion_tokens: Some(32_000),
        }
    }

    fn fixture_catalog() -> ModelCatalog {
        ModelCatalog {
            entries: vec![
                entry("x-ai/grok-4", "openrouter", CatalogSource::OpenRouter, Some(3.0)),
                entry("glm-5.3-flash", "zai", CatalogSource::ModelsDev, Some(0.15)),
            ],
        }
    }

    #[test]
    fn default_cache_ttl_is_one_day_and_clamps() {
        fn ttl(minutes: Option<u64>) -> u64 {
            struct Cfg {
                model_catalog: crate::agent::config::ModelCatalogConfig,
            }
            let mut cfg = Cfg {
                model_catalog: crate::agent::config::ModelCatalogConfig::default(),
            };
            cfg.model_catalog.cache_minutes = minutes;
            // 与 cache_ttl 相同的归一逻辑（不构造 MvpAgent）。
            cfg.model_catalog
                .cache_minutes
                .unwrap_or(DEFAULT_CACHE_MINUTES)
                .clamp(*CACHE_MINUTES_RANGE.start(), *CACHE_MINUTES_RANGE.end())
        }
        assert_eq!(ttl(None), 1440);
        assert_eq!(ttl(Some(1)), 5);
        assert_eq!(ttl(Some(100_000)), 20_160);
    }

    #[test]
    fn response_for_matches_requested_ids_and_keeps_nulls() {
        let cat = fixture_catalog();
        let requested: BTreeSet<String> = ["x-ai/grok-4", "totally-unknown"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let resp = response_for(&cat, 42, "fetch", &requested, None);
        assert_eq!(resp.entries.len(), 2);
        let grok = resp.entries.get("x-ai/grok-4").expect("grok").as_ref().expect("matched");
        assert_eq!(grok.model_id, "x-ai/grok-4");
        assert_eq!(grok.source, CatalogSource::OpenRouter);
        assert_eq!(grok.match_score, 100);
        assert_eq!(grok.pricing.input, Some(3.0));
        assert_eq!(grok.context_length, Some(128_000));
        assert!(resp.entries.get("totally-unknown").expect("unknown").is_none());
        assert_eq!(resp.cache, "fetch");
        assert_eq!(resp.fetched_at_ms, 42);
    }

    #[test]
    fn response_serializes_camel_case_and_keeps_null_entries() {
        let cat = fixture_catalog();
        let mut requested = BTreeSet::new();
        requested.insert("glm-5.3-flash".to_string());
        let resp = response_for(&cat, 7, "hit", &requested, None);
        let json = serde_json::to_value(&resp).expect("serialize");
        assert!(json.get("fetchedAtMs").is_some());
        assert!(json.get("cache").is_some());
        let entry = &json["entries"]["glm-5.3-flash"];
        assert_eq!(entry["matchScore"], 100);
        assert_eq!(entry["modelId"], "glm-5.3-flash");
        assert!(entry["contextLength"].is_u64());
    }

    #[test]
    fn stale_error_response_carries_error_and_entries() {
        let cat = fixture_catalog();
        let mut requested = BTreeSet::new();
        requested.insert("x-ai/grok-4".to_string());
        let resp = response_for(&cat, 9, "stale", &requested, Some("HTTP 503".into()));
        assert_eq!(resp.cache, "stale");
        assert_eq!(resp.error.as_deref(), Some("HTTP 503"));
        assert!(resp.entries["x-ai/grok-4"].is_some());
    }

    #[test]
    fn blank_and_empty_requested_ids_are_absent() {
        let cat = fixture_catalog();
        let resp = response_for(&cat, 1, "hit", &BTreeSet::new(), None);
        assert!(resp.entries.is_empty());
    }

    #[test]
    fn disk_cache_roundtrip_and_version_guard() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("prices.cache");
        assert!(disk_cache_load(&path).is_none(), "缺失文件按无缓存");

        let cat = fixture_catalog();
        disk_cache_store(&path, &cat, 1234);
        let (loaded, at) = disk_cache_load(&path).expect("roundtrip");
        assert_eq!(at, 1234);
        assert_eq!(loaded, cat);

        // 版本不符 → 视为损坏。
        let mutated: DiskCache = DiskCache {
            version: DISK_CACHE_VERSION + 1,
            fetched_at_ms: 42,
            catalog: cat.clone(),
        };
        std::fs::write(&path, serde_json::to_vec(&mutated).expect("serialize")).expect("write");
        assert!(disk_cache_load(&path).is_none(), "版本不符按缺失处理");

        // 内容损坏 → 视为缺失，不 panic。
        std::fs::write(&path, b"{not json").expect("write");
        assert!(disk_cache_load(&path).is_none());
    }

    #[test]
    fn disk_fresh_honors_ttl_and_clock_skew() {
        let ttl = Duration::from_secs(1440 * 60);
        let now = now_unix_ms();
        assert!(disk_fresh(now - 60_000, ttl), "1 分钟前算新");
        assert!(!disk_fresh(now - (1440 * 60 + 60) * 1000, ttl), "超 TTL 算旧");
        assert!(disk_fresh(now + 60_000, ttl), "时钟回拨（取数在未来）按新处理");
    }
}
