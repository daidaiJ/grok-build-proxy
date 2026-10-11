//! LOCAL: /auth 供应商配置面板 —— 分步向导（协议 → baseUrl → SK → 模型列表 → 确认写回）。
//!
//! 拍板口径（`docs-local/provider-onboarding-api-cost-todo.md` 七轮）：SK 明文
//! `api_key` 直写（本地掩码回显）；条目 id 用户缩写、唯一即可（已存在即拒绝）；
//! 写回走 toml_edit 定点编辑 + flock 守卫（shell `write_model_entries`）。
//! 模型列表步 = 关键词过滤 + 空格勾选多选（六轮拍板）；取数失败回退手填 slug。
//! 目录回填只在与新建模型兜底不同时写入（context_window 兜底 200_000，
//! `agent/config.rs:3704`；max_completion_tokens 缺省 = 不发送，目录有值即写；
//! 目录 `reasoning` = Some(true) 时写 `supports_reasoning_effort = true`，接入
//! 既有 reasoning-effort 机制）。
//!
//! 双宿主：会话内挂 `ActiveModal::AuthPanel`（AgentView），零配置首启的 welcome
//! 门挂 `AppView::welcome_auth_panel`（welcome 无会话上下文，而 BYOK 用户恰被困在
//! 该门后）。两宿主共用同一状态机与 outcome（Action 转发到统一 dispatch）。

use std::collections::BTreeSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};

use crate::slash::i18n::tr;
use xai_grok_sampling_types::ApiBackend;
use xai_grok_shell::extensions::provider_models::ProviderModelsResponse;
use xai_grok_shell::util::config::ModelEntryDraft;

/// 新建 `[model.*]` 条目缺省 `context_window` 的兜底值（shell `agent/config.rs:3704`）。
/// 目录值与它相同就不写（拍板「和默认配置值不一样」才写）。
const CONTEXT_WINDOW_FALLBACK: u64 = 200_000;

/// 面板内流转的 SK：手写 `Debug` 打码 + 不派生 `Display`/`Serialize`，
/// 保证任何 `{:?}` 日志落盘都不出现密钥。
pub struct SecretKey(String);

impl SecretKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    /// 仅 Effect 组包（进 Authorization 头）时取用。
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn masked_len(&self) -> usize {
        self.0.chars().count()
    }
    pub fn push_str(&mut self, value: &str) {
        self.0.push_str(value);
    }
    pub fn pop_char(&mut self) {
        self.0.pop();
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl PartialEq for SecretKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey([redacted])")
    }
}

/// 面板步骤。`Writing` 期间 Esc 被吞（写回任务照常完成，回包由 TaskResult 路由）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStep {
    Protocol,
    BaseUrl,
    ApiKey,
    Models,
    Confirm,
    Writing,
    Done,
}

/// 确认步的一行（一条待写条目）。id 可编辑；回填值已按兜底规则预筛。
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmRow {
    /// 路由 slug（API 请求发送的 model 名）。
    pub model: String,
    /// 用户可编辑的配置缩写 id。
    pub id: String,
    pub context_window: Option<u64>,
    pub max_completion_tokens: Option<u64>,
    /// 目录标注支持思考 → 写 `supports_reasoning_effort = true`。
    pub supports_reasoning_effort: bool,
}

/// 面板状态机。两宿主各持一份（同一时刻至多一个打开）。
pub struct AuthModalState {
    pub step: AuthStep,
    /// 步 1：协议高亮行（0=ChatCompletions 缺省）。
    pub protocol_cursor: usize,
    /// 步 2：baseUrl 输入缓冲。
    pub base_url_buf: String,
    /// 步 3：SK 输入（掩码回显）。
    pub api_key: SecretKey,
    /// 步 4：模型列表（取数 + 手填）。
    pub models: Vec<String>,
    /// 步 4：用户手填的 slug（重取后仍保留）。
    manual_slugs: Vec<String>,
    /// 步 4：关键词过滤（输入即过滤）。
    pub filter: String,
    /// 步 4：勾选的 slug 集合。
    pub selected: BTreeSet<String>,
    /// 步 4：高亮行（对过滤后的可见列表）。
    pub cursor: usize,
    /// 步 4：取数在途（单发守卫；回包无条件落）。
    pub fetch_in_flight: bool,
    /// 发起取数时的 base_url（陈旧回包比对）。
    pub requested_base_url: Option<String>,
    /// 步 4：取数错误（`None` = 无错）。
    pub fetch_error: Option<String>,
    /// 步 4：手填输入行激活。
    pub manual_mode: bool,
    pub manual_buf: String,
    /// 步 5：待写条目行。
    pub rows: Vec<ConfirmRow>,
    /// 步 5：高亮行。
    pub confirm_cursor: usize,
    /// 步 5：id 内联编辑激活（编辑高亮行）。
    pub editing_id: bool,
    pub id_buf: String,
    /// 打开时点的既有模型 id（冲突标记用）。
    pub existing_ids: BTreeSet<String>,
    /// 步 5：写回错误（TaskResult 落）。
    pub persist_error: Option<String>,
    /// 步 6：成功写入条数。
    pub written_count: Option<usize>,
    /// 步 6：写回后的模型列表热重载失败原因（`None` = 已重载或未触发）。
    pub reload_error: Option<String>,
}

impl AuthModalState {
    pub fn new(existing_ids: BTreeSet<String>) -> Self {
        Self {
            step: AuthStep::Protocol,
            protocol_cursor: 0,
            base_url_buf: String::new(),
            api_key: SecretKey::new(String::new()),
            models: Vec::new(),
            manual_slugs: Vec::new(),
            filter: String::new(),
            selected: BTreeSet::new(),
            cursor: 0,
            fetch_in_flight: false,
            requested_base_url: None,
            fetch_error: None,
            manual_mode: false,
            manual_buf: String::new(),
            rows: Vec::new(),
            confirm_cursor: 0,
            editing_id: false,
            id_buf: String::new(),
            existing_ids,
            persist_error: None,
            written_count: None,
            reload_error: None,
        }
    }

    /// 步 4 → 步 5：按勾选构建确认行，并请求目录回填（调用方据此推 Effect）。
    pub fn build_confirm_rows(&mut self) -> Vec<String> {
        let mut ids = Vec::new();
        self.rows = self
            .models
            .iter()
            .filter(|model| self.selected.contains(*model))
            .map(|model| {
                let (context_window, max_completion_tokens, supports_reasoning_effort) =
                    catalog_backfill(model);
                ids.push(model.clone());
                ConfirmRow {
                    id: derive_entry_id(model, &self.existing_ids),
                    model: model.clone(),
                    context_window,
                    max_completion_tokens,
                    supports_reasoning_effort,
                }
            })
            .collect();
        self.confirm_cursor = 0;
        self.persist_error = None;
        ids
    }

    /// 陈旧回包守卫：仍在步 4、有在途请求、且 base_url 与请求时一致才接受。
    pub fn accepts_provider_models_response(&self, response: &ProviderModelsResponse) -> bool {
        self.step == AuthStep::Models
            && self.fetch_in_flight
            && self.requested_base_url.as_deref() == Some(response.base_url.as_str())
    }

    /// 回包落库（调用方已过陈旧守卫）。
    pub fn apply_provider_models_response(&mut self, response: ProviderModelsResponse) {
        self.fetch_in_flight = false;
        if let Some(error) = response.error {
            self.fetch_error = Some(error);
            return;
        }
        self.fetch_error = None;
        // 取数结果与手填 slug 合并（手填是用户意图，重取不丢）。
        let mut models: Vec<String> = response.models.iter().map(|m| m.id.clone()).collect();
        for slug in &self.manual_slugs {
            if !models.contains(slug) {
                models.push(slug.clone());
            }
        }
        self.models = models;
        self.selected
            .retain(|slug| self.models.contains(slug));
        self.clamp_cursor();
    }

    /// 写回回包落库（`Writing` → `Done` / 退回 `Confirm` 展示错误）。
    pub fn apply_persist_result(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.written_count = Some(self.rows.len());
                self.step = AuthStep::Done;
            }
            Err(error) => {
                self.persist_error = Some(error);
                self.step = AuthStep::Confirm;
            }
        }
    }

    /// 写回成功后的模型列表热重载回包（`Err` 在完成页给出重启提示）。
    pub fn apply_reload_result(&mut self, result: Result<usize, String>) {
        if self.step == AuthStep::Done {
            self.reload_error = result.err();
        }
    }

    /// 目录回包落库后重算确认行的回填值（id 保持用户已编辑的形态）。
    pub fn refresh_backfill(&mut self) {
        for row in &mut self.rows {
            let (context_window, max_completion_tokens, supports_reasoning_effort) =
                catalog_backfill(&row.model);
            row.context_window = context_window;
            row.max_completion_tokens = max_completion_tokens;
            row.supports_reasoning_effort = supports_reasoning_effort;
        }
    }

    fn visible_models(&self) -> Vec<&String> {
        let query = self.filter.to_lowercase();
        self.models
            .iter()
            .filter(|model| query.is_empty() || model.to_lowercase().contains(&query))
            .collect()
    }

    fn clamp_cursor(&mut self) {
        let len = self.visible_models().len();
        if self.cursor >= len {
            self.cursor = len.saturating_sub(1);
        }
    }
}

/// 目录回填（T1 store）：命中才带值；`context_window` 与新建兜底相同则省略
/// （拍板「和默认配置值不一样」）；`max_completion_tokens` 缺省语义 = 不发送，
/// 目录有值即写；`reasoning` = Some(true) 才开 `supports_reasoning_effort`
///（None/false 都不写，网关不支持时不冒充）。
fn catalog_backfill(model_id: &str) -> (Option<u64>, Option<u64>, bool) {
    let matched = match crate::app::model_catalog_store::get(model_id) {
        Some(Some(matched)) => matched,
        _ => return (None, None, false),
    };
    let context_window = matched
        .context_length
        .filter(|window| *window != CONTEXT_WINDOW_FALLBACK);
    (
        context_window,
        matched.max_completion_tokens,
        matched.reasoning == Some(true),
    )
}

/// slug → 配置 id 缩写：非 `[a-zA-Z0-9_-]` 折成 `-`，收紧连字符，撞名追加序号。
fn derive_entry_id(slug: &str, taken: &BTreeSet<String>) -> String {
    let mut base: String = slug
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '-' })
        .collect();
    while base.contains("--") {
        base = base.replace("--", "-");
    }
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() { "model".to_string() } else { base };
    if !taken.contains(&base) {
        return base;
    }
    for suffix in 2.. {
        let candidate = format!("{base}-{suffix}");
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!("suffix loop always finds a free id")
}

/// 面板按键路由的产出。
#[derive(Debug, PartialEq)]
pub enum AuthModalOutcome {
    /// 关闭面板（宿主清掉自己的槽位）。
    Close,
    /// 步 4 发起取数（`base_url`、掩码 SK 原样带上）。
    FetchModels { base_url: String, api_key: SecretKey },
    /// 步 5 请求目录回填（只带需要取数的 id）。
    FetchCatalog { model_ids: Vec<String> },
    /// 步 5 提交写回。
    Persist { entries: Vec<ModelEntryDraft> },
    Changed,
    Unchanged,
}

/// 键盘路由。`Esc` 按步回退（首步关闭）；文本输入步吃字符/退格。
pub fn route_auth_modal_key(state: &mut AuthModalState, key: &KeyEvent) -> AuthModalOutcome {
    if key.kind == crossterm::event::KeyEventKind::Release {
        return AuthModalOutcome::Unchanged;
    }
    match state.step {
        AuthStep::Protocol => route_protocol(state, key),
        AuthStep::BaseUrl => route_base_url(state, key),
        AuthStep::ApiKey => route_api_key(state, key),
        AuthStep::Models => route_models(state, key),
        AuthStep::Confirm => route_confirm(state, key),
        AuthStep::Writing => AuthModalOutcome::Unchanged,
        AuthStep::Done => match key.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => AuthModalOutcome::Close,
            _ => AuthModalOutcome::Unchanged,
        },
    }
}

fn route_protocol(state: &mut AuthModalState, key: &KeyEvent) -> AuthModalOutcome {
    match key.code {
        KeyCode::Esc => AuthModalOutcome::Close,
        KeyCode::Up | KeyCode::Char('k') => {
            state.protocol_cursor = state.protocol_cursor.saturating_sub(1);
            AuthModalOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.protocol_cursor = (state.protocol_cursor + 1).min(2);
            AuthModalOutcome::Changed
        }
        KeyCode::Char(c @ '1'..='3') => {
            state.protocol_cursor = c as usize - '1' as usize;
            state.step = AuthStep::BaseUrl;
            AuthModalOutcome::Changed
        }
        KeyCode::Enter => {
            state.step = AuthStep::BaseUrl;
            AuthModalOutcome::Changed
        }
        _ => AuthModalOutcome::Unchanged,
    }
}

fn route_base_url(state: &mut AuthModalState, key: &KeyEvent) -> AuthModalOutcome {
    match key.code {
        KeyCode::Esc => {
            state.step = AuthStep::Protocol;
            AuthModalOutcome::Changed
        }
        KeyCode::Enter => {
            let trimmed = state.base_url_buf.trim().trim_end_matches('/').to_string();
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                state.base_url_buf = trimmed;
                state.step = AuthStep::ApiKey;
                AuthModalOutcome::Changed
            } else {
                AuthModalOutcome::Changed
            }
        }
        KeyCode::Backspace => {
            state.base_url_buf.pop();
            AuthModalOutcome::Changed
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.base_url_buf.clear();
            AuthModalOutcome::Changed
        }
        KeyCode::Char(_) => {
            if let KeyCode::Char(c) = key.code {
                state.base_url_buf.push(c);
            }
            AuthModalOutcome::Changed
        }
        _ => AuthModalOutcome::Unchanged,
    }
}

fn route_api_key(state: &mut AuthModalState, key: &KeyEvent) -> AuthModalOutcome {
    match key.code {
        KeyCode::Esc => {
            state.step = AuthStep::BaseUrl;
            AuthModalOutcome::Changed
        }
        KeyCode::Enter => {
            state.step = AuthStep::Models;
            begin_fetch(state)
        }
        KeyCode::Backspace => {
            state.api_key.pop_char();
            AuthModalOutcome::Changed
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.api_key.clear();
            AuthModalOutcome::Changed
        }
        KeyCode::Char(_) => {
            if let KeyCode::Char(c) = key.code {
                state.api_key.push_str(&c.to_string());
            }
            AuthModalOutcome::Changed
        }
        _ => AuthModalOutcome::Unchanged,
    }
}

fn route_models(state: &mut AuthModalState, key: &KeyEvent) -> AuthModalOutcome {
    if state.manual_mode {
        return route_manual_input(state, key);
    }
    match key.code {
        KeyCode::Esc => {
            state.step = AuthStep::ApiKey;
            AuthModalOutcome::Changed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.cursor = state.cursor.saturating_sub(1);
            AuthModalOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.cursor + 1 < state.visible_models().len() {
                state.cursor += 1;
            }
            AuthModalOutcome::Changed
        }
        KeyCode::Char(' ') => {
            let visible = state.visible_models();
            if let Some(slug) = visible.get(state.cursor) {
                let slug = (*slug).clone();
                if !state.selected.insert(slug.clone()) {
                    state.selected.remove(&slug);
                }
            }
            AuthModalOutcome::Changed
        }
        KeyCode::Char('r') => begin_fetch(state),
        KeyCode::Char('a') => {
            state.manual_mode = true;
            state.manual_buf.clear();
            AuthModalOutcome::Changed
        }
        // 过滤键入：可打印字符进过滤词。
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.filter.push(c);
            state.clamp_cursor();
            AuthModalOutcome::Changed
        }
        KeyCode::Backspace => {
            state.filter.pop();
            state.clamp_cursor();
            AuthModalOutcome::Changed
        }
        KeyCode::Enter => {
            if state.selected.is_empty() {
                return AuthModalOutcome::Changed;
            }
            let ids = state.build_confirm_rows();
            if ids.is_empty() {
                return AuthModalOutcome::Changed;
            }
            state.step = AuthStep::Confirm;
            AuthModalOutcome::FetchCatalog { model_ids: ids }
        }
        _ => AuthModalOutcome::Unchanged,
    }
}

fn route_manual_input(state: &mut AuthModalState, key: &KeyEvent) -> AuthModalOutcome {
    match key.code {
        KeyCode::Esc => {
            state.manual_mode = false;
            state.manual_buf.clear();
            AuthModalOutcome::Changed
        }
        KeyCode::Enter => {
            let slug = state.manual_buf.trim().to_string();
            if slug.is_empty() {
                return AuthModalOutcome::Changed;
            }
            if !state.models.contains(&slug) {
                state.models.push(slug.clone());
            }
            if !state.manual_slugs.contains(&slug) {
                state.manual_slugs.push(slug.clone());
            }
            state.selected.insert(slug);
            state.manual_buf.clear();
            state.manual_mode = false;
            state.clamp_cursor();
            AuthModalOutcome::Changed
        }
        KeyCode::Backspace => {
            state.manual_buf.pop();
            AuthModalOutcome::Changed
        }
        KeyCode::Char(c) => {
            state.manual_buf.push(c);
            AuthModalOutcome::Changed
        }
        _ => AuthModalOutcome::Unchanged,
    }
}

/// 进入取数（步 4 首次进入与 `r` 重试共用）。调用方把 outcome 转成 Effect。
pub fn begin_fetch(state: &mut AuthModalState) -> AuthModalOutcome {
    if state.fetch_in_flight {
        return AuthModalOutcome::Changed;
    }
    let base_url = state.base_url_buf.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() {
        return AuthModalOutcome::Changed;
    }
    state.fetch_in_flight = true;
    state.fetch_error = None;
    state.requested_base_url = Some(base_url.clone());
    AuthModalOutcome::FetchModels {
        base_url,
        api_key: SecretKey::new(state.api_key.expose().to_string()),
    }
}

fn route_confirm(state: &mut AuthModalState, key: &KeyEvent) -> AuthModalOutcome {
    if state.editing_id {
        return route_id_edit(state, key);
    }
    match key.code {
        KeyCode::Esc => {
            state.step = AuthStep::Models;
            AuthModalOutcome::Changed
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.confirm_cursor = state.confirm_cursor.saturating_sub(1);
            AuthModalOutcome::Changed
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.confirm_cursor + 1 < state.rows.len() {
                state.confirm_cursor += 1;
            }
            AuthModalOutcome::Changed
        }
        KeyCode::Char('e') if !state.rows.is_empty() => {
            state.editing_id = true;
            state.id_buf = state.rows[state.confirm_cursor].id.clone();
            AuthModalOutcome::Changed
        }
        KeyCode::Enter => submit_persist(state),
        _ => AuthModalOutcome::Unchanged,
    }
}

fn route_id_edit(state: &mut AuthModalState, key: &KeyEvent) -> AuthModalOutcome {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            if let Some(row) = state.rows.get_mut(state.confirm_cursor) {
                row.id = state.id_buf.trim().to_string();
            }
            state.id_buf.clear();
            state.editing_id = false;
            AuthModalOutcome::Changed
        }
        KeyCode::Backspace => {
            state.id_buf.pop();
            AuthModalOutcome::Changed
        }
        KeyCode::Char(c) => {
            state.id_buf.push(c);
            AuthModalOutcome::Changed
        }
        _ => AuthModalOutcome::Unchanged,
    }
}

fn submit_persist(state: &mut AuthModalState) -> AuthModalOutcome {
    if id_conflicts(state).next().is_some() {
        return AuthModalOutcome::Changed;
    }
    let entries = state
        .rows
        .iter()
        .map(|row| ModelEntryDraft {
            id: row.id.clone(),
            model: row.model.clone(),
            base_url: state.base_url_buf.trim().trim_end_matches('/').to_string(),
            api_key: state.api_key.expose().to_string(),
            api_backend: protocol_at(state.protocol_cursor),
            context_window: row.context_window,
            max_completion_tokens: row.max_completion_tokens,
            supports_reasoning_effort: row.supports_reasoning_effort,
        })
        .collect();
    state.persist_error = None;
    state.step = AuthStep::Writing;
    AuthModalOutcome::Persist { entries }
}

/// 冲突 id 迭代：空 id、既有 id、行间重复都算（确认步阻止提交并标红）。
fn id_conflicts(state: &AuthModalState) -> impl Iterator<Item = &str> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    state
        .rows
        .iter()
        .filter(move |row| {
            row.id.is_empty()
                || state.existing_ids.contains(&row.id)
                || !seen.insert(row.id.as_str())
        })
        .map(|row| row.id.as_str())
}

pub fn row_is_conflicted(state: &AuthModalState, index: usize) -> bool {
    let Some(row) = state.rows.get(index) else {
        return false;
    };
    row.id.is_empty()
        || state.existing_ids.contains(&row.id)
        || state.rows.iter().any(|other| {
            other.model != row.model && !row.id.is_empty() && other.id == row.id
        })
}

/// 步 1 的三选（与 `ApiBackend` serde 拼写一一对应）。
pub fn protocol_at(index: usize) -> ApiBackend {
    match index {
        1 => ApiBackend::Responses,
        2 => ApiBackend::Messages,
        _ => ApiBackend::ChatCompletions,
    }
}

/// 步 1 的展示标签。
pub fn protocol_label(index: usize) -> &'static str {
    match index {
        1 => "responses (/v1/responses)",
        2 => "messages (/v1/messages, Anthropic)",
        _ => "chat_completions (/v1/chat/completions)",
    }
}

/// 粘贴路由：文本输入态追加进当前缓冲，其余忽略。
pub fn handle_auth_paste(state: &mut AuthModalState, text: &str) -> AuthModalOutcome {
    let single_line = text.replace(['\r', '\n'], " ");
    match state.step {
        AuthStep::BaseUrl => {
            state.base_url_buf.push_str(&single_line);
            AuthModalOutcome::Changed
        }
        AuthStep::ApiKey => {
            state.api_key.push_str(&single_line);
            AuthModalOutcome::Changed
        }
        AuthStep::Models if state.manual_mode => {
            state.manual_buf.push_str(&single_line);
            AuthModalOutcome::Changed
        }
        AuthStep::Confirm if state.editing_id => {
            state.id_buf.push_str(&single_line);
            AuthModalOutcome::Changed
        }
        _ => AuthModalOutcome::Unchanged,
    }
}

/// 鼠标：面板内暂不响应（无按钮命中区），只吞事件避免穿透。
pub fn handle_auth_mouse(state: &AuthModalState, mouse: &MouseEvent) -> AuthModalOutcome {
    let _ = (state, mouse);
    AuthModalOutcome::Unchanged
}

// ---------------------------------------------------------------------------
// 渲染
// ---------------------------------------------------------------------------

const PANEL_WIDTH: u16 = 66;
const PANEL_HEIGHT: u16 = 18;

const STEP_TITLES: [&str; 5] = [
    "1/5 Protocol",
    "2/5 Base URL",
    "3/5 API key",
    "4/5 Models",
    "5/5 Confirm",
];

/// 面板渲染入口（两宿主共用；`area` 为宿主全屏区）。
pub fn render_auth_modal(buf: &mut ratatui::buffer::Buffer, area: Rect, state: &mut AuthModalState) {
    let theme = crate::theme::Theme::current();
    let width = PANEL_WIDTH.min(area.width.saturating_sub(4));
    let height = PANEL_HEIGHT.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let panel = Rect { x, y, width, height };

    Clear.render(panel, buf);
    let title = match state.step {
        AuthStep::Writing | AuthStep::Done => tr("Provider setup").to_string(),
        _ => {
            let step = STEP_TITLES[step_index(state.step)];
            format!("{} — {}", tr("Provider setup"), tr(step))
        }
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme.gray_bright))
        .title(Span::styled(title, Style::new().fg(theme.text_primary)));
    let inner = block.inner(panel);
    block.render(panel, buf);

    let lines: Vec<Line> = match state.step {
        AuthStep::Protocol => render_protocol(state, &theme),
        AuthStep::BaseUrl => render_base_url(state, &theme),
        AuthStep::ApiKey => render_api_key(state, &theme),
        AuthStep::Models => render_models(state, &theme),
        AuthStep::Confirm | AuthStep::Writing => render_confirm(state, &theme),
        AuthStep::Done => render_done(state, &theme),
    };
    // 提示行与其他面板同款贴底：内容顶对齐补空行，倒数第二行固定渲染当前步
    // 按键提示，最底行保留 Step x/5。
    let footer = footer_line(state);
    let hint = hint_line(&step_hints(state));
    let content_height = inner.height.saturating_sub(2) as usize;
    let mut shown: Vec<Line> = lines.into_iter().take(content_height).collect();
    while shown.len() < content_height {
        shown.push(Line::from(""));
    }
    shown.push(hint);
    shown.push(footer);
    Paragraph::new(shown).render(inner, buf);
}

/// 当前步的按键提示（固定渲染在面板底部；文案与 [`route_auth_modal_key`]
/// 的真实行为一一对应）。
fn step_hints(state: &AuthModalState) -> Vec<(&'static str, &'static str)> {
    match state.step {
        AuthStep::Protocol => vec![("↑/↓", "move"), ("Enter", "next"), ("Esc", "cancel")],
        AuthStep::BaseUrl | AuthStep::ApiKey => vec![("Enter", "next"), ("Esc", "back")],
        AuthStep::Models if state.manual_mode => {
            vec![("Enter", "add model"), ("Esc", "cancel")]
        }
        AuthStep::Models if state.fetch_in_flight => vec![("Esc", "back")],
        AuthStep::Models => vec![
            ("space", "toggle"),
            ("a", "add manually"),
            ("r", "retry"),
            ("Enter", "confirm"),
            ("Esc", "back"),
        ],
        AuthStep::Confirm if state.editing_id => vec![("Enter/Esc", "save id")],
        AuthStep::Confirm => vec![("e", "edit id"), ("Enter", "write"), ("Esc", "back")],
        AuthStep::Writing => vec![],
        AuthStep::Done => vec![("Enter", "close")],
    }
}

fn step_index(step: AuthStep) -> usize {
    match step {
        AuthStep::Protocol => 0,
        AuthStep::BaseUrl => 1,
        AuthStep::ApiKey => 2,
        AuthStep::Models => 3,
        AuthStep::Confirm | AuthStep::Writing => 4,
        AuthStep::Done => 4,
    }
}

fn hint_line(hints: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled((*key).to_string(), Style::new().add_modifier(ratatui::style::Modifier::BOLD)));
        spans.push(Span::raw(format!(" {}", tr(label))));
    }
    Line::from(spans)
}

fn render_protocol(state: &AuthModalState, theme: &crate::theme::Theme) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(tr(
        "Pick the API protocol your provider speaks:",
    ))];
    for i in 0..3 {
        let marker = if state.protocol_cursor == i { ">" } else { " " };
        let style = if state.protocol_cursor == i {
            Style::new().fg(theme.text_primary)
        } else {
            Style::new()
        };
        lines.push(Line::from(Span::styled(
            format!("{marker} {}", protocol_label(i)),
            style,
        )));
    }
    lines
}

fn render_base_url(state: &AuthModalState, theme: &crate::theme::Theme) -> Vec<Line<'static>> {
    let invalid = !state.base_url_buf.is_empty()
        && !state.base_url_buf.starts_with("http://")
        && !state.base_url_buf.starts_with("https://");
    vec![
        Line::from(tr("Provider base URL (e.g. https://api.x.ai/v1):")),
        Line::from(Span::styled(
            format!("> {}", state.base_url_buf),
            Style::new().fg(if invalid { theme.accent_error } else { theme.text_primary }),
        )),
        Line::from(""),
        Line::from(Span::styled(
            tr("Must start with http:// or https://"),
            Style::new().fg(theme.gray_bright),
        )),
    ]
}

fn render_api_key(state: &AuthModalState, theme: &crate::theme::Theme) -> Vec<Line<'static>> {
    let masked: String = "*".repeat(state.api_key.masked_len());
    vec![
        Line::from(tr("Provider API key (stored in plain text in config.toml):")),
        Line::from(Span::styled(
            format!("> {masked}"),
            Style::new().fg(theme.text_primary),
        )),
        Line::from(""),
        Line::from(Span::styled(
            tr("Leave empty for gateways that need no auth. Ctrl+U clears."),
            Style::new().fg(theme.gray_bright),
        )),
    ]
}

fn render_models(state: &AuthModalState, theme: &crate::theme::Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if state.manual_mode {
        lines.push(Line::from(tr("Model slug (Enter to add, Esc to cancel):")));
        lines.push(Line::from(Span::styled(
            format!("> {}", state.manual_buf),
            Style::new().fg(theme.text_primary),
        )));
        return lines;
    }
    if state.fetch_in_flight {
        lines.push(Line::from(tr("Fetching model list…")));
        lines.push(Line::from(""));
        return lines;
    }
    if let Some(error) = &state.fetch_error {
        lines.push(Line::from(Span::styled(
            format!("{}: {error}", tr("Fetch failed")),
            Style::new().fg(theme.accent_error),
        )));
        lines.push(Line::from(Span::styled(
            tr("Press r to retry, or a to add a model slug manually."),
            Style::new().fg(theme.gray_bright),
        )));
        lines.push(Line::from(""));
    } else if state.fetch_error.is_none() && state.models.is_empty() {
        lines.push(Line::from(tr("No models fetched yet.")));
        lines.push(Line::from(""));
    }
    let visible = state.visible_models();
    let query = if state.filter.is_empty() {
        String::new()
    } else {
        format!("/{} ", state.filter)
    };
    let position = if visible.is_empty() {
        "0/0".to_string()
    } else {
        format!("{}/{}", state.cursor + 1, visible.len())
    };
    lines.push(Line::from(Span::styled(
        format!("{query}{} {position}", tr("space toggle")),
        Style::new().fg(theme.gray_bright),
    )));
    let max_rows = 6usize;
    let start = state.cursor.saturating_sub(max_rows - 1).min(visible.len().saturating_sub(max_rows.min(visible.len())));
    for (i, slug) in visible.iter().enumerate().skip(start).take(max_rows) {
        let marker = if state.selected.contains(*slug) { "[x]" } else { "[ ]" };
        let style = if i == state.cursor {
            Style::new().fg(theme.text_primary)
        } else {
            Style::new()
        };
        lines.push(Line::from(Span::styled(
            format!("{marker} {slug}"),
            style,
        )));
    }
    lines
}

fn render_confirm(state: &AuthModalState, theme: &crate::theme::Theme) -> Vec<Line<'static>> {
    if state.step == AuthStep::Writing {
        return vec![Line::from(tr("Writing config.toml…"))];
    }
    let mut lines = Vec::new();
    lines.push(Line::from(tr("Entries to write to ~/.grok/config.toml:")));
    if let Some(error) = &state.persist_error {
        lines.push(Line::from(Span::styled(
            format!("{}: {error}", tr("Write failed")),
            Style::new().fg(theme.accent_error),
        )));
    }
    let any_conflict = (0..state.rows.len()).any(|i| row_is_conflicted(state, i));
    for (i, row) in state.rows.iter().enumerate() {
        let editing_here = state.editing_id && i == state.confirm_cursor;
        let conflicted = row_is_conflicted(state, i);
        let id_display = if editing_here {
            format!("> {}", state.id_buf)
        } else {
            row.id.clone()
        };
        let style = if conflicted {
            Style::new().fg(theme.accent_error)
        } else if i == state.confirm_cursor {
            Style::new().fg(theme.text_primary)
        } else {
            Style::new()
        };
        let mut detail = String::new();
        if let Some(window) = row.context_window {
            detail.push_str(&format!("  ctx={window}"));
        }
        if let Some(max) = row.max_completion_tokens {
            detail.push_str(&format!("  max_out={max}"));
        }
        if row.supports_reasoning_effort {
            detail.push_str("  think=on");
        }
        let conflict_note = if conflicted { tr("  (id taken)") } else { "" };
        lines.push(Line::from(Span::styled(
            format!("[{id_display}] model={}{}{detail}{conflict_note}", row.model, ""),
            style,
        )));
    }
    if any_conflict {
        lines.push(Line::from(Span::styled(
            tr("Resolve highlighted id conflicts before writing (e to edit)."),
            Style::new().fg(theme.accent_error),
        )));
    }
    lines
}

fn render_done(state: &AuthModalState, theme: &crate::theme::Theme) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(
            tr("Config written."),
            Style::new().fg(theme.text_primary),
        )),
        Line::from(""),
        Line::from(tr(
            "The new models are available via /model (hot reload).",
        )),
    ];
    if let Some(error) = &state.reload_error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            tr("Model reload failed"),
            Style::new().fg(theme.accent_error),
        )));
        lines.push(Line::from(Span::styled(
            format!("{error}"),
            Style::new().fg(theme.gray_bright),
        )));
        lines.push(Line::from(Span::styled(
            tr("Restart to pick up the new models in /model."),
            Style::new().fg(theme.gray_bright),
        )));
    }
    lines
}

fn footer_line(state: &AuthModalState) -> Line<'static> {
    let text = match state.step {
        AuthStep::Writing => tr("Writing…").to_string(),
        _ => format!("{} {}/5", tr("Step"), step_index(state.step) + 1),
    };
    Line::from(Span::styled(text, Style::new().fg(crate::theme::Theme::current().gray_bright)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;
    use xai_grok_shell::extensions::provider_models::ProviderModel;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: crossterm::event::KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn modal() -> AuthModalState {
        AuthModalState::new(BTreeSet::new())
    }

    /// 渲染到指定尺寸的缓冲并取出某一行的文本。
    fn row_text(area: Rect, state: &mut AuthModalState, y: u16) -> String {
        let mut buf = ratatui::buffer::Buffer::empty(area);
        render_auth_modal(&mut buf, area, state);
        (0..area.width)
            .map(|x| buf.cell((x, y)).map_or(" ", |c| c.symbol()).to_string())
            .collect()
    }

    #[test]
    fn hint_row_is_pinned_above_footer() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 66,
            height: 18,
        };
        // 面板在 area 内居中（与 render_auth_modal 同公式），行号按此推导：
        // 内区最底行 = Step x/5 页脚，其上一行 = 当前步按键提示。
        let height = PANEL_HEIGHT.min(area.height.saturating_sub(4));
        let y = (area.height.saturating_sub(height)) / 2;
        let footer_row = y + height - 2;
        let hint_row = footer_row - 1;
        let mut state = modal();
        assert!(row_text(area, &mut state, footer_row).contains("Step"));
        let hint = row_text(area, &mut state, hint_row);
        assert!(hint.contains("Esc"), "hint must be bottom-pinned: {hint:?}");
        assert!(hint.contains("Enter"), "hint lists keys: {hint:?}");
        // 提示行不在内容流中间（首内容行是标题文案）。
        let first = row_text(area, &mut state, y + 1);
        assert!(!first.contains("Esc"), "hint must not leak into content: {first:?}");
    }

    fn walk_to_models(state: &mut AuthModalState) -> AuthModalOutcome {
        state.step = AuthStep::BaseUrl;
        state.base_url_buf = "https://api.example.com/v1".to_string();
        state.step = AuthStep::ApiKey;
        route_auth_modal_key(state, &key(KeyCode::Enter))
    }

    #[test]
    fn secret_key_debug_is_redacted() {
        let mut key_buf = SecretKey::new("sk-super-secret");
        key_buf.push_str("-tail");
        let rendered = format!("{key_buf:?}");
        assert!(!rendered.contains("sk-super-secret"), "{rendered}");
        assert!(rendered.contains("[redacted]"));
        assert_eq!(key_buf.expose(), "sk-super-secret-tail");
        assert_eq!(key_buf.masked_len(), 20);
        key_buf.pop_char();
        key_buf.clear();
        assert!(key_buf.is_empty());
    }

    #[test]
    fn wizard_reaches_models_and_fetch_outcome_carries_credentials() {
        let mut state = modal();
        // 协议 → baseUrl → SK → Models 步（Enter 触发取数）。
        assert_eq!(state.step, AuthStep::Protocol);
        route_auth_modal_key(&mut state, &key(KeyCode::Enter));
        assert_eq!(state.step, AuthStep::BaseUrl);
        for c in "https://api.example.com/v1".chars() {
            route_auth_modal_key(&mut state, &key(KeyCode::Char(c)));
        }
        route_auth_modal_key(&mut state, &key(KeyCode::Enter));
        assert_eq!(state.step, AuthStep::ApiKey);
        for c in "sk-abc".chars() {
            route_auth_modal_key(&mut state, &key(KeyCode::Char(c)));
        }
        let outcome = route_auth_modal_key(&mut state, &key(KeyCode::Enter));
        assert_eq!(state.step, AuthStep::Models);
        match outcome {
            AuthModalOutcome::FetchModels { base_url, api_key } => {
                assert_eq!(base_url, "https://api.example.com/v1");
                assert_eq!(api_key.expose(), "sk-abc");
            }
            other => panic!("expected FetchModels, got {other:?}"),
        }
        assert!(state.fetch_in_flight);
        // 在途期间重复触发不发新请求。
        assert_eq!(begin_fetch(&mut state), AuthModalOutcome::Changed);
    }

    #[test]
    fn base_url_requires_scheme() {
        let mut state = modal();
        route_auth_modal_key(&mut state, &key(KeyCode::Enter)); // → BaseUrl
        for c in "api.example.com/v1".chars() {
            route_auth_modal_key(&mut state, &key(KeyCode::Char(c)));
        }
        route_auth_modal_key(&mut state, &key(KeyCode::Enter));
        assert_eq!(state.step, AuthStep::BaseUrl, "missing scheme must block");
        state.base_url_buf = "https://api.example.com/v1/".to_string();
        route_auth_modal_key(&mut state, &key(KeyCode::Enter));
        assert_eq!(state.step, AuthStep::ApiKey);
        assert_eq!(state.base_url_buf, "https://api.example.com/v1", "trailing slash trimmed");
    }

    #[test]
    fn models_response_applies_and_stale_guard_holds() {
        let mut state = modal();
        assert_eq!(walk_to_models(&mut state), AuthModalOutcome::FetchModels { base_url: "https://api.example.com/v1".into(), api_key: SecretKey::new(String::new()) });
        let response = ProviderModelsResponse {
            base_url: "https://api.example.com/v1".into(),
            models: vec![ProviderModel { id: "m-1".into(), owned_by: None }, ProviderModel { id: "m-2".into(), owned_by: None }],
            error: None,
        };
        assert!(state.accepts_provider_models_response(&response));
        state.apply_provider_models_response(response);
        assert!(!state.fetch_in_flight);
        assert_eq!(state.models, vec!["m-1", "m-2"]);
        // 手填 slug 在回包后保持勾选。
        state.selected.insert("m-1".to_string());
        // 陈旧回包（base_url 已改）被拒。
        let mut stale = AuthModalState::new(BTreeSet::new());
        stale.step = AuthStep::Models;
        stale.fetch_in_flight = true;
        stale.requested_base_url = Some("https://other.example.com".into());
        assert!(!stale.accepts_provider_models_response(&ProviderModelsResponse {
            base_url: "https://api.example.com/v1".into(),
            models: vec![],
            error: None,
        }));
    }

    #[test]
    fn fetch_error_shows_and_retry_clears() {
        let mut state = modal();
        walk_to_models(&mut state);
        state.apply_provider_models_response(ProviderModelsResponse {
            base_url: "https://api.example.com/v1".into(),
            models: vec![],
            error: Some("HTTP 401".into()),
        });
        assert_eq!(state.fetch_error.as_deref(), Some("HTTP 401"));
        // r 重试发起新请求。
        let outcome = route_auth_modal_key(&mut state, &key(KeyCode::Char('r')));
        assert!(matches!(outcome, AuthModalOutcome::FetchModels { .. }));
        assert!(state.fetch_in_flight && state.fetch_error.is_none());
    }

    #[test]
    fn filter_and_toggle_then_enter_builds_rows_and_moves_to_confirm() {
        let mut state = modal();
        walk_to_models(&mut state);
        state.apply_provider_models_response(ProviderModelsResponse {
            base_url: "https://api.example.com/v1".into(),
            models: vec![ProviderModel { id: "glm-5.3-flash".into(), owned_by: None }, ProviderModel { id: "deepseek/deepseek-v4-flash".into(), owned_by: None }],
            error: None,
        });
        // 过滤收窄。
        route_auth_modal_key(&mut state, &key(KeyCode::Char('g')));
        route_auth_modal_key(&mut state, &key(KeyCode::Char('l')));
        assert_eq!(state.visible_models().len(), 1);
        // 空格勾选 + Enter 进确认步。
        route_auth_modal_key(&mut state, &key(KeyCode::Char(' ')));
        assert!(state.selected.contains("glm-5.3-flash"));
        let outcome = route_auth_modal_key(&mut state, &key(KeyCode::Enter));
        assert_eq!(state.step, AuthStep::Confirm);
        match outcome {
            AuthModalOutcome::FetchCatalog { model_ids } => {
                assert_eq!(model_ids, vec!["glm-5.3-flash"]);
            }
            other => panic!("expected FetchCatalog, got {other:?}"),
        }
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.rows[0].model, "glm-5.3-flash");
        assert_eq!(state.rows[0].id, "glm-5-3-flash", "dots fold into dashes");
    }

    #[test]
    fn enter_without_selection_stays_on_models() {
        let mut state = modal();
        walk_to_models(&mut state);
        state.apply_provider_models_response(ProviderModelsResponse {
            base_url: "https://api.example.com/v1".into(),
            models: vec![ProviderModel { id: "m-1".into(), owned_by: None }],
            error: None,
        });
        assert_eq!(route_auth_modal_key(&mut state, &key(KeyCode::Enter)), AuthModalOutcome::Changed);
        assert_eq!(state.step, AuthStep::Models);
    }

    #[test]
    fn manual_add_selects_and_survives_response() {
        let mut state = modal();
        walk_to_models(&mut state);
        // 取数失败 → a 手填。
        state.fetch_error = Some("HTTP 500".into());
        route_auth_modal_key(&mut state, &key(KeyCode::Char('a')));
        assert!(state.manual_mode);
        for c in "my-custom-slug".chars() {
            route_auth_modal_key(&mut state, &key(KeyCode::Char(c)));
        }
        route_auth_modal_key(&mut state, &key(KeyCode::Enter));
        assert!(!state.manual_mode);
        assert!(state.selected.contains("my-custom-slug"));
        // 后续回包不带它：勾选保留，但 slug 不在列表里也保留手填项。
        state.apply_provider_models_response(ProviderModelsResponse {
            base_url: "https://api.example.com/v1".into(),
            models: vec![ProviderModel { id: "m-1".into(), owned_by: None }],
            error: None,
        });
        assert!(state.models.contains(&"my-custom-slug".to_string()));
    }

    #[test]
    fn id_conflicts_block_persist_until_resolved() {
        let mut state = AuthModalState::new(BTreeSet::from(["taken".to_string()]));
        state.step = AuthStep::Confirm;
        state.rows = vec![ConfirmRow {
            model: "m-1".into(),
            id: "taken".into(),
            context_window: None,
            max_completion_tokens: None,
            supports_reasoning_effort: false,
        }];
        // 冲突 → Enter 只刷屏不提交。
        assert_eq!(route_auth_modal_key(&mut state, &key(KeyCode::Enter)), AuthModalOutcome::Changed);
        assert_eq!(state.step, AuthStep::Confirm);
        assert!(row_is_conflicted(&state, 0));
        // e 编辑 id → 改名 → Enter 提交。
        route_auth_modal_key(&mut state, &key(KeyCode::Char('e')));
        assert!(state.editing_id);
        state.id_buf = "free".to_string();
        route_auth_modal_key(&mut state, &key(KeyCode::Enter)); // 提交编辑
        assert!(!state.editing_id);
        assert_eq!(state.rows[0].id, "free");
        state.base_url_buf = "https://api.example.com/v1".into();
        state.api_key = SecretKey::new("sk-x");
        let outcome = route_auth_modal_key(&mut state, &key(KeyCode::Enter));
        match outcome {
            AuthModalOutcome::Persist { entries } => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].id, "free");
                assert_eq!(entries[0].model, "m-1");
                assert_eq!(entries[0].base_url, "https://api.example.com/v1");
                assert_eq!(entries[0].api_key, "sk-x");
            }
            other => panic!("expected Persist, got {other:?}"),
        }
        assert_eq!(state.step, AuthStep::Writing);
        // Writing 步吞 Esc；回包推进。
        assert_eq!(route_auth_modal_key(&mut state, &key(KeyCode::Esc)), AuthModalOutcome::Unchanged);
        state.apply_persist_result(Ok(()));
        assert_eq!(state.step, AuthStep::Done);
        assert_eq!(route_auth_modal_key(&mut state, &key(KeyCode::Enter)), AuthModalOutcome::Close);
    }

    #[test]
    fn persist_error_returns_to_confirm() {
        let mut state = modal();
        state.step = AuthStep::Writing;
        state.rows = vec![ConfirmRow {
            model: "m".into(),
            id: "m".into(),
            context_window: None,
            max_completion_tokens: None,
            supports_reasoning_effort: false,
        }];
        state.apply_persist_result(Err("boom".into()));
        assert_eq!(state.step, AuthStep::Confirm);
        assert_eq!(state.persist_error.as_deref(), Some("boom"));
    }

    #[test]
    fn paste_lands_in_active_buffer() {
        let mut state = modal();
        state.step = AuthStep::BaseUrl;
        assert_eq!(handle_auth_paste(&mut state, "https://x.ai/v1"), AuthModalOutcome::Changed);
        assert_eq!(state.base_url_buf, "https://x.ai/v1");
        state.step = AuthStep::ApiKey;
        assert_eq!(handle_auth_paste(&mut state, "sk-paste"), AuthModalOutcome::Changed);
        assert_eq!(state.api_key.expose(), "sk-paste");
        // 列表步非手填态不吃粘贴。
        state.step = AuthStep::Models;
        assert_eq!(handle_auth_paste(&mut state, "junk"), AuthModalOutcome::Unchanged);
    }

    #[test]
    fn derive_entry_id_sanitizes_and_dedupes() {
        let taken = BTreeSet::from(["glm-5-3-flash".to_string()]);
        assert_eq!(derive_entry_id("glm-5.3-flash", &taken), "glm-5-3-flash-2");
        assert_eq!(derive_entry_id("deepseek/deepseek-v4-flash", &BTreeSet::new()), "deepseek-deepseek-v4-flash");
        assert_eq!(derive_entry_id("///", &BTreeSet::new()), "model", "all-folded slug falls back to model");
    }

    #[test]
    fn backfill_filters_fallback_context_window() {
        use xai_grok_model_catalog::{CatalogSource, ModelPricing};
        use xai_grok_shell::extensions::model_catalog::{CatalogMatch as ShellMatch, ModelCatalogResponse};
        // 与新建兜底相同的目录值不写（拍板「和默认配置值不一样」）。
        let mut entries = std::collections::BTreeMap::new();
        entries.insert(
            "auth-panel-ctx-fallback".to_string(),
            Some(ShellMatch {
                source: CatalogSource::OpenRouter,
                model_id: "auth-panel-ctx-fallback".into(),
                provider_id: "openrouter".into(),
                name: "n".into(),
                match_score: 100,
                pricing: ModelPricing::default(),
                context_length: Some(CONTEXT_WINDOW_FALLBACK),
                max_completion_tokens: Some(131_072),
                reasoning: Some(false),
            }),
        );
        entries.insert(
            "auth-panel-ctx-real".to_string(),
            Some(ShellMatch {
                source: CatalogSource::ModelsDev,
                model_id: "auth-panel-ctx-real".into(),
                provider_id: "zai".into(),
                name: "n".into(),
                match_score: 100,
                pricing: ModelPricing::default(),
                context_length: Some(1_048_576),
                max_completion_tokens: None,
                reasoning: Some(true),
            }),
        );
        crate::app::model_catalog_store::install(&ModelCatalogResponse {
            entries,
            fetched_at_ms: 1,
            cache: "fetch".into(),
            error: None,
        });
        let (window, max, reasoning) = catalog_backfill("auth-panel-ctx-fallback");
        assert_eq!(window, None, "fallback value must be omitted");
        assert_eq!(max, Some(131_072));
        assert!(!reasoning, "reasoning=false 不写支持位");
        let (window, max, reasoning) = catalog_backfill("auth-panel-ctx-real");
        assert_eq!(window, Some(1_048_576));
        assert_eq!(max, None, "None stays None (do not send)");
        assert!(reasoning, "reasoning=true 写支持位");
        // 目录未命中 = 能力未知，不写值。
        let backfill = catalog_backfill("unknown-model");
        assert_eq!(backfill, (None, None, false));
    }
}
