//! LOCAL(/lsp): `/lsp` 面板 dispatch 门控与消息回显测试。
//!
//! 写入用例触碰进程 grok home（`grok_home()` OnceLock 缓存，测试内无法隔离）；
//! 用 cwd `/tmp` 的夹具键与「写后即清」自我回收，不在真实 home 留下覆盖文件。

use super::*;
use crate::app::actions::LspConfigureRequest;

#[test]
fn lsp_set_on_session_with_history_writes_and_applies() {
    let mut app = test_app_with_agent();
    let id = AgentId(0);
    test_agent_mut(&mut app, id)
        .session
        .prompt_history
        .push("earlier turn".to_string());

    let effects = dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Set(true),
        },
        &mut app,
    );

    // prompt_history 是跨会话召回历史，不是回合数——写入恒放行，热加载由
    // shell 侧 turn_count 权威裁决。
    let text = last_system_text(&app, id);
    assert!(text.contains("已设置"), "expected the set confirmation, got: {text}");
    assert!(
        effects.iter().any(
            |e| matches!(e, Effect::ApplyLspTools { session_id: sid, .. } if sid == "test-session")
        ),
        "expected ApplyLspTools for the live session, got: {effects:?}"
    );

    // 写后自回收（覆盖文件删除，grok home 不留状态）。
    dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Clear,
        },
        &mut app,
    );
}

#[test]
fn lsp_clear_on_running_turn_still_writes() {
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
    assert!(text.contains("已清除"), "expected the clear confirmation, got: {text}");
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
    assert!(text.contains("已清除"), "got: {text}");
}

#[test]
fn lsp_panel_emits_fetch_effect_with_session_cwd_and_id() {
    let mut app = test_app_with_agent();
    let _ = test_agent(&app, AgentId(0)); // 夹具即活动 agent 视图

    let effects = dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Panel,
        },
        &mut app,
    );

    assert_eq!(effects.len(), 1, "panel must emit exactly one effect");
    assert!(
        matches!(
            &effects[0],
            Effect::FetchLspStatus { session_id: Some(sid), .. } if sid == "test-session"
        ),
        "expected FetchLspStatus carrying the session id"
    );
}

#[test]
fn lsp_panel_without_agent_session_reports_hint() {
    let mut app = test_app(); // 无 agent 视图（welcome/dashboard）
    let effects = dispatch(
        Action::LspConfigure {
            request: LspConfigureRequest::Panel,
        },
        &mut app,
    );
    assert!(effects.is_empty(), "no effect outside a session, got: {effects:?}");
    assert!(
        app.welcome_toast.as_ref().is_some_and(|(m, _)| m.contains("/lsp 面板")),
        "expected an actionable hint toast, got: {:?}",
        app.welcome_toast
    );
}

#[test]
fn lsp_status_fetch_opens_modal_with_checked_state() {
    use xai_grok_shell::extensions::lsp_status::LspServerStatus;
    let mut app = test_app_with_agent();
    let response = xai_grok_shell::extensions::lsp_status::LspStatusResponse {
        enabled: true,
        enabled_source: "default".to_string(),
        workspace_override: None,
        servers: vec![
            LspServerStatus {
                name: "rust-analyzer".to_string(),
                command: "rust-analyzer".to_string(),
                args: vec![],
                source: "builtin".to_string(),
                command_found: Some(true),
                disabled: false,
                running: None,
            },
            LspServerStatus {
                name: "jdtls".to_string(),
                command: "jdtls".to_string(),
                args: vec![],
                source: "builtin".to_string(),
                command_found: Some(false),
                disabled: true,
                running: None,
            },
        ],
        session: None,
        error: None,
    };

    let effects = dispatch(
        Action::TaskComplete(TaskResult::LspStatusFetched {
            cwd: std::path::PathBuf::from("/repo"),
            session_id: Some("test-session".to_string()),
            response,
        }),
        &mut app,
    );
    assert!(effects.is_empty(), "panel open has no effects, got: {effects:?}");
    let agent = test_agent(&app, AgentId(0));
    assert!(
        matches!(
            agent.active_modal,
            Some(crate::views::modal::ActiveModal::LspPanel { .. })
        ),
        "expected the LSP panel modal to open"
    );
}

#[test]
fn lsp_servers_confirm_writes_deny_list_and_applies_then_clears() {
    let mut app = test_app_with_agent();
    let cwd = std::path::PathBuf::from("/tmp");

    let effects = dispatch(
        Action::LspServersConfirm {
            cwd: cwd.clone(),
            session_id: Some("test-session".to_string()),
            disabled: vec!["jdtls".to_string(), "clangd".to_string()],
        },
        &mut app,
    );
    assert!(
        effects.iter().any(
            |e| matches!(e, Effect::ApplyLspTools { session_id: sid, .. } if sid == "test-session")
        ),
        "expected ApplyLspTools hot reload, got: {effects:?}"
    );
    let text = last_system_text(&app, AgentId(0));
    assert!(text.contains("已保存"), "got: {text}");
    assert!(text.contains("jdtls"), "deny list should be echoed, got: {text}");

    // 空确认 = 清除覆盖（与面板 `c` 重置同款「写后即清」自回收）。
    dispatch(
        Action::LspServersConfirm {
            cwd,
            session_id: None,
            disabled: vec![],
        },
        &mut app,
    );
    let text = last_system_text(&app, AgentId(0));
    assert!(text.contains("已清除"), "got: {text}");
}
