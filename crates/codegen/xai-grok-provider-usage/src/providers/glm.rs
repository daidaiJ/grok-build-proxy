//! 智谱 GLM Coding Plan `GET {host}/api/monitor/usage/quota/limit` 响应解析。

use crate::parse::{as_f64, mk_window};
use crate::types::{PlanUsageWindow, ProviderUsageError, UsageWindowKind};

/// `{"success":true,"data":{"level":"pro","limits":[
///   {"type":"TOKENS_LIMIT","unit":3,"number":5,"percentage":12,"nextResetTime":1758800000000},
///   {"type":"TOKENS_LIMIT","unit":6,"number":1,"percentage":40,"nextResetTime":…},
///   {"type":"TIME_LIMIT",…}]}}`（cc-switch/magpie 读源码一致 + magpie 文档注释实测形态）。
///
/// `percentage` 是**已用**；`unit:3`=小时（number:5 → 5h 窗）、`unit:6`=周（number
/// 1 或 7 都见过，只锚定 unit）；TIME_LIMIT 是 MCP 工具月额度，不拦模型，跳过、
/// 也不据此编造月度窗口。`success:false` + `code:1001` = 缺鉴权头（裸 key/Bearer
/// 分歧），交调用方换下一个尝试。鉴权先于路由：乱路径也回 1001，故不能用 401/404
/// 判端点存在性。
pub(crate) fn parse(
    v: &serde_json::Value,
) -> Result<(Option<String>, Vec<PlanUsageWindow>), ProviderUsageError> {
    if v.get("success").and_then(|b| b.as_bool()) == Some(false) {
        let code = v.get("code").and_then(|c| c.as_i64());
        let msg = v
            .get("msg")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        if code == Some(1001) {
            return Err(ProviderUsageError::TryNextAttempt {
                message: msg.to_string(),
            });
        }
        return Err(ProviderUsageError::Parse {
            message: msg.to_string(),
        });
    }
    let Some(data) = v.get("data") else {
        return Err(ProviderUsageError::Parse {
            message: "missing 'data' in the reply".into(),
        });
    };
    let plan_name = ["level", "planName", "planType", "plan_type", "packageName"]
        .iter()
        .find_map(|k| data.get(*k).and_then(|s| s.as_str()))
        .map(|s| s.to_string());
    let mut five_hour: Option<PlanUsageWindow> = None;
    let mut weekly: Option<PlanUsageWindow> = None;
    let mut unclassified: Vec<PlanUsageWindow> = Vec::new();
    if let Some(limits) = data.get("limits").and_then(|l| l.as_array()) {
        for item in limits {
            let limit_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
            // 大小写不敏感：上游若改大小写仍可识别（cc-switch 同款防御）。
            if !(limit_type.eq_ignore_ascii_case("TOKENS_LIMIT")
                || limit_type.eq_ignore_ascii_case("CREDIT_LIMIT"))
            {
                continue;
            }
            let pct = item.get("percentage").and_then(as_f64);
            let resets_at = item.get("nextResetTime").and_then(crate::parse::parse_epoch_ms);
            let win = mk_window(UsageWindowKind::FiveHour, pct, resets_at);
            match item.get("unit").and_then(|u| u.as_i64()) {
                Some(3) if five_hour.is_none() => five_hour = Some(win),
                Some(6) if weekly.is_none() => {
                    weekly = Some(PlanUsageWindow {
                        kind: UsageWindowKind::Weekly,
                        ..win
                    })
                }
                // unit 缺失或不识别：进兜底桶。
                _ => unclassified.push(win),
            }
        }
    }
    // 兜底：无 nextResetTime 的优先归 5h（5h 桶在 0% 等状态下可能无 reset），其余按
    // reset 升序依次填空缺槽位。不能按 reset 排序直接定窗口身份——周期末尾每周窗口
    // 可能比 5h 更早重置（cc-switch issue #3036）。
    unclassified.sort_by_key(|w| (w.resets_at_ms.is_some(), w.resets_at_ms));
    for win in unclassified {
        if five_hour.is_none() {
            five_hour = Some(win);
        } else if weekly.is_none() {
            weekly = Some(PlanUsageWindow {
                kind: UsageWindowKind::Weekly,
                ..win
            });
        } else {
            break;
        }
    }
    let windows = [five_hour, weekly].into_iter().flatten().collect();
    Ok((plan_name, windows))
}

#[cfg(test)]
mod tests {
    use crate::types::{PlanUsageSnapshot, ProviderUsageError, UsageProviderId, UsageWindowKind};
    use crate::test_support::{parse_err, parse_ok, pct_of};

    fn snap(status: u16, body: &str) -> PlanUsageSnapshot {
        parse_ok(UsageProviderId::GlmCoding, status, body)
    }

    #[test]
    fn windows_by_unit_and_time_limit_skipped() {
        let s = snap(
            200,
            r#"{"success":true,"data":{"level":"pro","limits":[
                {"type":"TOKENS_LIMIT","unit":3,"number":5,"percentage":12,"nextResetTime":1758800000000},
                {"type":"TOKENS_LIMIT","unit":6,"number":1,"percentage":40,"nextResetTime":1759400000000},
                {"type":"TIME_LIMIT","unit":5,"number":1,"percentage":3,"nextResetTime":1760000000000}]}}"#,
        );
        assert_eq!(s.plan_name.as_deref(), Some("pro"));
        assert_eq!(s.windows.len(), 2, "TIME_LIMIT 不进窗口集");
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(12.0));
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(40.0));
        assert_eq!(s.windows[0].resets_at_ms, Some(1_758_800_000_000));
    }

    #[test]
    fn credit_limit_type_recognized_case_insensitive() {
        let s = snap(
            200,
            r#"{"success":true,"data":{"limits":[
                {"type":"credit_limit","unit":3,"percentage":7},
                {"type":"TOKENS_LIMIT","unit":6,"percentage":55,"nextResetTime":1759400000000}]}}"#,
        );
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(7.0));
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(55.0));
    }

    #[test]
    fn unclassified_fill_empty_slots_resetless_first() {
        let s = snap(
            200,
            r#"{"success":true,"data":{"limits":[
                {"type":"TOKENS_LIMIT","percentage":20,"nextResetTime":1758800000000},
                {"type":"TOKENS_LIMIT","percentage":60}]}}"#,
        );
        // 无 reset 的归 5h；有 reset 的归周。
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(60.0));
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(20.0));
    }

    #[test]
    fn plan_name_fallback_fields() {
        let s = snap(
            200,
            r#"{"success":true,"data":{"planName":"GLM Coding Pro","limits":[]}}"#,
        );
        assert_eq!(s.plan_name.as_deref(), Some("GLM Coding Pro"));
        let s = snap(200, r#"{"success":true,"data":{"packageName":"Max","limits":[]}}"#);
        assert_eq!(s.plan_name.as_deref(), Some("Max"));
    }

    #[test]
    fn empty_limits_is_empty_snapshot() {
        let s = snap(200, r#"{"success":true,"data":{"level":"lite","limits":[]}}"#);
        assert!(s.windows.is_empty());
    }

    #[test]
    fn code_1001_envelope_asks_next_attempt() {
        let body = r#"{"code":1001,"msg":"Header中未收到Authorization参数，无法进行身份验证。","success":false}"#;
        match parse_err(UsageProviderId::GlmCoding, 200, body) {
            ProviderUsageError::TryNextAttempt { message } => {
                assert!(message.contains("Authorization"));
            }
            other => panic!("expected TryNextAttempt, got {other:?}"),
        }
    }

    #[test]
    fn success_false_other_code_is_parse_error() {
        let body = r#"{"code":1002,"msg":"token 无效","success":false}"#;
        match parse_err(UsageProviderId::GlmCoding, 200, body) {
            ProviderUsageError::Parse { message } => assert_eq!(message, "token 无效"),
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[test]
    fn missing_data_is_parse_error() {
        assert!(matches!(
            parse_err(UsageProviderId::GlmCoding, 200, r#"{"success":true}"#),
            ProviderUsageError::Parse { .. }
        ));
    }
}
