//! LOCAL(minimal-style): persisted minimal output-style toggle state.
//!
//! `/style` flips this flag; the value is read once at every session spawn
//! (primary and subagent), so a toggle always lands on the NEXT session while
//! the current session only picks it up if no model call has happened yet
//! (the rewrite gate lives in the `/style` executor).

use std::path::{Path, PathBuf};

use xai_grok_tools::util::grok_home::grok_home;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OutputStyleState {
    pub minimal: bool,
}

const STATE_FILE: &str = "output_style.json";

fn state_path_at_home(home: &Path) -> PathBuf {
    home.join(STATE_FILE)
}

fn parse_state(raw: &str) -> OutputStyleState {
    // Tolerant parse: missing/corrupt file means "off"; the file only ever
    // carries one key, so a strict schema would buy nothing.
    let minimal = serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| v.get("minimal").and_then(|m| m.as_bool()))
        .unwrap_or(false);
    OutputStyleState { minimal }
}

pub(crate) fn read_state_at_home(home: &Path) -> OutputStyleState {
    match std::fs::read_to_string(state_path_at_home(home)) {
        Ok(raw) => parse_state(&raw),
        Err(_) => OutputStyleState { minimal: false },
    }
}

pub(crate) fn write_state_at_home(home: &Path, state: OutputStyleState) -> std::io::Result<()> {
    let json = serde_json::json!({ "minimal": state.minimal }).to_string();
    std::fs::write(state_path_at_home(home), json)
}

/// Session-spawn read: is the minimal style overlay enabled?
pub(crate) fn minimal_style_enabled() -> bool {
    read_state_at_home(&grok_home()).minimal
}

/// `/style` write: persist the toggle for future session spawns.
pub(crate) fn store_minimal_style(enabled: bool) -> std::io::Result<()> {
    write_state_at_home(&grok_home(), OutputStyleState { minimal: enabled })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_reads_as_disabled() {
        let dir = std::env::temp_dir().join(format!("output-style-missing-{}", std::process::id()));
        assert!(!read_state_at_home(&dir).minimal);
    }

    #[test]
    fn state_roundtrips_and_tolerates_corruption() {
        let dir = std::env::temp_dir().join(format!("output-style-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default()));
        std::fs::create_dir_all(&dir).expect("temp home");
        assert!(!read_state_at_home(&dir).minimal);

        write_state_at_home(&dir, OutputStyleState { minimal: true }).expect("write");
        assert!(read_state_at_home(&dir).minimal);

        write_state_at_home(&dir, OutputStyleState { minimal: false }).expect("write off");
        assert!(!read_state_at_home(&dir).minimal);

        std::fs::write(state_path_at_home(&dir), "not json").expect("corrupt");
        assert!(!read_state_at_home(&dir).minimal, "corrupt file degrades to off");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
