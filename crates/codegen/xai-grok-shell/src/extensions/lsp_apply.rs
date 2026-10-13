//! `x.ai/lspApply` extension handler — `/lsp on|off` 的零回合热加载执行面。
//!
//! 用户在零回合会话里 `/lsp on|off` 时，pager 写完工作区覆盖后调本方法：shell
//! 重解门禁 → 组装/摘除 LSP 后端 → `SessionCommand::ReloadLspHarness` 重建
//! harness（definition 与 system prompt 不变，复用 model-switch 的零回合重建
//! 簿记，含工具清单重广播）。带历史回合的会话拒绝（改动对新会话生效）。

use std::path::PathBuf;

use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use super::{ExtResult, to_raw_response};
use crate::agent::MvpAgent;

/// 响应体。`ok: false` 时 `error` 说明原因；`ok: true, changed: false` 表示
/// 目标状态与现状一致（或无 server 可装配，见 `note`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspApplyResponse {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub changed: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}

pub async fn handle(agent: &MvpAgent, args: &acp::ExtRequest) -> ExtResult {
    let params: serde_json::Value = serde_json::from_str(args.params.get())
        .map_err(|e| acp::Error::invalid_params().data(e.to_string()))?;
    let cwd: PathBuf = params
        .get("cwd")
        .and_then(|v| v.as_str())
        .ok_or_else(|| acp::Error::invalid_params().data("missing cwd"))?
        .into();
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| acp::Error::invalid_params().data("missing session_id"))?;

    let sid = acp::SessionId::new(session_id);
    let Some(handle) = agent.resident_handle(&sid) else {
        return to_raw_response(&LspApplyResponse {
            ok: false,
            error: Some("会话未就绪（正在加载或非本进程 resident），热加载拒绝".to_string()),
            ..Default::default()
        });
    };
    // 零回合门（与 model switch 的 zero-turn rebuild 同一信号源）。
    let turn_count = handle
        .signals_handle
        .snapshot()
        .await
        .map(|s| s.turn_count)
        .unwrap_or(0);
    if turn_count > 0 {
        return to_raw_response(&LspApplyResponse {
            ok: false,
            error: Some(format!(
                "会话已有 {turn_count} 个回合，热加载跳过；改动对新会话生效"
            )),
            ..Default::default()
        });
    }

    let (enabled, _source, _override) = super::lsp_status::resolve_gate(agent, &cwd);
    let current = handle.tool_context.lsp.is_some();
    let backend = if enabled {
        match agent
            .build_lsp_backend(&cwd, handle.tool_context.process_scope.clone())
            .await
        {
            Some((backend, _names)) => Some(backend),
            None => {
                return to_raw_response(&LspApplyResponse {
                    ok: true,
                    changed: false,
                    enabled,
                    note: Some(
                        "LSP 工具门禁为开，但没有可用的语言服务器（未检测到 builtin，也未配置 lsp.json）"
                            .to_string(),
                    ),
                    ..Default::default()
                });
            }
        }
    } else {
        None
    };

    if backend.is_some() == current {
        return to_raw_response(&LspApplyResponse {
            ok: true,
            changed: false,
            enabled,
            ..Default::default()
        });
    }

    let system_prompt_label = {
        let cfg = agent.cfg.borrow();
        let models = agent.models_manager.models();
        let model = crate::agent::config::find_model_by_id(&models, handle.model_id.0.as_ref());
        crate::util::config::resolve_system_prompt_label(
            &cfg,
            handle.model_id.0.as_ref(),
            model.map(|e| &e.info),
        )
    };
    let (tx, rx) = oneshot::channel();
    let _ = handle.cmd_tx.send(crate::session::SessionCommand::ReloadLspHarness {
        lsp: backend,
        system_prompt_label,
        responds_to: tx,
    });
    match rx.await {
        Ok(Ok(())) => to_raw_response(&LspApplyResponse {
            ok: true,
            changed: true,
            enabled,
            ..Default::default()
        }),
        Ok(Err(e)) => to_raw_response(&LspApplyResponse {
            ok: false,
            error: Some(format!("harness 重建失败: {e}")),
            ..Default::default()
        }),
        Err(_) => to_raw_response(&LspApplyResponse {
            ok: false,
            error: Some("会话 actor 已关闭".to_string()),
            ..Default::default()
        }),
    }
}
