//! Command Code `GET /alpha/billing/credits` 响应解析。

use crate::parse::{as_f64, fmt_num, mk_window, parse_epoch_ms};
use crate::types::{PlanUsageWindow, ProviderUsageError, UsageWindowKind};

/// `/alpha/billing/credits`（端到端实测结构，调研文档 §3；对照 magpie `cmdCredits`）：
/// `{"credits":{"planId":"individual-goat-monthly","monthlyCredits":41.2,
/// "purchasedCredits":5,"freeCredits":0},"windowLimits":{"limited":true,
/// "fiveHour":{"used":3.1,"cap":10,"exceeded":false,"resetAt":1790000000000},
/// "weekly":{…}}}`。
///
/// `used`/`cap` 是美元，`used_pct = used/cap×100`；`resetAt` 毫秒（CLI 用
/// `Date.now()` 直接比较，容忍秒/RFC3339）。`cap` 缺失/0 → `used_pct=None`，金额
/// 进 note。每窗 `exceeded` 才是权威拦停判据（进 note），顶层 `limited` 仅标志位
/// （进 note）。无套餐的 pay-as-you-go key：credits 在、windowLimits 缺 → 空窗口集。
pub(crate) fn parse(
    v: &serde_json::Value,
) -> Result<(Option<String>, Vec<PlanUsageWindow>), ProviderUsageError> {
    let plan_name = v
        .pointer("/credits/planId")
        .and_then(|p| p.as_str())
        .map(|s| s.to_string());
    let Some(windows) = v.get("windowLimits") else {
        return Ok((plan_name, Vec::new()));
    };
    let limited_flag = windows
        .get("limited")
        .and_then(|b| b.as_bool())
        .unwrap_or(false);
    let mut out = Vec::new();
    for (key, kind) in [
        ("fiveHour", UsageWindowKind::FiveHour),
        ("weekly", UsageWindowKind::Weekly),
    ] {
        let Some(w) = windows.get(key) else {
            continue;
        };
        let used = w.get("used").and_then(as_f64);
        let cap = w.get("cap").and_then(as_f64);
        let resets_at = w.get("resetAt").and_then(parse_epoch_ms);
        let mut note = match (used, cap) {
            (Some(used), Some(cap)) if cap > 0.0 => {
                Some(format!("${} / ${}", fmt_num(used), fmt_num(cap)))
            }
            (Some(used), _) => Some(format!("${} (cap unknown)", fmt_num(used))),
            _ => None,
        };
        let used_pct = match (used, cap) {
            (Some(used), Some(cap)) if cap > 0.0 => Some(crate::parse::clamp_pct(used / cap * 100.0)),
            _ => None,
        };
        if w.get("exceeded").and_then(|b| b.as_bool()) == Some(true) {
            note = Some(match note {
                Some(n) => format!("{n} · exceeded"),
                None => "exceeded".to_string(),
            });
        }
        if limited_flag {
            note = Some(match note {
                Some(n) => format!("{n} · limited"),
                None => "limited".to_string(),
            });
        }
        if note.is_none() && used_pct.is_none() {
            continue;
        }
        let mut win = mk_window(kind, used_pct, resets_at);
        win.note = note;
        out.push(win);
    }
    Ok((plan_name, out))
}

#[cfg(test)]
mod tests {
    use crate::types::{PlanUsageSnapshot, ProviderUsageError, UsageProviderId, UsageWindowKind};
    use crate::test_support::{parse_err, parse_ok, pct_of};

    fn snap(status: u16, body: &str) -> PlanUsageSnapshot {
        parse_ok(UsageProviderId::CommandCode, status, body)
    }

    #[test]
    fn windows_dollar_ratio_and_notes() {
        // 结构逐字对齐调研文档 §3 实测（值取 magpie cmdCredits 文档注释）。
        let s = snap(
            200,
            r#"{"credits":{"planId":"individual-goat-monthly","monthlyCredits":41.2,
                "purchasedCredits":5,"freeCredits":0},
                "windowLimits":{"limited":true,
                "fiveHour":{"used":3.1,"cap":10,"resetAt":1790000000000},
                "weekly":{"used":12,"cap":40,"resetAt":1790400000000}}}"#,
        );
        assert_eq!(s.plan_name.as_deref(), Some("individual-goat-monthly"));
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(31.0));
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(30.0));
        let five = &s.windows[0];
        assert_eq!(five.kind, UsageWindowKind::FiveHour);
        assert_eq!(five.resets_at_ms, Some(1_790_000_000_000));
        assert_eq!(five.note.as_deref(), Some("$3.10 / $10 · limited"));
        assert_eq!(s.windows[1].note.as_deref(), Some("$12 / $40 · limited"));
    }

    #[test]
    fn exceeded_is_authoritative_note() {
        let s = snap(
            200,
            r#"{"credits":{"planId":"goat"},"windowLimits":{
                "fiveHour":{"used":10,"cap":10,"exceeded":true,"resetAt":1790000000000}}}"#,
        );
        let five = &s.windows[0];
        assert_eq!(five.used_pct, Some(100.0));
        assert_eq!(five.note.as_deref(), Some("$10 / $10 · exceeded"));
    }

    #[test]
    fn missing_cap_keeps_amount_in_note() {
        let s = snap(
            200,
            r#"{"credits":{},"windowLimits":{"fiveHour":{"used":3.5}}}"#,
        );
        let five = &s.windows[0];
        assert_eq!(five.used_pct, None);
        assert_eq!(five.note.as_deref(), Some("$3.50 (cap unknown)"));
    }

    #[test]
    fn no_window_limits_is_empty_snapshot() {
        let s = snap(200, r#"{"credits":{"planId":"paygo","monthlyCredits":10}}"#);
        assert!(s.windows.is_empty());
        assert_eq!(s.plan_name.as_deref(), Some("paygo"));
    }

    #[test]
    fn reset_at_tolerates_seconds_and_rfc3339() {
        let s = snap(
            200,
            r#"{"windowLimits":{"fiveHour":{"used":1,"cap":2,"resetAt":1790000000}}}"#,
        );
        assert_eq!(s.windows[0].resets_at_ms, Some(1_790_000_000_000));
        let s = snap(
            200,
            r#"{"windowLimits":{"fiveHour":{"used":1,"cap":2,"resetAt":"2026-08-26T14:12:03Z"}}}"#,
        );
        assert_eq!(s.windows[0].resets_at_ms, Some(1_787_753_523_000));
    }

    #[test]
    fn http_401_unauthorized_envelope() {
        let body = r#"{"success":false,"error":{"code":"UNAUTHORIZED"}}"#;
        match parse_err(UsageProviderId::CommandCode, 401, body) {
            ProviderUsageError::Unauthorized { message } => {
                assert_eq!(message, "UNAUTHORIZED");
            }
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }

    #[test]
    fn http_403_is_unauthorized() {
        assert!(matches!(
            parse_err(UsageProviderId::CommandCode, 403, "{}"),
            ProviderUsageError::Unauthorized { .. }
        ));
    }
}
