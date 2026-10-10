//! MiniMax Coding Plan `GET /v1/token_plan/remains` 响应解析。

use crate::parse::{as_f64, mk_window, parse_epoch_ms};
use crate::types::{PlanUsageWindow, ProviderUsageError, UsageWindowKind};

/// `{"model_remains":[{"model_name":"general",
///   "start_time":…,"end_time":…,"current_interval_remaining_percent":100,
///   "current_interval_status":1,"current_interval_total_count":0,
///   "weekly_start_time":…,"weekly_end_time":…,"current_weekly_remaining_percent":100,
///   "current_weekly_status":1,"current_weekly_total_count":0}],
///  "base_resp":{"status_code":0,"status_msg":"success"}}`
/// （magpie `readMiniMaxPlan` 文档注释 + cc-switch `parse_minimax_tiers`）。
///
/// 只取 `general`（编程套餐；video 等另桶不拦模型；缺失时退回第一个具名桶）。
/// 剩余百分比反转为已用。status：3 = 无此限额（跳过），2 = 已用尽（100%，不看
/// 百分比），1 = 正常。无周限套餐 weekly_status=3。MiniMax 对拒绝的 key 也回 200，
/// 原因在 base_resp。时间戳秒/毫秒都见过（以 1e12 为界）。
pub(crate) fn parse(
    v: &serde_json::Value,
) -> Result<(Option<String>, Vec<PlanUsageWindow>), ProviderUsageError> {
    if let Some(base) = v.get("base_resp") {
        let code = base.get("status_code").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = base
                .get("status_msg")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(ProviderUsageError::Parse {
                message: format!("API error (code {code}): {msg}"),
            });
        }
    }
    let Some(remains) = v.get("model_remains").and_then(|r| r.as_array()) else {
        return Err(ProviderUsageError::Parse {
            message: "no model_remains in the reply".into(),
        });
    };
    let bucket = remains
        .iter()
        .find(|b| {
            b.get("model_name")
                .and_then(|n| n.as_str())
                .map(|n| n.eq_ignore_ascii_case("general"))
                .unwrap_or(false)
        })
        .or_else(|| remains.first());
    let Some(bucket) = bucket else {
        return Err(no_plan());
    };
    let interval_status = bucket.get("current_interval_status").and_then(|s| s.as_i64());
    let weekly_status = bucket.get("current_weekly_status").and_then(|s| s.as_i64());
    // 桶级「不在套餐内」：两窗都无限（status 3）且计数全 0 → 整桶跳过。
    let zero = |f: &serde_json::Value| f.as_f64() == Some(0.0);
    if interval_status == Some(3)
        && weekly_status == Some(3)
        && bucket
            .get("current_interval_total_count")
            .map(zero)
            .unwrap_or(false)
        && bucket
            .get("current_weekly_total_count")
            .map(zero)
            .unwrap_or(false)
    {
        return Err(no_plan());
    }
    // 单窗：status 3 跳过；status 2 = 用尽（100%）；否则 100 − 剩余。
    let minimax_window = |status: Option<i64>,
                          left_key: &str,
                          end_key: &str,
                          kind: UsageWindowKind|
     -> Option<PlanUsageWindow> {
        if status == Some(3) {
            return None;
        }
        let left = bucket.get(left_key).and_then(as_f64);
        if left.is_none() && status != Some(2) {
            return None;
        }
        let used_pct = if status == Some(2) {
            Some(100.0)
        } else {
            left.map(|l| crate::parse::clamp_pct(100.0 - l))
        };
        Some(mk_window(kind, used_pct, bucket.get(end_key).and_then(parse_epoch_ms)))
    };
    let mut out = Vec::new();
    if let Some(w) = minimax_window(
        interval_status,
        "current_interval_remaining_percent",
        "end_time",
        UsageWindowKind::FiveHour,
    ) {
        out.push(w);
    }
    if let Some(w) = minimax_window(
        weekly_status,
        "current_weekly_remaining_percent",
        "weekly_end_time",
        UsageWindowKind::Weekly,
    ) {
        out.push(w);
    }
    if out.is_empty() {
        return Err(no_plan());
    }
    Ok((None, out))
}

fn no_plan() -> ProviderUsageError {
    ProviderUsageError::Parse {
        message: "no plan bucket in the reply".into(),
    }
}

#[cfg(test)]
mod tests {
    use crate::types::{PlanUsageSnapshot, ProviderUsageError, UsageProviderId, UsageWindowKind};
    use crate::test_support::{parse_err, parse_ok, pct_of};

    fn snap(status: u16, body: &str) -> PlanUsageSnapshot {
        parse_ok(UsageProviderId::MiniMaxCoding, status, body)
    }

    #[test]
    fn general_bucket_remaining_inverted() {
        let s = snap(
            200,
            r#"{"model_remains":[
                {"model_name":"general",
                 "start_time":1760000000000,"end_time":1760018000000,
                 "current_interval_remaining_percent":70,"current_interval_status":1,
                 "current_interval_total_count":0,
                 "weekly_start_time":1759400000000,"weekly_end_time":1760004400000,
                 "current_weekly_remaining_percent":30,"current_weekly_status":1,
                 "current_weekly_total_count":0},
                {"model_name":"video","current_interval_remaining_percent":50,
                 "current_interval_status":1,"current_weekly_status":3}],
                "base_resp":{"status_code":0,"status_msg":"success"}}"#,
        );
        assert_eq!(s.windows.len(), 2);
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(30.0), "剩余反转");
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(70.0));
        assert_eq!(s.windows[0].resets_at_ms, Some(1_760_018_000_000));
        assert_eq!(s.windows[1].resets_at_ms, Some(1_760_004_400_000));
    }

    #[test]
    fn no_weekly_when_status_unlimited() {
        let s = snap(
            200,
            r#"{"model_remains":[{"model_name":"general",
                "end_time":1760018000000,"current_interval_remaining_percent":10,
                "current_interval_status":1,"current_weekly_status":3,
                "current_interval_total_count":0,"current_weekly_total_count":0}],
                "base_resp":{"status_code":0,"status_msg":"success"}}"#,
        );
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].kind, UsageWindowKind::FiveHour);
        assert_eq!(s.windows[0].used_pct, Some(90.0));
    }

    #[test]
    fn status_2_is_used_up() {
        let s = snap(
            200,
            r#"{"model_remains":[{"model_name":"general",
                "current_interval_status":2,"current_weekly_status":1,
                "current_weekly_remaining_percent":5}],
                "base_resp":{"status_code":0,"status_msg":"success"}}"#,
        );
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(100.0));
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(95.0));
    }

    #[test]
    fn weekly_status_2_without_left_is_used_up() {
        let s = snap(
            200,
            r#"{"model_remains":[{"model_name":"general",
                "end_time":1760018000000,"current_interval_remaining_percent":10,
                "current_interval_status":1,"current_weekly_status":2}],
                "base_resp":{"status_code":0,"status_msg":"success"}}"#,
        );
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(100.0));
    }

    #[test]
    fn bucket_without_general_falls_back_to_first() {
        let s = snap(
            200,
            r#"{"model_remains":[{"model_name":"minimax-m3",
                "end_time":1760018000000,"current_interval_remaining_percent":40,
                "current_interval_status":1,"current_weekly_status":3}],
                "base_resp":{"status_code":0,"status_msg":"success"}}"#,
        );
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(60.0));
    }

    #[test]
    fn seconds_timestamps_accepted() {
        let s = snap(
            200,
            r#"{"model_remains":[{"model_name":"general",
                "end_time":1760018000,"current_interval_remaining_percent":40,
                "current_interval_status":1,"current_weekly_status":3}],
                "base_resp":{"status_code":0,"status_msg":"success"}}"#,
        );
        assert_eq!(s.windows[0].resets_at_ms, Some(1_760_018_000_000));
    }

    #[test]
    fn base_resp_error_is_parse_error() {
        let body = r#"{"base_resp":{"status_code":1004,"status_msg":"invalid key"}}"#;
        match parse_err(UsageProviderId::MiniMaxCoding, 200, body) {
            ProviderUsageError::Parse { message } => {
                assert!(message.contains("1004") && message.contains("invalid key"));
            }
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[test]
    fn not_in_plan_bucket_is_parse_error() {
        let body = r#"{"model_remains":[{"model_name":"video",
            "current_interval_status":3,"current_weekly_status":3,
            "current_interval_total_count":0,"current_weekly_total_count":0}],
            "base_resp":{"status_code":0,"status_msg":"success"}}"#;
        assert!(matches!(
            parse_err(UsageProviderId::MiniMaxCoding, 200, body),
            ProviderUsageError::Parse { .. }
        ));
    }

    #[test]
    fn missing_model_remains_is_parse_error() {
        assert!(matches!(
            parse_err(UsageProviderId::MiniMaxCoding, 200, r#"{"base_resp":{"status_code":0}}"#),
            ProviderUsageError::Parse { .. }
        ));
    }
}
