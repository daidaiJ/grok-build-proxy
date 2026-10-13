//! `x.ai/lspStatus` extension handler — `/lsp status` 的数据面。
//!
//! 一次回包给全：门禁判定（含来源层级）、本工作区覆盖、生效 server 清单
//! （config + builtin 检测，带来源与命令探测结果），以及活动会话的 LSP 后端
//! 快照（启动阶段 + 各 server 运行态）。会话快照走 `resident_handle` 直读
//! `tool_context.lsp`，不动会话命令循环。

use std::path::{Path, PathBuf};

use agent_client_protocol as acp;
use serde::{Deserialize, Serialize};

use super::{ExtResult, to_raw_response};
use crate::agent::MvpAgent;
use xai_grok_tools::implementations::lsp::LspStartupState;

/// 门禁判定（与 `resolve_lsp_tools_for_cwd` 同序）+ 来源标签 + 工作区覆盖。
/// `lsp_status` 与 `lsp_apply` 共用，保证展示面与执行面读到同一份真相。
pub(super) fn resolve_gate(
    agent: &MvpAgent,
    cwd: &Path,
) -> (bool, &'static str, Option<bool>) {
    let cfg = agent.cfg.borrow();
    let sources = cfg.feature_sources(crate::agent::config::Feature::LspTools);
    let workspace_override = crate::util::workspace_overrides::read_workspace_override(
        cwd,
        crate::util::workspace_overrides::LSP_TOOLS_OVERRIDE_KEY,
    );
    let (enabled, source) = if let Some(pin) = sources.pin.or(sources.env) {
        (
            pin,
            if sources.pin.is_some() { "pin" } else { "env" },
        )
    } else if let Some(value) = workspace_override {
        (value, "workspace_override")
    } else if let Some(value) = sources.config {
        (value, "config")
    } else if let Some(value) = sources.remote {
        (value, "remote")
    } else {
        (true, "default")
    };
    drop(cfg);
    (enabled, source, workspace_override)
}

/// 响应体。`session: None` = 没有可查询的活动会话或该会话未装配 LSP 后端。
/// `error` 只由 pager 侧解析/发送失败填充（错误折进回包体，状态块总能落盘）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspStatusResponse {
    #[serde(default)]
    pub enabled: bool,
    /// "pin" | "env" | "workspace_override" | "config" | "remote" | "default"
    #[serde(default)]
    pub enabled_source: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub workspace_override: Option<bool>,
    #[serde(default)]
    pub servers: Vec<LspServerStatus>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub session: Option<LspSessionStatus>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspServerStatus {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    /// `ConfigSource::display_label()`（builtin / user: … / project: … / plugin: …）。
    pub source: String,
    /// 裸命令名的 PATH 探测结果；路径形态的 command 不探测，保持 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_found: Option<bool>,
    /// 会话后端快照里的运行态；无会话可查时 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspSessionStatus {
    /// "not_started" | "starting" | "ready" | "failed" | "unknown"
    pub startup: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_error: Option<String>,
    pub running_servers: Vec<String>,
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
        .map(str::to_owned);

    // ── 门禁：来源标签见 resolve_gate ──
    let (enabled, enabled_source, workspace_override) = resolve_gate(agent, &cwd);

    // ── server 清单：config + builtin 检测，folder-trust 只影响 project 条目 ──
    // 插件面与 session spawn 同源：launch-dir 共享快照（per-cwd 新建会偏离 spawn 实际生效面）。
    let registry = agent.plugin_registry_for_cwd(None).await;
    let active: Vec<_> = registry
        .as_ref()
        .map(|r| r.active_plugins())
        .unwrap_or_default();
    let (plugin_lsp_paths, plugin_names): (Vec<PathBuf>, Vec<&str>) = active
        .iter()
        .filter_map(|p| p.lsp_config_path.clone().map(|path| (path, p.name.as_str())))
        .unzip();
    let (plugin_inline_lsp, inline_names): (Vec<&serde_json::Value>, Vec<&str>) = active
        .iter()
        .filter_map(|p| p.inline_lsp_servers.as_ref().map(|v| (v, p.name.as_str())))
        .unzip();
    let sourced = xai_grok_tools::implementations::lsp::config::load_servers_with_builtins_sourced(
        &cwd,
        &plugin_lsp_paths,
        &plugin_inline_lsp,
        &plugin_names,
        &inline_names,
    );
    let project_allowed = crate::agent::folder_trust::project_scope_allowed(&cwd);

    // 会话后端快照（cfg borrow 已释放，可安全 await）。
    let session_backend = match &session_id {
        Some(sid) => agent
            .resident_handle(&acp::SessionId::new(sid.as_str()))
            .and_then(|h| h.tool_context.lsp.clone()),
        None => None,
    };
    let snapshot = match &session_backend {
        Some(backend) => backend.status_snapshot().await,
        None => None,
    };

    let servers = sourced
        .into_iter()
        .filter(|(_, (_, source))| {
            project_allowed || !matches!(source, xai_grok_tools::types::config_source::ConfigSource::Project { .. })
        })
        .map(|(name, (cfg, source))| {
            let command_found = if cfg.command.contains('/') || cfg.command.contains('\\') {
                None
            } else {
                let dirs: Vec<PathBuf> = std::env::var_os("PATH")
                    .map(|p| std::env::split_paths(&p).collect())
                    .unwrap_or_default();
                Some(
                    xai_grok_tools::implementations::lsp::builtin::probe_command(
                        &dirs, &cfg.command,
                    )
                    .is_some(),
                )
            };
            LspServerStatus {
                running: snapshot
                    .as_ref()
                    .map(|snap| snap.running_servers.contains(&name)),
                name,
                command: cfg.command,
                args: cfg.args,
                source: source.display_label(),
                command_found,
            }
        })
        .collect();

    let session = snapshot.map(|snap| {
        let (startup, startup_error) = match snap.startup {
            Some(LspStartupState::NotStarted) => ("not_started", None),
            Some(LspStartupState::Starting) => ("starting", None),
            Some(LspStartupState::Ready) => ("ready", None),
            Some(LspStartupState::Failed(error)) => ("failed", Some(error)),
            None => ("unknown", None),
        };
        LspSessionStatus {
            startup: startup.to_string(),
            startup_error,
            running_servers: snap.running_servers,
        }
    });

    to_raw_response(&LspStatusResponse {
        enabled,
        enabled_source: enabled_source.to_string(),
        workspace_override,
        servers,
        session,
        ..Default::default()
    })
}
