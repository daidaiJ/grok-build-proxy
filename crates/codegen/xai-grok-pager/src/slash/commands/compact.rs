//! LOCAL(compact-instructions): `/compact [instructions]` — optional instructions tell the
//! summarizer what to preserve.
//! `run` returns `CommandResult::QueueCommand` so the dispatch layer enqueues it as `QueueEntryKind::Command`.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

/// Length cap shared with the shell's ACP handler, which enforces it on the wire.
const MAX_INSTRUCTIONS_CHARS: usize =
    xai_grok_shell::session::helpers::session_compact::MAX_COMPACTION_INSTRUCTIONS_CHARS;

/// Compact the conversation history.
pub struct CompactCommand;

impl SlashCommand for CompactCommand {
    slash_meta! {
        name: "compact",
        description: "Compact conversation history",
        usage: "/compact [instructions]",
        takes_args: true,
        session_scoped: true,
        arg_placeholder: "compaction instructions",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let instructions = args.trim();
        if instructions.is_empty() {
            return CommandResult::QueueCommand("/compact".to_string());
        }
        if instructions.chars().count() > MAX_INSTRUCTIONS_CHARS {
            return CommandResult::Error(format!(
                "/compact instructions are limited to {MAX_INSTRUCTIONS_CHARS} characters."
            ));
        }
        CommandResult::QueueCommand(format!("/compact {instructions}"))
    }
}
