//! DeepSeek 开放平台 `GET /user/balance` 响应解析。

use crate::parse::{as_f64, fmt_num, mk_window};
use crate::types::{PlanUsageWindow, ProviderUsageError, UsageWindowKind};

/// 官方文档 https://api-docs.deepseek.com/zh-cn/api/get-user-balance：
/// `{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"110.00",
/// "granted_balance":"10.00","topped_up_balance":"100.00"}]}`（金额为字符串，币种
/// 取值 CNY/USD）。
///
/// 按量计费余额（调研文档「官方 API key 直连」：金额，不是百分比）：每个币种一条
/// [`UsageWindowKind::Balance`] 窗口，`used_pct` 恒 `None`，金额进 `note`
/// （`granted_balance` > 0 时附赠金标注；`topped_up_balance` = 余额 − 赠金，不重复
/// 展示）；`is_available=false`（余额不可用于 API 调用）按 Command Code `limited`
/// 先例追加 `· unavailable` 标记。`balance_infos` 缺失/空或没有可用条目 = Parse
/// 错误。
pub(crate) fn parse(
    v: &serde_json::Value,
) -> Result<(Option<String>, Vec<PlanUsageWindow>), ProviderUsageError> {
    let entries = v
        .get("balance_infos")
        .and_then(|b| b.as_array())
        .ok_or_else(|| ProviderUsageError::Parse {
            message: "no balance_infos in the reply".into(),
        })?;
    let unavailable = v.get("is_available").and_then(|b| b.as_bool()) == Some(false);
    let mut out = Vec::new();
    for entry in entries {
        let Some(total) = entry.get("total_balance").and_then(as_f64) else {
            continue;
        };
        let currency = entry.get("currency").and_then(|c| c.as_str()).unwrap_or("");
        let mut note = money(currency, total);
        if let Some(granted) = entry.get("granted_balance").and_then(as_f64)
            && granted > 0.0
        {
            note.push_str(&format!(" \u{b7} grants {}", money(currency, granted)));
        }
        if unavailable {
            note.push_str(" \u{b7} unavailable");
        }
        let mut win = mk_window(UsageWindowKind::Balance, None, None);
        win.note = Some(note);
        out.push(win);
    }
    if out.is_empty() {
        return Err(ProviderUsageError::Parse {
            message: "no usable balance entries in the reply".into(),
        });
    }
    Ok((None, out))
}

/// 金额展示：已知币种用符号前缀，未知币种回退货币代码（空值只显数字）。
fn money(currency: &str, amount: f64) -> String {
    match currency {
        "CNY" => format!("¥{}", fmt_num(amount)),
        "USD" => format!("${}", fmt_num(amount)),
        other if other.is_empty() => fmt_num(amount),
        other => format!("{other} {}", fmt_num(amount)),
    }
}

#[cfg(test)]
mod tests {
    use crate::types::{PlanUsageSnapshot, ProviderUsageError, UsageProviderId, UsageWindowKind};
    use crate::test_support::{parse_err, parse_ok};

    fn snap(status: u16, body: &str) -> PlanUsageSnapshot {
        parse_ok(UsageProviderId::DeepSeek, status, body)
    }

    #[test]
    fn doc_example_cny_balance_with_grants() {
        // 结构逐字对齐官方文档示例。
        let s = snap(
            200,
            r#"{"is_available":true,"balance_infos":[{"currency":"CNY",
                "total_balance":"110.00","granted_balance":"10.00","topped_up_balance":"100.00"}]}"#,
        );
        assert_eq!(s.provider, UsageProviderId::DeepSeek);
        assert_eq!(s.plan_name, None);
        assert_eq!(s.windows.len(), 1);
        let w = &s.windows[0];
        assert_eq!(w.kind, UsageWindowKind::Balance);
        assert_eq!(w.used_pct, None);
        assert_eq!(w.resets_at_ms, None);
        assert_eq!(w.note.as_deref(), Some("¥110 · grants ¥10"));
    }

    #[test]
    fn zero_grants_note_is_amount_only() {
        let s = snap(
            200,
            r#"{"is_available":true,"balance_infos":[{"currency":"USD",
                "total_balance":"12.50","granted_balance":"0.00","topped_up_balance":"12.50"}]}"#,
        );
        assert_eq!(s.windows[0].note.as_deref(), Some("$12.50"));
    }

    #[test]
    fn multi_currency_keeps_one_window_each() {
        let s = snap(
            200,
            r#"{"balance_infos":[{"currency":"CNY","total_balance":"110.00"},
                                 {"currency":"USD","total_balance":"3.40"}]}"#,
        );
        assert_eq!(s.windows.len(), 2);
        assert_eq!(s.windows[0].kind, UsageWindowKind::Balance);
        assert_eq!(s.windows[0].note.as_deref(), Some("¥110"));
        assert_eq!(s.windows[1].note.as_deref(), Some("$3.40"));
    }

    #[test]
    fn unavailable_flag_appended() {
        let s = snap(
            200,
            r#"{"is_available":false,"balance_infos":[{"currency":"CNY","total_balance":"0.00"}]}"#,
        );
        assert_eq!(s.windows[0].note.as_deref(), Some("¥0 · unavailable"));
    }

    #[test]
    fn unknown_currency_falls_back_to_code() {
        let s = snap(
            200,
            r#"{"balance_infos":[{"currency":"EUR","total_balance":"5.20"}]}"#,
        );
        assert_eq!(s.windows[0].note.as_deref(), Some("EUR 5.20"));
    }

    #[test]
    fn missing_or_empty_balance_infos_is_parse_error() {
        assert!(matches!(
            parse_err(UsageProviderId::DeepSeek, 200, r#"{}"#),
            ProviderUsageError::Parse { .. }
        ));
        assert!(matches!(
            parse_err(
                UsageProviderId::DeepSeek,
                200,
                r#"{"is_available":true,"balance_infos":[]}"#
            ),
            ProviderUsageError::Parse { .. }
        ));
        // 条目缺 total_balance 视为不可用条目；全空 → Parse。
        assert!(matches!(
            parse_err(
                UsageProviderId::DeepSeek,
                200,
                r#"{"balance_infos":[{"currency":"CNY"}]}"#
            ),
            ProviderUsageError::Parse { .. }
        ));
    }

    #[test]
    fn http_401_extracts_openai_style_envelope() {
        let body = r#"{"error":{"message":"Authentication Fails (no such user)",
                          "type":"authentication_error"}}"#;
        match parse_err(UsageProviderId::DeepSeek, 401, body) {
            ProviderUsageError::Unauthorized { message } => {
                assert_eq!(message, "Authentication Fails (no such user)");
            }
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }

    #[test]
    fn http_500_is_http_error() {
        assert!(matches!(
            parse_err(UsageProviderId::DeepSeek, 500, "boom"),
            ProviderUsageError::Http { status: 500, .. }
        ));
    }
}
