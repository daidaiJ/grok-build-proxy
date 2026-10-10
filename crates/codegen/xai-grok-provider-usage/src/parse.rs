//! 解析分发与共享取值帮助。

use crate::types::{PlanUsageSnapshot, PlanUsageWindow, ProviderUsageError, UsageProviderId, UsageWindowKind};
use crate::providers;

/// 解析一次 HTTP 响应为快照。2xx 之外的按凭据/HTTP 分层，2xx 交给各供应商解析器。
pub(super) fn parse_response(
    provider: UsageProviderId,
    status: u16,
    body: &str,
    now_ms: u64,
) -> Result<PlanUsageSnapshot, ProviderUsageError> {
    if !(200..300).contains(&status) {
        let message =
            error_message(provider, body).unwrap_or_else(|| format!("HTTP {status}"));
        // 403 + opencode = 无 Go 订阅资格；401/403 其余 = key 无效。都归凭据问题。
        return Err(if status == 401 || status == 403 {
            ProviderUsageError::Unauthorized { message }
        } else {
            ProviderUsageError::Http { status, message }
        });
    }
    let value: serde_json::Value = serde_json::from_str(body).map_err(|e| {
        ProviderUsageError::Parse {
            message: format!("invalid JSON: {e}"),
        }
    })?;
    let (plan_name, windows) = match provider {
        UsageProviderId::OpenCodeGo => providers::opencode_go::parse(&value)?,
        UsageProviderId::CommandCode => providers::commandcode::parse(&value)?,
        UsageProviderId::GlmCoding => providers::glm::parse(&value)?,
        UsageProviderId::KimiCoding => providers::kimi::parse(&value)?,
        UsageProviderId::MiniMaxCoding => providers::minimax::parse(&value)?,
    };
    Ok(finish(provider, plan_name, windows, now_ms))
}

/// 从错误响应体提取人读信息（各家信封形态不同；提取不到返回 None）。
fn error_message(provider: UsageProviderId, body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    match provider {
        // {"type":"error","error":{"type":"AuthError","message":"Missing API key."}}
        UsageProviderId::OpenCodeGo => {
            Some(value.pointer("/error/message")?.as_str()?.to_string())
        }
        // {"success":false,"error":{"code":"UNAUTHORIZED",...}}
        UsageProviderId::CommandCode => {
            let code = value.pointer("/error/code").and_then(|v| v.as_str())?;
            let detail = value
                .pointer("/error/message")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if detail.is_empty() {
                Some(code.to_string())
            } else {
                Some(format!("{code}: {detail}"))
            }
        }
        _ => None,
    }
}

/// 数字或数字字符串 → f64（Kimi 的计数以字符串形态出现）。
pub(crate) fn as_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// 时间戳宽容解析：epoch 秒 / 毫秒（以 1e12 为界）或 RFC 3339 字符串 → epoch ms。
pub(crate) fn parse_epoch_ms(v: &serde_json::Value) -> Option<u64> {
    match v {
        serde_json::Value::Number(_) => {
            let f = v.as_f64()?;
            if f <= 0.0 {
                return None;
            }
            Some(if f < 1e12 { (f * 1000.0) as u64 } else { f as u64 })
        }
        serde_json::Value::String(s) => parse_epoch_ms_str(s),
        _ => None,
    }
}

fn parse_epoch_ms_str(s: &str) -> Option<u64> {
    let t = s.trim();
    if let Ok(f) = t.parse::<f64>() {
        if f <= 0.0 {
            return None;
        }
        return Some(if f < 1e12 { (f * 1000.0) as u64 } else { f as u64 });
    }
    chrono::DateTime::parse_from_rfc3339(t)
        .ok()
        .map(|dt| dt.timestamp_millis() as u64)
}

pub(crate) fn clamp_pct(pct: f64) -> f64 {
    pct.clamp(0.0, 100.0)
}

/// 数值展示：整数不带小数，小数最多两位（note 里的金额/计数）。
pub(crate) fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

pub(crate) fn mk_window(
    kind: UsageWindowKind,
    used_pct: Option<f64>,
    resets_at_ms: Option<u64>,
) -> PlanUsageWindow {
    PlanUsageWindow {
        kind,
        used_pct: used_pct.map(clamp_pct),
        resets_at_ms,
        note: None,
    }
}

/// 组装并排序快照（渲染优先级 FiveHour → Weekly → Monthly）。
pub(crate) fn finish(
    provider: UsageProviderId,
    plan_name: Option<String>,
    mut windows: Vec<PlanUsageWindow>,
    now_ms: u64,
) -> PlanUsageSnapshot {
    windows.sort_by_key(|w| w.kind.rank());
    PlanUsageSnapshot {
        provider,
        plan_name,
        windows,
        fetched_at_ms: now_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_ms_accepts_seconds_millis_and_rfc3339() {
        assert_eq!(
            parse_epoch_ms(&serde_json::json!(1_790_000_000i64)),
            Some(1_790_000_000_000)
        );
        assert_eq!(
            parse_epoch_ms(&serde_json::json!(1_790_000_000_000i64)),
            Some(1_790_000_000_000)
        );
        assert_eq!(
            parse_epoch_ms(&serde_json::json!("2026-08-26T14:12:03.000Z")),
            Some(1_787_753_523_000)
        );
        assert_eq!(parse_epoch_ms(&serde_json::json!(0)), None);
        assert_eq!(parse_epoch_ms(&serde_json::json!("bogus")), None);
    }

    #[test]
    fn as_f64_numbers_and_numeric_strings() {
        assert_eq!(as_f64(&serde_json::json!(12)), Some(12.0));
        assert_eq!(as_f64(&serde_json::json!(3.5)), Some(3.5));
        assert_eq!(as_f64(&serde_json::json!(" 88 ")), Some(88.0));
        assert_eq!(as_f64(&serde_json::json!("x")), None);
        assert_eq!(as_f64(&serde_json::json!(null)), None);
    }

    #[test]
    fn fmt_num_integer_and_fraction() {
        assert_eq!(fmt_num(12.0), "12");
        assert_eq!(fmt_num(3.1), "3.10");
        assert_eq!(fmt_num(41.25), "41.25");
    }

    #[test]
    fn clamp_pct_bounds() {
        assert_eq!(clamp_pct(-5.0), 0.0);
        assert_eq!(clamp_pct(150.0), 100.0);
        assert_eq!(clamp_pct(42.0), 42.0);
    }

    #[test]
    fn finish_sorts_windows_by_rank() {
        let snap = finish(
            UsageProviderId::MiniMaxCoding,
            None,
            vec![
                mk_window(UsageWindowKind::Weekly, Some(1.0), None),
                mk_window(UsageWindowKind::FiveHour, Some(2.0), None),
            ],
            0,
        );
        assert_eq!(snap.windows[0].kind, UsageWindowKind::FiveHour);
        assert_eq!(snap.windows[1].kind, UsageWindowKind::Weekly);
        assert_eq!(snap.fetched_at_ms, 0);
    }
}
