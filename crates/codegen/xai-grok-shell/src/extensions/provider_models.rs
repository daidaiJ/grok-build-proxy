//! LOCAL: 供应商模型列表扩展（`x.ai/providerModels`）—— /auth 面板「自动检索模型」步。
//!
//! 对用户给的 BYOK base URL 做 `GET {base_url}/models`（OpenAI 兼容网关普遍支持，
//! Anthropic 形态同为 `data[]`；裸数组也接受），返回模型 id 清单。失败不抛 ACP 错，
//! 一律进回包 `error` 字段（在途标志总能清除，与 modelCatalog 同纪律）。
//!
//! SK 纪律：`api_key` 只进 Authorization 头；不进日志 / 错误消息 / Debug。

use std::time::Duration;

use serde::{Deserialize, Serialize};

use agent_client_protocol as acp;

use crate::agent::MvpAgent;
use super::{ExtResult, to_raw_response};

/// 单次请求超时（源站尾部延迟留足余量，v1.0.47 用量取数同款教训）。
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// 一条供应商模型列表项。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owned_by: Option<String>,
}

/// `x.ai/providerModels` 回包。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelsResponse {
    /// 请求时的 base URL（调用方比对陈旧回包）。
    pub base_url: String,
    pub models: Vec<ProviderModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 直连（缺省）或跟随进程级出口代理，与 modelCatalog 同一客户端纪律。
/// `[network] insecure_tls_hosts` 名单内的 IP 端点（自签证书）跳过证书验证——
/// 面板检索时用户填的就是待接入的 base URL，此时还没有 model 条目。
fn http_client(url: &str) -> reqwest::Client {
    let configure = |builder: reqwest::ClientBuilder| {
        xai_grok_extra_ca::apply_insecure_tls(builder.tcp_nodelay(true), url)
    };
    xai_grok_extra_ca::build_reqwest_client(configure).unwrap_or_else(|_| reqwest::Client::new())
}

/// base URL → models 端点（容忍尾斜杠）。
fn models_url(base_url: &str) -> String {
    format!("{}/models", base_url.trim_end_matches('/'))
}

/// 从响应体提取模型 id 清单。接受 `{"data": [{"id": .., "owned_by": ..}]}`、
/// Anthropic 同形态与裸数组；非对象项 / 非字符串 id 跳过。
fn parse_models(body: &str) -> Vec<ProviderModel> {
    let value: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let list = match &value {
        serde_json::Value::Array(items) => Some(items),
        serde_json::Value::Object(map) => map.get("data").and_then(|d| d.as_array()),
        _ => None,
    };
    list.unwrap_or(&Vec::new())
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.trim().to_string();
            if id.is_empty() {
                return None;
            }
            Some(ProviderModel {
                id,
                owned_by: item
                    .get("owned_by")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            })
        })
        .collect()
}

#[tracing::instrument(skip_all, fields(method = %args.method))]
pub async fn handle(_agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let params: serde_json::Value = serde_json::from_str(args.params.get())
        .map_err(|e| acp::Error::invalid_params().data(e.to_string()))?;
    let base_url = params
        .get("base_url")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| acp::Error::invalid_params().data("missing base_url"))?
        .trim_end_matches('/')
        .to_string();
    let api_key = params
        .get("api_key")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let mut response = fetch_models(&http_client(&base_url), &base_url, api_key.as_deref()).await;
    // 回包里的 base_url 统一为去尾斜杠形态，调用方比对用。
    response.base_url = base_url;
    to_raw_response(&response)
}

async fn fetch_models(
    client: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
) -> ProviderModelsResponse {
    let mut failure = |error: String| ProviderModelsResponse {
        base_url: base_url.to_string(),
        models: Vec::new(),
        error: Some(error),
    };
    let mut builder = client
        .get(models_url(base_url))
        .timeout(REQUEST_TIMEOUT)
        .header("Accept", "application/json");
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        builder = builder.bearer_auth(key.trim());
    }
    let response = match builder.send().await {
        Ok(r) => r,
        Err(e) => return failure(format!("request failed: {e}")),
    };
    let status = response.status().as_u16();
    // body 读取失败单独报因，绝不吞成空串。
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => return failure(format!("response body read failed (HTTP {status}): {e}")),
    };
    if status != 200 {
        return failure(format!("HTTP {status}"));
    }
    let models = parse_models(&body);
    if models.is_empty() {
        return failure("no models in response".to_string());
    }
    tracing::debug!(count = models.len(), "provider models: fetched");
    ProviderModelsResponse {
        base_url: base_url.to_string(),
        models,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_url_trims_trailing_slash() {
        assert_eq!(
            models_url("https://api.example.com/v1"),
            "https://api.example.com/v1/models"
        );
        assert_eq!(
            models_url("https://api.example.com/v1/"),
            "https://api.example.com/v1/models"
        );
    }

    #[test]
    fn parses_openai_data_shape() {
        let body = r#"{"object":"list","data":[
            {"id":"glm-5.3-flash","object":"model","owned_by":"zai"},
            {"id":"deepseek-v4","object":"model"}
        ]}"#;
        let models = parse_models(body);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0], ProviderModel { id: "glm-5.3-flash".into(), owned_by: Some("zai".into()) });
        assert_eq!(models[1].owned_by, None);
    }

    #[test]
    fn parses_bare_array_shape() {
        let models = parse_models(r#"[{"id":"m1"},{"id":"m2"}]"#);
        assert_eq!(models.len(), 2);
        assert_eq!(models[1].id, "m2");
    }

    #[test]
    fn skips_non_string_and_blank_ids() {
        let body = r#"{"data":[{"id":42},{"id":"  "},{"id":"ok"},{"nope":true}]}"#;
        let models = parse_models(body);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "ok");
    }

    #[test]
    fn garbage_body_yields_empty() {
        assert!(parse_models("not json").is_empty());
        assert!(parse_models("{\"data\": 3}").is_empty());
    }

    #[test]
    fn response_serializes_camel_case() {
        let response = ProviderModelsResponse {
            base_url: "https://api.example.com/v1".into(),
            models: vec![ProviderModel { id: "m".into(), owned_by: None }],
            error: None,
        };
        let json = serde_json::to_value(&response).unwrap();
        assert!(json.get("baseUrl").is_some());
        assert!(json.get("error").is_none(), "absent error must be skipped");
        assert!(json["models"][0].get("ownedBy").is_none());
    }
}
