//! LOCAL: `/stats` 的模态窗口——按时间窗 × model 聚合的用量/性能报表。
//!
//! 视觉与交互对齐 [`crate::views::usage_modal`]（同一 `modal_window` 框架：边框、
//! 标签栏、页脚快捷键、滚动），标签页即时间窗（5h / 一天 / 一周）+ 工具输出压缩账本。
//! 数据是本地同步读取的 JSONL 账本，不需要 usage modal 那套异步 fetch。
//!
//! 折叠规则：时间窗标签页内每个 model 一个卡片块，超过 [`MAX_CARDS`] 张的其余部分
//! 收进 `… +N more` 一行（可用 `a` 展开/收起），避免 model 多时窗口无限长。

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

use crate::slash::i18n::tr;
use crate::theme::Theme;
use crate::views::modal_window::{
    self as mw, ModalSizing, ModalWindowConfig, ModalWindowState, Shortcut,
};
use xai_grok_tools::model_usage_ledger::{
    self, ModelCallSample, ModelUsageAggregate, Window, fmt_ms_pair, fmt_tokens, fmt_tps_pair,
};

/// 页脚快捷键：复制整份报表。
pub const COPY_STATS_SHORTCUT: usize = 1;

/// 展开/收起超出部分（`a`，页脚图例）。
const EXPAND_TOGGLE_LABEL: &str = "a all models";
/// 未展开时每个时间窗最多画多少个 model 卡片。
const MAX_CARDS: usize = 8;
/// 指标数值右对齐宽度（`45.2k` / `900/1100` 都落在 8 列内）。
const VALUE_WIDTH: usize = 8;

/// 模态窗口持有的数据：打开时同步读取账本，之后不再变化（本地文件，无异步刷新）。
pub struct StatsModalState {
    pub window: ModalWindowState,
    pub active_tab: StatsTab,
    pub scroll: u16,
    /// 账本采样（聚合在每个标签页渲染时按窗口筛选）。
    pub samples: Vec<ModelCallSample>,
    /// 打开时刻，所有窗口共用同一基准，避免逐帧漂移。
    pub now_unix_ms: u64,
    /// 工具输出压缩账本段（`format_stats_report` 原文，逐行）。
    pub compression: Vec<String>,
    /// 时间窗标签页是否展开全部 model 卡片。
    pub expanded: bool,
    /// 最近一帧的内容区几何。
    content_rect: Rect,
}

impl StatsModalState {
    pub fn new(tab: StatsTab, samples: Vec<ModelCallSample>, compression: Vec<String>) -> Self {
        Self {
            window: ModalWindowState::with_tabs(StatsTab::ALL.len()),
            active_tab: tab,
            scroll: 0,
            samples,
            now_unix_ms: ModelCallSample::now_unix_ms(),
            compression,
            expanded: false,
            content_rect: Rect::default(),
        }
    }

    pub fn set_tab(&mut self, tab: StatsTab) {
        if self.active_tab != tab {
            self.active_tab = tab;
            self.scroll = 0;
        }
    }

    fn step_tab(&mut self, forward: bool) {
        let n = StatsTab::ALL.len();
        let i = self.active_tab.index();
        let next = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        self.set_tab(StatsTab::ALL[next]);
    }

    fn scroll_to(&mut self, offset: u16) {
        self.scroll = offset;
    }

    /// 按行数滚动（滚轮/其它宿主转发）；`delta` 为负时向上。
    pub fn scroll_by(&mut self, delta: i32) {
        let step = delta.unsigned_abs().min(u16::MAX as u32) as u16;
        self.scroll = if delta < 0 {
            self.scroll.saturating_sub(step)
        } else {
            self.scroll.saturating_add(step)
        };
    }

    fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
        self.scroll = 0;
    }

    /// 整份报表的可复制文本（与窗口内显示同源，仅去掉左右对齐空格）。
    pub fn copy_all(&self) -> String {
        let mut out = Vec::new();
        for tab in StatsTab::ALL {
            out.push(format!("== {} ==", tab.label()));
            match tab {
                StatsTab::Compression => out.extend(self.compression.iter().cloned()),
                _ => {
                    let rows = self.rows_for(tab.window());
                    if rows.is_empty() {
                        out.push(tr("no calls in this window").to_string());
                    } else {
                        for row in &rows {
                            out.push(model_card_text(row));
                        }
                    }
                }
            }
        }
        out.join("\n")
    }

    fn rows_for(&self, window: Window) -> Vec<ModelUsageAggregate> {
        model_usage_ledger::aggregate(&self.samples, window.len_ms(), self.now_unix_ms)
    }
}

/// `/stats` 窗口的标签页：三个时间窗 + 工具输出压缩账本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsTab {
    Window(Window),
    Compression,
}

impl StatsTab {
    pub const ALL: [StatsTab; 4] = [
        StatsTab::Window(Window::FiveHours),
        StatsTab::Window(Window::Day),
        StatsTab::Window(Window::Week),
        StatsTab::Compression,
    ];

    pub fn window(self) -> Window {
        match self {
            StatsTab::Window(w) => w,
            StatsTab::Compression => Window::FiveHours,
        }
    }

    pub fn from_window(w: Window) -> Self {
        StatsTab::Window(w)
    }

    /// 标签文案 = 时间窗标签（`5h`/`day`/`week`）+ 压缩段标题，都走 i18n 表。
    pub fn label(self) -> &'static str {
        match self {
            StatsTab::Window(w) => w.label(),
            StatsTab::Compression => "Compression",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Self {
        *Self::ALL.get(i).unwrap_or(&Self::ALL[0])
    }
}

/// 一次按键/鼠标路由的结果：宿主拥有模态槽位与剪贴板，每个变体都是对宿主的请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsModalOutcome {
    /// 关闭窗口（Esc、`[✗]`、点击窗口外）。
    Close,
    /// 复制整份报表（`y` 或页脚按钮）。
    CopyText(String),
    Changed,
    Unchanged,
}

/// 窗口 chrome：无边框标题，标签栏即表头（与 usage modal 一致）。
fn chrome_cfg() -> ModalWindowConfig<'static> {
    ModalWindowConfig {
        title: "",
        tabs: None,
        shortcuts: &[],
        sizing: ModalSizing::default(),
        fold_info: None,
    }
}

/// 先走 chrome（Esc 关闭、标签点击），再走内容键。
pub fn route_stats_modal_key(state: &mut StatsModalState, key: &KeyEvent) -> StatsModalOutcome {
    match mw::handle_modal_key(&mut state.window, key, &chrome_cfg()) {
        mw::ModalWindowOutcome::CloseRequested => StatsModalOutcome::Close,
        mw::ModalWindowOutcome::Unhandled => handle_stats_modal_key(state, key),
        // chrome 未声明 tabs / shortcuts / fold，下列分支不会触发
        mw::ModalWindowOutcome::Handled
        | mw::ModalWindowOutcome::TabChanged(_)
        | mw::ModalWindowOutcome::ShortcutActivated(_)
        | mw::ModalWindowOutcome::CollapseGroup
        | mw::ModalWindowOutcome::ExpandGroup
        | mw::ModalWindowOutcome::CollapseDetails
        | mw::ModalWindowOutcome::ExpandDetails
        | mw::ModalWindowOutcome::JumpToParent(_) => StatsModalOutcome::Changed,
    }
}

pub fn route_stats_modal_mouse(
    state: &mut StatsModalState,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> StatsModalOutcome {
    match mw::handle_modal_mouse(&mut state.window, kind, column, row) {
        mw::ModalWindowOutcome::CloseRequested => StatsModalOutcome::Close,
        mw::ModalWindowOutcome::TabChanged(idx) => {
            state.set_tab(StatsTab::from_index(idx));
            StatsModalOutcome::Changed
        }
        mw::ModalWindowOutcome::ShortcutActivated(id) => {
            if id == COPY_STATS_SHORTCUT {
                StatsModalOutcome::CopyText(state.copy_all())
            } else {
                StatsModalOutcome::Changed
            }
        }
        mw::ModalWindowOutcome::Handled => StatsModalOutcome::Changed,
        mw::ModalWindowOutcome::Unhandled => handle_stats_modal_mouse(state, kind),
        mw::ModalWindowOutcome::CollapseGroup
        | mw::ModalWindowOutcome::ExpandGroup
        | mw::ModalWindowOutcome::CollapseDetails
        | mw::ModalWindowOutcome::ExpandDetails
        | mw::ModalWindowOutcome::JumpToParent(_) => StatsModalOutcome::Changed,
    }
}

fn handle_stats_modal_key(state: &mut StatsModalState, key: &KeyEvent) -> StatsModalOutcome {
    // BackTab 与 `G` 合法地带 SHIFT；只拒绝真正的组合键
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
    {
        return StatsModalOutcome::Unchanged;
    }
    match key.code {
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
            state.step_tab(true);
            StatsModalOutcome::Changed
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
            state.step_tab(false);
            StatsModalOutcome::Changed
        }
        KeyCode::Char(c @ '1'..='4') => {
            state.set_tab(StatsTab::from_index(c as usize - '1' as usize));
            StatsModalOutcome::Changed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.scroll_to(state.scroll.saturating_sub(1));
            StatsModalOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.scroll_to(state.scroll.saturating_add(1));
            StatsModalOutcome::Changed
        }
        KeyCode::PageUp => {
            state.scroll_to(state.scroll.saturating_sub(10));
            StatsModalOutcome::Changed
        }
        KeyCode::PageDown => {
            state.scroll_to(state.scroll.saturating_add(10));
            StatsModalOutcome::Changed
        }
        KeyCode::Home => {
            state.scroll_to(0);
            StatsModalOutcome::Changed
        }
        KeyCode::End | KeyCode::Char('G') => {
            state.scroll_to(u16::MAX);
            StatsModalOutcome::Changed
        }
        KeyCode::Char('a') => {
            state.toggle_expanded();
            StatsModalOutcome::Changed
        }
        KeyCode::Char('y') => StatsModalOutcome::CopyText(state.copy_all()),
        _ => StatsModalOutcome::Unchanged,
    }
}

fn handle_stats_modal_mouse(
    state: &mut StatsModalState,
    kind: MouseEventKind,
) -> StatsModalOutcome {
    match kind {
        MouseEventKind::ScrollUp => {
            state.scroll_to(state.scroll.saturating_sub(3));
            StatsModalOutcome::Changed
        }
        MouseEventKind::ScrollDown => {
            state.scroll_to(state.scroll.saturating_add(3));
            StatsModalOutcome::Changed
        }
        _ => StatsModalOutcome::Unchanged,
    }
}

pub fn render_stats_modal(
    buf: &mut Buffer,
    area: Rect,
    state: &mut StatsModalState,
    theme: &Theme,
) {
    let labels: Vec<&str> = StatsTab::ALL.iter().map(|t| t.label()).collect();
    state.window.active_tab = state.active_tab.index();

    let mut shortcuts: Vec<Shortcut> = vec![
        Shortcut {
            label: "Tab switch",
            clickable: false,
            id: 0,
        },
        Shortcut {
            label: "\u{2191}/\u{2193} scroll",
            clickable: false,
            id: 0,
        },
    ];
    if state.active_tab != StatsTab::Compression {
        shortcuts.push(Shortcut {
            label: EXPAND_TOGGLE_LABEL,
            clickable: false,
            id: 0,
        });
    }
    shortcuts.push(Shortcut {
        label: "y copy",
        clickable: true,
        id: COPY_STATS_SHORTCUT,
    });
    shortcuts.push(Shortcut {
        label: "Esc close",
        clickable: false,
        id: 0,
    });

    // 与 usage modal 同尺寸档位：内容窄、行数随 model 数增长
    let sizing = ModalSizing {
        width_pct: 0.65,
        max_width: 100,
        min_width: 44,
        v_margin: 2,
        h_pad: 2,
        v_pad: 2,
        footer_lines: 3,
    };
    let config = ModalWindowConfig {
        title: "",
        tabs: Some(&labels),
        shortcuts: &shortcuts,
        sizing,
        fold_info: None,
    };

    let Some(mca) = mw::render_modal_window(buf, area, &mut state.window, &config, theme) else {
        state.content_rect = Rect::default();
        return;
    };
    let content = mca.content;
    state.content_rect = content;

    let lines = tab_lines(state, theme, content.width);
    let max_scroll = lines.len().saturating_sub(content.height as usize);
    state.scroll = (state.scroll as usize).min(max_scroll) as u16;

    let visible: Vec<Line> = lines
        .into_iter()
        .skip(state.scroll as usize)
        .take(content.height as usize)
        .collect();
    Paragraph::new(visible).render(content, buf);
}

fn tab_lines(state: &StatsModalState, theme: &Theme, width: u16) -> Vec<Line<'static>> {
    match state.active_tab {
        StatsTab::Compression => state
            .compression
            .iter()
            .map(|l| plain(theme, l.clone()))
            .collect(),
        StatsTab::Window(window) => {
            let rows = state.rows_for(window);
            if rows.is_empty() {
                return vec![muted_line(theme, tr("no calls in this window"))];
            }
            let shown = if state.expanded {
                rows.len()
            } else {
                rows.len().min(MAX_CARDS)
            };
            let mut lines: Vec<Line<'static>> = Vec::new();
            for (i, row) in rows.iter().take(shown).enumerate() {
                if i > 0 {
                    lines.push(Line::default());
                }
                lines.extend(model_card_lines(row, theme, width));
            }
            if shown < rows.len() {
                lines.push(Line::default());
                // 整句成键（计数运行时插值），与 P2/P3 的计数模板一致
                let template = tr("\u{2026} +{n} more (press a to show all)");
                lines.push(muted_line(
                    theme,
                    template.replace("{n}", &(rows.len() - shown).to_string()),
                ));
            }
            lines
        }
    }
}

fn plain(theme: &Theme, s: impl Into<String>) -> Line<'static> {
    Line::styled(s.into(), Style::default().fg(theme.text_primary))
}

fn muted_line(theme: &Theme, s: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(s.into(), theme.muted()))
}

fn header_style(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_primary)
        .add_modifier(Modifier::BOLD)
}

/// 一张 model 卡片：model id + 调用数一行，token 与性能各一行（指标名左、数值右对齐）。
fn model_card_lines(row: &ModelUsageAggregate, theme: &Theme, width: u16) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(row.model_id.clone(), header_style(theme)),
        Span::styled(
            format!("  {} {}", row.calls, tr("calls")),
            Style::default().fg(theme.gray_dim),
        ),
    ])];

    let mut token_pairs: Vec<(&'static str, String)> = vec![
        (tr("input"), fmt_tokens(row.prompt_tokens)),
        (tr("output"), fmt_tokens(row.completion_tokens)),
        (tr("cache read"), fmt_tokens(row.cached_read_tokens)),
        (tr("cache write"), fmt_tokens(row.cache_creation_tokens)),
    ];
    if row.reasoning_tokens > 0 {
        token_pairs.push((tr("reasoning"), fmt_tokens(row.reasoning_tokens)));
    }
    token_pairs.push((
        tr("cache hit"),
        format!("{:.1}%", row.cache_hit_rate * 100.0),
    ));
    lines.extend(metric_rows(&token_pairs, theme, width));

    let perf_pairs: Vec<(&'static str, String)> = vec![
        (
            "ttft p50/p90",
            fmt_ms_pair(row.ttft_p50_ms, row.ttft_p90_ms, tr("n/a")),
        ),
        (
            "tps p50/p90",
            fmt_tps_pair(row.tps_p50, row.tps_p90, tr("n/a")),
        ),
    ];
    lines.extend(metric_rows(&perf_pairs, theme, width));
    lines
}

/// 把 `(标签, 数值)` 两列一行地铺开：标签左对齐、数值右对齐到列宽。
/// 面板过窄时退化为每行一个指标，避免数值被挤到边框外。
fn metric_rows(pairs: &[(&'static str, String)], theme: &Theme, width: u16) -> Vec<Line<'static>> {
    let label_w = pairs
        .iter()
        .map(|(label, _)| label.width())
        .max()
        .unwrap_or(0);
    let col_w = label_w + 1 + VALUE_WIDTH;
    // 列间距 2 列；面板装不下时至少保留一列
    let per_row = (((width as usize) + 2) / (col_w + 2)).max(1);
    let mut lines = Vec::new();
    for chunk in pairs.chunks(per_row) {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (i, (label, value)) in chunk.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(
                (*label).to_string(),
                Style::default().fg(theme.gray_dim),
            ));
            let gap = label_w.saturating_sub(label.width())
                + 1
                + VALUE_WIDTH.saturating_sub(value.width());
            spans.push(Span::raw(" ".repeat(gap)));
            spans.push(Span::styled(
                value.clone(),
                Style::default().fg(theme.text_primary),
            ));
        }
        lines.push(Line::from(spans));
    }
    lines
}

/// 卡片块的纯文本版（复制用）。
fn model_card_text(row: &ModelUsageAggregate) -> String {
    let mut parts = vec![format!("{} · {} {}", row.model_id, row.calls, tr("calls"))];
    let mut tokens = vec![
        format!("{} {}", tr("input"), fmt_tokens(row.prompt_tokens)),
        format!("{} {}", tr("output"), fmt_tokens(row.completion_tokens)),
        format!(
            "{} {}",
            tr("cache read"),
            fmt_tokens(row.cached_read_tokens)
        ),
        format!(
            "{} {}",
            tr("cache write"),
            fmt_tokens(row.cache_creation_tokens)
        ),
    ];
    if row.reasoning_tokens > 0 {
        tokens.push(format!(
            "{} {}",
            tr("reasoning"),
            fmt_tokens(row.reasoning_tokens)
        ));
    }
    tokens.push(format!(
        "{} {:.1}%",
        tr("cache hit"),
        row.cache_hit_rate * 100.0
    ));
    parts.push(tokens.join(" · "));
    parts.push(format!(
        "ttft p50/p90 {} · tps p50/p90 {}",
        fmt_ms_pair(row.ttft_p50_ms, row.ttft_p90_ms, tr("n/a")),
        fmt_tps_pair(row.tps_p50, row.tps_p90, tr("n/a")),
    ));
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};
    use xai_grok_tools::model_usage_ledger::{WINDOW_5H_MS, WINDOW_DAY_MS, WINDOW_WEEK_MS};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn sample(ts: u64, model: &str, prompt: u64, cached: u64) -> ModelCallSample {
        ModelCallSample {
            ts_unix_ms: ts,
            model_id: model.to_string(),
            prompt_tokens: prompt,
            completion_tokens: 500,
            cached_prompt_tokens: cached,
            cache_creation_tokens: 200,
            reasoning_tokens: 10,
            ttft_ms: Some(900),
            tps: Some(50.0),
            duration_ms: 2000,
        }
    }

    fn state_with_samples() -> StatsModalState {
        let now = 1_000_000_000_000;
        let mut state = StatsModalState::new(
            StatsTab::Window(Window::FiveHours),
            vec![
                sample(now - 1000, "m-a", 1000, 800),
                sample(now - 2000, "m-a", 500, 500),
                // 5h 窗外、日窗内
                sample(now - WINDOW_5H_MS - 1000, "m-b", 777, 0),
                // 日窗外、周窗内
                sample(now - WINDOW_DAY_MS - 1000, "m-c", 55, 0),
                // 周窗外
                sample(now - WINDOW_WEEK_MS - 1000, "m-d", 1, 0),
            ],
            vec!["Tool-output compression (experimental)".to_string()],
        );
        state.now_unix_ms = now;
        state
    }

    fn rendered(state: &mut StatsModalState) -> String {
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        let theme = Theme::current();
        render_stats_modal(&mut buf, area, state, &theme);
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| {
                        buf.cell((x, y))
                            .map(|c| c.symbol().to_string())
                            .unwrap_or_default()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn tabs_are_three_windows_plus_compression_in_order() {
        let labels: Vec<&str> = StatsTab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(labels, ["Last 5h", "Last day", "Last week", "Compression"]);
        assert_eq!(StatsTab::Window(Window::Day).index(), 1);
        assert_eq!(StatsTab::from_index(3), StatsTab::Compression);
    }

    #[test]
    fn tab_step_wraps_and_resets_scroll() {
        let mut state = state_with_samples();
        state.scroll = 5;
        assert_eq!(
            handle_stats_modal_key(&mut state, &key(KeyCode::Tab)),
            StatsModalOutcome::Changed
        );
        assert_eq!(state.active_tab, StatsTab::Window(Window::Day));
        assert_eq!(state.scroll, 0, "切标签重置滚动");
        for _ in 0..3 {
            handle_stats_modal_key(&mut state, &key(KeyCode::Tab));
        }
        assert_eq!(
            state.active_tab,
            StatsTab::Window(Window::FiveHours),
            "循环"
        );
        handle_stats_modal_key(&mut state, &key(KeyCode::BackTab));
        assert_eq!(state.active_tab, StatsTab::Compression, "反向循环");
    }

    #[test]
    fn digit_keys_jump_to_tabs() {
        let mut state = state_with_samples();
        handle_stats_modal_key(&mut state, &key(KeyCode::Char('3')));
        assert_eq!(state.active_tab, StatsTab::Window(Window::Week));
        handle_stats_modal_key(&mut state, &key(KeyCode::Char('4')));
        assert_eq!(state.active_tab, StatsTab::Compression);
    }

    #[test]
    fn escape_closes_and_ctrl_chords_are_ignored() {
        let mut state = state_with_samples();
        assert_eq!(
            route_stats_modal_key(&mut state, &key(KeyCode::Esc)),
            StatsModalOutcome::Close
        );
        let ctrl_tab = KeyEvent {
            code: KeyCode::Char('y'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert_eq!(
            handle_stats_modal_key(&mut state, &ctrl_tab),
            StatsModalOutcome::Unchanged
        );
    }

    #[test]
    fn window_tab_filters_samples_by_its_own_window() {
        let state = state_with_samples();
        let five_h = tab_text(&state, StatsTab::Window(Window::FiveHours));
        assert!(five_h.contains("m-a"), "{five_h}");
        assert!(!five_h.contains("m-b"), "5h 窗不该含 5h 外的样本: {five_h}");

        let day = tab_text(&state, StatsTab::Window(Window::Day));
        assert!(day.contains("m-a") && day.contains("m-b"), "{day}");
        assert!(!day.contains("m-c"), "日窗不该含日窗外的样本: {day}");

        let week = tab_text(&state, StatsTab::Window(Window::Week));
        assert!(week.contains("m-c"), "{week}");
        assert!(!week.contains("m-d"), "周窗不该含周窗外的样本: {week}");
    }

    /// 渲染指定标签页的内容文本。必须显式切换 `active_tab`——`tab_lines` 读的是
    /// 状态里的当前标签页，只传参数不改状态会一直渲染同一个标签页。
    fn tab_text(state: &StatsModalState, tab: StatsTab) -> String {
        let mut view = StatsModalState::new(tab, state.samples.clone(), state.compression.clone());
        view.now_unix_ms = state.now_unix_ms;
        view.expanded = state.expanded;
        tab_lines(&view, &Theme::current(), 80)
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn empty_window_shows_placeholder() {
        let mut state =
            StatsModalState::new(StatsTab::Window(Window::Week), Vec::new(), Vec::new());
        state.now_unix_ms = 1_000_000_000_000;
        let text = tab_text(&state, StatsTab::Window(Window::Week));
        assert!(text.contains("no calls in this window"), "{text}");
    }

    #[test]
    fn card_collapses_after_max_cards_and_a_expands() {
        let now = 1_000_000_000_000;
        let samples: Vec<ModelCallSample> = (0..MAX_CARDS + 3)
            .map(|i| sample(now - 1000, &format!("m-{i}"), 1000 - i as u64, 0))
            .collect();
        let mut state =
            StatsModalState::new(StatsTab::Window(Window::FiveHours), samples, Vec::new());
        state.now_unix_ms = now;
        let text = tab_text(&state, StatsTab::Window(Window::FiveHours));
        assert!(text.contains("+3 more"), "{text}");

        let mut expanded_state = state;
        handle_stats_modal_key(&mut expanded_state, &key(KeyCode::Char('a')));
        assert!(expanded_state.expanded);
        let text = tab_text(&expanded_state, StatsTab::Window(Window::FiveHours));
        assert!(!text.contains("more"), "展开后不该再有折叠提示: {text}");
    }

    #[test]
    fn compression_tab_renders_passthrough_lines() {
        let state = state_with_samples();
        let text = tab_text(&state, StatsTab::Compression);
        assert!(
            text.contains("Tool-output compression (experimental)"),
            "{text}"
        );
        assert!(!text.contains("m-a"), "压缩标签页不该混入用量卡片: {text}");
    }

    #[test]
    fn y_copies_every_section() {
        let state = state_with_samples();
        let text = state.copy_all();
        assert!(text.contains("== Last 5h =="), "{text}");
        assert!(text.contains("== Last day =="), "{text}");
        assert!(text.contains("== Last week =="), "{text}");
        assert!(text.contains("== Compression =="), "{text}");
        assert!(text.contains("m-a · 2 calls"), "{text}");
        assert!(text.contains("m-c · 1 calls"), "{text}");
        assert!(!text.contains("m-d"), "周窗外的样本不该进报表: {text}");
    }

    #[test]
    fn render_smoke_paints_tabs_footer_and_cards() {
        let mut state = state_with_samples();
        let text = rendered(&mut state);
        assert!(text.contains("Last 5h"), "{text}");
        assert!(text.contains("Compression"), "{text}");
        assert!(text.contains("Esc close"), "{text}");
        // 首屏只画可见行，这里只断言卡片与页脚确实绘出（逐行内容由 tab_text 用例覆盖）
        assert!(text.contains("m-a"), "{text}");
        assert!(text.contains("2 calls"), "{text}");
    }

    /// `render_stats_modal` 在渲染时把滚动量钳到内容高度内。
    #[test]
    fn scroll_end_is_clamped_to_content_height_at_render_time() {
        let mut state = state_with_samples();
        handle_stats_modal_key(&mut state, &key(KeyCode::End));
        assert_eq!(state.scroll, u16::MAX, "End 先记一个哨兵值");
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        render_stats_modal(&mut buf, area, &mut state, &Theme::current());
        assert!(
            state.scroll < u16::MAX,
            "渲染时必须把滚动量钳到内容高度内，实际 {}",
            state.scroll
        );
    }

    #[test]
    fn window_ms_values_match_the_advertised_names() {
        assert_eq!(Window::FiveHours.len_ms(), WINDOW_5H_MS);
        assert_eq!(Window::Day.len_ms(), WINDOW_DAY_MS);
        assert_eq!(Window::Week.len_ms(), WINDOW_WEEK_MS);
    }
}
