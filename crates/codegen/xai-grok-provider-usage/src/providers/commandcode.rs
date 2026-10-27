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

/// 互补合并（`crate::parse_merged` 钩子）：第二段 `GET /alpha/usage/summary`
/// 是账期聚合（实测 2026-10-27：`totalMonthlyCredits` = 本账期已用 credits，
/// `periodBasis:"billing-period"`），与第一段 credits 的月度剩余
/// `monthlyCredits` 合成月度窗口：total = remaining + used，used_pct =
/// used/total×100，note 带 $ 金额。summary 非 2xx / 无字段 / `periodBasis`
/// 变化 / credits 段无 monthlyCredits（pay-as-you-go）→ 不加月度窗，单段行为
/// 不变（上游不直接给月度 used/cap，此为推算口径）。
pub(crate) fn merge(
    windows: &mut Vec<PlanUsageWindow>,
    parts: &[(u16, &str)],
) -> Result<(), ProviderUsageError> {
    let Some((credits_status, credits_body)) = parts.first() else {
        return Ok(());
    };
    if !(200..300).contains(credits_status) {
        return Ok(());
    }
    let Ok(credits) = serde_json::from_str::<serde_json::Value>(credits_body) else {
        return Ok(());
    };
    let Some(remaining) = credits.pointer("/credits/monthlyCredits").and_then(as_f64) else {
        return Ok(());
    };
    let Some((summary_status, summary_body)) = parts.get(1) else {
        return Ok(());
    };
    if !(200..300).contains(summary_status) {
        return Ok(());
    }
    let Ok(summary) = serde_json::from_str::<serde_json::Value>(summary_body) else {
        return Ok(());
    };
    if summary
        .get("periodBasis")
        .and_then(|b| b.as_str())
        .is_some_and(|b| b != "billing-period")
    {
        return Ok(());
    }
    let Some(used) = summary
        .get("totalMonthlyCredits")
        .and_then(as_f64)
        .or_else(|| summary.get("totalCredits").and_then(as_f64))
        .filter(|used| *used >= 0.0)
    else {
        return Ok(());
    };
    let total = remaining + used;
    if total <= 0.0 {
        return Ok(());
    }
    let mut win = mk_window(
        UsageWindowKind::Monthly,
        Some(crate::parse::clamp_pct(used / total * 100.0)),
        None,
    );
    win.note = Some(format!("${} / ${} \u{b7} credits", fmt_num(used), fmt_num(total)));
    windows.push(win);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::types::{PlanUsageSnapshot, ProviderUsageError, UsageProviderId, UsageWindowKind};
    use crate::test_support::{parse_err, parse_merged_ok, parse_ok, pct_of};

    fn snap(status: u16, body: &str) -> PlanUsageSnapshot {
        parse_ok(UsageProviderId::CommandCode, status, body)
    }

    const CREDITS: &str = r#"{"credits":{"planId":"individual-goat","monthlyCredits":65.4663592199,
        "purchasedCredits":0,"freeCredits":0},
        "windowLimits":{"limited":true,"exceeded":null,
        "fiveHour":{"used":0.0375,"cap":14,"exceeded":false,"resetAt":1793120809374},
        "weekly":{"used":1.0311,"cap":35,"exceeded":false,"resetAt":1793695849254}}}"#;
    const SUMMARY: &str = r#"{"totalCount":3015,"totalCost":4.6065888259,"totalCredits":4.6065888259,
        "totalMonthlyCredits":4.6065888259,"periodBasis":"billing-period"}"#;

    #[test]
    fn merged_summary_adds_monthly_window() {
        // 2026-10-27 活体 probe 数值：月度 = 剩余 65.47 + 账期已用 4.61 → 6.57%。
        let s = parse_merged_ok(UsageProviderId::CommandCode, &[(200, CREDITS), (200, SUMMARY)]);
        assert_eq!(s.windows.len(), 3);
        let month = s
            .windows
            .iter()
            .find(|w| w.kind == UsageWindowKind::Monthly)
            .expect("monthly window");
        let expected_pct: f64 = 4.6065888259 / (65.4663592199 + 4.6065888259) * 100.0;
        assert_eq!(month.used_pct, Some(expected_pct.clamp(0.0, 100.0)));
        assert_eq!(month.note.as_deref(), Some("$4.61 / $70.07 \u{b7} credits"));
        assert_eq!(month.resets_at_ms, None, "账期重置在上游 summary 未给");
        // 5h/周窗不受 merge 影响。
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(0.0375 / 14.0 * 100.0));
    }

    #[test]
    fn merged_single_part_is_credits_only() {
        let s = parse_merged_ok(UsageProviderId::CommandCode, &[(200, CREDITS)]);
        assert_eq!(s.windows.len(), 2);
        assert!(!s.windows.iter().any(|w| w.kind == UsageWindowKind::Monthly));
    }

    #[test]
    fn merged_summary_failure_keeps_credits_windows() {
        let s = parse_merged_ok(
            UsageProviderId::CommandCode,
            &[(200, CREDITS), (500, "boom")],
        );
        assert_eq!(s.windows.len(), 2);
        // 第一段 401 照常归 Unauthorized（merge 不改变凭据分层）。
        let err = crate::parse_merged(UsageProviderId::CommandCode, &[(401, r#"{"error":{"code":"UNAUTHORIZED"}}"#), (200, SUMMARY)], 0)
            .expect_err("401");
        assert!(matches!(err, ProviderUsageError::Unauthorized { .. }));
    }

    #[test]
    fn merged_period_basis_mismatch_skips_monthly() {
        let s = parse_merged_ok(
            UsageProviderId::CommandCode,
            &[(200, CREDITS), (200, r#"{"totalMonthlyCredits":9,"periodBasis":"daily"}"#)],
        );
        assert_eq!(s.windows.len(), 2);
    }

    #[test]
    fn merged_paygo_key_still_gets_monthly_credits_window() {
        // pay-as-you-go 无滚动窗口，但 credits 段有 monthlyCredits → 月度窗照常合成。
        let s = parse_merged_ok(
            UsageProviderId::CommandCode,
            &[(200, r#"{"credits":{"monthlyCredits":10}}"#), (200, SUMMARY)],
        );
        let month = s
            .windows
            .iter()
            .find(|w| w.kind == UsageWindowKind::Monthly)
            .expect("monthly from credits pool");
        let expected_pct: f64 = 4.6065888259 / (10.0 + 4.6065888259) * 100.0;
        assert_eq!(month.used_pct, Some(expected_pct.clamp(0.0, 100.0)));
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
