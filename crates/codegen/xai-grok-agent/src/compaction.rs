#[derive(Debug, Clone)]
pub struct CompactionPolicy {
    /// Auto-compaction triggers when this percentage of the context window is used.
    pub auto_compact_threshold_percent: u32,

    /// Model to use for generating the compaction summary.
    /// None means use the session's current model.
    pub compact_model: Option<String>,

    /// Whether to run a memory flush turn before each compaction.
    /// When enabled, the session actor asks the model to summarize important information from the conversation before it's compacted.
    /// Requires the memory system to be enabled.
    pub memory_flush_enabled: bool,

    /// Per-compaction wall-clock budget (seconds); a generation exceeding it is cut and retried.
    /// This is the backstop for runaway reasoning that token limits miss.
    pub wall_clock_budget_secs: u64,

    /// Two-pass compaction: when usage approaches the threshold, the earlier history is speculatively summarized in the background (pass 1).
    /// At compaction, that note and the recent tail are summarized together (pass 2).
    /// Resolved from the `two_pass_compaction` config flag at session build; `false` keeps the legacy single-pass path.
    pub two_pass_enabled: bool,

    /// LOCAL(token-economy P1): carry the original goal and verbatim user
    /// messages across compactions via a deterministic relay anchor.
    /// Defaults to on as long-horizon insurance; the 5-round bench showed no
    /// measurable short-run win, so `shell.relay_compact` now reports how often
    /// the summary would have dropped an anchor message.
    pub relay_anchor_enabled: bool,
}

impl Default for CompactionPolicy {
    fn default() -> Self {
        Self {
            auto_compact_threshold_percent: 85,
            compact_model: None,
            memory_flush_enabled: false,
            wall_clock_budget_secs: 300,
            two_pass_enabled: false,
            relay_anchor_enabled: true,
        }
    }
}
