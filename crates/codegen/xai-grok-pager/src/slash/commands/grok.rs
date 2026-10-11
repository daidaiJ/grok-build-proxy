use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

pub struct GrokCommand;

impl SlashCommand for GrokCommand {
    slash_meta! {
        name: "grok",
        description: "Log in or re-authenticate with your Grok account",
        usage: "/grok",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::Login)
    }
}
