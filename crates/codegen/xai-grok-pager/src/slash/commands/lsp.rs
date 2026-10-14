//! LOCAL(/lsp): `/lsp on|off|status|clear` — workspace 级 LSP 工具启停。
//!
//! 命令本身只解析参数并回 `Action::LspConfigure`；cwd 解析、新会话门、grok-home
//! 覆盖文件的写入都在 dispatch 层（`dispatch/router.rs` 的 `dispatch_lsp_configure`），
//! 因为 `CommandExecCtx` 不携带会话 cwd 与回合状态。生效点在 session spawn 的
//! agent definition 构建（shell `resolve_lsp_tools_for_cwd`），对本会话不生效。
//! status 的数据面走 `x.ai/lspStatus` 扩展（异步），渲染在 [`format_status`]。

use crate::app::actions::{Action, LspConfigureRequest};
use crate::slash::command::{CommandExecCtx, CommandResult, SlashCommand, slash_meta};
use xai_grok_shell::extensions::lsp_status::LspStatusResponse;

pub struct LspCommand;

impl SlashCommand for LspCommand {
    slash_meta! {
        name: "lsp",
        description: "Toggle LSP tools for this workspace",
        usage: "/lsp [on|off|select|status|clear]",
        takes_args: true,
        arg_placeholder: "on|off|select|status|clear",
    }

    fn run(&self, _ctx: &mut CommandExecCtx, args: &str) -> CommandResult {
        let request = match args.trim().to_ascii_lowercase().as_str() {
            "" | "status" => LspConfigureRequest::Status,
            "on" | "开启" => LspConfigureRequest::Set(true),
            "off" | "关闭" => LspConfigureRequest::Set(false),
            "clear" | "reset" | "auto" => LspConfigureRequest::Clear,
            "select" | "选择" | "勾选" | "勾选面板" => LspConfigureRequest::Select,
            other => {
                return CommandResult::Error(format!(
                    "用法：/lsp [on|off|select|status|clear]（未知参数 “{other}”；select = 勾选本工作区启用的 server；clear = 删除本工作区覆盖，恢复跟随全局配置）"
                ));
            }
        };
        CommandResult::Action(Action::LspConfigure { request })
    }
}

/// `/lsp status` 回包 → 展示文本。门禁一行、覆盖一行、server 清单（含来源与
/// 运行态）、会话后端状态。空清单给出可执行的下一步（装 server 或写 lsp.json）。
pub(crate) fn format_status(response: &LspStatusResponse) -> String {
    if let Some(error) = &response.error {
        return format!("LSP 状态查询失败：{error}");
    }
    let mut lines = Vec::new();

    let source_label = match response.enabled_source.as_str() {
        "pin" => "策略 pin",
        "env" => "环境变量 GROK_LSP_TOOLS",
        "workspace_override" => "/lsp 工作区覆盖",
        "config" => "[features] lsp_tools",
        "remote" => "服务端设置",
        _ => "默认开",
    };
    lines.push(format!(
        "LSP 工具：{}（来源：{source_label}）",
        if response.enabled { "启用" } else { "禁用" }
    ));
    match response.workspace_override {
        Some(true) => lines.push("本工作区覆盖：on（下次会话启用 LSP 工具）".to_string()),
        Some(false) => lines.push("本工作区覆盖：off（下次会话禁用 LSP 工具）".to_string()),
        None => lines.push("本工作区覆盖：未设置（跟随全局）".to_string()),
    }

    if response.servers.is_empty() {
        lines.push("语言服务器：无".to_string());
        lines.push(
            "  安装 rust-analyzer / gopls / pyright / jdtls / clangd 会自动接入；也可在 ~/.grok/lsp.json 配置。"
                .to_string(),
        );
    } else {
        lines.push(format!("语言服务器（{}）：", response.servers.len()));
        for s in &response.servers {
            let mut line = format!("  {} — {} — {}", s.name, s.source, s.command);
            if s.command_found == Some(false) {
                line.push_str("（命令未找到）");
            }
            if s.disabled {
                line.push_str("（已禁用 — /lsp select 勾选恢复）");
            }
            if let Some(running) = s.running {
                line.push_str(if running { " — 运行中" } else { " — 未运行" });
            }
            lines.push(line);
        }
    }

    match &response.session {
        Some(session) => {
            let startup_label = match session.startup.as_str() {
                "not_started" => "未启动",
                "starting" => "启动中",
                "ready" => "就绪",
                "failed" => "失败",
                _ => "未知",
            };
            let mut line = format!("本会话后端：{startup_label}");
            if let Some(error) = &session.startup_error {
                line.push_str(&format!("（{error}）"));
            }
            lines.push(line);
        }
        None => lines.push("当前无活动会话或本会话未装配 LSP 后端。".to_string()),
    }
    lines.push(
        "用 /lsp select 勾选本工作区要启用的 server（取消勾选的将被禁用，配置/自动检测到的都会列出）。"
            .to_string(),
    );
    lines.push(
        "/lsp 的改动立即热加载到零回合会话；已有回合的会话对新会话生效。".to_string(),
    );
    lines.join("\n")
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
        assert_eq!(expect(run_with("select")), Some(LspConfigureRequest::Select));
        assert_eq!(expect(run_with("选择")), Some(LspConfigureRequest::Select));
    }

    #[test]
    fn rejects_unknown_args() {
        assert!(matches!(run_with("maybe"), CommandResult::Error(_)));
    }
}
