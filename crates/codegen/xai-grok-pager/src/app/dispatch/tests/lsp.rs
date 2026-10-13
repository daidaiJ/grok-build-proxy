//! LOCAL(/lsp): `/lsp` dispatch 门控与消息回显测试。
//!
//! 写入用例触碰进程 grok home（`grok_home()` OnceLock 缓存，测试内无法隔离）；
//! 用 cwd `/tmp` 的夹具键与「写后即清」自我回收，不在真实 home 留下覆盖文件。

use super::*;
use crate::app::actions::LspConfigureRequest;
use xai_grok_shell::extensions::lsp_status::LspStatusResponse;

fn format_status(response: &LspStatusResponse) -> String {
    crate::slash::commands::lsp::format_status(response)
}

#[test]
fn lsp_set_rejected_on_session_with_history() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    test_agent_mut(&mut app, id)
        .session
        .prompt_history
        .push("earlier turn".to_string());

    dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Set(true),
        },
        &mut app,
    );

    let text = last_system_text(&app, id);
    assert!(text.contains("拒绝"), "expected the fresh-session refusal, got: {text}");
    assert!(text.contains("/clear"), "refusal must point at /clear, got: {text}");
}

#[test]
fn lsp_clear_rejected_on_running_turn() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    test_agent_mut(&mut app, id).session.state = AgentState::TurnRunning;

    dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Clear,
        },
        &mut app,
    );

    let text = last_system_text(&app, id);
    assert!(text.contains("拒绝"), "expected the fresh-session refusal, got: {text}");
}

#[test]
fn lsp_set_writes_override_on_fresh_session_and_clear_removes_it() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    // 夹具即全新会话：session_id Some + Idle + 无历史。

    dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Set(true),
        },
        &mut app,
    );
    let text = last_system_text(&app, id);
    assert!(text.contains("已设置"), "expected the set confirmation, got: {text}");
    assert!(text.contains("启用"), "got: {text}");

    // 清除并自回收（覆盖文件删除，grok home 不留状态）。
    dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Clear,
        },
        &mut app,
    );
    let text = last_system_text(&app, id);
    assert!(text.contains("已清除"), "expected the clear confirmation, got: {text}");
}

#[test]
fn lsp_status_emits_fetch_effect_with_session_cwd_and_id() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    let _ = test_agent(&app, id); // 夹具即活动 agent 视图

    let effects = dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Status,
        },
        &mut app,
    );

    assert_eq!(effects.len(), 1, "status must emit exactly one effect");
    assert!(
        matches!(
            &effects[0],
            Effect::FetchLspStatus { session_id: Some(sid), .. } if sid == "test-session"
        ),
        "expected FetchLspStatus carrying the session id"
    );
}

#[test]
fn lsp_status_renders_servers_and_session_state() {
    use xai_grok_shell::extensions::lsp_status::{LspServerStatus, LspSessionStatus};
    let response = LspStatusResponse {        enabled: true,
        enabled_source: "default".to_string(),
        workspace_override: None,
        servers: vec![
            LspServerStatus {
                name: "rust-analyzer".to_string(),
                command: "rust-analyzer".to_string(),
                args: vec![],
                source: "builtin".to_string(),
                command_found: None,
                running: Some(true),
            },
            LspServerStatus {
                name: "gopls".to_string(),
                command: "gopls".to_string(),
                args: vec![],
                source: "user: /home/.grok/lsp.json".to_string(),
                command_found: Some(false),
                running: Some(false),
            },
        ],
        session: Some(LspSessionStatus {
            startup: "ready".to_string(),
            startup_error: None,
            running_servers: vec!["rust-analyzer".to_string()],
        }),
        error: None,
    };

    let text = crate::slash::commands::lsp::format_status(&response);
    assert!(text.contains("LSP 工具：启用（来源：默认开）"), "got: {text}");
    assert!(text.contains("本工作区覆盖：未设置"), "got: {text}");
    assert!(
        text.contains("rust-analyzer — builtin — rust-analyzer — 运行中"),
        "got: {text}"
    );
    assert!(
        text.contains("gopls — user: /home/.grok/lsp.json — gopls（命令未找到） — 未运行"),
        "got: {text}"
    );
    assert!(text.contains("本会话后端：就绪"), "got: {text}");
}

#[test]
fn lsp_status_renders_error_and_empty_cases() {
    let error = LspStatusResponse {
        error: Some("boom".to_string()),
        ..Default::default()
    };
    assert!(format_status(&error).contains("LSP 状态查询失败：boom"));

    let empty = LspStatusResponse {
        enabled: false,
        enabled_source: "workspace_override".to_string(),
        workspace_override: Some(false),
        ..Default::default()
    };
    let text = format_status(&empty);
    assert!(text.contains("LSP 工具：禁用（来源：/lsp 工作区覆盖）"), "got: {text}");
    assert!(text.contains("本工作区覆盖：off"), "got: {text}");
    assert!(text.contains("语言服务器：无"), "got: {text}");
    assert!(text.contains("当前无活动会话或本会话未装配 LSP 后端。"), "got: {text}");
}
