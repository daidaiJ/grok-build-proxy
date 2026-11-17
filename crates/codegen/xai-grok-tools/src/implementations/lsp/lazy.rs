//! Lazy [`LspBackend`] handle.
//!
//! Session spawn must decide whether to advertise the `lsp` tool, but discovering language
//! servers (config layers, plugin LSP configs, builtin PATH probes) and constructing the
//! manager is startup work a session may never need. This handle answers the trait immediately
//! and runs the supplied factory on the first real call (edit notification, diagnostics pull,
//! or an explicit `lsp` invocation).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::manager::DiagnosticsSummary;
use super::types::{
    DiskChangeKind, FileDiagnosticEntry, LspBackend, LspBackendStatus, LspToolInput, LspToolResult,
};

/// Builds the real backend. Runs at most once, on the first call that needs it.
pub type LspBackendFactory = Box<
    dyn FnOnce() -> Pin<Box<dyn Future<Output = Option<Arc<dyn LspBackend>>> + Send>> + Send + Sync,
>;

const UNAVAILABLE: &str =
    "LSP is unavailable: no language server is configured for this workspace.";

struct LazyInner {
    factory: tokio::sync::Mutex<Option<LspBackendFactory>>,
    backend: tokio::sync::OnceCell<Option<Arc<dyn LspBackend>>>,
}

impl LazyInner {
    async fn get(&self) -> Option<Arc<dyn LspBackend>> {
        let resolved = self
            .backend
            .get_or_init(|| async {
                let factory = self.factory.lock().await.take();
                match factory {
                    Some(factory) => factory().await,
                    None => None,
                }
            })
            .await;
        resolved.as_ref().cloned()
    }

    fn ready(&self) -> bool {
        self.backend
            .get()
            .and_then(Option::as_ref)
            .is_some_and(|backend| backend.is_ready())
    }
}

/// See the module docs. Cloneable through the returned `Arc<dyn LspBackend>`.
pub struct LazyLspBackend {
    inner: Arc<LazyInner>,
}

impl LazyLspBackend {
    /// Wrap `factory` as a backend handle. The factory is consumed on first use.
    pub fn new(factory: LspBackendFactory) -> Arc<dyn LspBackend> {
        Arc::new(Self {
            inner: Arc::new(LazyInner {
                factory: tokio::sync::Mutex::new(Some(factory)),
                backend: tokio::sync::OnceCell::new(),
            }),
        })
    }
}

#[async_trait::async_trait]
impl LspBackend for LazyLspBackend {
    fn ensure_started_background(&self) {
        // Starting is deferred; a caller that wants a warm backend still gets one, off-thread.
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            if let Some(backend) = inner.get().await {
                backend.ensure_started_background();
            }
        });
    }

    async fn ensure_ready(&self) -> Result<(), String> {
        match self.inner.get().await {
            Some(backend) => backend.ensure_ready().await,
            None => Err(UNAVAILABLE.to_string()),
        }
    }

    fn is_ready(&self) -> bool {
        self.inner.ready()
    }

    async fn dispatch(&self, input: &LspToolInput) -> LspToolResult {
        match self.inner.get().await {
            Some(backend) => backend.dispatch(input).await,
            None => LspToolResult {
                text: UNAVAILABLE.to_string(),
                is_error: true,
            },
        }
    }

    async fn drain_diagnostics(&self, timeout: std::time::Duration) -> Option<DiagnosticsSummary> {
        match self.inner.get().await {
            Some(backend) => backend.drain_diagnostics(timeout).await,
            None => None,
        }
    }

    async fn notify_file_changed(&self, path: &std::path::Path, content: &str) {
        if let Some(backend) = self.inner.get().await {
            backend.notify_file_changed(path, content).await;
        }
    }

    async fn notify_file_event(
        &self,
        path: &std::path::Path,
        content: Option<&str>,
        kind: DiskChangeKind,
    ) {
        if let Some(backend) = self.inner.get().await {
            backend.notify_file_event(path, content, kind).await;
        }
    }

    async fn read_diagnostics(&self, paths: &[std::path::PathBuf]) -> Vec<FileDiagnosticEntry> {
        match self.inner.get().await {
            Some(backend) => backend.read_diagnostics(paths).await,
            None => Vec::new(),
        }
    }

    async fn rename(
        &self,
        path: &std::path::Path,
        line: u32,
        character: u32,
        new_name: &str,
    ) -> Result<Vec<(std::path::PathBuf, String)>, String> {
        match self.inner.get().await {
            Some(backend) => backend.rename(path, line, character, new_name).await,
            None => Err(UNAVAILABLE.to_string()),
        }
    }

    async fn status_snapshot(&self) -> Option<LspBackendStatus> {
        match self.inner.get().await {
            Some(backend) => backend.status_snapshot().await,
            None => None,
        }
    }
}
