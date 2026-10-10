//! `x.ai/providerUsage` extension handler — 当前模型供应商的套餐用量快照（`/usage` 面板）。
//!
//! 当前模型 `base_url` 匹配到支持「推理同一把 SK 查套餐用量」的供应商时，按序请求
//! 该供应商的用量端点并归一成 [`PlanUsageSnapshot`]；TTL 内直接回缓存（默认 5 分钟，
//! `[provider_usage] cache_minutes` 可调）。`provider: null` = 无匹配供应商，面板
//! 保持现状。设计见 `docs-local/usage/provider-quota-display-todo.md`。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};

use super::{ExtResult, to_raw_response};
use crate::agent::MvpAgent;
use xai_grok_provider_usage::{
    parse_response, prepare_requests, provider_for_base_url, PlanUsageSnapshot,
    ProviderUsageError, UsageProviderId,
};

/// 默认缓存分钟数（用户拍板：默认 5 分钟，单位分钟）。
const DEFAULT_CACHE_MINUTES: u64 = 5;

/// 响应体。`provider: None` = 当前模型无匹配供应商。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsageResponse {
    pub provider: Option<UsageProviderId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<PlanUsageSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// `missing_key` | `unauthorized` | `http` | `parse`，UI 分层文案用。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
}

fn not_configured() -> ProviderUsageResponse {
    ProviderUsageResponse {
        provider: None,
        snapshot: None,
        error: None,
        error_kind: None,
    }
}

fn error_response(provider: UsageProviderId, error: &ProviderUsageError) -> ProviderUsageResponse {
    let (kind, message) = match error {
        ProviderUsageError::MissingKey => (
            "missing_key",
            "No API key resolved for this model.".to_string(),
        ),
        ProviderUsageError::Unauthorized { message } => ("unauthorized", message.clone()),
        ProviderUsageError::Http { status, message } => {
            if *status == 0 {
                ("http", message.clone())
            } else {
                ("http", format!("HTTP {status}: {message}"))
            }
        }
        ProviderUsageError::Parse { message } => ("parse", message.clone()),
        // 全部尝试用尽仍未过鉴权（GLM 裸 key/Bearer 都被拒）。
        ProviderUsageError::TryNextAttempt { message } => ("unauthorized", message.clone()),
    };
    ProviderUsageResponse {
        provider: Some(provider),
        snapshot: None,
        error: Some(message),
        error_kind: Some(kind.to_string()),
    }
}

/// 进程级快照缓存：键 = (model_id, provider)。换模型天然换键，TTL 兜底陈旧。
struct Cached {
    snapshot: PlanUsageSnapshot,
    at: Instant,
}

fn cache() -> &'static Mutex<HashMap<(String, UsageProviderId), Cached>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, UsageProviderId), Cached>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 缓存 TTL（配置 `cache_minutes`，缺省 5，clamp 1–120）。
fn cache_ttl(agent: &MvpAgent) -> Duration {
    let minutes = agent
        .cfg
        .borrow()
        .provider_usage
        .cache_minutes
        .unwrap_or(DEFAULT_CACHE_MINUTES)
        .clamp(1, 120);
    Duration::from_secs(minutes * 60)
}

fn cache_get(
    model_id: &str,
    provider: UsageProviderId,
    ttl: Duration,
) -> Option<PlanUsageSnapshot> {
    let map = cache().lock().ok()?;
    let cached = map.get(&(model_id.to_string(), provider))?;
    if cached.at.elapsed() >= ttl {
        return None;
    }
    Some(cached.snapshot.clone())
}

fn cache_put(model_id: &str, provider: UsageProviderId, snapshot: PlanUsageSnapshot) {
    if let Ok(mut map) = cache().lock() {
        map.insert(
            (model_id.to_string(), provider),
            Cached {
                snapshot,
                at: Instant::now(),
            },
        );
    }
}

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let params: serde_json::Value = serde_json::from_str(args.params.get())
        .map_err(|e| acp::Error::invalid_params().data(e.to_string()))?;
    let model_id = params
        .get("model_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| acp::Error::invalid_params().data("missing model_id"))?
        .to_string();

    let models = agent.models_manager.models();
    let Some(entry) = models.get(&model_id) else {
        return to_raw_response(&not_configured());
    };
    // 供应商由 base_url host 匹配（用户拍板：拿 base_url 去匹配就行）；无匹配 = 未配置。
    let Some(provider) = provider_for_base_url(&entry.info().base_url) else {
        return to_raw_response(&not_configured());
    };

    let ttl = cache_ttl(agent);
    if let Some(snapshot) = cache_get(&model_id, provider, ttl) {
        tracing::debug!(provider = provider.as_str(), %model_id, "provider usage: cache hit");
        return to_raw_response(&ProviderUsageResponse {
            provider: Some(provider),
            snapshot: Some(snapshot),
            error: None,
            error_kind: None,
        });
    }

    let Some(api_key) = entry.own_credential() else {
        tracing::debug!(provider = provider.as_str(), %model_id, "provider usage: no own credential");
        return to_raw_response(&error_response(
            provider,
            &ProviderUsageError::MissingKey,
        ));
    };

    let requests = prepare_requests(provider, &api_key, Some(&entry.info().base_url));
    let client = http_client(entry.info().use_proxy);
    match fetch_snapshot(&client, provider, &requests).await {
        Ok(snapshot) => {
            xai_grok_telemetry::unified_log::info(
                "provider usage: fetched",
                None,
                Some(serde_json::json!({
                    "provider": provider.as_str(),
                    "modelId": model_id,
                    "windows": snapshot.windows.len(),
                })),
            );
            cache_put(&model_id, provider, snapshot.clone());
            to_raw_response(&ProviderUsageResponse {
                provider: Some(provider),
                snapshot: Some(snapshot),
                error: None,
                error_kind: None,
            })
        }
        Err(e) => {
            let message = match &e {
                ProviderUsageError::Unauthorized { message }
                | ProviderUsageError::Http { message, .. }
                | ProviderUsageError::Parse { message }
                | ProviderUsageError::TryNextAttempt { message } => message.clone(),
                ProviderUsageError::MissingKey => "missing key".to_string(),
            };
            xai_grok_telemetry::unified_log::warn(
                "provider usage: fetch failed",
                None,
                Some(serde_json::json!({
                    "provider": provider.as_str(),
                    "modelId": model_id,
                    "error": message,
                })),
            );
            to_raw_response(&error_response(provider, &e))
        }
    }
}

/// 直连（默认）或走进程级出口代理（该模型 `use_proxy = true` 时），与采样路径的
/// 代理语义一致。用量端点多数与推理同 host，代理决策跟着模型走。
fn http_client(use_proxy: bool) -> reqwest::Client {
    let configure = |builder: reqwest::ClientBuilder| builder.tcp_nodelay(true);
    let result = if use_proxy {
        xai_grok_extra_ca::build_reqwest_client(configure)
    } else {
        xai_grok_extra_ca::build_reqwest_client_no_proxy(configure)
    };
    result.unwrap_or_else(|_| reqwest::Client::new())
}

/// 按序尝试 [`PreparedRequest`]：GLM 的裸 key/Bearer 信封重试与 MiniMax 的新旧
/// 路径 404 都换下一个；其余错误直接落。
async fn fetch_snapshot(
    client: &reqwest::Client,
    provider: UsageProviderId,
    requests: &[xai_grok_provider_usage::PreparedRequest],
) -> Result<PlanUsageSnapshot, ProviderUsageError> {
    if requests.is_empty() {
        return Err(ProviderUsageError::MissingKey);
    }
    let mut last: Option<ProviderUsageError> = None;
    for (index, req) in requests.iter().enumerate() {
        let has_more = index + 1 < requests.len();
        let mut builder = client.get(&req.url).timeout(Duration::from_secs(5));
        for (key, value) in &req.headers {
            builder = builder.header(key, value);
        }
        let response = match builder.send().await {
            Ok(response) => response,
            Err(e) => {
                tracing::debug!(provider = provider.as_str(), url = %req.url, error = %e, "provider usage: request failed");
                last = Some(ProviderUsageError::Http {
                    status: 0,
                    message: format!("request failed: {e}"),
                });
                continue;
            }
        };
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        match parse_response(provider, status, &body, now_unix_ms()) {
            Ok(snapshot) => return Ok(snapshot),
            Err(ProviderUsageError::TryNextAttempt { message }) if has_more => {
                tracing::debug!(provider = provider.as_str(), message, "provider usage: retrying with next auth mode");
                last = Some(ProviderUsageError::TryNextAttempt { message });
            }
            Err(ProviderUsageError::Http { status: 404, message }) if has_more => {
                tracing::debug!(provider = provider.as_str(), message, "provider usage: endpoint variant 404, trying next");
                last = Some(ProviderUsageError::Http {
                    status: 404,
                    message,
                });
            }
            Err(e) => return Err(e),
        }
    }
    Err(match last {
        Some(ProviderUsageError::TryNextAttempt { message }) => {
            ProviderUsageError::Unauthorized { message }
        }
        Some(other) => other,
        None => ProviderUsageError::Parse {
            message: "no requests to try".to_string(),
        },
    })
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

    #[test]
    fn default_cache_ttl_is_five_minutes() {
        // 缺省 5 分钟（拍板 4）；clamp 逻辑覆盖 1–120。
        struct Cfg {
            cache_minutes: Option<u64>,
        }
        fn ttl(minutes: Option<u64>) -> u64 {
            minutes.unwrap_or(DEFAULT_CACHE_MINUTES).clamp(1, 120)
        }
        assert_eq!(ttl(None), 5);
        assert_eq!(ttl(Some(0)), 1);
        assert_eq!(ttl(Some(3)), 3);
        assert_eq!(ttl(Some(1_000)), 120);
        let _ = Cfg {
            cache_minutes: None,
        };
    }

    #[test]
    fn error_response_maps_kinds() {
        let resp = error_response(
            UsageProviderId::OpenCodeGo,
            &ProviderUsageError::Unauthorized {
                message: "no subscription".into(),
            },
        );
        assert_eq!(resp.error_kind.as_deref(), Some("unauthorized"));
        assert_eq!(resp.error.as_deref(), Some("no subscription"));
        assert!(resp.snapshot.is_none());
        assert_eq!(resp.provider, Some(UsageProviderId::OpenCodeGo));

        let resp = error_response(
            UsageProviderId::CommandCode,
            &ProviderUsageError::Http {
                status: 500,
                message: "boom".into(),
            },
        );
        assert_eq!(resp.error_kind.as_deref(), Some("http"));
        assert_eq!(resp.error.as_deref(), Some("HTTP 500: boom"));

        // 传输层失败（status 0）不再重复 "HTTP 0" 前缀。
        let resp = error_response(
            UsageProviderId::CommandCode,
            &ProviderUsageError::Http {
                status: 0,
                message: "request failed: timeout".into(),
            },
        );
        assert_eq!(resp.error.as_deref(), Some("request failed: timeout"));
    }

    #[test]
    fn response_serializes_camel_case_and_omits_empty() {
        let json = serde_json::to_value(not_configured()).unwrap();
        assert_eq!(json, serde_json::json!({ "provider": null }));
    }
}
