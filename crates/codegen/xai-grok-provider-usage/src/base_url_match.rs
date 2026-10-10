//! base_url host 匹配：从模型的推理 base_url 推断用量供应商。

use crate::types::UsageProviderId;

/// 按模型 `base_url` 的 host/路径特征匹配供应商（对齐 cc-switch
/// `CodingPlanProvider::from_base_url` 与 magpie `planQuotaSourceOf` 的 host 判别）。
///
/// 返回 `None` = 该模型的来源没有可查的套餐用量接口，视同未配置（面板零变化）。
/// 各家特征：
/// - OpenCode Go：`opencode.ai` 且路径带 `/zen/go`（Go 网关前缀）；
/// - Command Code：host `api.commandcode.ai`；
/// - GLM Coding Plan：`bigmodel.cn`（国内站）或 `api.z.ai`（国际站）；
/// - Kimi For Coding：host `api.kimi.com`/`api.kimi.ai` 且路径带 `/coding`
///   （同 host 还有非 coding API，须限定）；
/// - MiniMax：`api.minimaxi.com`/`api.minimax.io`/`api.minimax.cn`（国内新推理
///   域名无额度接口公开出处，但同一账号体系，查询仍走 api.minimaxi.com）。
pub fn provider_for_base_url(base_url: &str) -> Option<UsageProviderId> {
    let url = base_url.to_lowercase();
    if url.contains("opencode.ai") && url.contains("/zen/go") {
        Some(UsageProviderId::OpenCodeGo)
    } else if url.contains("api.commandcode.ai") {
        Some(UsageProviderId::CommandCode)
    } else if url.contains("bigmodel.cn") || url.contains("api.z.ai") {
        Some(UsageProviderId::GlmCoding)
    } else if (url.contains("api.kimi.com") || url.contains("api.kimi.ai"))
        && url.contains("/coding")
    {
        Some(UsageProviderId::KimiCoding)
    } else if url.contains("api.minimaxi.com")
        || url.contains("api.minimax.io")
        || url.contains("api.minimax.cn")
    {
        Some(UsageProviderId::MiniMaxCoding)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::UsageProviderId::*;

    #[test]
    fn matches_provider_hosts() {
        assert_eq!(
            provider_for_base_url("https://opencode.ai/zen/go/v1"),
            Some(OpenCodeGo)
        );
        assert_eq!(
            provider_for_base_url("https://api.commandcode.ai/provider/v1"),
            Some(CommandCode)
        );
        assert_eq!(
            provider_for_base_url("https://open.bigmodel.cn/api/paas/v4"),
            Some(GlmCoding)
        );
        assert_eq!(
            provider_for_base_url("https://api.z.ai/api/coding/paas/v4"),
            Some(GlmCoding)
        );
        assert_eq!(
            provider_for_base_url("https://api.kimi.com/coding"),
            Some(KimiCoding)
        );
        assert_eq!(
            provider_for_base_url("https://api.kimi.ai/coding/v1"),
            Some(KimiCoding)
        );
        assert_eq!(
            provider_for_base_url("https://api.minimaxi.com/v1"),
            Some(MiniMaxCoding)
        );
        assert_eq!(
            provider_for_base_url("https://api.minimax.io/v1"),
            Some(MiniMaxCoding)
        );
        assert_eq!(
            provider_for_base_url("https://api.minimax.cn/v1"),
            Some(MiniMaxCoding)
        );
    }

    #[test]
    fn non_provider_hosts_and_paths_do_not_match() {
        // kimi 同 host 的非 coding 路径不匹配
        assert_eq!(provider_for_base_url("https://api.kimi.com/v1"), None);
        // OpenCode 非 Go 前缀不匹配
        assert_eq!(provider_for_base_url("https://opencode.ai/zen/v1"), None);
        // 常见第三方与 xAI 本家不匹配
        assert_eq!(provider_for_base_url("https://api.x.ai/v1"), None);
        assert_eq!(
            provider_for_base_url("https://ark.cn-beijing.volces.com/api/coding/v3"),
            None
        );
        assert_eq!(
            provider_for_base_url("https://api.cline.bot/api/v1"),
            None
        );
        assert_eq!(provider_for_base_url("token-unlimited.com/v1"), None);
        assert_eq!(provider_for_base_url(""), None);
    }

    #[test]
    fn match_is_case_insensitive() {
        assert_eq!(
            provider_for_base_url("https://API.CommandCode.AI/provider/v1"),
            Some(CommandCode)
        );
    }
}
