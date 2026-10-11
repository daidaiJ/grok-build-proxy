//! 端点组装：目录取数请求（免鉴权，带 UA —— modelq 先例）。

use crate::types::{CatalogSource, PreparedRequest};

/// 目录请求公共头：`Accept: application/json` + UA（OpenRouter 对无 UA 客户端限流更狠）。
pub fn catalog_headers() -> Vec<(String, String)> {
    vec![
        ("Accept".into(), "application/json".into()),
        ("User-Agent".into(), "grok-build-proxy".into()),
    ]
}

/// 组装按序目录请求：models.dev 主、OpenRouter 兜底（拍板 4，2026-10-11 修订：
/// models.dev 以裸 model id 挂在原厂 provider 下，同时内嵌 `openrouter` 镜像覆盖
/// `vendor/model` 形态——单源即同时命中两类 id；OpenRouter 官方 API 全为
/// `vendor/model` 形态，裸网关 id 会失配，退居兜底）。调用方按序尝试，
/// 首个「HTTP 200 且解析成功」的来源生效；全部失败才报错。
pub fn prepare_requests() -> Vec<(CatalogSource, PreparedRequest)> {
    vec![
        (
            CatalogSource::ModelsDev,
            PreparedRequest {
                url: "https://models.dev/api.json".into(),
                headers: catalog_headers(),
            },
        ),
        (
            CatalogSource::OpenRouter,
            PreparedRequest {
                url: "https://openrouter.ai/api/v1/models".into(),
                headers: catalog_headers(),
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_dev_first_then_openrouter() {
        let reqs = prepare_requests();
        assert_eq!(reqs.len(), 2);
        assert_eq!(reqs[0].0, CatalogSource::ModelsDev);
        assert_eq!(reqs[0].1.url, "https://models.dev/api.json");
        assert_eq!(reqs[1].0, CatalogSource::OpenRouter);
        assert_eq!(reqs[1].1.url, "https://openrouter.ai/api/v1/models");
    }

    #[test]
    fn requests_carry_ua_and_accept_no_auth() {
        for (_, req) in prepare_requests() {
            assert!(req.headers.contains(&("User-Agent".to_string(), "grok-build-proxy".to_string())));
            assert!(req.headers.contains(&("Accept".to_string(), "application/json".to_string())));
            assert!(
                !req.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("authorization")),
                "目录请求免鉴权"
            );
        }
    }
}
