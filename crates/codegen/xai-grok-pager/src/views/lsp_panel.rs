//! LOCAL(/lsp): LSP 统一操作面板 —— `/lsp` 的唯一交互面：状态显示（门禁来源 /
//! 工作区覆盖 / 会话后端）、启用关闭（`e` 翻转，写 workspace 覆盖并热加载，
//! 面板保持打开）、覆盖重置（`c` 清除本工作区覆盖恢复跟随全局）、server 自动
//! 发现清单（config + builtin PATH 检测，启用时自动接入）与手动勾选确认
//! （空格勾选，Enter 确认写本工作区禁用清单并热加载）。
//! 窗口 chrome 走 [`crate::views::modal_window`]（与 usage/stats modal 同款
//! 尺寸档位与页脚快捷键），Esc/关闭按钮由 chrome 承担。

use std::collections::BTreeSet;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::prelude::{Buffer, Line, Modifier, Span, Style};
use ratatui::widgets::{Paragraph, Widget};

use crate::slash::i18n::tr;
use crate::theme::Theme;
use crate::views::modal_window::{
    self as mw, ModalSizing, ModalWindowConfig, ModalWindowState, Shortcut,
};
use xai_grok_shell::extensions::lsp_status::{LspServerStatus, LspStatusResponse};

/// 门禁来源标签（面板状态头渲染用）。
pub(crate) fn enabled_source_label(source: &str) -> &'static str {
    match source {
        "pin" => "策略 pin",
        "env" => "环境变量 GROK_LSP_TOOLS",
        "workspace_override" => "/lsp 工作区覆盖",
        "config" => "[features] lsp_tools",
        "remote" => "服务端设置",
        _ => "默认开",
    }
}

/// 会话后端启动阶段标签（面板会话行渲染用）。
pub(crate) fn startup_label(startup: &str) -> &'static str {
    match startup {
        "not_started" => "未启动",
        "starting" => "启动中",
        "ready" => "就绪",
        "failed" => "失败",
        _ => "未知",
    }
}

pub struct LspPanelState {
    /// 确认/翻转时写覆盖文件用的目标工作区与活动会话（打开面板时的快照）。
    pub cwd: PathBuf,
    pub session_id: Option<String>,
    /// 打开面板时的门禁快照（`x.ai/lspStatus` 回包）。
    pub enabled: bool,
    pub enabled_source: String,
    pub workspace_override: Option<bool>,
    /// 会话后端快照（None = 无活动会话或未装配 LSP 后端）。
    pub session_startup: Option<String>,
    pub session_startup_error: Option<String>,
    /// 候选清单：config + builtin 检测（lspStatus 回包原样）。
    pub servers: Vec<LspServerStatus>,
    /// 当前勾选（= 要启用的）；初始值 = 打开面板时的生效面（未被禁用的）。
    pub checked: BTreeSet<String>,
    pub cursor: usize,
    /// 公共 chrome 状态（关闭按钮命中区 / 快捷键 hover 等）。
    pub window: ModalWindowState,
    /// 内容滚动偏移（清单超过内容区时跟随光标）。
    pub scroll: u16,
}

impl LspPanelState {
    pub fn new(cwd: PathBuf, session_id: Option<String>, response: LspStatusResponse) -> Self {
        let LspStatusResponse {
            enabled,
            enabled_source,
            workspace_override,
            servers,
            session,
            ..
        } = response;
        let session_startup = session.as_ref().map(|s| s.startup.clone());
        let session_startup_error = session.as_ref().and_then(|s| s.startup_error.clone());
        let checked = servers
            .iter()
            .filter(|s| !s.disabled)
            .map(|s| s.name.clone())
            .collect();
        Self {
            cwd,
            session_id,
            enabled,
            enabled_source,
            workspace_override,
            session_startup,
            session_startup_error,
            servers,
            checked,
            cursor: 0,
            window: ModalWindowState::new(),
            scroll: 0,
        }
    }

    pub fn move_up(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.servers.len() {
            self.cursor += 1;
        }
    }

    pub fn toggle(&mut self) {
        if let Some(server) = self.servers.get(self.cursor) {
            if !self.checked.remove(&server.name) {
                self.checked.insert(server.name.clone());
            }
        }
    }

    /// 全选（有任何未勾选时）或全不选（全部已勾选时）。
    pub fn toggle_all(&mut self) {
        if self.checked.len() == self.servers.len() {
            self.checked.clear();
        } else {
            self.checked = self.servers.iter().map(|s| s.name.clone()).collect();
        }
    }

    /// `e`：启用/关闭本地翻转（工作区覆盖随新值显示）。写盘与热加载由
    /// `Action::LspConfigure(Set)` 在 dispatch 层完成，面板保持打开。
    pub fn toggle_enabled(&mut self) -> bool {
        self.enabled = !self.enabled;
        self.workspace_override = Some(self.enabled);
        self.enabled
    }

    /// `c`：清除本工作区覆盖（显示面即时回到「跟随全局」；写盘与热加载在
    /// dispatch 层 `Action::LspConfigure(Clear)`，面板保持打开）。
    pub fn clear_overrides(&mut self) {
        self.workspace_override = None;
    }

    /// 确认语义：返回要禁用的名单（全集 − 勾选集）。空 = 全部启用（清除覆盖）。
    pub fn disabled_on_confirm(&self) -> Vec<String> {
        self.servers
            .iter()
            .filter(|s| !self.checked.contains(&s.name))
            .map(|s| s.name.clone())
            .collect()
    }

    /// 清单区首行的内容行号（状态头 + 可选禁用提示 + 空行之后）。
    fn list_start(&self) -> usize {
        if self.enabled {
            3
        } else {
            4
        }
    }
}

pub enum LspPanelOutcome {
    /// 勾选/移动等状态变化，需要重绘。
    Changed,
    /// `e` —— 启用/关闭翻转（面板本地态已更新），调用方发
    /// `Action::LspConfigure(Set)` 立即写覆盖并热加载；面板保持打开。
    ToggleEnabled(bool),
    /// `c` —— 清除本工作区覆盖（面板本地态已回「跟随全局」），调用方发
    /// `Action::LspConfigure(Clear)`；面板保持打开。
    ClearOverrides,
    /// Esc —— 关闭面板，不落任何配置。
    Cancel,
    /// Enter —— 按 [`LspPanelState::disabled_on_confirm`] 语义提交。
    Confirm,
}

pub fn route_lsp_panel_key(state: &mut LspPanelState, key: &KeyEvent) -> LspPanelOutcome {
    match key.code {
        KeyCode::Esc => LspPanelOutcome::Cancel,
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_up();
            LspPanelOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_down();
            LspPanelOutcome::Changed
        }
        KeyCode::Char(' ') => {
            state.toggle();
            LspPanelOutcome::Changed
        }
        KeyCode::Char('a') => {
            state.toggle_all();
            LspPanelOutcome::Changed
        }
        KeyCode::Char('e') | KeyCode::Char('t') => {
            LspPanelOutcome::ToggleEnabled(state.toggle_enabled())
        }
        KeyCode::Char('c') => {
            state.clear_overrides();
            LspPanelOutcome::ClearOverrides
        }
        KeyCode::Enter | KeyCode::Char('s') => LspPanelOutcome::Confirm,
        _ => LspPanelOutcome::Changed,
    }
}

pub fn render_lsp_panel(
    buf: &mut Buffer,
    area: Rect,
    state: &mut LspPanelState,
    compact: bool,
    theme: &Theme,
) {
    let shortcuts: Vec<Shortcut> = [
        "e enable/disable",
        "c reset overrides",
        "\u{2191}/\u{2193} move",
        "space toggle",
        "a toggle all",
        "Enter confirm",
        "Esc close",
    ]
    .iter()
    .map(|label| Shortcut {
        label,
        clickable: false,
        id: 0,
    })
    .collect();

    // 与 usage/stats modal 同尺寸档位（窄内容但行数随 server 数增长）。
    let sizing = ModalSizing {
        width_pct: 0.65,
        max_width: 100,
        min_width: 44,
        v_margin: 2,
        h_pad: 2,
        v_pad: 2,
        footer_lines: 3,
    }
    .with_compact(compact);
    let config = ModalWindowConfig {
        title: tr("LSP"),
        tabs: None,
        shortcuts: &shortcuts,
        sizing,
        fold_info: None,
    };

    let Some(mca) = mw::render_modal_window(buf, area, &mut state.window, &config, theme) else {
        return;
    };
    let content = mca.content;

    let lines = panel_lines(state, theme);
    let max_scroll = lines.len().saturating_sub(content.height as usize);
    // 光标行跟随滚动（清单超长时不被滚出视野）。
    let cursor_row = state.list_start() + state.cursor;
    if cursor_row < state.scroll as usize {
        state.scroll = cursor_row as u16;
    } else if cursor_row >= state.scroll as usize + content.height as usize {
        state.scroll = (cursor_row + 1 - content.height as usize) as u16;
    }
    state.scroll = state.scroll.min(max_scroll as u16);

    let visible: Vec<Line> = lines
        .into_iter()
        .skip(state.scroll as usize)
        .take(content.height as usize)
        .collect();
    Paragraph::new(visible).render(content, buf);
}

fn panel_lines(state: &LspPanelState, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // 状态头：门禁 + 来源 + 工作区覆盖。
    let state_label = if state.enabled {
        tr("enabled")
    } else {
        tr("disabled")
    };
    lines.push(Line::from(Span::styled(
        format!(
            "{}：{}（{}：{}）",
            tr("LSP tools"),
            state_label,
            tr("source"),
            enabled_source_label(&state.enabled_source)
        ),
        Style::new().fg(theme.text_primary).add_modifier(Modifier::BOLD),
    )));
    let override_text = match state.workspace_override {
        Some(true) => tr("Workspace override: on (LSP tools enabled for next sessions)").to_string(),
        Some(false) => {
            tr("Workspace override: off (LSP tools disabled for next sessions)").to_string()
        }
        None => tr("Workspace override: not set (follows global config)").to_string(),
    };
    lines.push(Line::from(Span::styled(
        override_text,
        Style::new().fg(theme.gray_bright),
    )));
    if !state.enabled {
        lines.push(Line::from(Span::styled(
            tr("Disabled — press e to enable; installed servers are auto-detected.").to_string(),
            Style::new().fg(theme.warning),
        )));
    }
    lines.push(Line::from(""));

    // server 清单（config + builtin 自动发现）。
    if state.servers.is_empty() {
        lines.push(Line::from(tr(
            "No language servers detected or configured.",
        )));
    } else {
        for (index, server) in state.servers.iter().enumerate() {
            let marker = if state.checked.contains(&server.name) {
                "[x]"
            } else {
                "[ ]"
            };
            let mut row = format!("{marker} {} — {} — {}", server.name, server.source, server.command);
            if server.command_found == Some(false) {
                row.push(' ');
                row.push_str(tr("(command not found)"));
            }
            if let Some(running) = server.running {
                let label = if running { tr("running") } else { tr("not running") };
                row.push_str(&format!(" — {label}"));
            }
            let is_cursor = index == state.cursor;
            let style = if is_cursor {
                Style::new().fg(theme.text_primary).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme.text_secondary)
            };
            let cursor_prefix = if is_cursor { "> " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(cursor_prefix.to_string(), Style::new().fg(theme.selection_border)),
                Span::styled(row, style),
            ]));
        }
    }

    lines.push(Line::from(""));
    // 会话后端快照。
    match &state.session_startup {
        Some(startup) => {
            let mut line = format!("{}：{}", tr("Session backend"), startup_label(startup));
            if let Some(error) = &state.session_startup_error {
                line.push_str(&format!("（{error}）"));
            }
            lines.push(Line::from(Span::styled(
                line,
                Style::new().fg(theme.text_secondary),
            )));
        }
        None => lines.push(Line::from(Span::styled(
            tr("No active session or no LSP backend in this session.").to_string(),
            Style::new().fg(theme.text_secondary),
        ))),
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;
    use xai_grok_shell::extensions::lsp_status::{LspServerStatus, LspSessionStatus};

    fn server(name: &str, disabled: bool) -> LspServerStatus {
        LspServerStatus {
            name: name.to_string(),
            command: format!("{name}-cmd"),
            args: vec![],
            source: "builtin".to_string(),
            command_found: Some(true),
            disabled,
            running: None,
        }
    }

    fn response(servers: Vec<LspServerStatus>, enabled: bool) -> LspStatusResponse {
        LspStatusResponse {
            enabled,
            enabled_source: "default".to_string(),
            workspace_override: None,
            servers,
            session: Some(LspSessionStatus {
                startup: "ready".to_string(),
                startup_error: None,
                running_servers: vec![],
            }),
            error: None,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: crossterm::event::KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn initial_state_follows_response_and_enabled_set() {
        let state = LspPanelState::new(
            PathBuf::from("/repo"),
            None,
            response(
                vec![server("rust-analyzer", false), server("jdtls", true)],
                true,
            ),
        );
        assert!(state.enabled);
        assert!(state.checked.contains("rust-analyzer"));
        assert!(!state.checked.contains("jdtls"));
        assert_eq!(state.disabled_on_confirm(), vec!["jdtls".to_string()]);
        assert_eq!(state.session_startup.as_deref(), Some("ready"));
    }

    #[test]
    fn toggle_enabled_flips_local_state_and_keeps_panel_semantics() {
        let mut state = LspPanelState::new(PathBuf::from("/repo"), None, response(vec![], false));
        assert!(matches!(
            route_lsp_panel_key(&mut state, &key(KeyCode::Char('e'))),
            LspPanelOutcome::ToggleEnabled(true)
        ));
        assert!(state.enabled);
        assert_eq!(state.workspace_override, Some(true));
        // 关闭再翻转一次。
        assert!(matches!(
            route_lsp_panel_key(&mut state, &key(KeyCode::Char('t'))),
            LspPanelOutcome::ToggleEnabled(false)
        ));
        assert!(!state.enabled);
    }

    #[test]
    fn clear_overrides_outcome_resets_local_view() {
        let mut state =
            LspPanelState::new(PathBuf::from("/repo"), None, response(vec![], false));
        assert!(matches!(
            route_lsp_panel_key(&mut state, &key(KeyCode::Char('e'))),
            LspPanelOutcome::ToggleEnabled(true)
        ));
        assert_eq!(state.workspace_override, Some(true));
        assert!(matches!(
            route_lsp_panel_key(&mut state, &key(KeyCode::Char('c'))),
            LspPanelOutcome::ClearOverrides
        ));
        assert_eq!(state.workspace_override, None);
    }

    #[test]
    fn toggle_and_toggle_all() {
        let mut state = LspPanelState::new(
            PathBuf::from("/repo"),
            None,
            response(vec![server("a", false), server("b", false)], true),
        );
        assert!(matches!(
            route_lsp_panel_key(&mut state, &key(KeyCode::Char(' '))),
            LspPanelOutcome::Changed
        ));
        assert!(!state.checked.contains("a"), "cursor row 0 toggled off");
        state.move_down();
        assert!(route_lsp_panel_key(&mut state, &key(KeyCode::Down)).is_changed());
        assert!(route_lsp_panel_key(&mut state, &key(KeyCode::Char(' '))).is_changed());
        assert!(!state.checked.contains("b"));
        assert_eq!(
            state.disabled_on_confirm(),
            vec!["a".to_string(), "b".to_string()]
        );
        // a：先全选再全不选。
        assert!(route_lsp_panel_key(&mut state, &key(KeyCode::Char('a'))).is_changed());
        assert_eq!(state.checked.len(), 2);
        assert!(route_lsp_panel_key(&mut state, &key(KeyCode::Char('a'))).is_changed());
        assert!(state.checked.is_empty());
    }

    #[test]
    fn cursor_bounds_and_confirm_outcome() {
        let mut state = LspPanelState::new(
            PathBuf::from("/repo"),
            Some("s-1".to_string()),
            response(vec![server("a", false)], true),
        );
        assert!(route_lsp_panel_key(&mut state, &key(KeyCode::Up)).is_changed());
        assert_eq!(state.cursor, 0, "cursor stays at top");
        state.move_down();
        assert_eq!(state.cursor, 0, "single row cannot move down");
        assert!(matches!(
            route_lsp_panel_key(&mut state, &key(KeyCode::Enter)),
            LspPanelOutcome::Confirm
        ));
        assert!(matches!(
            route_lsp_panel_key(&mut state, &key(KeyCode::Esc)),
            LspPanelOutcome::Cancel
        ));
    }
}

impl LspPanelOutcome {
    fn is_changed(&self) -> bool {
        matches!(self, LspPanelOutcome::Changed)
    }
}
