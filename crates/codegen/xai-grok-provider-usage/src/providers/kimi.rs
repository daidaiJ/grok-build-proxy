//! Kimi For Coding `GET /coding/v1/usages` 响应解析。

use crate::parse::{as_f64, fmt_num, mk_window, parse_epoch_ms};
use crate::types::{PlanUsageWindow, ProviderUsageError, UsageWindowKind};

/// `{"usage":{"limit":"100","used":"12","resetTime":"2026-09-30T05:24:18.44Z"},
///   "limits":[{"window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},
///              "detail":{"limit":"100","remaining":"88","resetTime":"…"}}]}`
/// （magpie `readKimiCode` 文档注释，对齐 kimi-cli `/usage`）。
///
/// 根 `usage` 是周 allowance，`limits[]` 各是更短窗口（5h 桶）；计数可能以字符串
/// 出现，`used` 直接给或 `remaining` 反推（`used_pct = used/limit×100`）。limits 条目
/// 的窗口身份按 span：`duration × timeUnit` ≥ 24h 归周（根 usage 缺失时兜底），否则
/// 归 5h（只取第一个）。
pub(crate) fn parse(
    v: &serde_json::Value,
) -> Result<(Option<String>, Vec<PlanUsageWindow>), ProviderUsageError> {
    fn counted(d: &serde_json::Value) -> Option<(f64, Option<f64>, Option<u64>)> {
        let limit = d.get("limit").and_then(as_f64)?;
        if limit <= 0.0 {
            return None;
        }
        let used = match d.get("used").and_then(as_f64) {
            Some(used) => Some(used),
            None => d.get("remaining").and_then(as_f64).map(|r| limit - r),
        };
        let resets_at = ["resetTime", "resetAt", "reset_at", "reset_time"]
            .iter()
            .find_map(|k| d.get(*k).and_then(parse_epoch_ms));
        Some((limit, used, resets_at))
    }
    let mut five_hour: Option<PlanUsageWindow> = None;
    let mut weekly: Option<PlanUsageWindow> = None;
    if let Some(usage) = v.get("usage")
        && let Some((limit, used, resets_at)) = counted(usage)
    {
        let mut win =
            mk_window(UsageWindowKind::Weekly, used.map(|u| u / limit * 100.0), resets_at);
        if let Some(used) = used {
            win.note = Some(format!("{} / {}", fmt_num(used), fmt_num(limit)));
        }
        weekly = Some(win);
    }
    if let Some(limits) = v.get("limits").and_then(|l| l.as_array()) {
        for item in limits {
            let detail = item.get("detail").or_else(|| item.get("window"));
            let Some(detail) = detail else { continue };
            let Some((limit, used, resets_at)) = counted(detail) else {
                continue;
            };
            let span_hours = item
                .get("window")
                .map(|w| {
                    let n = w.get("duration").and_then(as_f64).unwrap_or(0.0);
                    let unit = w.get("timeUnit").and_then(|u| u.as_str()).unwrap_or("");
                    if unit.contains("MINUTE") {
                        n / 60.0
                    } else if unit.contains("HOUR") {
                        n
                    } else if unit.contains("DAY") {
                        n * 24.0
                    } else {
                        0.0
                    }
                })
                .unwrap_or(0.0);
            let mut win = mk_window(
                if span_hours >= 24.0 {
                    UsageWindowKind::Weekly
                } else {
                    UsageWindowKind::FiveHour
                },
                used.map(|u| u / limit * 100.0),
                resets_at,
            );
            if let Some(used) = used {
                win.note = Some(format!("{} / {}", fmt_num(used), fmt_num(limit)));
            }
            match win.kind {
                UsageWindowKind::FiveHour if five_hour.is_none() => five_hour = Some(win),
                UsageWindowKind::Weekly if weekly.is_none() => weekly = Some(win),
                _ => {}
            }
        }
    }
    let windows: Vec<PlanUsageWindow> = [five_hour, weekly].into_iter().flatten().collect();
    if windows.is_empty() {
        return Err(ProviderUsageError::Parse {
            message: "no usage in the reply".into(),
        });
    }
    Ok((None, windows))
}

#[cfg(test)]
mod tests {
    use crate::types::{PlanUsageSnapshot, ProviderUsageError, UsageProviderId, UsageWindowKind};
    use crate::test_support::{parse_err, parse_ok, pct_of};

    fn snap(status: u16, body: &str) -> PlanUsageSnapshot {
        parse_ok(UsageProviderId::KimiCoding, status, body)
    }

    #[test]
    fn windows_from_limits_and_usage() {
        // 结构逐字对齐 magpie readKimiCode 文档注释（字符串计数形态）。
        let s = snap(
            200,
            r#"{"usage":{"limit":"100","used":"12","resetTime":"2026-09-30T05:24:18.44Z"},
                "limits":[{"window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE"},
                           "detail":{"limit":"100","remaining":"88","resetTime":"2026-09-30T01:24:18.44Z"}}]}"#,
        );
        assert_eq!(s.windows.len(), 2);
        let five = &s.windows[0];
        assert_eq!(five.kind, UsageWindowKind::FiveHour);
        assert_eq!(five.used_pct, Some(12.0), "(100-88)/100");
        assert_eq!(five.note.as_deref(), Some("12 / 100"));
        assert!(five.resets_at_ms.is_some());
        let week = &s.windows[1];
        assert_eq!(week.kind, UsageWindowKind::Weekly);
        assert_eq!(week.used_pct, Some(12.0));
        assert!(week.resets_at_ms.is_some());
    }

    #[test]
    fn numeric_counts_and_remaining_only() {
        let s = snap(200, r#"{"usage":{"limit":50,"remaining":10},"limits":[]}"#);
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(80.0));
        assert_eq!(s.windows[0].note.as_deref(), Some("40 / 50"));
    }

    #[test]
    fn detail_falls_back_to_window_row() {
        let s = snap(
            200,
            r#"{"usage":{"limit":100,"used":30},
                "limits":[{"window":{"duration":300,"timeUnit":"TIME_UNIT_MINUTE",
                                     "limit":100,"remaining":50}}]}"#,
        );
        assert_eq!(pct_of(&s, UsageWindowKind::FiveHour), Some(50.0));
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(30.0));
    }

    #[test]
    fn day_span_limit_maps_to_weekly() {
        let s = snap(
            200,
            r#"{"limits":[{"window":{"duration":7,"timeUnit":"TIME_UNIT_DAY"},
                            "detail":{"limit":100,"used":20}}]}"#,
        );
        assert_eq!(s.windows[0].kind, UsageWindowKind::Weekly);
        assert_eq!(pct_of(&s, UsageWindowKind::Weekly), Some(20.0));
    }

    #[test]
    fn no_usage_is_parse_error() {
        assert!(matches!(
            parse_err(UsageProviderId::KimiCoding, 200, r#"{"limits":[]}"#),
            ProviderUsageError::Parse { .. }
        ));
        assert!(matches!(
            parse_err(UsageProviderId::KimiCoding, 200, r#"{}"#),
            ProviderUsageError::Parse { .. }
        ));
    }
}
