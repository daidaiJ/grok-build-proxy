//! 端点组装：每供应商的按序取数请求（URL + 请求头）。

use crate::types::{PreparedRequest, UsageProviderId};

/// 组装某供应商的按序取数请求。`base_url` 是该模型的推理 base_url（选站提示）。
/// key 为空时返回空列表（调用方落 [`crate::ProviderUsageError::MissingKey`]）。
pub fn prepare_requests(
    provider: UsageProviderId,
    api_key: &str,
    base_url: Option<&str>,
) -> Vec<PreparedRequest> {
    let key = api_key.trim();
    if key.is_empty() {
        return Vec::new();
    }
    let url = |origin: &str, path: &str| format!("{}{}", origin.trim_end_matches('/'), path);
    match provider {
        UsageProviderId::OpenCodeGo => vec![PreparedRequest {
            url: url("https://opencode.ai", "/zen/go/v1/usage"),
            headers: bearer(key),
        }],
        UsageProviderId::CommandCode => vec![PreparedRequest {
            url: url("https://api.commandcode.ai", "/alpha/billing/credits"),
            headers: bearer(key),
        }],
        UsageProviderId::GlmCoding => glm_requests(key, base_url, &url),
        UsageProviderId::KimiCoding => vec![PreparedRequest {
            url: url(&kimi_origin(base_url), "/coding/v1/usages"),
            headers: bearer(key),
        }],
        UsageProviderId::MiniMaxCoding => minimax_requests(key, base_url, &url),
    }
}

fn bearer(key: &str) -> Vec<(String, String)> {
    vec![("Authorization".into(), format!("Bearer {key}"))]
}

/// GLM：选站（base_url 含 bigmodel.cn → 国内站，否则国际站；不跨站回退，两站 key
/// 不通用）+ 鉴权前缀分歧（社区实现两派）：先裸 key，命中 `code:1001` 信封再换 Bearer。
fn glm_requests(key: &str, base_url: Option<&str>, url: &dyn Fn(&str, &str) -> String) -> Vec<PreparedRequest> {
    let origin = match base_url {
        Some(b) if b.to_lowercase().contains("bigmodel.cn") => "https://open.bigmodel.cn",
        _ => "https://api.z.ai",
    };
    let mk = |auth: String| PreparedRequest {
        url: url(origin, "/api/monitor/usage/quota/limit"),
        headers: vec![
            ("Authorization".into(), auth),
            ("Accept".into(), "application/json".into()),
            ("Accept-Language".into(), "en-US,en".into()),
        ],
    };
    vec![mk(key.to_string()), mk(format!("Bearer {key}"))]
}

/// MiniMax：国际 api.minimax.io / 国内 api.minimaxi.com（沿用旧域名，国内新推理
/// 域名 api.minimax.cn 无该接口文档）。路径新旧两条，先新（`/v1/token_plan/remains`，
/// magpie #387）后旧（`/v1/api/openplatform/coding_plan/remains`）。
fn minimax_requests(key: &str, base_url: Option<&str>, url: &dyn Fn(&str, &str) -> String) -> Vec<PreparedRequest> {
    let origin = match base_url {
        Some(b) if b.to_lowercase().contains("minimax.io") => "https://api.minimax.io",
        _ => "https://api.minimaxi.com",
    };
    vec![
        PreparedRequest {
            url: url(origin, "/v1/token_plan/remains"),
            headers: bearer(key),
        },
        PreparedRequest {
            url: url(origin, "/v1/api/openplatform/coding_plan/remains"),
            headers: bearer(key),
        },
    ]
}

/// Kimi：默认 api.kimi.com；base_url 指向 kimi 其他 host（如 api.kimi.ai）时跟随。
fn kimi_origin(base_url: Option<&str>) -> String {
    match base_url.and_then(origin_of) {
        Some(o) if o.contains("kimi") => o,
        _ => "https://api.kimi.com".to_string(),
    }
}

/// 从 base_url 提取 scheme+host origin（如 `https://api.kimi.com/coding` → `https://api.kimi.com`）。
fn origin_of(base_url: &str) -> Option<String> {
    let rest = base_url.split_once("://")?;
    let (scheme, tail) = rest;
    let host_end = tail.find('/').unwrap_or(tail.len());
    if host_end == 0 {
        return None;
    }
    Some(format!("{scheme}://{}", &tail[..host_end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_providers_single_request() {
        for provider in [
            UsageProviderId::OpenCodeGo,
            UsageProviderId::CommandCode,
            UsageProviderId::KimiCoding,
        ] {
            let reqs = prepare_requests(provider, "sk-test", None);
            assert_eq!(reqs.len(), 1, "{provider:?}");
            assert_eq!(
                reqs[0].headers,
                vec![("Authorization".to_string(), "Bearer sk-test".to_string())]
            );
        }
        assert_eq!(
            prepare_requests(UsageProviderId::OpenCodeGo, "sk-test", None)[0].url,
            "https://opencode.ai/zen/go/v1/usage"
        );
        assert_eq!(
            prepare_requests(UsageProviderId::CommandCode, "sk-test", None)[0].url,
            "https://api.commandcode.ai/alpha/billing/credits"
        );
        assert_eq!(
            prepare_requests(UsageProviderId::KimiCoding, "sk-test", None)[0].url,
            "https://api.kimi.com/coding/v1/usages"
        );
    }

    #[test]
    fn glm_station_selection_and_auth_attempts() {
        let reqs =
            prepare_requests(UsageProviderId::GlmCoding, "k1", Some("https://open.bigmodel.cn/api/paas/v4"));
        assert_eq!(reqs.len(), 2);
        assert!(reqs[0]
            .url
            .starts_with("https://open.bigmodel.cn/api/monitor/usage/quota/limit"));
        assert_eq!(reqs[0].headers[0], ("Authorization".to_string(), "k1".to_string()));
        assert_eq!(
            reqs[1].headers[0],
            ("Authorization".to_string(), "Bearer k1".to_string())
        );
        // 非国内站 → 国际站；未配 base_url 同。
        let reqs = prepare_requests(UsageProviderId::GlmCoding, "k1", Some("https://api.z.ai/api/paas/v4"));
        assert!(reqs[0].url.starts_with("https://api.z.ai/"));
        let reqs = prepare_requests(UsageProviderId::GlmCoding, "k1", None);
        assert!(reqs[0].url.starts_with("https://api.z.ai/"));
    }

    #[test]
    fn minimax_station_selection_and_paths() {
        let reqs = prepare_requests(UsageProviderId::MiniMaxCoding, "k1", Some("https://api.minimax.io/v1"));
        assert_eq!(reqs.len(), 2);
        assert_eq!(reqs[0].url, "https://api.minimax.io/v1/token_plan/remains");
        assert_eq!(
            reqs[1].url,
            "https://api.minimax.io/v1/api/openplatform/coding_plan/remains"
        );
        let reqs = prepare_requests(UsageProviderId::MiniMaxCoding, "k1", None);
        assert_eq!(reqs[0].url, "https://api.minimaxi.com/v1/token_plan/remains");
    }

    #[test]
    fn kimi_follows_base_host() {
        let reqs = prepare_requests(UsageProviderId::KimiCoding, "k1", Some("https://api.kimi.ai/coding"));
        assert_eq!(reqs[0].url, "https://api.kimi.ai/coding/v1/usages");
        // 非 kimi host 的 base_url 不影响默认端点
        let reqs = prepare_requests(UsageProviderId::KimiCoding, "k1", Some("https://example.com/v1"));
        assert_eq!(reqs[0].url, "https://api.kimi.com/coding/v1/usages");
    }

    #[test]
    fn blank_key_is_empty() {
        assert!(prepare_requests(UsageProviderId::CommandCode, "  ", None).is_empty());
    }

    #[test]
    fn origin_of_extracts_scheme_and_host() {
        assert_eq!(
            origin_of("https://api.kimi.com/coding"),
            Some("https://api.kimi.com".to_string())
        );
        assert_eq!(origin_of("api.kimi.com/coding"), None);
        assert_eq!(origin_of("https://"), None);
    }
}
