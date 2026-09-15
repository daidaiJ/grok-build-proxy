//! Cross-cutting reminder: notifies LSP of file changes and drains diagnostics.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::implementations::lsp::{DiskChangeKind, LspBackend};
use crate::types::output::{
    ApplyPatchFileResult, ApplyPatchOutput, SearchReplaceOutput, ToolOutput,
};
use crate::types::resources::SharedResources;
use crate::types::tool::Reminder;

/// After a diagnostics injection, an edit landing inside this window skips its
/// own drain: every pending file stays open with the servers, so the next
/// reminder's drain reports the whole burst as one merged summary instead of
/// one overlapping summary per edit. The price is rare — a burst that ends the
/// turn injects on the next tool call (or the next turn) rather than never.
const INJECT_DEBOUNCE: Duration = Duration::from_millis(750);

/// Drain bookkeeping shared through [`SharedResources`], so every reminder call
/// in the process sees the same last injection.
#[derive(Default)]
struct DrainDebounce {
    last_inject: Option<Instant>,
}

pub struct LspDiagnosticsReminder;

#[async_trait::async_trait]
impl Reminder for LspDiagnosticsReminder {
    async fn collect_reminders(
        &self,
        resources: SharedResources,
        tool_output: &ToolOutput,
    ) -> Vec<String> {
        let lsp = {
            let res = resources.lock().await;
            match res.get::<Arc<dyn LspBackend>>() {
                Some(h) => h.clone(),
                None => return vec![],
            }
        };

        lsp.ensure_started_background();

        // Structured mutations we ourselves made. bash/git have no file list;
        // watching the workspace for those is the OS-watcher leak this path
        // exists to avoid.
        for (path, content, kind) in disk_events(tool_output) {
            lsp.notify_file_event(&path, content.as_deref(), kind).await;
        }

        // A drain that just ran leaves the servers mid-analysis; draining again
        // for an edit inside the window would inject a summary about text that
        // is already covered, and block on it besides.
        {
            let mut res = resources.lock().await;
            let debounce = res.get_or_default::<DrainDebounce>();
            if debounce
                .last_inject
                .is_some_and(|at| at.elapsed() < INJECT_DEBOUNCE)
            {
                return vec![];
            }
        }

        // Drain any pending diagnostics (from this or previous edits). One drain
        // reports every pending file, which is what merges a burst of edits.
        if let Some(summary) = lsp
            .drain_diagnostics(crate::implementations::lsp::DIAGNOSTICS_DRAIN_TIMEOUT)
            .await
        {
            let mut res = resources.lock().await;
            res.get_or_default::<DrainDebounce>().last_inject = Some(Instant::now());
            return vec![summary.text];
        }

        vec![]
    }
}

fn disk_events(tool_output: &ToolOutput) -> Vec<(PathBuf, Option<String>, DiskChangeKind)> {
    match tool_output {
        ToolOutput::SearchReplace(SearchReplaceOutput::EditsApplied(edits)) => {
            let kind = if edits.old_string.is_empty() {
                DiskChangeKind::Created
            } else {
                DiskChangeKind::Changed
            };
            let content = std::fs::read_to_string(&edits.absolute_path).ok();
            vec![(edits.absolute_path.clone(), content, kind)]
        }
        ToolOutput::ApplyPatch(ApplyPatchOutput::Success { files, .. }) => {
            files.iter().flat_map(apply_patch_events).collect()
        }
        _ => Vec::new(),
    }
}

fn apply_patch_events(
    file: &ApplyPatchFileResult,
) -> Vec<(PathBuf, Option<String>, DiskChangeKind)> {
    match file.action.as_str() {
        "added" => vec![(
            file.path.clone(),
            Some(file.new_text.clone()),
            DiskChangeKind::Created,
        )],
        "deleted" => vec![(file.path.clone(), None, DiskChangeKind::Deleted)],
        "moved" => {
            let mut events = vec![(file.path.clone(), None, DiskChangeKind::Deleted)];
            if let Some(dest) = &file.move_to {
                events.push((
                    dest.clone(),
                    Some(file.new_text.clone()),
                    DiskChangeKind::Created,
                ));
            }
            events
        }
        _ => vec![(
            file.path.clone(),
            Some(file.new_text.clone()),
            DiskChangeKind::Changed,
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::lsp::{DiagnosticsSummary, FileDiagnosticEntry, LspToolInput, LspToolResult};
    use crate::types::output::SearchReplaceEditsApplied;
    use crate::types::resources::Resources;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct FakeBackend {
        drain_calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl LspBackend for FakeBackend {
        fn ensure_started_background(&self) {}

        async fn ensure_ready(&self) -> Result<(), String> {
            Ok(())
        }

        fn is_ready(&self) -> bool {
            true
        }

        async fn dispatch(&self, _input: &LspToolInput) -> LspToolResult {
            LspToolResult {
                text: String::new(),
                is_error: false,
            }
        }

        async fn drain_diagnostics(&self, _timeout: Duration) -> Option<DiagnosticsSummary> {
            self.drain_calls.fetch_add(1, Ordering::SeqCst);
            Some(DiagnosticsSummary {
                text: "<lsp-diagnostics>a problem</lsp-diagnostics>".into(),
                file_count: 1,
                diagnostic_count: 1,
            })
        }

        async fn notify_file_changed(&self, _path: &std::path::Path, _content: &str) {}

        async fn notify_file_event(
            &self,
            _path: &std::path::Path,
            _content: Option<&str>,
            _kind: DiskChangeKind,
        ) {
        }

        async fn read_diagnostics(&self, _paths: &[std::path::PathBuf]) -> Vec<FileDiagnosticEntry> {
            Vec::new()
        }
    }

    fn edit_output() -> ToolOutput {
        ToolOutput::SearchReplace(SearchReplaceOutput::EditsApplied(SearchReplaceEditsApplied {
            old_string: "a".into(),
            new_string: "b".into(),
            tool_output_for_prompt: String::new(),
            tool_output_for_prompt_concise: None,
            absolute_path: PathBuf::from("edited.ts"),
            edits: Default::default(),
            patch: None,
            unicode_normalized: false,
        }))
    }

    fn shared_resources() -> SharedResources {
        Arc::new(tokio::sync::Mutex::new(Resources::new()))
    }

    async fn drain_calls(fake: &FakeBackend) -> usize {
        fake.drain_calls.load(Ordering::SeqCst)
    }

    async fn backdate_last_inject(resources: &SharedResources) {
        resources
            .lock()
            .await
            .get_or_default::<DrainDebounce>()
            .last_inject = Some(Instant::now() - INJECT_DEBOUNCE);
    }

    #[tokio::test]
    async fn edits_inside_the_debounce_window_share_one_drain() {
        let resources = shared_resources();
        let fake = Arc::new(FakeBackend::default());
        resources
            .lock()
            .await
            .insert::<Arc<dyn LspBackend>>(fake.clone());
        let reminder = LspDiagnosticsReminder;

        let first = reminder
            .collect_reminders(resources.clone(), &edit_output())
            .await;
        assert_eq!(first.len(), 1, "the first edit drains and injects");
        assert_eq!(drain_calls(&fake).await, 1);

        let second = reminder
            .collect_reminders(resources.clone(), &edit_output())
            .await;
        assert!(
            second.is_empty(),
            "an edit inside the window defers to the next drain"
        );
        assert_eq!(drain_calls(&fake).await, 1);

        backdate_last_inject(&resources).await;
        let third = reminder
            .collect_reminders(resources.clone(), &edit_output())
            .await;
        assert_eq!(third.len(), 1, "past the window, the drain runs again");
        assert_eq!(drain_calls(&fake).await, 2);
    }
}
