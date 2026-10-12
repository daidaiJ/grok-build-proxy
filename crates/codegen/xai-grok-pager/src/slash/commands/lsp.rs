//! LOCAL(/lsp): `/lsp on|off|status|clear` — workspace 级 LSP 工具启停。
//!
//! 命令本身只解析参数并回 `Action::LspConfigure`；cwd 解析、新会话门、grok-home
//! 覆盖文件的写入都在 dispatch 层（`dispatch/router.rs` 的 `dispatch_lsp_configure`），
//! 因为 `CommandExecCtx` 不携带会话 cwd 与回合状态。生效点在 session spawn 的
//! agent definition 构建（shell `resolve_lsp_tools_for_cwd`），对本会话不生效。

use crate::app::actions::{Action, LspConfigureRequest};
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

pub struct LspCommand;

impl SlashCommand for LspCommand {
    slash_meta! {
        name: "lsp",
        description: "Toggle LSP tools for this workspace",
        usage: "/lsp [on|off|status|clear]",
        takes_args: true,
        arg_placeholder: "on|off|status|clear",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let request = match args.trim().to_ascii_lowercase().as_str() {
            "" | "status" => LspConfigureRequest::Status,
            "on" | "开启" => LspConfigureRequest::Set(true),
            "off" | "关闭" => LspConfigureRequest::Set(false),
            "clear" | "reset" | "auto" => LspConfigureRequest::Clear,
            other => {
                return CommandResult::Error(format!(
                    "用法：/lsp [on|off|status|clear]（未知参数 “{other}”；clear = 删除本工作区覆盖，恢复跟随全局配置）"
                ));
            }
        };
        CommandResult::Action(Action::LspConfigure { request })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::model_state::ModelState;
    use crate::app::bundle::BundleState;
    use crate::settings::PagerLocalSnapshot;

    fn run_with(args: &str) -> CommandResult {
        let models = ModelState::default();
        let bundle = BundleState::default();
        let mut ctx = CommandExecCtx {
            models: &models,
            session_id: None,
            bundle_state: &bundle,
            screen_mode: crate::app::ScreenMode::Minimal,
            billing_surface_visible: true,
            usage_command_visible: true,
            pager_state: PagerLocalSnapshot::default(),
        };
        LspCommand.run(&mut ctx, args)
    }

    #[test]
    fn parses_on_off_status_clear() {
        let expect = |result: CommandResult| match result {
            CommandResult::Action(Action::LspConfigure { request }) => Some(request),
            _ => None,
        };
        assert_eq!(expect(run_with("on")), Some(LspConfigureRequest::Set(true)));
        assert_eq!(expect(run_with("off")), Some(LspConfigureRequest::Set(false)));
        assert_eq!(expect(run_with("status")), Some(LspConfigureRequest::Status));
        // 裸 `/lsp` 也是 status 查询。
        assert_eq!(expect(run_with("")), Some(LspConfigureRequest::Status));
        assert_eq!(expect(run_with("clear")), Some(LspConfigureRequest::Clear));
    }

    #[test]
    fn rejects_unknown_args() {
        assert!(matches!(run_with("maybe"), CommandResult::Error(_)));
    }
}
