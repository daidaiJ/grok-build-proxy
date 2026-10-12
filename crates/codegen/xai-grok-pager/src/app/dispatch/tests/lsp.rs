//! LOCAL(/lsp): `/lsp` dispatch 门控与消息回显测试。
//!
//! 写入用例触碰进程 grok home（`grok_home()` OnceLock 缓存，测试内无法隔离）；
//! 用 cwd `/tmp` 的夹具键与「写后即清」自我回收，不在真实 home 留下覆盖文件。

use super::*;
use crate::app::actions::LspConfigureRequest;

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
fn lsp_status_reports_registration_and_override() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    test_agent_mut(&mut app, id).session.available_tools =
        Some(std::iter::once("lsp".to_string()).collect());

    dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Status,
        },
        &mut app,
    );

    let text = last_system_text(&app, id);
    assert!(text.contains("LSP 工具已注册"), "got: {text}");
    assert!(text.contains("未设置"), "got: {text}");
    assert!(text.contains("生效点为新会话"), "got: {text}");
}
