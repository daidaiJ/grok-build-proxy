//! `grok stats`: token/cost aggregates across every recorded session, bucketed
//! per session, per local day, and per ISO week. Reads the `usage.json` each
//! session persists; it creates no grok home and never writes.

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Local};
use serde::Serialize;
use xai_grok_config::decode_cwd_from_dirname;
use xai_grok_shell::session::usage_file::{SessionUsageFile, UsageSummary};
use xai_grok_tools::implementations::output_compression::{
    ToolOutputCompressionStats, is_enabled as compression_enabled, stats_for_display,
};

/// Bumped when a `--json` field changes meaning or is removed; additions are free.
const SCHEMA_VERSION: u32 = 1;

/// USD ticks (1e10 per USD), matching `UsageSummary::cost_usd_ticks`.
const TICKS_PER_USD: f64 = 10_000_000_000.0;

#[derive(Clone, Debug, clap::Args)]
#[command(
    after_help = "Buckets come from each turn's end timestamp, in your local timezone. \
Session rows aggregate the same turns, so `--days 7` shrinks a session to the \
turns it ran inside the window. A turn whose timestamp cannot be parsed only \
appears when no `--days` window is set. Every view also carries a per-model \
breakdown; `--model` narrows everything to models whose id contains the text."
)]
pub struct StatsArgs {
    /// Emit machine-readable JSON output.
    #[arg(long)]
    pub json: bool,
    /// Only count turns that ended within the last N days.
    #[arg(long)]
    pub days: Option<u32>,
    /// Only count usage whose model id contains this text (case-insensitive).
    #[arg(long)]
    pub model: Option<String>,
    /// Session rows kept, newest first. 0 keeps every session.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

pub fn run(args: StatsArgs) -> Result<()> {
    // resolve_grok_home resolves the home the way the registry does, unlike xai_grok_config::grok_home()
    let grok_home = xai_fast_worktree::resolve_grok_home()?;
    let sessions = collect_sessions(&grok_home)
        .with_context(|| format!("cannot read sessions under {}", grok_home.display()))?;
    let report = aggregate(
        &sessions,
        args.days,
        args.model.as_deref(),
        Local::now(),
        Some(grok_home.as_path()),
    );
    let mut out = std::io::stdout().lock();
    if args.json {
        writeln!(out, "{}", serde_json::to_string_pretty(&report)?)?;
    } else {
        display::print_report(&report, args.limit, &mut out);
    }
    Ok(())
}

/// One parsed `usage.json`, with the project decoded from its directory.
pub(crate) struct SessionRecord {
    pub session_id: String,
    pub project: Option<String>,
    pub file: SessionUsageFile,
}

/// One `usage.json` row: the session a turn belongs to and when it ended.
struct TurnRecord<'a> {
    session_id: &'a str,
    ended_at: Option<DateTime<FixedOffset>>,
    usage: &'a UsageSummary,
}

pub(crate) fn collect_sessions(grok_home: &Path) -> Result<Vec<SessionRecord>> {
    let mut records = Vec::new();
    let mut files = Vec::new();
    collect_usage_files(&grok_home.join("sessions"), 0, &mut files);
    for path in files {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        // A session dir exists before its usage.json is written; an all-whitespace
        // file is what an in-flight atomic rename leaves, not a report worth failing on.
        if data.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let file: SessionUsageFile = match serde_json::from_slice(&data) {
            Ok(file) => file,
            Err(err) => {
                tracing::debug!(path = %path.display(), %err, "skipping unreadable usage.json");
                continue;
            }
        };
        // `.cwd` marker / URL-encoded dirname, so the row names the project like `sessions` does.
        let project = path
            .parent()
            .and_then(|dir| dir.file_name())
            .and_then(|name| decode_cwd_from_dirname(Path::new(name)));
        records.push(SessionRecord {
            session_id: file.session_id.clone(),
            project,
            file,
        });
    }
    Ok(records)
}

/// `usage.json` lives at `<encoded-cwd>/<session-id>/usage.json`; subagent
/// child sessions write top-level dirs, so walk a couple of levels instead of
/// assuming exactly two.
fn collect_usage_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_usage_files(&path, depth + 1, out);
        } else if path.file_name().is_some_and(|name| name == "usage.json") {
            out.push(path);
        }
    }
}

#[derive(Debug, Default, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub turns: u64,
    pub model_calls: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    /// Sum of the costs reported; `None` when no turn reported one.
    pub cost_usd: Option<f64>,
    /// A turn reported no cost, so the sum under-counts.
    pub cost_partial: bool,
}

impl Totals {
    fn fold(&mut self, usage: &UsageSummary) {
        self.turns += 1;
        self.model_calls += usage.model_calls;
        self.input_tokens += usage.input_tokens;
        self.output_tokens += usage.output_tokens;
        self.cached_read_tokens += usage.cached_read_tokens;
        self.cache_creation_tokens += usage.cache_creation_tokens;
        self.reasoning_tokens += usage.reasoning_tokens;
        self.total_tokens += usage.total_tokens;
        match usage.cost_usd_ticks {
            Some(ticks) => {
                let usd = ticks as f64 / TICKS_PER_USD;
                self.cost_usd = Some(self.cost_usd.unwrap_or(0.0) + usd);
            }
            None => self.cost_partial = true,
        }
    }

    /// Column sums for the session table's TOTAL row. `turns` counts sessions here.
    fn sum_of<'a>(rows: impl Iterator<Item = &'a Self>) -> Self {
        let mut total = Self::default();
        for row in rows {
            total.add(row);
        }
        total
    }

    /// Field-wise add, cost partial-flags OR-ing. `add` on rows that were
    /// themselves sums keeps `cost_usd` `None` while every row is `None`.
    fn add(&mut self, other: &Self) {
        self.turns += other.turns;
        self.model_calls += other.model_calls;
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cached_read_tokens += other.cached_read_tokens;
        self.cache_creation_tokens += other.cache_creation_tokens;
        self.reasoning_tokens += other.reasoning_tokens;
        self.total_tokens += other.total_tokens;
        self.cost_usd = match (self.cost_usd, other.cost_usd) {
            (Some(a), Some(b)) => Some(a + b),
            (a, b) => a.or(b),
        };
        self.cost_partial |= other.cost_partial;
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub session_id: String,
    pub project: Option<String>,
    /// Local time of the last counted turn, `%Y-%m-%d %H:%M`.
    pub last_activity: Option<String>,
    #[serde(flatten)]
    pub totals: Totals,
    /// Busiest model first, so a session mixing models stays legible.
    pub models: Vec<ModelRow>,
    pub primary_model: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BucketRow {
    /// Local day (`2026-09-15`), ISO week (`2026-W38`), or a model id.
    pub key: String,
    pub sessions: u64,
    #[serde(flatten)]
    pub totals: Totals,
    /// Per-model split of this bucket; busiest first.
    pub models: Vec<ModelRow>,
}

/// One model's share of a session or bucket. `turns` counts the turns that
/// used this model, which can overlap across models within one turn.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    pub model_id: String,
    #[serde(flatten)]
    pub totals: Totals,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatsReport {
    pub schema_version: u32,
    pub generated_at: String,
    /// Newest activity first.
    pub sessions: Vec<SessionRow>,
    pub days: Vec<BucketRow>,
    pub weeks: Vec<BucketRow>,
    /// The whole window, split by model; busiest first.
    pub models: Vec<ModelRow>,
    /// LOCAL: experimental tool-output compression ledger. Omitted when empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output_compression: Option<CompressionStatsJson>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompressionStatsJson {
    pub enabled: bool,
    pub compressed_calls: u64,
    pub skipped_calls: u64,
    pub no_win_calls: u64,
    pub retrieve_calls: u64,
    pub original_tokens: u64,
    pub saved_tokens: u64,
    pub expanded_tokens: u64,
    pub retrieved_tokens: u64,
    pub net_tokens: i64,
    pub saved_ratio: f64,
    pub net_ratio: f64,
    pub io_write_ops: u64,
    pub io_read_ops: u64,
    pub io_write_ms: f64,
    pub io_read_ms: f64,
    pub io_write_avg_ms: f64,
    pub io_read_avg_ms: f64,
    pub io_total_ms: f64,
    pub io_write_bytes: u64,
    pub io_read_bytes: u64,
}

impl CompressionStatsJson {
    fn from_stats(stats: &ToolOutputCompressionStats, enabled: bool) -> Self {
        Self {
            enabled,
            compressed_calls: stats.compressed_calls,
            skipped_calls: stats.skipped_calls,
            no_win_calls: stats.no_win_calls,
            retrieve_calls: stats.retrieve_calls,
            original_tokens: stats.original_tokens,
            saved_tokens: stats.saved_tokens,
            expanded_tokens: stats.expanded_tokens,
            retrieved_tokens: stats.retrieved_tokens,
            net_tokens: stats.net_tokens(),
            saved_ratio: stats.saved_ratio(),
            net_ratio: stats.net_ratio(),
            io_write_ops: stats.io_write_ops,
            io_read_ops: stats.io_read_ops,
            io_write_ms: stats.io_write_ns as f64 / 1_000_000.0,
            io_read_ms: stats.io_read_ns as f64 / 1_000_000.0,
            io_write_avg_ms: stats.io_write_avg_ms(),
            io_read_avg_ms: stats.io_read_avg_ms(),
            io_total_ms: stats.io_total_ms(),
            io_write_bytes: stats.io_write_bytes,
            io_read_bytes: stats.io_read_bytes,
        }
    }

    pub(crate) fn as_stats(&self) -> ToolOutputCompressionStats {
        ToolOutputCompressionStats {
            compressed_calls: self.compressed_calls,
            skipped_calls: self.skipped_calls,
            no_win_calls: self.no_win_calls,
            retrieve_calls: self.retrieve_calls,
            original_tokens: self.original_tokens,
            saved_tokens: self.saved_tokens,
            expanded_tokens: self.expanded_tokens,
            retrieved_tokens: self.retrieved_tokens,
            io_write_ops: self.io_write_ops,
            io_read_ops: self.io_read_ops,
            io_write_ns: (self.io_write_ms * 1_000_000.0) as u64,
            io_read_ns: (self.io_read_ms * 1_000_000.0) as u64,
            io_write_bytes: self.io_write_bytes,
            io_read_bytes: self.io_read_bytes,
            updated_at_unix: None,
        }
    }
}

/// Folds every recorded turn into the three views. Pure so tests can pin the
/// bucketing without a grok home on disk.
pub(crate) fn aggregate(
    records: &[SessionRecord],
    days: Option<u32>,
    model: Option<&str>,
    now: DateTime<Local>,
    grok_home: Option<&Path>,
) -> StatsReport {
    let cutoff = days
        .map(|d| now - ChronoDuration::days(i64::from(d)))
        .map(|cutoff| cutoff.fixed_offset());
    let session_rows = records.iter().map(|record| {
        let turns: Vec<TurnRecord> = record
            .file
            .turns
            .iter()
            .map(|turn| TurnRecord {
                session_id: &record.session_id,
                ended_at: parse_ended_at(&turn.ended_at),
                usage: &turn.usage,
            })
            .filter(|turn| keep_turn(turn, cutoff))
            .collect();
        (record, turns)
    });

    let mut sessions: Vec<SessionRow> = Vec::new();
    let mut day_buckets: BTreeMap<String, BucketAcc> = BTreeMap::new();
    let mut week_buckets: BTreeMap<String, BucketAcc> = BTreeMap::new();
    let mut every_model: BTreeMap<String, Totals> = BTreeMap::new();

    for (record, turns) in session_rows {
        let mut totals = Totals::default();
        let mut models: BTreeMap<String, Totals> = BTreeMap::new();
        let mut last_activity: Option<DateTime<Local>> = None;
        for turn in &turns {
            let counted = fold_turn(&mut totals, &mut models, turn.usage, model);
            if !counted {
                continue;
            }
            if let Some(ended_at) = turn.ended_at {
                let local = ended_at.with_timezone(&Local);
                last_activity = last_activity.max(Some(local));
                let day = day_buckets
                    .entry(local.format("%Y-%m-%d").to_string())
                    .or_default();
                fold_turn(&mut day.totals, &mut day.models, turn.usage, model);
                day.session_ids.insert(turn.session_id.to_owned());
                let week = week_buckets
                    .entry(local.format("%G-W%V").to_string())
                    .or_default();
                fold_turn(&mut week.totals, &mut week.models, turn.usage, model);
                week.session_ids.insert(turn.session_id.to_owned());
            }
        }
        if totals.turns == 0 {
            continue;
        }
        for (model_id, model_totals) in &models {
            every_model
                .entry(model_id.clone())
                .or_default()
                .add(model_totals);
        }
        let model_rows = model_rows(models);
        // The session's own busiest model. A file without per-model splits
        // falls back to the primary recorded at write time.
        let primary_model = model_rows
            .first()
            .map(|m| m.model_id.clone())
            .or_else(|| record.file.session.primary_model_id.clone());
        sessions.push(SessionRow {
            session_id: record.session_id.clone(),
            project: record.project.clone(),
            last_activity: last_activity.map(|at| at.format("%Y-%m-%d %H:%M").to_string()),
            models: model_rows,
            primary_model,
            totals,
        });
    }
    sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

    let compression = stats_for_display(grok_home);
    let tool_output_compression = if compression.compressed_calls == 0
        && compression.skipped_calls == 0
        && compression.retrieve_calls == 0
        && compression.original_tokens == 0
    {
        None
    } else {
        Some(CompressionStatsJson::from_stats(
            &compression,
            compression_enabled(),
        ))
    };
    StatsReport {
        schema_version: SCHEMA_VERSION,
        generated_at: now.to_rfc3339(),
        sessions,
        days: day_buckets
            .into_iter()
            .map(|(key, acc)| acc.finish(key))
            .collect(),
        weeks: week_buckets
            .into_iter()
            .map(|(key, acc)| acc.finish(key))
            .collect(),
        models: model_rows(every_model),
        tool_output_compression,
    }
}

/// Fold one turn into `totals` and the per-model map, honoring the `--model`
/// filter. Without a filter the turn counts whole; with one, only its matching
/// per-model entries count, and a turn with nothing matching counts nothing.
/// Returns whether anything was counted.
fn fold_turn(
    totals: &mut Totals,
    models: &mut BTreeMap<String, Totals>,
    usage: &UsageSummary,
    model: Option<&str>,
) -> bool {
    match model {
        None => {
            totals.fold(usage);
            for (model_id, entry) in &usage.model_usage {
                models.entry(model_id.clone()).or_default().fold(entry);
            }
            true
        }
        Some(filter) => {
            let mut counted = false;
            for (model_id, entry) in &usage.model_usage {
                if !model_id_matches(model_id, filter) {
                    continue;
                }
                totals.fold(entry);
                models.entry(model_id.clone()).or_default().fold(entry);
                counted = true;
            }
            counted
        }
    }
}

/// Case-insensitive substring, so `grok-4` reaches `grok-4-fast` too.
fn model_id_matches(model_id: &str, filter: &str) -> bool {
    model_id.to_lowercase().contains(&filter.to_lowercase())
}

/// Busiest first: most calls, then most tokens.
fn model_rows(models: BTreeMap<String, Totals>) -> Vec<ModelRow> {
    let mut rows: Vec<ModelRow> = models
        .into_iter()
        .map(|(model_id, totals)| ModelRow { model_id, totals })
        .collect();
    rows.sort_by(|a, b| {
        (b.totals.model_calls, b.totals.total_tokens)
            .cmp(&(a.totals.model_calls, a.totals.total_tokens))
    });
    rows
}

/// `--days` drops turns older than the window; an unparsable timestamp has no
/// bucket to prove it belongs, so it only counts when no window is set.
fn keep_turn(turn: &TurnRecord, cutoff: Option<DateTime<FixedOffset>>) -> bool {
    match (cutoff, turn.ended_at) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(cutoff), Some(ended_at)) => ended_at >= cutoff,
    }
}

fn parse_ended_at(text: &str) -> Option<DateTime<FixedOffset>> {
    if text.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(text).ok()
}

/// Running day/week accumulation; `BTreeMap` keeps the rows date-sorted and
/// the per-model split stable.
#[derive(Default)]
struct BucketAcc {
    session_ids: HashSet<String>,
    totals: Totals,
    models: BTreeMap<String, Totals>,
}

impl BucketAcc {
    fn finish(self, key: String) -> BucketRow {
        BucketRow {
            key,
            sessions: self.session_ids.len() as u64,
            totals: self.totals,
            models: model_rows(self.models),
        }
    }
}

mod display;

#[cfg(test)]
mod tests;
