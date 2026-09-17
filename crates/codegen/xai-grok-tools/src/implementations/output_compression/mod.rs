//! LOCAL: experimental in-process tool-output compression (headroom-style).
//!
//! Default **off**. Only the tool-result prompt text is rewritten; tool schemas
//! are never touched. Retrieval is pull-style (`expand_output` + `<<ccr:HASH>>`).

mod compress;
mod detect;
mod expand;
mod runtime;
mod store;

pub use expand::{ExpandOutputInput, ExpandOutputTool};
pub use runtime::{
    CcrConfig, CompressionStrategiesSpec, SessionCompressionPolicy, ToolOutputCompressionConfig,
    ToolOutputCompressionStats, ccr_is_enabled, current_runtime_for_session, format_stats_report,
    is_enabled, load_persisted_stats, set_runtime, snapshot_stats, stats_for_display,
};

pub(crate) use compress::maybe_compress_prompt;
