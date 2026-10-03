use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

/// One canonical context edit targeting a prior `spawn_agent` call (by its journal seq).
/// History (the journal) is never rewritten; edits only shape the *visible face* of later
/// spawn prompts: the targeted child's raw output text (the needle) is replaced by the
/// digest placeholder (or removed by `hide`). Engine-resolved and carried in the spawn
/// payload, so the edit sequence participates in the request-hash chain — an edited
/// script diverges on replay like any other host-call change.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextEdit {
    /// Journal seq of the targeted `spawn_agent` call.
    pub target: u64,
    /// `"replace"` keeps a digest placeholder; `"hide"` replaces the output with an
    /// omission marker. Anything else fails at substitution time.
    pub action: String,
    /// Free-form digest text supplied by the script (e.g. a summarizer agent's output).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// Raw output text of the targeted spawn, resolved from the journal. `None` when the
    /// child's output was not a plain string (structured outputs are not substitutable
    /// in v1) — such edits are dropped at resolution time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needle: Option<String>,
}

impl ContextEdit {
    /// What the model sees in place of the targeted child's raw output.
    pub fn placeholder(&self) -> String {
        match self.action.as_str() {
            "replace" => format!(
                "[agent #{} result: {}]",
                self.target,
                self.digest.as_deref().unwrap_or("")
            ),
            _ => format!("[agent #{} result omitted]", self.target),
        }
    }
}

/// Apply the resolved context edits to a child prompt's visible face. Journal history
/// is untouched — this only shapes what a spawn is about to see. A needle that never
/// occurs (the script transformed or truncated the output before embedding it) is a
/// no-op: the edit never pretends to have been applied.
pub fn apply_visible_edits(prompt: String, edits: &[ContextEdit]) -> String {
    let mut prompt = prompt;
    for edit in edits {
        if let Some(needle) = edit.needle.as_deref().filter(|needle| !needle.is_empty()) {
            prompt = prompt.replace(needle, &edit.placeholder());
        }
    }
    prompt
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentOpts {
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub agent_type: Option<String>,
    #[serde(default)]
    pub capability_mode: Option<String>,
    #[serde(default)]
    pub isolation_worktree: bool,
    #[serde(default)]
    pub fork_context: bool,
    #[serde(default)]
    pub resume_from: Option<String>,
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub phase: Option<String>,
    /// Canonical context edits applied to this spawn's prompt visible face. Never
    /// authored by the script directly: the engine resolves them from journaled
    /// `context_edit` calls right before the payload is hashed, overriding anything
    /// a hand-built opts map may have carried. Absent from the payload when empty so
    /// journals written before this feature replay with unchanged request hashes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_edits: Vec<ContextEdit>,
    /// Explicit tool-face allowlist for the child (canonical tool ids). `None` keeps
    /// the full declared face; a list trims the declaration face only — it never
    /// widens permissions (capability modes still intersect). Absent from the payload
    /// when `None`, so journals written before this feature replay unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub agent_id: String,
    pub success: bool,
    pub output: serde_json::Value,
    pub cancelled: bool,
    pub tokens_used: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BudgetState {
    pub total: Option<u64>,
    pub spent: u64,
    pub reserved: u64,
    pub remaining: Option<u64>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum HostError {
    #[error("workflow agent-call quota exceeded: requested {requested}, maximum {maximum}")]
    AgentCallQuotaExceeded { requested: u64, maximum: u64 },
    #[error("workflow token budget exceeded")]
    BudgetExceeded,
    #[error("workflow cancelled")]
    Cancelled,
    #[error("unsupported in this context: {0}")]
    Unsupported(String),
    #[error("host failure: {0}")]
    Failed(String),
}

#[derive(Debug)]
pub enum WorkflowHostRequest {
    ReserveAgentCalls {
        count: u64,
        reply: oneshot::Sender<Result<(), HostError>>,
    },
    ReleaseAgentCalls {
        count: u64,
        reply: oneshot::Sender<Result<(), HostError>>,
    },
    SpawnAgent {
        opts: AgentOpts,
        reply: oneshot::Sender<Result<AgentResult, HostError>>,
    },
    Phase {
        title: String,
        replayed: bool,
    },
    Log {
        message: String,
        replayed: bool,
    },
    Telemetry {
        name: String,
        fields: serde_json::Value,
        replayed: bool,
    },
    BudgetQuery {
        reply: oneshot::Sender<Result<BudgetState, HostError>>,
    },
    RenderTemplate {
        name: String,
        vars: serde_json::Value,
        reply: oneshot::Sender<Result<String, HostError>>,
    },
    WriteScratchFile {
        name: String,
        content: String,
        reply: oneshot::Sender<Result<String, HostError>>,
    },
    ReadScratchFile {
        name: String,
        reply: oneshot::Sender<Result<String, HostError>>,
    },
    GitDiffSince {
        commit: String,
        reply: oneshot::Sender<Result<String, HostError>>,
    },
}

impl WorkflowHostRequest {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ReserveAgentCalls { .. } => "reserve_agent_calls",
            Self::ReleaseAgentCalls { .. } => "release_agent_calls",
            Self::SpawnAgent { .. } => "spawn_agent",
            Self::Phase { .. } => "phase",
            Self::Log { .. } => "log",
            Self::Telemetry { .. } => "telemetry",
            Self::BudgetQuery { .. } => "budget",
            Self::RenderTemplate { .. } => "render_template",
            Self::WriteScratchFile { .. } => "write_scratch_file",
            Self::ReadScratchFile { .. } => "read_scratch_file",
            Self::GitDiffSince { .. } => "git_diff_since",
        }
    }
}

#[cfg(test)]
mod visible_edits_tests {
    use super::*;

    #[test]
    fn apply_visible_edits_replaces_hides_and_skips_missing_needles() {
        let prompt = "before RAW MIDDLE after NOISE tail".to_string();
        let edits = vec![
            ContextEdit {
                target: 0,
                action: "replace".into(),
                digest: Some("3 findings".into()),
                needle: Some("RAW MIDDLE".into()),
            },
            ContextEdit {
                target: 1,
                action: "hide".into(),
                digest: None,
                needle: Some("NOISE".into()),
            },
            ContextEdit {
                target: 2,
                action: "replace".into(),
                digest: Some("never present".into()),
                needle: Some("ABSENT".into()),
            },
            ContextEdit {
                target: 3,
                action: "replace".into(),
                digest: Some("empty needle".into()),
                needle: Some(String::new()),
            },
        ];
        let prompt = apply_visible_edits(prompt, &edits);
        assert_eq!(prompt, "before [agent #0 result: 3 findings] after [agent #1 result omitted] tail");
    }
}
