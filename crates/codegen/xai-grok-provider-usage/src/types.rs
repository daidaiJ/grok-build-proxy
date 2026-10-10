//! 快照 schema 与供应商/窗口枚举。

use serde::{Deserialize, Serialize};

/// 支持的供应商（配置值 = [`UsageProviderId::as_str`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UsageProviderId {
    OpenCodeGo,
    CommandCode,
    /// 智谱 GLM Coding Plan（国际站 api.z.ai / 国内站 open.bigmodel.cn）。
    GlmCoding,
    /// Kimi For Coding（月之暗面 Kimi Code 套餐）。
    KimiCoding,
    /// MiniMax Coding Plan。
    MiniMaxCoding,
}

impl UsageProviderId {
    /// 全部合法配置值，供配置校验与错误提示。
    pub const ALL: [UsageProviderId; 5] = [
        UsageProviderId::OpenCodeGo,
        UsageProviderId::CommandCode,
        UsageProviderId::GlmCoding,
        UsageProviderId::KimiCoding,
        UsageProviderId::MiniMaxCoding,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            UsageProviderId::OpenCodeGo => "opencode-go",
            UsageProviderId::CommandCode => "commandcode",
            UsageProviderId::GlmCoding => "glm-coding",
            UsageProviderId::KimiCoding => "kimi-coding",
            UsageProviderId::MiniMaxCoding => "minimax",
        }
    }

    /// 面板来源标注用的显示名（按调研文档约定供应商名不翻译）。
    pub fn display_name(self) -> &'static str {
        match self {
            UsageProviderId::OpenCodeGo => "OpenCode Go",
            UsageProviderId::CommandCode => "Command Code",
            UsageProviderId::GlmCoding => "GLM Coding Plan",
            UsageProviderId::KimiCoding => "Kimi For Coding",
            UsageProviderId::MiniMaxCoding => "MiniMax Coding Plan",
        }
    }

    /// 解析 `[model.<id>] usage_provider` 配置值；未知值返回 `None`（调用方 warn + 视同未配置）。
    pub fn from_config_value(value: &str) -> Option<Self> {
        UsageProviderId::ALL
            .iter()
            .copied()
            .find(|p| p.as_str() == value)
    }
}

/// 套餐用量窗口粒度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageWindowKind {
    /// 5 小时滚动窗口。
    FiveHour,
    /// 周窗口。
    Weekly,
    /// 月窗口（仅在无 5h/周窗口时由渲染层展示）。
    Monthly,
}

impl UsageWindowKind {
    /// 渲染优先级：5h > 周 > 月（用户拍板 2）。
    pub fn rank(self) -> u8 {
        match self {
            UsageWindowKind::FiveHour => 0,
            UsageWindowKind::Weekly => 1,
            UsageWindowKind::Monthly => 2,
        }
    }
}

/// 一次成功查询的套餐用量快照。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanUsageSnapshot {
    pub provider: UsageProviderId,
    /// 套餐名（GLM `data.level`、Command Code `credits.planId` 等；无则为 None）。
    pub plan_name: Option<String>,
    /// 适配器产出的全量窗口（按 FiveHour → Weekly → Monthly 排序；月度裁剪由渲染层做）。
    pub windows: Vec<PlanUsageWindow>,
    /// 查询完成时刻（epoch ms）。
    pub fetched_at_ms: u64,
}

/// 单个用量窗口。`used_pct` 恒为**已用**口径（0–100），未知（如上游缺 cap）为 `None`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanUsageWindow {
    pub kind: UsageWindowKind,
    pub used_pct: Option<f64>,
    /// 窗口重置时刻（epoch ms）。
    pub resets_at_ms: Option<u64>,
    /// 原样透传的补充信息（status 原文、$ 金额、limited/exceeded 标志）。
    pub note: Option<String>,
}

/// 适配器层错误。进 UI 的三类文案：凭据问题（Unauthorized）/ 服务或网络（Http）/ 解析（Parse）。
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderUsageError {
    /// 模型未解析出 API key。
    MissingKey,
    /// 401/403 或等价的凭据信封：key 无效或无订阅资格（如 OpenCode Go 无订阅 403）。
    Unauthorized { message: String },
    /// 非 2xx 的 HTTP 错误。
    Http { status: u16, message: String },
    /// 2xx 但响应不符合预期（业务错误信封、结构缺失等）。
    Parse { message: String },
    /// 需要换下一个 [`PreparedRequest`] 重试（GLM 裸 key/Bearer 分歧、MiniMax 旧路径）。
    /// 没有剩余尝试时调用方应把 `message` 落为最终错误。
    TryNextAttempt { message: String },
}

/// 一次取数尝试：URL + 请求头。HTTP 由调用方执行。
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRequest {
    pub url: String,
    /// 有序请求头（GLM 首次尝试是裸 key，无 Bearer 前缀）。
    pub headers: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_id_config_values_roundtrip() {
        for p in UsageProviderId::ALL {
            assert_eq!(UsageProviderId::from_config_value(p.as_str()), Some(p));
        }
        assert_eq!(UsageProviderId::from_config_value("nope"), None);
        assert_eq!(UsageProviderId::from_config_value("commandcode"), Some(UsageProviderId::CommandCode));
    }

    #[test]
    fn display_names_cover_all_providers() {
        for p in UsageProviderId::ALL {
            assert!(!p.display_name().is_empty());
        }
    }

    #[test]
    fn window_rank_orders_five_hour_weekly_monthly() {
        assert!(UsageWindowKind::FiveHour.rank() < UsageWindowKind::Weekly.rank());
        assert!(UsageWindowKind::Weekly.rank() < UsageWindowKind::Monthly.rank());
    }
}
