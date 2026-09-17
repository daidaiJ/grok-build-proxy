use super::*;
use chrono::TimeZone;
use xai_grok_shell::session::usage_file::TurnUsage;

fn now() -> DateTime<Local> {
    Local::now()
}

fn rfc3339(at: DateTime<Local>) -> String {
    at.fixed_offset().to_rfc3339()
}

fn summary(
    input: u64,
    output: u64,
    model_calls: u64,
    cost_usd_ticks: Option<i64>,
) -> UsageSummary {
    UsageSummary {
        input_tokens: input,
        output_tokens: output,
        total_tokens: input + output,
        model_calls,
        cost_usd_ticks,
        turn_count: 1,
        ..UsageSummary::default()
    }
}

fn turn(number: u32, ended_at: &str, usage: UsageSummary) -> TurnUsage {
    TurnUsage {
        turn_number: number,
        ended_at: ended_at.into(),
        usage,
    }
}

fn record(session_id: &str, project: &str, turns: Vec<TurnUsage>) -> SessionRecord {
    let mut file = SessionUsageFile::new(session_id);
    file.turns = turns;
    file.updated_at = "2026-09-15T09:00:00+00:00".into();
    SessionRecord {
        session_id: session_id.into(),
        project: Some(project.into()),
        file,
    }
}

/// A fixed wall-clock time today, so fixtures never straddle midnight
/// (a run started just after midnight put `now - 30h` on yesterday).
fn local_today(now: DateTime<Local>, hour: u32, minute: u32) -> DateTime<Local> {
    let naive = now.date_naive().and_hms_opt(hour, minute, 0).unwrap();
    Local
        .from_local_datetime(&naive)
        .single()
        .unwrap_or(now)
}
fn day_key(at: DateTime<Local>) -> String {
    at.format("%Y-%m-%d").to_string()
}

fn week_key(at: DateTime<Local>) -> String {
    at.format("%G-W%V").to_string()
}

#[test]
fn sessions_days_and_weeks_bucket_the_same_turns() {
    let now = now();
    // Two turns inside one local day (anchored at today 09:00/14:00, never
    // straddling midnight), a fourth outside the ISO week.
    let early = local_today(now, 9, 0);
    let late = local_today(now, 14, 0);
    let last_week = now - ChronoDuration::days(10);
    let records = vec![
        record(
            "sess-1",
            "/work/alpha",
            vec![
                turn(1, &rfc3339(early), summary(100, 10, 2, Some(5_000_000_000))),
                turn(2, &rfc3339(late), summary(50, 5, 1, None)),
            ],
        ),
        record(
            "sess-2",
            "/work/beta",
            vec![turn(1, &rfc3339(last_week), summary(10, 1, 1, Some(1_000_000_000)))],
        ),
    ];

    let report = aggregate(&records, None, None, now, None);

    assert_eq!(report.sessions.len(), 2);
    // Newest activity first.
    assert_eq!(report.sessions[0].session_id, "sess-1");
    let first = &report.sessions[0];
    assert_eq!(first.totals.turns, 2);
    assert_eq!(first.totals.input_tokens, 150);
    assert_eq!(first.totals.output_tokens, 15);
    assert_eq!(first.totals.model_calls, 3);
    assert_eq!(first.totals.cost_usd, Some(0.5));
    assert!(first.totals.cost_partial, "turn 2 reported no cost");

    // Both sess-1 turns land in their local day; sess-2's in the prior week.
    let early_day = day_key(early);
    let day = report
        .days
        .iter()
        .find(|row| row.key == early_day)
        .expect("sess-1's day bucket");
    assert_eq!(day.totals.turns, 2);
    assert_eq!(day.totals.input_tokens, 150);
    assert_eq!(day.sessions, 1);

    let early_week = week_key(early);
    let week = report
        .weeks
        .iter()
        .find(|row| row.key == early_week)
        .expect("sess-1's week bucket");
    assert_eq!(week.totals.turns, 2);
    assert_ne!(
        week_key(last_week), early_week,
        "the fixtures must span two ISO weeks for this split to mean anything"
    );
    let last_week_row = report
        .weeks
        .iter()
        .find(|row| row.key == week_key(last_week))
        .expect("sess-2's week bucket");
    assert_eq!(last_week_row.totals.turns, 1);
    assert_eq!(last_week_row.totals.cost_usd, Some(0.1));
    assert!(!last_week_row.totals.cost_partial);
    // Rows stay date-ordered.
    let days: Vec<&str> = report.days.iter().map(|row| row.key.as_str()).collect();
    let mut sorted = days.clone();
    sorted.sort();
    assert_eq!(days, sorted);
}

#[test]
fn days_window_drops_older_and_unparsable_turns() {
    let now = now();
    let fresh = now - ChronoDuration::hours(1);
    let stale = now - ChronoDuration::days(20);
    let records = vec![record(
        "sess-1",
        "/work/alpha",
        vec![
            turn(1, &rfc3339(stale), summary(999, 999, 9, None)),
            turn(2, &rfc3339(fresh), summary(10, 1, 1, None)),
            turn(3, "", summary(888, 888, 8, None)),
        ],
    )];

    let windowed = aggregate(&records, Some(7), None, now, None);
    assert_eq!(windowed.sessions.len(), 1, "a session with turns left stays");
    assert_eq!(windowed.sessions[0].totals.turns, 1);
    assert_eq!(windowed.sessions[0].totals.input_tokens, 10);
    assert!(windowed.days.iter().all(|row| row.totals.input_tokens == 10));

    // No window: the unparsable stamp counts, the stale one too.
    let all = aggregate(&records, None, None, now, None);
    assert_eq!(all.sessions[0].totals.turns, 3);
    assert_eq!(all.sessions[0].totals.input_tokens, 999 + 10 + 888);
}

#[test]
fn a_session_with_no_surviving_turns_is_dropped() {
    let now = now();
    let stale = now - ChronoDuration::days(30);
    let records = vec![record(
        "sess-old",
        "/work/gamma",
        vec![turn(1, &rfc3339(stale), summary(1, 1, 1, None))],
    )];
    assert!(aggregate(&records, Some(7), None, now, None).sessions.is_empty());
    assert_eq!(aggregate(&records, None, None, now, None).sessions.len(), 1);
}

fn model_entry(input: u64, output: u64, calls: u64, ticks: Option<i64>) -> UsageSummary {
    UsageSummary {
        input_tokens: input,
        output_tokens: output,
        total_tokens: input + output,
        model_calls: calls,
        cost_usd_ticks: ticks,
        ..UsageSummary::default()
    }
}

fn turn_with_models(number: u32, ended_at: &str, models: Vec<(&str, UsageSummary)>) -> TurnUsage {
    let mut usage = UsageSummary::default();
    for (id, entry) in &models {
        usage.input_tokens += entry.input_tokens;
        usage.output_tokens += entry.output_tokens;
        usage.total_tokens += entry.total_tokens;
        usage.model_calls += entry.model_calls;
        usage.cost_usd_ticks = match (usage.cost_usd_ticks, entry.cost_usd_ticks) {
            (Some(a), Some(b)) => Some(a + b),
            (a, b) => a.or(b),
        };
    }
    usage.turn_count = 1;
    usage.model_usage = models
        .into_iter()
        .map(|(id, entry)| (id.to_owned(), entry))
        .collect();
    turn(number, ended_at, usage)
}

#[test]
fn every_view_splits_by_model_id() {
    let now = now();
    let at = now - ChronoDuration::hours(2);
    let records = vec![record(
        "sess-mix",
        "/work/mixed",
        vec![turn_with_models(
            1,
            &rfc3339(at),
            vec![
                ("grok-build", model_entry(100, 10, 3, Some(5_000_000_000))),
                ("glm-5", model_entry(20, 2, 1, None)),
            ],
        )],
    )];

    let report = aggregate(&records, None, None, now, None);

    let session = &report.sessions[0];
    assert_eq!(session.models.len(), 2);
    // Busiest first: grok-build has more calls.
    assert_eq!(session.models[0].model_id, "grok-build");
    assert_eq!(session.models[0].totals.input_tokens, 100);
    assert_eq!(session.primary_model.as_deref(), Some("grok-build"));

    let day = &report.days[0];
    assert_eq!(day.models.len(), 2);
    assert_eq!(day.models[0].model_id, "grok-build");
    assert_eq!(day.totals.input_tokens, 120, "bucket totals cover both models");

    assert_eq!(report.models.len(), 2);
    assert_eq!(report.models[0].totals.model_calls, 3);
}

#[test]
fn model_filter_narrows_every_view() {
    let now = now();
    let at = now - ChronoDuration::hours(2);
    let records = vec![
        record(
            "sess-mix",
            "/work/mixed",
            vec![turn_with_models(
                1,
                &rfc3339(at),
                vec![
                    ("grok-build", model_entry(100, 10, 2, Some(5_000_000_000))),
                    ("glm-5", model_entry(20, 2, 1, None)),
                ],
            )],
        ),
        record(
            "sess-other",
            "/work/other",
            vec![turn_with_models(
                1,
                &rfc3339(at),
                vec![("glm-5", model_entry(50, 5, 1, None))],
            )],
        ),
    ];

    let report = aggregate(&records, None, Some("grok"), now, None);

    assert_eq!(
        report.sessions.len(),
        1,
        "a session with no matching model drops out entirely"
    );
    let session = &report.sessions[0];
    assert_eq!(session.totals.input_tokens, 100, "only the matching model's tokens");
    assert_eq!(session.models.len(), 1);
    assert_eq!(session.primary_model.as_deref(), Some("grok-build"));
    assert_eq!(report.days[0].totals.input_tokens, 100);
    assert_eq!(report.weeks[0].totals.model_calls, 2);
    assert_eq!(report.models.len(), 1);

    // The filter is a case-insensitive substring.
    let wide = aggregate(&records, None, Some("GLM"), now, None);
    assert_eq!(wide.models.len(), 1);
    assert_eq!(wide.models[0].model_id, "glm-5");
    assert_eq!(wide.sessions.len(), 2);
}

#[test]
fn a_session_without_per_model_splits_falls_back_to_the_recorded_primary() {
    let now = now();
    let at = now - ChronoDuration::hours(1);
    let mut file = SessionUsageFile::new("sess-plain");
    file.turns = vec![turn(1, &rfc3339(at), summary(10, 1, 1, None))];
    file.session.primary_model_id = Some("recorded-model".into());
    let records = vec![SessionRecord {
        session_id: "sess-plain".into(),
        project: None,
        file,
    }];

    let report = aggregate(&records, None, None, now, None);
    assert!(report.sessions[0].models.is_empty());
    assert_eq!(
        report.sessions[0].primary_model.as_deref(),
        Some("recorded-model")
    );
}

#[test]
fn no_records_is_an_empty_report_not_an_error() {
    let report = aggregate(&[], None, None, now(), None);
    assert!(report.sessions.is_empty());
    assert!(report.days.is_empty());
    assert!(report.weeks.is_empty());
}

#[test]
fn session_totals_never_report_cost_when_no_turn_did() {
    let now = now();
    let at = now - ChronoDuration::hours(1);
    let records = vec![record(
        "sess-free",
        "/work/delta",
        vec![turn(1, &rfc3339(at), summary(1, 1, 1, None))],
    )];
    let report = aggregate(&records, None, None, now, None);
    assert_eq!(report.sessions[0].totals.cost_usd, None);
    assert!(report.sessions[0].totals.cost_partial);
}
