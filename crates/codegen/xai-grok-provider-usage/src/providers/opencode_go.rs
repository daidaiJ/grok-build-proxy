//! OpenCode Go `GET /zen/go/v1/usage` 响应解析。

use crate::parse::{as_f64, mk_window, parse_epoch_ms};
use crate::types::{PlanUsageWindow, ProviderUsageError, UsageWindowKind};

/// `{"usage":{"rolling"|"weekly"|"monthly":{"status":"ok"|"rate-limited",
/// "percent":0-100 已用,"resetsAt":"RFC3339"}}}`（上游
/// `packages/console/app/src/routes/zen/go/v1/usage.ts`）。
///
/// 该端点未文档化且上线当天就改过一次形态（旧扁平 `rollingUsage` 已作废），逐窗口
/// 防御解析：缺 percent 的窗口跳过，不整体失败（仅 usage 整个缺失才报错）。
/// percent=0 时上游 resetsAt 是「now+窗口时长」的占位值（滚动窗按最后记账时间清零，
/// 此时窗口早已过期），丢弃。`status` 语义未穷举，非 "ok" 的原样进 note。
pub(crate) fn parse(
    v: &serde_json::Value,
) -> Result<(Option<String>, Vec<PlanUsageWindow>), ProviderUsageError> {
    const WINDOWS: [(&str, UsageWindowKind); 3] = [
        ("rolling", UsageWindowKind::FiveHour),
        ("weekly", UsageWindowKind::Weekly),
        ("monthly", UsageWindowKind::Monthly),
    ];
    let Some(usage) = v.get("usage") else {
        return Err(ProviderUsageError::Parse {
            message: "no usage in the reply".into(),
        });
    };
    let mut out = Vec::new();
    for (key, kind) in WINDOWS {
        let Some(w) = usage.get(key) else {
            continue;
        };
        let Some(pct) = w.get("percent").and_then(as_f64) else {
            continue;
        };
        let resets_at = if pct > 0.0 {
            w.get("resetsAt").and_then(parse_epoch_ms)
        } else {
            None
        };
        let mut win = mk_window(kind, Some(pct), resets_at);
        if let Some(status) = w.get("status").and_then(|s| s.as_str()) {
            if status != "ok" {
                win.note = Some(status.to_string());
            }
        }
        out.push(win);
    }
    if out.is_empty() {
        return Err(ProviderUsageError::Parse {
            message: "no usage in the reply".into(),
        });
    }
    Ok((None, out))
}

#[cfg(test)]
mod tests {
    use crate::types::{PlanUsageSnapshot, ProviderUsageError, UsageProviderId, UsageWindowKind};
    use crate::test_support::{parse_err, parse_ok, pct_of};

    fn snap(status: u16, body: &str) -> PlanUsageSnapshot {
        parse_ok(UsageProviderId::OpenCodeGo, status, body)
    }

    #[test]
    fn three_windows_used_semantics() {
        let s = snap(
            200,
            r#"{"usage":{
                "rolling":{"status":"ok","percent":37,"resetsAt":"2026-08-26T14:12:03.000Z"},
                "weekly":{"status":"ok","percent":12,"resetsAt":"2026-08-31T00:00:00Z"},
                "monthly":{"status":"rate-limited","percent":100,"resetsAt":"2026-09-01T00:00:00Z"}}}"#,
        );
        assert_eq!(s.provider, UsageProviderId::OpenCodeGo);
        assert_eq!(s.windows.len(), 3);
        assert_eq!(s.windows[0].kind, UsageWindowKind::FiveHour);
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(37.0));
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(12.0));
        assert_eq!(pct_of(&s, UsageWindowKind::Monthly), Some(100.0));
        assert_eq!(s.windows[0].resets_at_ms, Some(1_787_753_523_000));
        // status 非 "ok" 原样进 note
        assert_eq!(s.windows[2].note.as_deref(), Some("rate-limited"));
        assert_eq!(s.windows[0].note, None);
    }

    #[test]
    fn zero_percent_drops_placeholder_reset() {
        let s = snap(
            200,
            r#"{"usage":{"rolling":{"status":"ok","percent":0,
                "resetsAt":"2026-08-26T14:12:03.000Z"}}}"#,
        );
        let five = &s.windows[0];
        assert_eq!(five.used_pct, Some(0.0));
        assert_eq!(five.resets_at_ms, None, "0% 的 resetsAt 是占位值，丢弃");
    }

    #[test]
    fn missing_percent_window_skipped() {
        let s = snap(
            200,
            r#"{"usage":{"rolling":{"status":"ok","percent":37,
                "resetsAt":"2026-08-26T14:12:03.000Z"},
                "weekly":{"status":"ok"}}}"#,
        );
        assert_eq!(s.windows.len(), 1);
        assert_eq!(s.windows[0].kind, UsageWindowKind::FiveHour);
    }

    #[test]
    fn empty_usage_is_parse_error() {
        assert!(matches!(
            parse_err(UsageProviderId::OpenCodeGo, 200, r#"{"usage":{}}"#),
            ProviderUsageError::Parse { .. }
        ));
    }

    #[test]
    fn no_usage_field_is_parse_error() {
        assert!(matches!(
            parse_err(UsageProviderId::OpenCodeGo, 200, r#"{}"#),
            ProviderUsageError::Parse { .. }
        ));
    }

    #[test]
    fn http_403_no_subscription_is_unauthorized() {
        let body = r#"{"type":"error","error":{"type":"EntitlementError",
            "message":"OpenCode Go subscription required."}}"#;
        match parse_err(UsageProviderId::OpenCodeGo, 403, body) {
            ProviderUsageError::Unauthorized { message } => {
                assert!(message.contains("subscription"), "{message}");
            }
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }

    #[test]
    fn http_401_auth_error_is_unauthorized() {
        let body = r#"{"type":"error","error":{"type":"AuthError","message":"Missing API key."}}"#;
        match parse_err(UsageProviderId::OpenCodeGo, 401, body) {
            ProviderUsageError::Unauthorized { message } => {
                assert_eq!(message, "Missing API key.");
            }
            other => panic!("expected Unauthorized, got {other:?}"),
        }
    }

    #[test]
    fn http_500_is_http_error() {
        match parse_err(UsageProviderId::OpenCodeGo, 500, "boom") {
            ProviderUsageError::Http { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "HTTP 500");
            }
            other => panic!("expected Http, got {other:?}"),
        }
    }
}
