use std::collections::BTreeMap;

use super::config::LspServerConfig;
use super::manager::DiagnosticsSummary;

/// LSP configuration passed from shell. Same pattern as `WebSearchConfig`.
#[derive(Debug, Clone, Default)]
pub enum LspConfig {
    #[default]
    Disabled,
    Enabled {
        cwd: std::path::PathBuf,
        servers: BTreeMap<String, LspServerConfig>,
    },
}

impl LspConfig {
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled { servers, .. } if !servers.is_empty())
    }
}

pub struct LspToolResult {
    pub text: String,
    pub is_error: bool,
}

/// Kind of a disk mutation we ourselves made. Mapped onto LSP `FileChangeType`
/// only at the watched-files boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskChangeKind {
    Created,
    Changed,
    Deleted,
}

impl DiskChangeKind {
    pub(crate) fn to_lsp(self) -> async_lsp::lsp_types::FileChangeType {
        use async_lsp::lsp_types::FileChangeType;
        match self {
            Self::Created => FileChangeType::CREATED,
            Self::Changed => FileChangeType::CHANGED,
            Self::Deleted => FileChangeType::DELETED,
        }
    }
}

/// Trait object interface for LSP operations.
///
/// Implemented by `LspBackendAdapter` which wraps `LspManager`.
#[async_trait::async_trait]
pub trait LspBackend: Send + Sync + 'static {
    fn ensure_started_background(&self);

    async fn ensure_ready(&self) -> Result<(), String>;

    fn is_ready(&self) -> bool;

    async fn dispatch(&self, input: &LspToolInput) -> LspToolResult;

    async fn drain_diagnostics(&self, timeout: std::time::Duration) -> Option<DiagnosticsSummary>;

    async fn notify_file_changed(&self, path: &std::path::Path, content: &str);

    /// Structured disk mutation, including creates and deletes.
    async fn notify_file_event(
        &self,
        path: &std::path::Path,
        content: Option<&str>,
        kind: DiskChangeKind,
    );

    /// Read diagnostics for specific file paths. For each path, opens the file with the LSP if not
    /// already open, waits briefly for diagnostics to settle, then returns all ERROR/WARNING
    /// diagnostics grouped by file.
    async fn read_diagnostics(&self, paths: &[std::path::PathBuf]) -> Vec<FileDiagnosticEntry>;

    /// Request `textDocument/rename` at the position and return the resulting
    /// `(path, new_content)` pairs. The caller owns reading current disk text
    /// (done here) and writing it. `Err` on protocol/apply failure; the
    /// default rejects, so test doubles without a manager need nothing.
    async fn rename(
        &self,
        path: &std::path::Path,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<Vec<(std::path::PathBuf, String)>, String> {
        let _ = (path, line, character, new_name);
        Err("rename is not supported by this backend".into())
    }

    /// Lifecycle snapshot for status surfaces. The default returns `None` —
    /// test doubles without a real manager have nothing to report.
    async fn status_snapshot(&self) -> Option<LspBackendStatus> {
        None
    }
}

/// A single diagnostic entry returned by `LspBackend::read_diagnostics`.
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub severity: DiagnosticSeverityLevel,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub source: Option<String>,
    /// Diagnostic code from the language server (e.g. `"2322"` for TS type errors).
    pub code: Option<String>,
    /// `true` when this diagnostic was computed against an older version of the file.
    pub is_stale: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticSeverityLevel {
    Error,
    Warning,
}

/// Diagnostics for a single file.
#[derive(Debug, Clone)]
pub struct FileDiagnosticEntry {
    pub path: String,
    pub diagnostics: Vec<DiagnosticEntry>,
}

/// Startup phase of the LSP backend, for status surfaces (`/lsp status`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspStartupState {
    NotStarted,
    Starting,
    Ready,
    Failed(String),
}

/// Point-in-time snapshot of a backend for `/lsp status`: the bootstrap phase
/// and which configured servers currently have a live client.
#[derive(Debug, Clone, Default)]
pub struct LspBackendStatus {
    /// `None` when the adapter has no bootstrap state to report.
    pub startup: Option<LspStartupState>,
    pub running_servers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LspOperation {
    GoToDefinition,
    FindReferences,
    Hover,
    GoToImplementation,
    DocumentSymbol,
    WorkspaceSymbol,
    Diagnostics,
    WorkspaceDiagnostics,
}

impl std::fmt::Display for LspOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GoToDefinition => write!(f, "goToDefinition"),
            Self::FindReferences => write!(f, "findReferences"),
            Self::Hover => write!(f, "hover"),
            Self::GoToImplementation => write!(f, "goToImplementation"),
            Self::DocumentSymbol => write!(f, "documentSymbol"),
            Self::WorkspaceSymbol => write!(f, "workspaceSymbol"),
            Self::Diagnostics => write!(f, "diagnostics"),
            Self::WorkspaceDiagnostics => write!(f, "workspaceDiagnostics"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct LspToolInput {
    #[schemars(description = "The LSP operation to perform.")]
    pub operation: LspOperation,
    #[schemars(description = "Absolute path to the file.")]
    #[serde(default)]
    pub file_path: Option<String>,
    #[schemars(description = "0-indexed line number.")]
    #[serde(default)]
    pub line: Option<u32>,
    #[schemars(description = "0-indexed column number.")]
    #[serde(default)]
    pub character: Option<u32>,
    #[schemars(description = "Symbol name or partial name (workspaceSymbol only).")]
    #[serde(default)]
    pub query: Option<String>,
    #[schemars(
        description = "Max diagnostics returned (workspaceDiagnostics only, default 100)."
    )]
    #[serde(default)]
    pub limit: Option<u32>,
}
