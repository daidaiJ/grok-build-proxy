//! LOCAL(compress-fast): `/compress-fast` clears old tool-result bodies without a model call.
//! `run` returns `CommandResult::QueueCommand` so the dispatch layer tracks it as a command.

use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

/// Shell response from `x.ai/session/compressFast` (snake-case skip reason is kept as a string so
/// the pager does not depend on the chat-state crate).
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressFastResponse {
    #[serde(default)]
    pub cleared_tool_results: usize,
    #[serde(default)]
    pub bytes_freed: u64,
    #[serde(default)]
    pub skipped: Option<String>,
}

/// Drop old tool results from the conversation; no model call.
pub struct CompressFastCommand;

impl SlashCommand for CompressFastCommand {
    slash_meta! {
        name: "compress-fast",
        description: "Clear old tool results without a model call",
        usage: "/compress-fast",
        takes_args: false,
        session_scoped: true,
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::QueueCommand("/compress-fast".to_string())
    }
}
