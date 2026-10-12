//! Per-workspace feature overrides under grok-home, written by the pager's `/lsp`
//! slash command and read at session spawn when the agent definition is built.
//!
//! File layout: `<grok_home>/feature-overrides/<encode_cwd_dirname(cwd)>.json` holding a
//! flat map of feature key → bool (`{"lsp_tools": true}`). Writer and reader both hold the
//! cwd, so the encoded dirname is a deterministic key (no `.cwd` metadata needed). Writes
//! are owner-only via [`write_secure_file`]; the file is tiny and single-writer in practice
//! (one TUI per workspace), so an in-place write is enough — a torn write parses as invalid
//! JSON and reads as `None` (fall back to the global tier), never as an unintended value.

use std::io;
use std::path::{Path, PathBuf};

/// Feature key for the LSP tools family, matching the registry key of `Feature::LspTools`.
pub const LSP_TOOLS_OVERRIDE_KEY: &str = "lsp_tools";

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct WorkspaceFeatureOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lsp_tools: Option<bool>,
}

impl WorkspaceFeatureOverrides {
    fn get(&self, key: &str) -> Option<bool> {
        match key {
            LSP_TOOLS_OVERRIDE_KEY => self.lsp_tools,
            _ => None,
        }
    }
}

fn feature_overrides_dir_in(grok_home: &Path) -> PathBuf {
    grok_home.join("feature-overrides")
}

/// [`workspace_override_path_in`] against the process grok home.
pub fn workspace_override_path(cwd: &Path) -> PathBuf {
    workspace_override_path_in(&crate::util::grok_home::grok_home(), cwd)
}

fn workspace_override_path_in(grok_home: &Path, cwd: &Path) -> PathBuf {
    feature_overrides_dir_in(grok_home).join(format!(
        "{}.json",
        crate::util::grok_home::encode_cwd_dirname(&cwd.to_string_lossy())
    ))
}

/// [`read_workspace_override_in`] against the process grok home.
pub fn read_workspace_override(cwd: &Path, key: &str) -> Option<bool> {
    read_workspace_override_in(&crate::util::grok_home::grok_home(), cwd, key)
}

fn read_workspace_override_in(grok_home: &Path, cwd: &Path, key: &str) -> Option<bool> {
    let path = workspace_override_path_in(grok_home, cwd);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::debug!(error = %e, path = %path.display(), "failed to read workspace feature overrides");
            return None;
        }
    };
    match serde_json::from_str::<WorkspaceFeatureOverrides>(&content) {
        Ok(overrides) => overrides.get(key),
        Err(e) => {
            tracing::debug!(error = %e, path = %path.display(), "invalid workspace feature overrides JSON; ignoring");
            None
        }
    }
}

/// [`write_workspace_override_in`] against the process grok home.
pub fn write_workspace_override(cwd: &Path, key: &str, value: Option<bool>) -> io::Result<()> {
    write_workspace_override_in(&crate::util::grok_home::grok_home(), cwd, key, value)
}

/// Set (`Some`) or clear (`None`) one per-workspace feature override.
/// `None` removes the key; a file left with no keys is deleted outright so
/// `feature-overrides/` only holds workspaces with active overrides.
/// Unknown keys fail loud: the typed map has no slot to store them.
pub fn write_workspace_override_in(
    grok_home: &Path,
    cwd: &Path,
    key: &str,
    value: Option<bool>,
) -> io::Result<()> {
    let path = workspace_override_path_in(grok_home, cwd);
    let mut overrides = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<WorkspaceFeatureOverrides>(&content)
            .unwrap_or_default(),
        Err(_) => WorkspaceFeatureOverrides::default(),
    };
    let previous = match key {
        LSP_TOOLS_OVERRIDE_KEY => std::mem::replace(&mut overrides.lsp_tools, value),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown workspace feature override key: {key}"),
            ))
        }
    };
    if previous == value && !(value.is_none() && path.exists()) {
        // Nothing to change (an existing file with a clearing write still gets cleaned up).
        return Ok(());
    }
    if value.is_none() {
        // No active keys left: remove the file instead of storing an empty map.
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        };
    }
    let content = serde_json::to_string(&overrides).expect("serialize workspace overrides");
    crate::util::secure_file::write_secure_file(&path, content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempHome(std::path::PathBuf);

    impl TempHome {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "grok-ws-ovr-{tag}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).expect("create temp home");
            Self(dir)
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const A: &str = if cfg!(windows) {
        r"D:\proj\alpha"
    } else {
        "/proj/alpha"
    };
    const B: &str = if cfg!(windows) {
        r"D:\proj\beta"
    } else {
        "/proj/beta"
    };

    #[test]
    fn roundtrip_write_read_clear() {
        let home = TempHome::new("roundtrip");
        let cwd = Path::new(A);
        assert_eq!(read_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY), None);

        write_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY, Some(true)).expect("write");
        assert_eq!(
            read_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY),
            Some(true)
        );

        write_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY, Some(false)).expect("write");
        assert_eq!(
            read_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY),
            Some(false)
        );

        write_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY, None).expect("clear");
        assert_eq!(read_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY), None);
        // Clearing the only key removes the file outright.
        assert!(!workspace_override_path_in(&home.0, cwd).exists());
    }

    #[test]
    fn workspaces_are_keyed_independently() {
        let home = TempHome::new("keyed");
        let (a, b) = (Path::new(A), Path::new(B));
        write_workspace_override_in(&home.0, a, LSP_TOOLS_OVERRIDE_KEY, Some(true)).expect("write a");
        assert_eq!(
            read_workspace_override_in(&home.0, b, LSP_TOOLS_OVERRIDE_KEY),
            None,
            "another workspace must not inherit the override"
        );
        assert_ne!(workspace_override_path_in(&home.0, a), workspace_override_path_in(&home.0, b));
    }

    #[test]
    fn invalid_json_reads_as_none() {
        let home = TempHome::new("invalid");
        let cwd = Path::new(A);
        let path = workspace_override_path_in(&home.0, cwd);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, "{not json").expect("seed");
        assert_eq!(read_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY), None);
        // A later write recovers from the invalid file instead of failing forever.
        write_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY, Some(true)).expect("write");
        assert_eq!(read_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY), Some(true));
    }

    #[test]
    fn same_value_write_is_skipped() {
        let home = TempHome::new("idempotent");
        let cwd = Path::new(A);
        write_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY, Some(true)).expect("write");
        let before = std::fs::read_to_string(workspace_override_path_in(&home.0, cwd)).expect("read");
        write_workspace_override_in(&home.0, cwd, LSP_TOOLS_OVERRIDE_KEY, Some(true)).expect("write");
        let after = std::fs::read_to_string(workspace_override_path_in(&home.0, cwd)).expect("read");
        assert_eq!(before, after);
    }
}
