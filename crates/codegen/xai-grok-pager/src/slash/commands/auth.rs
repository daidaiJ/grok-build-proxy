use crate::app::actions::Action;
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

/// LOCAL: /auth 供应商配置面板（BYOK 五步向导；/grok 保持为 xAI 账号登录）。
pub struct AuthCommand;

impl SlashCommand for AuthCommand {
    slash_meta! {
        name: "auth",
        description: "Configure a model provider (base URL, API key, models)",
        usage: "/auth",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, _args: &str) -> CommandResult {
        CommandResult::Action(Action::OpenAuthPanel)
    }
}
