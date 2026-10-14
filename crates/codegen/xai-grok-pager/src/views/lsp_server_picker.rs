//! LOCAL(/lsp select): LSP server 勾选面板 —— 自动检测（builtin PATH）与
/// 配置（user/project/plugin）发现的 server 全部列出，用户空格勾选本工作区要
/// 启用的 server，Enter 确认后把取消勾选的名单写进 workspace 禁用清单
/// （shell `workspace_overrides::lsp_disabled_servers`）并对零回合会话热加载。
/// 结构与 [`crate::views::auth_modal`] 同款：自带窗口 chrome + route_key + render。

use std::collections::BTreeSet;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::prelude::{Buffer, Line, Modifier, Span, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::slash::i18n::tr;
use xai_grok_shell::extensions::lsp_status::LspServerStatus;

/// 面板尺寸（内容行随 server 数量自适应，上下限封顶）。
const PANEL_MIN_WIDTH: u16 = 64;
const PANEL_MIN_HEIGHT: u16 = 10;

pub struct LspServerPickerState {
    /// 确认时写覆盖文件用的目标工作区与活动会话（来自 `/lsp select` 发起时的快照）。
    pub cwd: PathBuf,
    pub session_id: Option<String>,
    /// 候选清单：config + builtin 检测（lspStatus 回包原样）。
    pub servers: Vec<LspServerStatus>,
    /// 当前勾选（= 要启用的）；初始值 = 打开面板时的生效面（未被禁用的）。
    pub checked: BTreeSet<String>,
    pub cursor: usize,
}

impl LspServerPickerState {
    pub fn new(
        cwd: PathBuf,
        session_id: Option<String>,
        servers: Vec<LspServerStatus>,
    ) -> Self {
        let checked = servers
            .iter()
            .filter(|s| !s.disabled)
            .map(|s| s.name.clone())
            .collect();
        Self {
            cwd,
            session_id,
            servers,
            checked,
            cursor: 0,
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

    /// 确认语义：返回要禁用的名单（全集 − 勾选集）。空 = 全部启用（清除覆盖）。
    pub fn disabled_on_confirm(&self) -> Vec<String> {
        self.servers
            .iter()
            .filter(|s| !self.checked.contains(&s.name))
            .map(|s| s.name.clone())
            .collect()
    }
}

pub enum LspPickerOutcome {
    /// 勾选/移动等状态变化，需要重绘。
    Changed,
    /// Esc —— 关闭面板，不落任何配置。
    Cancel,
    /// Enter —— 按 [`LspServerPickerState::disabled_on_confirm`] 语义提交。
    Confirm,
}

pub fn route_lsp_picker_key(
    state: &mut LspServerPickerState,
    key: &KeyEvent,
) -> LspPickerOutcome {
    match key.code {
        KeyCode::Esc => LspPickerOutcome::Cancel,
        KeyCode::Up | KeyCode::Char('k') => {
            state.move_up();
            LspPickerOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.move_down();
            LspPickerOutcome::Changed
        }
        KeyCode::Char(' ') => {
            state.toggle();
            LspPickerOutcome::Changed
        }
        KeyCode::Char('a') => {
            state.toggle_all();
            LspPickerOutcome::Changed
        }
        KeyCode::Enter | KeyCode::Char('s') => LspPickerOutcome::Confirm,
        _ => LspPickerOutcome::Changed,
    }
}

pub fn render_lsp_server_picker(buf: &mut Buffer, area: Rect, state: &LspServerPickerState) {
    let theme = crate::theme::Theme::current();
    let content_lines = 3 + state.servers.len().max(1) as u16; // 说明行 + 空行 + 清单
    let width = PANEL_MIN_WIDTH.min(area.width.saturating_sub(4));
    let height = (PANEL_MIN_HEIGHT + content_lines).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let panel = Rect { x, y, width, height };

    Clear.render(panel, buf);
    let title = tr("LSP servers");
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme.gray_bright))
        .title(Span::styled(
            title,
            Style::new().fg(theme.text_primary).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(panel);
    block.render(panel, buf);

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        tr("Space to select servers to enable in this workspace; unchecked ones are disabled."),
        Style::new().fg(theme.gray_bright),
    )));
    lines.push(Line::from(""));

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
                row.push_str(&tr("(command not found)").to_string());
            }
            let is_cursor = index == state.cursor;
            let style = if is_cursor {
                Style::new().fg(theme.text_primary).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(theme.text_secondary)
            };
            let cursor_prefix = if is_cursor { "> " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(
                    cursor_prefix.to_string(),
                    Style::new().fg(theme.selection_border),
                ),
                Span::styled(row, style),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(hint_line(&[
        ("↑/↓", "move"),
        ("space", "toggle"),
        ("a", "toggle all"),
        ("Enter", "confirm"),
        ("Esc", "cancel"),
    ]));

    let content_height = inner.height as usize;
    lines.truncate(content_height);
    Paragraph::new(lines).render(inner, buf);
}

/// 按键提示行（auth_modal hint_line 同款格式）。
fn hint_line(hints: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            (*key).to_string(),
            Style::new().add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {}", tr(label))));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

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

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: crossterm::event::KeyModifiers::empty(),
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn initial_checked_follows_current_enabled_set() {
        let state = LspServerPickerState::new(
            PathBuf::from("/repo"),
            None,
            vec![server("rust-analyzer", false), server("jdtls", true)],
        );
        assert!(state.checked.contains("rust-analyzer"));
        assert!(!state.checked.contains("jdtls"));
        assert_eq!(state.disabled_on_confirm(), vec!["jdtls".to_string()]);
    }

    #[test]
    fn toggle_and_toggle_all() {
        let mut state = LspServerPickerState::new(
            PathBuf::from("/repo"),
            None,
            vec![server("a", false), server("b", false)],
        );
        assert!(route_lsp_picker_key(&mut state, &key(KeyCode::Char(' '))).is_changed());
        assert!(!state.checked.contains("a"), "cursor row 0 toggled off");
        state.move_down();
        assert!(route_lsp_picker_key(&mut state, &key(KeyCode::Down)).is_changed());
        assert!(route_lsp_picker_key(&mut state, &key(KeyCode::Char(' '))).is_changed());
        assert!(!state.checked.contains("b"));
        assert_eq!(
            state.disabled_on_confirm(),
            vec!["a".to_string(), "b".to_string()]
        );
        // a：先全选再全不选。
        assert!(route_lsp_picker_key(&mut state, &key(KeyCode::Char('a'))).is_changed());
        assert_eq!(state.checked.len(), 2);
        assert!(route_lsp_picker_key(&mut state, &key(KeyCode::Char('a'))).is_changed());
        assert!(state.checked.is_empty());
    }

    #[test]
    fn cursor_bounds_and_confirm_outcome() {
        let mut state = LspServerPickerState::new(
            PathBuf::from("/repo"),
            Some("s-1".to_string()),
            vec![server("a", false)],
        );
        assert!(route_lsp_picker_key(&mut state, &key(KeyCode::Up)).is_changed());
        assert_eq!(state.cursor, 0, "cursor stays at top");
        state.move_down();
        assert_eq!(state.cursor, 0, "single row cannot move down");
        assert!(matches!(
            route_lsp_picker_key(&mut state, &key(KeyCode::Enter)),
            LspPickerOutcome::Confirm
        ));
        assert!(matches!(
            route_lsp_picker_key(&mut state, &key(KeyCode::Esc)),
            LspPickerOutcome::Cancel
        ));
    }
}

impl LspPickerOutcome {
    fn is_changed(&self) -> bool {
        matches!(self, LspPickerOutcome::Changed)
    }
}
