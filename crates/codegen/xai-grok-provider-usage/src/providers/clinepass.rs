//! ClinePass `GET /api/v1/users/me/plan/usage-limits` 响应解析。

use crate::parse::{clamp_pct, mk_window, parse_epoch_ms};
use crate::types::{PlanUsageWindow, ProviderUsageError, UsageWindowKind};

/// 2026-10-27 活体 probe（本机 ClinePass SK，HTTP 200）：
/// `{"data":{"limits":[{"type":"five_hour","percentUsed":32,
/// "resetsAt":"2026-10-27T17:55:51.323980658Z"},
/// {"type":"weekly","percentUsed":32,"resetsAt":"…"},
/// {"type":"monthly","percentUsed":23,"resetsAt":"…"}]},"success":true}`
/// （端点与结构参照 pi 插件 pi-clinepass 0.1.7 `fetchPlanLimits`；同接口的第三方
/// 实现还有 yetone/magpie 的 cline 配额源。）
///
/// `percentUsed` 已是已用口径（0–100，浮点或整数），`resetsAt` 为 RFC 3339
/// （纳秒精度，`parse_epoch_ms` 宽容解析）。每条 limit 一个窗口（FiveHour /
/// Weekly / Monthly）；`limits` 缺失或没有可识别条目 = Parse 错误。
pub(crate) fn parse(
    v: &serde_json::Value,
) -> Result<(Option<String>, Vec<PlanUsageWindow>), ProviderUsageError> {
    // data 信封内取 limits；顶层 limits 同样接受（宽容探针/上游形态变化）。
    let limits = v
        .pointer("/data/limits")
        .or_else(|| v.get("limits"))
        .and_then(|l| l.as_array())
        .ok_or_else(|| ProviderUsageError::Parse {
            message: "no limits in the reply".into(),
        })?;
    let mut out = Vec::new();
    for item in limits {
        let Some(kind) = item
            .get("type")
            .and_then(|t| t.as_str())
            .and_then(window_kind)
        else {
            continue;
        };
        let Some(pct) = item.get("percentUsed").and_then(|p| p.as_f64()) else {
            continue;
        };
        let resets_at = item.get("resetsAt").and_then(parse_epoch_ms);
        out.push(mk_window(kind, Some(clamp_pct(pct)), resets_at));
    }
    if out.is_empty() {
        return Err(ProviderUsageError::Parse {
            message: "no usable limits in the reply".into(),
        });
    }
    Ok((None, out))
}

/// 上游窗口类型 → 本仓窗口粒度（未知类型丢弃）。
fn window_kind(type_str: &str) -> Option<UsageWindowKind> {
    match type_str {
        "five_hour" => Some(UsageWindowKind::FiveHour),
        "weekly" => Some(UsageWindowKind::Weekly),
        "monthly" => Some(UsageWindowKind::Monthly),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::types::{PlanUsageSnapshot, ProviderUsageError, UsageProviderId, UsageWindowKind};
    use crate::test_support::{parse_err, parse_ok, pct_of};

    fn snap(status: u16, body: &str) -> PlanUsageSnapshot {
        parse_ok(UsageProviderId::ClinePass, status, body)
    }

    #[test]
    fn windows_from_data_envelope() {
        // 结构逐字对齐 2026-10-27 活体 probe（纳秒精度 resetsAt）。
        let s = snap(
            200,
            r#"{"data":{"limits":[
                {"type":"five_hour","percentUsed":32,"resetsAt":"2026-10-27T17:55:51.323980658Z"},
                {"type":"weekly","percentUsed":32,"resetsAt":"2026-11-01T03:27:39.327152388Z"},
                {"type":"monthly","percentUsed":23,"resetsAt":"2026-11-07T13:16:26.330313148Z"}]},
                "success":true}"#,
        );
        assert_eq!(s.windows.len(), 3);
        let five = &s.windows[0];
        assert_eq!(five.kind, UsageWindowKind::FiveHour);
        assert_eq!(five.used_pct, Some(32.0));
        assert_eq!(
            five.resets_at_ms,
            Some(1_793_123_751_323),
            "纳秒精度截到毫秒"
        );
        let week = &s.windows[1];
        assert_eq!(week.kind, UsageWindowKind::Weekly);
        assert_eq!(week.used_pct, Some(32.0));
        let month = &s.windows[2];
        assert_eq!(month.kind, UsageWindowKind::Monthly);
        assert_eq!(month.used_pct, Some(23.0));
        // percentUsed 已是已用口径，note 不携带补充信息。
        assert!(s.windows.iter().all(|w| w.note.is_none()));
        assert_eq!(s.plan_name, None);
    }

    #[test]
    fn integer_percent_and_bare_limits_array() {
        let s = snap(
            200,
            r#"{"limits":[{"type":"five_hour","percentUsed":7,"resetsAt":0}]}"#,
        );
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(7.0));
        // 非法 resetsAt 被丢弃，窗口保留。
        assert_eq!(s.windows[0].resets_at_ms, None);
    }

    #[test]
    fn unknown_types_and_missing_percent_skipped() {
        let s = snap(
            200,
            r#"{"data":{"limits":[
                {"type":"daily","percentUsed":50},
                {"type":"weekly"},
                {"type":"monthly","percentUsed":10}]}}"#,
        );
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].kind, UsageWindowKind::Monthly);
    }

    #[test]
    fn no_limits_is_parse_error() {
        assert!(matches!(
            parse_err(UsageProviderId::ClinePass, 200, r#"{"success":true}"#),
            ProviderUsageError::Parse { .. }
        ));
        assert!(matches!(
            parse_err(UsageProviderId::ClinePass, 200, r#"{"data":{"limits":[]}}"#),
            ProviderUsageError::Parse { .. }
        ));
    }

    #[test]
    fn error_status_maps_to_unauthorized_or_http() {
        assert!(matches!(
            parse_err(UsageProviderId::ClinePass, 401, r#"{"error":"unauthorized"}"#),
            ProviderUsageError::Unauthorized { .. }
        ));
        assert!(matches!(
            parse_err(UsageProviderId::ClinePass, 500, "boom"),
            ProviderUsageError::Http { .. }
        ));
    }
}
