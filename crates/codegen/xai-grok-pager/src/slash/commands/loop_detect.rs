//! LOCAL: `/loop-detect` — abort the turn vs record-only for client loop detection.
//!
//! Process-level override, instant. Persistent default is
//! `[features] loop_detect_abort` / `GROK_LOOP_DETECT_ABORT`.

use xai_grok_tools::loop_detect_ledger;

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};
use crate::slash::i18n::tr;

pub struct LoopDetectCommand;

impl SlashCommand for LoopDetectCommand {
    slash_meta! {
        name: "loop-detect",
        description: "Loop detect: abort the turn or only record to /stats",
        usage: "/loop-detect [abort|record|default]",
        takes_args: true,
        arg_placeholder: "abort|record|default",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        match args.trim().to_ascii_lowercase().as_str() {
            "" | "status" => CommandResult::Message(status_text()),
            "abort" | "stop" | "中止" => {
                loop_detect_ledger::set_abort_override(true);
                CommandResult::Message(
                    tr("Loop detect: abort the turn when a loop is detected (this process).")
                        .to_string(),
                )
            }
            "record" | "log" | "记录" => {
                loop_detect_ledger::set_abort_override(false);
                CommandResult::Message(
                    tr("Loop detect: record to /stats only; the turn continues (this process).")
                        .to_string(),
                )
            }
            "default" | "config" | "默认" => {
                loop_detect_ledger::clear_abort_override();
                CommandResult::Message(
                    tr("Loop detect: follow [features] loop_detect_abort / GROK_LOOP_DETECT_ABORT.")
                        .to_string(),
                )
            }
            other => CommandResult::Error(format!(
                "Unknown argument '{other}'. Usage: /loop-detect [abort|record|default]"
            )),
        }
    }
}

fn status_text() -> String {
    match loop_detect_ledger::abort_override() {
        Some(true) => tr("Loop detect mode: abort (process override)").to_string(),
        Some(false) => tr("Loop detect mode: record-only (process override)").to_string(),
        None => tr(
            "Loop detect mode: config default (abort unless [features] loop_detect_abort = false)",
        )
        .to_string(),
    }
}
