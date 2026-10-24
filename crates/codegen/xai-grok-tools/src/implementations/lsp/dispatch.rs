//! Bridges xai-grok-tools LspBackend trait to LspManager.
//!
//! `dispatch_on_sockets` is a thin router; each LSP operation has its own helper.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex as TokioMutex;

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    self, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, HoverParams, Location, ReferenceContext, ReferenceParams,
    SymbolInformation, TextDocumentIdentifier, TextDocumentPositionParams, WorkspaceSymbolParams,
};

use super::{LspToolInput, LspToolResult, LspBackendStatus, LspStartupState};

use super::config::REQUEST_TIMEOUT;
use super::diagnostics::DiagnosticsStore;
use super::documents::Documents;
use super::format::{
    format_diagnostic_entries, format_diagnostic_line, flatten_document_symbols,
    format_locations_labeled, format_symbols, markup_string_to_text,
};
use super::manager::LspManager;
use super::pull::{PullDiagnostics, PullSupport};
use super::{DiagnosticEntry, DiagnosticSeverityLevel, LspError, file_uri, text_document_position};

// ── On-demand diagnostics (Diagnostics / WorkspaceDiagnostics ops) ──────

/// How long a single-file diagnostics query waits for the first verdict on a
/// freshly opened file, matching the wait the reminder path pays after an open.
const DIAGNOSTICS_FRESH_WAIT: std::time::Duration = std::time::Duration::from_secs(1);
/// Settle window when the store already holds an older verdict: the short gap
/// that lets a verdict on the latest edit arrive before we answer from the
/// previous one.
const DIAGNOSTICS_SETTLE_WAIT: std::time::Duration = std::time::Duration::from_millis(300);
/// One pull round trip, for servers that only speak when asked. Their
/// `textDocument/diagnostic` budget is [`super::pull::REQUEST_TIMEOUT`], but a
/// tool call should not ride a full ten seconds.
const DIAGNOSTICS_PULL_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// Default cap for `workspaceDiagnostics` output, aligned with qwen-code.
pub const WORKSPACE_DIAGNOSTICS_LIMIT: u32 = 100;

/// The shared handles of the client that owns a file, cloned so the settle
/// wait runs without holding the manager lock.
pub(super) struct DiagnosticsHandles {
    pub server_name: String,
    pub store: DiagnosticsStore,
    pub documents: Documents,
    pub pull: PullDiagnostics,
}

/// Resolve the owning client for `path`, auto-opening the file so the server
/// starts analyzing it.
pub(super) async fn resolve_diagnostics_handles(
    mgr: &mut LspManager,
    path: &Path,
) -> Result<DiagnosticsHandles, String> {
    let Some((server_name, _)) = super::config::resolve_server(&mgr.servers, path) else {
        return Err(format!("No LSP server configured for {}", path.display()));
    };
    mgr.socket_for_file(path).await;
    let Some(client) = mgr.clients.get(&server_name) else {
        return Err(format!("No LSP server configured for {}", path.display()));
    };
    Ok(DiagnosticsHandles {
        server_name,
        store: client.diagnostics.clone(),
        documents: client.documents.clone(),
        pull: client.pull.clone(),
    })
}

/// Wait for the server's verdict on the current version of `uri`, up to
/// `budget`. Returns whether one arrived; an expired budget means the caller
/// answers with what it has plus a "not settled" note.
async fn settle_for_verdict(
    store: &DiagnosticsStore,
    uri: &str,
    version: Option<i32>,
    budget: std::time::Duration,
) -> bool {
    let Some(version) = version else {
        return false;
    };
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        if store.answered_for(uri, version) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// Filter a store's items for a uri down to reportable entries, marking each
/// stale when the verdict predates the version we asked about.
fn reportable_entries(
    store: &DiagnosticsStore,
    uri: &str,
    current_version: Option<i32>,
) -> Vec<DiagnosticEntry> {
    let covers = store.covers(uri);
    store
        .items(uri)
        .into_iter()
        .filter_map(|d| {
            let severity = match d.severity {
                Some(async_lsp::lsp_types::DiagnosticSeverity::ERROR) => {
                    DiagnosticSeverityLevel::Error
                }
                Some(async_lsp::lsp_types::DiagnosticSeverity::WARNING) => {
                    DiagnosticSeverityLevel::Warning
                }
                _ => return None,
            };
            Some(DiagnosticEntry {
                severity,
                line: d.range.start.line + 1,
                column: d.range.start.character + 1,
                message: d.message.clone(),
                source: d.source.clone(),
                code: d.code.as_ref().map(|c| match c {
                    async_lsp::lsp_types::NumberOrString::String(s) => s.clone(),
                    async_lsp::lsp_types::NumberOrString::Number(n) => n.to_string(),
                }),
                is_stale: covers
                    .zip(current_version)
                    .is_some_and(|(covers, version)| covers < version),
            })
        })
        .collect()
}

/// The `diagnostics` op: one file, settle first, pull-once for servers that
/// never push.
pub(super) async fn read_file_diagnostics(
    handles: &DiagnosticsHandles,
    path: &Path,
) -> LspToolResult {
    let Ok(uri) = file_uri(path) else {
        return err_result(format!("Invalid file path: {}", path.display()));
    };
    let uri_str = uri.to_string();
    let version = handles.documents.version(&uri_str);

    // A server that never pushes only speaks when asked: trigger one pull so
    // the query is not answered from an empty store. `will_answer` re-checks
    // the same gate, so a server that started publishing in between is safe.
    let pulled = !handles.store.server_publishes()
        && handles.pull.support() == PullSupport::Asking
        && handles.pull.will_answer(uri.clone());

    let budget = if pulled {
        DIAGNOSTICS_PULL_WAIT
    } else if version.is_some() && handles.store.covers(&uri_str).is_none() {
        DIAGNOSTICS_FRESH_WAIT
    } else {
        DIAGNOSTICS_SETTLE_WAIT
    };
    let settled = settle_for_verdict(&handles.store, &uri_str, version, budget).await;

    let entries = reportable_entries(&handles.store, &uri_str, version);
    let covers = handles.store.covers(&uri_str);
    let mut text = match (entries.is_empty(), covers.is_some()) {
        (true, true) => format!("clean (server={})", handles.server_name),
        (true, false) => format!("no diagnostics reported yet (server={})", handles.server_name),
        (false, _) => format!(
            "Diagnostics for {} ({}):",
            path.display(),
            handles.server_name
        ),
    };
    if !entries.is_empty() {
        text.push('\n');
        text.push_str(&format_diagnostic_entries(&entries));
    }
    if !settled {
        text.push_str(
            "\nNote: the server has not reported for the latest version of this file yet; \
             results may be out of date — re-query in a moment to confirm.",
        );
    }
    ok_result(text)
}

/// One server's block of the `workspaceDiagnostics` output. Returns the block
/// text and the number of entry lines actually shown (budget-bounded).
fn workspace_server_block(
    server_name: &str,
    client: &super::client::LspClient,
    remaining: usize,
    total: &mut usize,
) -> (String, usize) {
    let mut uris = client.documents.uris();
    uris.sort();
    let mut lines: Vec<String> = Vec::new();
    let mut shown_here = 0usize;
    let mut any_answered = false;
    for uri in uris {
        if client.diagnostics.covers(&uri).is_some() {
            any_answered = true;
        }
        let entries = reportable_entries(&client.diagnostics, &uri, client.documents.version(&uri));
        if entries.is_empty() {
            continue;
        }
        *total += entries.len();
        let remaining_budget = remaining.saturating_sub(shown_here);
        if remaining_budget == 0 {
            // Budget spent: the file still counts toward `total`, but a bare
            // path header with nothing under it would be noise.
            continue;
        }
        let path = async_lsp::lsp_types::Url::parse(&uri)
            .ok()
            .and_then(|u| u.to_file_path().ok())
            .map(|p| p.display().to_string())
            .unwrap_or(uri.clone());
        lines.push(format!("  {path}"));
        for entry in entries {
            if shown_here >= remaining {
                break;
            }
            lines.push(single_entry_line(&entry));
            shown_here += 1;
        }
    }
    let mut block = format!("== server {server_name} ==");
    if lines.is_empty() {
        // Every file's entries are pushed with their path header, so an empty
        // line list means nothing reportable was found at all.
        if any_answered {
            block.push_str(&format!("\n  clean (server={server_name})"));
        } else {
            let pull_hint = if !client.diagnostics.server_publishes()
                && client.pull.support() == PullSupport::Asking
            {
                " — pull-only server; query a file with the diagnostics operation"
            } else {
                ""
            };
            block.push_str(&format!(
                "\n  no diagnostics reported yet (server={server_name}){pull_hint}"
            ));
        }
    } else {
        for line in lines {
            block.push('\n');
            block.push_str(&line);
        }
    }
    (block, shown_here)
}

fn single_entry_line(entry: &DiagnosticEntry) -> String {
    format_diagnostic_line(entry, "    ")
}

/// The `workspaceDiagnostics` op: aggregate every tracked document across all
/// servers. Tracked only — no disk sweep — and the empty result says so.
pub(super) fn read_workspace_diagnostics(mgr: &LspManager, limit: u32) -> LspToolResult {
    if mgr.clients.is_empty() {
        return err_result("No LSP servers are running.".into());
    }
    let limit = limit.max(1) as usize;
    let mut total = 0usize;
    let mut shown = 0usize;
    let mut blocks: Vec<String> = Vec::new();
    let mut server_names: Vec<&String> = mgr.clients.keys().collect();
    server_names.sort();
    for server_name in server_names {
        let client = &mgr.clients[server_name];
        let (block, shown_here) =
            workspace_server_block(server_name, client, limit.saturating_sub(shown), &mut total);
        shown += shown_here;
        blocks.push(block);
    }
    let mut text = blocks.join("\n");
    if total > shown {
        text.push_str(&format!(
            "\n\nShowing {shown} of {total} diagnostics (limit {limit}); \
             pass a larger `limit` parameter to see more."
        ));
    }
    text.push_str(
        "\nNote: covers only documents tracked by the servers (opened or edited this session); \
         an empty result is not a whole-workspace verdict.",
    );
    ok_result(text)
}

// ── Public adapter ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum StartupState {
    Starting,
    Ready,
    Failed(String),
}
// A server absent from the map has never been asked to start — the lazy
// default. Only ops that touch its files (or an explicit eager request)
// move it to `Starting`.

struct StartupCoordinator {
    states: TokioMutex<HashMap<String, StartupState>>,
    notify: tokio::sync::Notify,
    pre_ready_file_changes: TokioMutex<HashMap<PathBuf, (Option<String>, super::DiskChangeKind)>>,
    /// Servers whose last spawn attempt failed, with when it failed. Within
    /// the TTL a failing server is not asked again — otherwise every file of
    /// that language pays a doomed spawn attempt (crush's recentlyUnavailable).
    recently_unavailable: TokioMutex<HashMap<String, std::time::Instant>>,
}

/// How long a failed spawn is left alone before the next file access retries it.
const RECENTLY_UNAVAILABLE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

pub struct LspBackendAdapter {
    lsp_manager: Arc<tokio::sync::Mutex<LspManager>>,
    startup: Arc<StartupCoordinator>,
}

impl LspBackendAdapter {
    pub fn new(lsp_manager: Arc<tokio::sync::Mutex<LspManager>>) -> Self {
        Self {
            lsp_manager,
            startup: Arc::new(StartupCoordinator {
                states: TokioMutex::new(HashMap::new()),
                notify: tokio::sync::Notify::new(),
                pre_ready_file_changes: TokioMutex::new(HashMap::new()),
                recently_unavailable: TokioMutex::new(HashMap::new()),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) async fn is_recently_unavailable(&self, server: &str) -> bool {
        self.startup
            .recently_unavailable
            .lock()
            .await
            .contains_key(server)
    }

    fn spawn_server_start(
        lsp_manager: Arc<tokio::sync::Mutex<LspManager>>,
        startup: Arc<StartupCoordinator>,
        server: String,
    ) {
        tokio::spawn(async move {
            let result = start_server(lsp_manager, startup.clone(), &server).await;
            {
                let mut unavailable = startup.recently_unavailable.lock().await;
                let mut states = startup.states.lock().await;
                match &result {
                    Ok(()) => {
                        states.insert(server.clone(), StartupState::Ready);
                        unavailable.remove(&server);
                    }
                    Err(error) => {
                        states.insert(server.clone(), StartupState::Failed(error.clone()));
                        unavailable.insert(server.clone(), std::time::Instant::now());
                    }
                }
            }
            startup.notify.notify_waiters();
        });
    }

    /// Trigger one server's start if it is not on its way already. A server
    /// inside its recently-unavailable TTL is left alone.
    async fn ensure_started_one(&self, server: &str) {
        Self::ensure_started_one_with(self.lsp_manager.clone(), self.startup.clone(), server)
            .await;
    }

    async fn ensure_started_one_with(
        lsp_manager: Arc<tokio::sync::Mutex<LspManager>>,
        startup: Arc<StartupCoordinator>,
        server: &str,
    ) {
        {
            let unavailable = startup.recently_unavailable.lock().await;
            if let Some(when) = unavailable.get(server)
                && when.elapsed() < RECENTLY_UNAVAILABLE_TTL
            {
                return;
            }
        }
        {
            let states = startup.states.lock().await;
            if matches!(
                states.get(server),
                Some(StartupState::Starting | StartupState::Ready)
            ) {
                return;
            }
        }
        {
            let mut states = startup.states.lock().await;
            states.insert(server.to_string(), StartupState::Starting);
        }
        Self::spawn_server_start(lsp_manager, startup, server.to_string());
    }

    /// Trigger every configured server (the eager contract, on demand).
    async fn ensure_started_all_with(
        lsp_manager: Arc<tokio::sync::Mutex<LspManager>>,
        startup: Arc<StartupCoordinator>,
    ) {
        let names: Vec<String> = {
            let mgr = lsp_manager.lock().await;
            mgr.servers.keys().cloned().collect()
        };
        for name in names {
            Self::ensure_started_one_with(lsp_manager.clone(), startup.clone(), &name).await;
        }
    }

    /// Wait until `server` reaches a terminal state. `Err` on Failed.
    async fn wait_ready_one(&self, server: &str) -> Result<(), String> {
        loop {
            let notified = {
                let states = self.startup.states.lock().await;
                match states.get(server) {
                    Some(StartupState::Ready) => return Ok(()),
                    Some(StartupState::Failed(error)) => return Err(error.clone()),
                    // Absent + no task? `ensure_started_one` was already
                    // called by the caller; treat as still starting.
                    _ => self.startup.notify.notified(),
                }
            };
            notified.await;
        }
    }

    async fn ensure_ready_for_server(&self, server: &str) -> Result<(), String> {
        self.ensure_started_one(server).await;
        self.wait_ready_one(server).await
    }

    /// Ensure the server owning `path` is started and ready. A path no
    /// server claims is `Ok` — the caller reports "No LSP server configured".
    async fn ensure_ready_for_file(&self, path: &std::path::Path) -> Result<(), String> {
        let owner = {
            let mgr = self.lsp_manager.lock().await;
            super::config::resolve_server(&mgr.servers, path).map(|(name, _)| name)
        };
        match owner {
            Some(server) => self.ensure_ready_for_server(&server).await,
            None => Ok(()),
        }
    }

    /// Ensure every configured server is started and at least one succeeded —
    /// the eager contract, on demand (workspace-wide ops, explicit enable).
    async fn ensure_ready_all(&self) -> Result<(), String> {
        Self::ensure_started_all_with(self.lsp_manager.clone(), self.startup.clone()).await;
        loop {
            let notified = {
                let states = self.startup.states.lock().await;
                let pending = states
                    .values()
                    .any(|state| matches!(state, StartupState::Starting));
                if !pending {
                    let any_ready = states
                        .values()
                        .any(|state| matches!(state, StartupState::Ready));
                    return if any_ready {
                        Ok(())
                    } else {
                        Err("No LSP servers started successfully.".to_string())
                    };
                }
                self.startup.notify.notified()
            };
            notified.await;
        }
    }

    async fn is_server_ready(&self, server: &str) -> bool {
        matches!(
            self.startup.states.lock().await.get(server),
            Some(StartupState::Ready)
        )
    }
}

/// One server's lazy start: start it in the manager, replay the pre-ready
/// file changes that belong to it (plus ownerless watcher-only events, which
/// the first server to start consumes), and enroll its crash monitor.
async fn start_server(
    lsp_manager: Arc<tokio::sync::Mutex<LspManager>>,
    startup: Arc<StartupCoordinator>,
    server: &str,
) -> Result<(), String> {
    let restartable = {
        let mut mgr = lsp_manager.lock().await;
        mgr.ensure_server_started(server).await?;
        let pending: Vec<(PathBuf, Option<String>, super::DiskChangeKind)> = {
            let mut queue = startup.pre_ready_file_changes.lock().await;
            let taken: Vec<PathBuf> = queue
                .keys()
                .filter(|path| {
                    super::config::resolve_server(&mgr.servers, path)
                        .is_none_or(|(name, _)| name == server)
                })
                .cloned()
                .collect();
            taken
                .iter()
                .filter_map(|path| {
                    queue
                        .remove(path)
                        .map(|(content, kind)| (path.clone(), content, kind))
                })
                .collect()
        };
        for (path, content, kind) in &pending {
            mgr.notify_file_event(path, content.as_deref(), *kind);
        }
        mgr.restartable_servers().contains(&server.to_string())
    };
    if restartable {
        // Hand the monitor a `Weak` so it never keeps the manager (and its
        // language-server children) alive past the owning session.
        let mgr_weak = Arc::downgrade(&lsp_manager);
        tokio::spawn(crate::implementations::lsp::restart_monitor(
            mgr_weak,
            server.to_string(),
        ));
    }
    Ok(())
}

impl Drop for LspBackendAdapter {
    fn drop(&mut self) {
        tracing::debug!("LspBackendAdapter dropping, initiating LSP shutdown");
        let mgr = self.lsp_manager.clone();
        let _ = tokio::runtime::Handle::try_current().map(|handle| {
            handle.spawn(async move {
                mgr.lock().await.shutdown().await;
            })
        });
    }
}

#[async_trait::async_trait]
impl super::LspBackend for LspBackendAdapter {
    fn ensure_started_background(&self) {
        let lsp_manager = self.lsp_manager.clone();
        let startup = self.startup.clone();
        // Best-effort: constructed outside a runtime (sync test fixtures like
        // `WorkspaceOps::for_test`), skip the eager start — the first in-runtime
        // use starts the servers via `ensure_ready`, and a stray spawn must not
        // panic the caller.
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            tracing::debug!("lsp: no tokio runtime; deferring server startup to first use");
            return;
        };
        handle.spawn(async move {
            LspBackendAdapter::ensure_started_all_with(lsp_manager, startup).await;
        });
    }

    async fn ensure_ready(&self) -> Result<(), String> {
        self.ensure_ready_all().await
    }

    fn is_ready(&self) -> bool {
        self.startup
            .states
            .try_lock()
            .map(|states| {
                states
                    .values()
                    .any(|state| matches!(state, StartupState::Ready))
            })
            .unwrap_or(false)
    }

    async fn rename(
        &self,
        path: &std::path::Path,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<Vec<(std::path::PathBuf, String)>, String> {
        use async_lsp::lsp_types::RenameParams;

        let owner = {
            let mgr = self.lsp_manager.lock().await;
            super::config::resolve_server(&mgr.servers, path)
                .map(|(name, _)| name)
                .ok_or_else(|| format!("No LSP server configured for {}", path.display()))?
        };
        self.ensure_ready_for_server(&owner).await?;

        // Auto-open the target so the server definitely has it, then rename.
        let mut socket = {
            let mut mgr = self.lsp_manager.lock().await;
            mgr.socket_for_file(path)
                .await
                .ok_or_else(|| format!("No LSP server configured for {}", path.display()))?
        };
        let params = RenameParams {
            text_document_position: text_document_position(path, line, character)
                .map_err(|e| format!("{e}"))?,
            new_name: new_name.to_string(),
            work_done_progress_params: Default::default(),
        };
        let result = timed_request(socket.rename(params))
            .await
            .map_err(|e| format!("{e}"))?;
        let Some(workspace_edit) = result else {
            return Err(format!(
                "Nothing renameable at {}:{line}:{character} — navigate to the symbol first",
                path.display()
            ));
        };

        // 原子预检: read every affected file and apply its edits in memory
        // before anything is written — one bad offset rejects the whole batch.
        let file_edits = super::rename::workspace_edit_to_file_edits(workspace_edit)?;
        let mut out = Vec::with_capacity(file_edits.len());
        for (uri, text_edits) in file_edits {
            let file_path = uri
                .to_file_path()
                .map_err(|_| format!("non-file uri in rename result: {uri}"))?;
            let content = tokio::fs::read_to_string(&file_path)
                .await
                .map_err(|e| format!("cannot read {}: {e}", file_path.display()))?;
            let new_content = super::rename::apply_text_edits(&content, &text_edits)?;
            out.push((file_path, new_content));
        }
        Ok(out)
    }

    async fn status_snapshot(&self) -> Option<LspBackendStatus> {
        let states = self.startup.states.lock().await;
        let running_servers = {
            let mgr = self.lsp_manager.lock().await;
            mgr.clients.keys().cloned().collect()
        };
        // Aggregate: any Starting dominates (something is on its way); else
        // any Failed speaks for itself; a server that never started simply
        // does not appear — its configured name still shows in the /lsp
        // status panel as not running (lazy: started on first access).
        let startup = if states.is_empty() {
            LspStartupState::NotStarted
        } else if states
            .values()
            .any(|state| matches!(state, StartupState::Starting))
        {
            LspStartupState::Starting
        } else if let Some(StartupState::Failed(error)) = states
            .values()
            .find(|state| matches!(state, StartupState::Failed(_)))
        {
            LspStartupState::Failed(error.clone())
        } else {
            LspStartupState::Ready
        };
        drop(states);
        Some(LspBackendStatus {
            startup: Some(startup),
            running_servers,
        })
    }

    async fn dispatch(&self, input: &LspToolInput) -> LspToolResult {
        use super::LspOperation;

        // The diagnostics ops read the per-server stores rather than dispatch
        // requests, so they are routed before socket selection. The settle
        // wait runs on cloned handles, not under the manager lock.
        // Lazy start: a file-scoped op starts only the server owning the
        // file; workspace-scoped ops start everything (they asked for it).
        match input.operation {
            LspOperation::WorkspaceDiagnostics => {
                if let Err(error) = self.ensure_ready_all().await {
                    return err_result(format!("LSP startup failed: {error}"));
                }
                let limit = input.limit.unwrap_or(WORKSPACE_DIAGNOSTICS_LIMIT);
                let mgr = self.lsp_manager.lock().await;
                return read_workspace_diagnostics(&mgr, limit);
            }
            LspOperation::Diagnostics => {
                let Some(ref fp) = input.file_path else {
                    return err_result("Required: file_path.".into());
                };
                let path = PathBuf::from(fp);
                if let Err(error) = self.ensure_ready_for_file(&path).await {
                    return err_result(format!("LSP startup failed: {error}"));
                }
                let handles = {
                    let mut mgr = self.lsp_manager.lock().await;
                    match resolve_diagnostics_handles(&mut mgr, &path).await {
                        Ok(handles) => handles,
                        Err(msg) => return err_result(msg),
                    }
                };
                return read_file_diagnostics(&handles, &path).await;
            }
            _ => {}
        }

        // Socket ops: start only what the op needs, before taking the
        // manager lock for socket selection.
        match input.operation {
            LspOperation::WorkspaceSymbol => {
                if let Err(error) = self.ensure_ready_all().await {
                    return err_result(format!("LSP startup failed: {error}"));
                }
            }
            _ => {
                let Some(fp) = input.file_path.as_deref() else {
                    return err_result("Required: file_path.".into());
                };
                let path = PathBuf::from(fp);
                let owner = {
                    let mgr = self.lsp_manager.lock().await;
                    super::config::resolve_server(&mgr.servers, &path).map(|(name, _)| name)
                };
                let Some(server) = owner else {
                    return err_result(format!(
                        "No LSP server configured for {}",
                        path.display()
                    ));
                };
                if let Err(error) = self.ensure_ready_for_server(&server).await {
                    return err_result(format!("LSP startup failed: {error}"));
                }
            }
        }

        // Brief lock: auto-open + clone socket(s). Then drop lock.
        let sockets = {
            let mut mgr = self.lsp_manager.lock().await;
            match input.operation {
                LspOperation::WorkspaceSymbol => {
                    if mgr.clients.is_empty() {
                        return err_result("No LSP servers are running.".into());
                    }
                    DispatchSockets::All(mgr.all_sockets())
                }
                _ => {
                    let path = PathBuf::from(input.file_path.as_deref().expect("checked above"));
                    match mgr.socket_for_file(&path).await {
                        Some(s) => DispatchSockets::One(s),
                        None => {
                            return err_result(format!(
                                "No LSP server configured for {}",
                                path.display()
                            ));
                        }
                    }
                }
            }
        };
        // Lock dropped — dispatch on cloned socket(s).
        dispatch_on_sockets(input, sockets).await
    }

    async fn drain_diagnostics(
        &self,
        timeout: std::time::Duration,
    ) -> Option<super::manager::DiagnosticsSummary> {
        if !self.is_ready() {
            return None;
        }
        super::manager::drain_lsp_diagnostics(&self.lsp_manager, timeout).await
    }

    async fn notify_file_changed(&self, path: &std::path::Path, content: &str) {
        self.notify_file_event(path, Some(content), super::DiskChangeKind::Changed)
            .await;
    }

    async fn notify_file_event(
        &self,
        path: &std::path::Path,
        content: Option<&str>,
        kind: super::DiskChangeKind,
    ) {
        let owner = {
            let mgr = self.lsp_manager.lock().await;
            super::config::resolve_server(&mgr.servers, path).map(|(name, _)| name)
        };
        let owner_ready = match &owner {
            Some(server) => self.is_server_ready(server).await,
            None => false,
        };
        if owner_ready {
            self.lsp_manager
                .lock()
                .await
                .notify_file_event(path, content, kind);
        } else {
            // Queue first, so the start task's drain cannot overtake the
            // insert, then make sure the owning server is on its way — the
            // read/write lazy trigger (crush's canAutoStart). Ownerless paths
            // (watcher-only extensions) just queue for the next start.
            self.startup
                .pre_ready_file_changes
                .lock()
                .await
                .insert(path.to_path_buf(), (content.map(str::to_string), kind));
            if let Some(server) = owner {
                self.ensure_started_one(&server).await;
            }
        }
    }

    async fn read_diagnostics(
        &self,
        paths: &[std::path::PathBuf],
    ) -> Vec<super::FileDiagnosticEntry> {
        if self.ensure_ready().await.is_err() {
            return vec![];
        }

        // Open files that aren't tracked yet so the LSP can analyze them.
        {
            let mut mgr = self.lsp_manager.lock().await;
            for path in paths {
                let _ = mgr.socket_for_file(path).await;
            }
        }

        // Wait briefly after opening files. This is the native-LSP analogue of the IDE wait between
        // TrackModel and the second diagnostics call: opening/tracking a file starts analysis,
        // while diagnostics arrive later through publishDiagnostics.
        let notify = {
            let mgr = self.lsp_manager.lock().await;
            mgr.diagnostics_ready.clone()
        };
        let notified = notify.notified();
        let _ = tokio::time::timeout(std::time::Duration::from_millis(1000), notified).await;

        // Collect diagnostics from all clients for the requested paths.
        let mgr = self.lsp_manager.lock().await;
        let mut results = Vec::new();
        for path in paths {
            let uri = match super::file_uri(path) {
                Ok(u) => u,
                Err(_) => continue,
            };
            let uri_str = uri.to_string();
            let mut file_diagnostics = Vec::new();

            for client in mgr.clients.values() {
                for d in client.diagnostics.items(&uri_str) {
                    let severity = match d.severity {
                        Some(async_lsp::lsp_types::DiagnosticSeverity::ERROR) => {
                            super::DiagnosticSeverityLevel::Error
                        }
                        Some(async_lsp::lsp_types::DiagnosticSeverity::WARNING) => {
                            super::DiagnosticSeverityLevel::Warning
                        }
                        _ => continue,
                    };
                    file_diagnostics.push(super::DiagnosticEntry {
                        severity,
                        // LSP uses 0-based positions; convert to 1-based
                        // for display (L{line}:{column}).
                        line: d.range.start.line + 1,
                        column: d.range.start.character + 1,
                        message: d.message.clone(),
                        source: d.source.clone(),
                        code: None,
                        is_stale: false,
                    });
                }
            }

            results.push(super::FileDiagnosticEntry {
                path: path.display().to_string(),
                diagnostics: file_diagnostics,
            });
        }
        results
    }
}

// ── Internal types ──────────────────────────────────────────────────────

enum DispatchSockets {
    One(async_lsp::ServerSocket),
    All(Vec<async_lsp::ServerSocket>),
}

fn err_result(msg: String) -> LspToolResult {
    LspToolResult {
        text: msg,
        is_error: true,
    }
}

fn ok_result(text: String) -> LspToolResult {
    LspToolResult {
        text,
        is_error: false,
    }
}

fn require_position(input: &LspToolInput) -> Result<(PathBuf, TextDocumentPositionParams), String> {
    let (Some(fp), Some(line), Some(character)) = (&input.file_path, input.line, input.character)
    else {
        return Err("Required: file_path, line, character.".into());
    };
    let path = PathBuf::from(fp);
    let params = text_document_position(&path, line, character).map_err(|e| format!("{e}"))?;
    Ok((path, params))
}

/// Awaits an LSP request future with a standard timeout and error mapping.
async fn timed_request<F, T, E>(fut: F) -> Result<T, LspError>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio::time::timeout(REQUEST_TIMEOUT, fut)
        .await
        .map_err(|_| LspError::RequestFailed("request timed out".into()))?
        .map_err(|e| LspError::RequestFailed(format!("{e}")))
}

/// Distinguishes validation errors (missing params) from LSP protocol errors.
enum DispatchError {
    /// Missing or invalid input parameters.
    Validation(String),
    /// LSP request failed or timed out.
    Lsp(LspError),
}

impl From<String> for DispatchError {
    fn from(msg: String) -> Self {
        Self::Validation(msg)
    }
}

impl From<LspError> for DispatchError {
    fn from(e: LspError) -> Self {
        Self::Lsp(e)
    }
}

// ── Router ──────────────────────────────────────────────────────────────

async fn dispatch_on_sockets(input: &LspToolInput, sockets: DispatchSockets) -> LspToolResult {
    use super::LspOperation;

    match sockets {
        DispatchSockets::One(mut socket) => {
            let result = match input.operation {
                LspOperation::GoToDefinition => dispatch_goto(input, &mut socket, true).await,
                LspOperation::GoToImplementation => dispatch_goto(input, &mut socket, false).await,
                LspOperation::FindReferences => dispatch_references(input, &mut socket).await,
                LspOperation::Hover => dispatch_hover(input, &mut socket).await,
                LspOperation::DocumentSymbol => dispatch_document_symbols(input, &mut socket).await,
                LspOperation::WorkspaceSymbol => unreachable!(),
                // Routed to the store readers before socket selection.
                LspOperation::Diagnostics | LspOperation::WorkspaceDiagnostics => unreachable!(),
            };
            match result {
                Ok(text) => ok_result(text),
                Err(DispatchError::Validation(msg)) => err_result(msg),
                Err(DispatchError::Lsp(e)) => {
                    tracing::warn!(error = %e, "LSP tool failed");
                    err_result(format!("LSP error: {e}"))
                }
            }
        }
        DispatchSockets::All(sockets) => dispatch_workspace_symbols(input, sockets).await,
    }
}

// ── Per-operation helpers ───────────────────────────────────────────────

/// Handles both GoToDefinition and GoToImplementation (same params/response shape).
async fn dispatch_goto(
    input: &LspToolInput,
    socket: &mut async_lsp::ServerSocket,
    is_definition: bool,
) -> Result<String, DispatchError> {
    let (_path, pos) = require_position(input)?;
    let params = GotoDefinitionParams {
        text_document_position_params: pos,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let label = if is_definition {
        "Definition"
    } else {
        "Implementations"
    };
    let fut = if is_definition {
        socket.definition(params)
    } else {
        socket.implementation(params)
    };
    let response = timed_request(fut).await?;
    let locs = parse_goto_response(response);
    Ok(format_locations_labeled(label, &locs))
}

async fn dispatch_references(
    input: &LspToolInput,
    socket: &mut async_lsp::ServerSocket,
) -> Result<String, DispatchError> {
    let (_path, pos) = require_position(input)?;
    let params = ReferenceParams {
        text_document_position: pos,
        context: ReferenceContext {
            include_declaration: true,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let result = timed_request(socket.references(params)).await?;
    Ok(format_locations_labeled(
        "References",
        &result.unwrap_or_default(),
    ))
}

async fn dispatch_hover(
    input: &LspToolInput,
    socket: &mut async_lsp::ServerSocket,
) -> Result<String, DispatchError> {
    let (_path, pos) = require_position(input)?;
    let params = HoverParams {
        text_document_position_params: pos,
        work_done_progress_params: Default::default(),
    };
    let result = timed_request(socket.hover(params)).await?;
    Ok(result
        .map(|h| match h.contents {
            lsp_types::HoverContents::Scalar(ms) => markup_string_to_text(ms),
            lsp_types::HoverContents::Array(arr) => arr
                .into_iter()
                .map(markup_string_to_text)
                .collect::<Vec<_>>()
                .join("\n"),
            lsp_types::HoverContents::Markup(mc) => mc.value,
        })
        .unwrap_or_else(|| "No hover information available.".to_string()))
}

async fn dispatch_document_symbols(
    input: &LspToolInput,
    socket: &mut async_lsp::ServerSocket,
) -> Result<String, DispatchError> {
    let Some(ref fp) = input.file_path else {
        return Err(DispatchError::Validation("Required: file_path.".into()));
    };
    let uri = file_uri(Path::new(fp))?;
    let params = DocumentSymbolParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let result = timed_request(socket.document_symbol(params)).await?;
    Ok(match result {
        Some(DocumentSymbolResponse::Flat(s)) => format_symbols(&s),
        Some(DocumentSymbolResponse::Nested(n)) => {
            let mut flat = Vec::new();
            flatten_document_symbols(&n, &uri, &mut flat);
            format_symbols(&flat)
        }
        None => format_symbols(&[]),
    })
}

async fn dispatch_workspace_symbols(
    input: &LspToolInput,
    sockets: Vec<async_lsp::ServerSocket>,
) -> LspToolResult {
    let Some(ref query) = input.query else {
        return err_result("Required: query (string).".into());
    };
    let mut all_symbols = Vec::new();
    let mut last_err: Option<String> = None;
    for mut s in sockets {
        let params = WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        match tokio::time::timeout(REQUEST_TIMEOUT, s.symbol(params)).await {
            Ok(Ok(Some(lsp_types::WorkspaceSymbolResponse::Flat(symbols)))) => {
                all_symbols.extend(symbols)
            }
            Ok(Ok(Some(lsp_types::WorkspaceSymbolResponse::Nested(ws)))) => {
                all_symbols.extend(ws.into_iter().map(workspace_symbol_to_info));
            }
            Ok(Ok(None)) => {}
            Ok(Err(e)) => last_err = Some(format!("{e}")),
            Err(_) => last_err = Some("request timed out".into()),
        }
    }
    if all_symbols.is_empty() {
        match last_err {
            Some(msg) => err_result(format!("LSP error: {msg}")),
            None => ok_result(format_symbols(&[])),
        }
    } else {
        ok_result(format_symbols(&all_symbols))
    }
}

// ── Response converters ─────────────────────────────────────────────────

fn parse_goto_response(response: Option<GotoDefinitionResponse>) -> Vec<Location> {
    match response {
        Some(GotoDefinitionResponse::Scalar(l)) => vec![l],
        Some(GotoDefinitionResponse::Array(l)) => l,
        Some(GotoDefinitionResponse::Link(links)) => links
            .into_iter()
            .map(|link| Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            })
            .collect(),
        None => vec![],
    }
}

fn workspace_symbol_to_info(ws: lsp_types::WorkspaceSymbol) -> SymbolInformation {
    let loc = match ws.location {
        lsp_types::OneOf::Left(l) => l,
        lsp_types::OneOf::Right(d) => Location {
            uri: d.uri,
            range: Default::default(),
        },
    };
    #[allow(deprecated)]
    SymbolInformation {
        name: ws.name,
        kind: ws.kind,
        tags: ws.tags,
        deprecated: None,
        location: loc,
        container_name: ws.container_name,
    }
}
