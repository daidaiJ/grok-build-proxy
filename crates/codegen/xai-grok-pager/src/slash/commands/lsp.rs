//! LOCAL(/lsp): `/lsp` —— 打开 LSP 统一操作面板（唯一交互面）。
//!
//! 面板内完成：状态显示（门禁来源/工作区覆盖/server 清单/会话后端）、`e` 启用
//! 关闭、`c` 清除本工作区覆盖、server 自动发现清单与空格勾选 + Enter 确认。
//! 命令本身只回 `Action::LspConfigure(Panel)`；cwd 解析、新会话门、grok-home
//! 覆盖文件的写入都在 dispatch 层（`dispatch/router.rs` 的 `dispatch_lsp_configure`），
//! 因为 `CommandExecCtx` 不携带会话 cwd 与回合状态。数据面走 `x.ai/lspStatus`
//! 扩展（异步），渲染在 [`crate::views::lsp_panel`]。

use crate::app::actions::{Action, LspConfigureRequest};
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};

pub struct LspCommand;

impl SlashCommand for LspCommand {
    slash_meta! {
        name: "lsp",
        description: "Open the LSP panel (status, enable/disable, server picker)",
        usage: "/lsp",
        takes_args: false,
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        match args.trim().to_ascii_lowercase().as_str() {
            // 裸 `/lsp`（别名 panel/面板）= 打开统一操作面板。
            "" | "panel" | "面板" => {
                CommandResult::Action(Action::LspConfigure { request: LspConfigureRequest::Panel })
            }
            _ => CommandResult::Error(
                "用法：/lsp —— 打开 LSP 面板，启用/关闭、状态显示与 server 勾选确认都在面板内完成。"
                    .to_string(),
            ),
        }
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
    fn bare_command_opens_panel() {
        let expect = |result: CommandResult| match result {
            CommandResult::Action(Action::LspConfigure { request }) => Some(request),
            _ => None,
        };
        assert_eq!(expect(run_with("")), Some(LspConfigureRequest::Panel));
        assert_eq!(expect(run_with("panel")), Some(LspConfigureRequest::Panel));
        assert_eq!(expect(run_with("面板")), Some(LspConfigureRequest::Panel));
    }

    #[test]
    fn rejects_option_subcommands() {
        // 面板化后不再保留 option 子命令：on/off/status/clear/select 一律拒。
        for args in ["on", "off", "status", "clear", "select", "maybe"] {
            assert!(matches!(run_with(args), CommandResult::Error(_)), "args: {args}");
        }
    }
}
