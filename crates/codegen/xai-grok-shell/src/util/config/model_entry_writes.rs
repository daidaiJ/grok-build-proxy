//! LOCAL: /auth 面板的 `[model.<id>]` 写回 —— toml_edit 定点编辑 + config 写守卫。
//!
//! 拍板（`docs-local/provider-onboarding-api-cost-todo.md` 七轮）：SK 明文 `api_key`
//! 直写；条目 id 由用户给缩写（唯一即可，已存在即拒绝，不做覆盖）；回写机制走
//! toml_edit DocumentMut（注释/未知段/格式保留），守卫与 settings 写回同源
//! （[`lock_config_writes`] = SAVE_LOCK ⊃ init flock）。目录回填值（context_window /
//! max_completion_tokens）由调用方按「与新建模型兜底不同才写」预筛后传入。

use anyhow::{Context, Result};
use std::path::Path;
use xai_grok_config::fs_atomic::BoundDest;
use xai_grok_sampling_types::ApiBackend;

/// 一条待写回的 BYOK 模型条目（/auth 面板产出）。
///
/// `Debug` 手写打码：draft 携带明文 SK，任何 `{:?}` 落盘都不得出现密钥。
#[derive(Clone, PartialEq)]
pub struct ModelEntryDraft {
    /// 用户给的配置缩写 id（`[model.<id>]` 的 `<id>`，唯一即可）。
    pub id: String,
    /// 路由 slug（API 请求里发送的 model 名）。
    pub model: String,
    /// 供应商 base URL，如 `https://api.x.ai/v1`。
    pub base_url: String,
    /// 供应商 API key（拍板：明文直写 `api_key`）。
    pub api_key: String,
    /// 协议；`ChatCompletions` 为 config 缺省，写回时省略。
    pub api_backend: ApiBackend,
    /// 目录回填的上下文窗口（调用方按「与兜底不同才写」预筛）。
    pub context_window: Option<u64>,
    /// 目录回填的最大输出（缺省语义 = 不发送，目录有值即写）。
    pub max_completion_tokens: Option<u64>,
    /// 目录回填的思考支持位（目录 `reasoning` = Some(true) 才写 true，
    /// 开启既有 reasoning-effort 机制；缺省不写）。
    pub supports_reasoning_effort: bool,
}

impl std::fmt::Debug for ModelEntryDraft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelEntryDraft")
            .field("id", &self.id)
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("api_key", &"[redacted]")
            .field("api_backend", &self.api_backend)
            .field("context_window", &self.context_window)
            .field("max_completion_tokens", &self.max_completion_tokens)
            .field("supports_reasoning_effort", &self.supports_reasoning_effort)
            .finish()
    }
}

/// `ApiBackend` 的 config 拼写（须与 serde `rename_all = "snake_case"` 一致）。
fn api_backend_str(backend: &ApiBackend) -> &'static str {
    match backend {
        ApiBackend::ChatCompletions => "chat_completions",
        ApiBackend::Responses => "responses",
        ApiBackend::Messages => "messages",
    }
}

/// 把一批 BYOK 条目写回用户 `config.toml` 的 `[model.<id>]`。
///
/// 全批原子：任一 id 冲突则整批拒绝、文件不动。已有同 id 条目一律拒绝
/// （拍板「id 唯一」，不做覆盖，避免静默丢手工字段）。错误消息不携带 SK。
pub async fn write_model_entries(entries: &[ModelEntryDraft]) -> Result<()> {
    // 'static 闭包需要 owned 数据：draft 含明文 SK，仅在本函数栈内短暂持有。
    let owned = entries.to_vec();
    let guard = super::persist::lock_config_writes()
        .await
        .map_err(|error| anyhow::anyhow!("lock config.toml to save model entries: {error}"))?;
    guard
        .run_blocking(move || write_model_entries_locked(&owned))
        .await
        .map_err(|error| anyhow::anyhow!("model entry persistence task failed: {error}"))?
}

fn write_model_entries_locked(entries: &[ModelEntryDraft]) -> Result<()> {
    let path = super::mcp::user_config_path();
    let (destination, content) = super::persist::read_follow_bound(&path)
        .map_err(|error| anyhow::anyhow!("read {} to save model entries: {error}", path.display()))?;
    let mut doc = parse_config_document(&content, &path)?;
    let model_table = doc
        .entry("model")
        .or_insert(toml_edit::table())
        .as_table_mut()
        .with_context(|| format!("{}: [model] must be a table", path.display()))?;
    for entry in entries {
        if model_table.contains_key(entry.id.as_str()) {
            anyhow::bail!(
                "{}: [model.{}] already exists; pick a different id (existing entries are never overwritten)",
                path.display(),
                entry.id,
            );
        }
    }
    for entry in entries {
        model_table.insert(entry.id.as_str(), draft_to_item(entry));
    }
    let content = doc.to_string();
    super::persist::atomic_write_follow_bound(&path, &destination, &content)
        .map_err(|error| anyhow::anyhow!("write {} to save model entries: {error}", path.display()))
}

/// 解析现有 config 为 toml_edit 文档；空白文件视作空文档，非空白不可解析一律拒绝
/// （宁可让用户手修语法，也不覆盖丢字段）。
fn parse_config_document(content: &str, path: &Path) -> Result<toml_edit::DocumentMut> {
    if content.trim().is_empty() {
        return Ok(toml_edit::DocumentMut::new());
    }
    content.parse::<toml_edit::DocumentMut>().map_err(|error| {
        anyhow::anyhow!(
            "refusing to overwrite unparseable {}: {error}; \
             save a backup and fix the syntax error before retrying",
            path.display(),
        )
    })
}

/// 条目 → toml_edit 表。键序 = 用户手写 config 的惯例序；`ChatCompletions`
/// 是 serde 缺省值，省略不写（「与默认不同才写」同口径）。
fn draft_to_item(entry: &ModelEntryDraft) -> toml_edit::Item {
    let mut table = toml_edit::Table::new();
    table.insert("model", toml_edit::value(entry.model.as_str()));
    table.insert("base_url", toml_edit::value(entry.base_url.as_str()));
    table.insert("api_key", toml_edit::value(entry.api_key.as_str()));
    if entry.api_backend != ApiBackend::ChatCompletions {
        table.insert(
            "api_backend",
            toml_edit::value(api_backend_str(&entry.api_backend)),
        );
    }
    if let Some(window) = entry.context_window {
        table.insert("context_window", toml_edit::value(window as i64));
    }
    if let Some(max_tokens) = entry.max_completion_tokens {
        table.insert("max_completion_tokens", toml_edit::value(max_tokens as i64));
    }
    if entry.supports_reasoning_effort {
        table.insert("supports_reasoning_effort", toml_edit::value(true));
    }
    toml_edit::Item::Table(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(id: &str, model: &str, api_backend: ApiBackend) -> ModelEntryDraft {
        ModelEntryDraft {
            id: id.to_string(),
            model: model.to_string(),
            base_url: "https://api.example.com/v1".to_string(),
            api_key: "sk-test-secret".to_string(),
            api_backend,
            context_window: None,
            max_completion_tokens: None,
            supports_reasoning_effort: false,
        }
    }

    fn write_at(path: &Path, entries: &[ModelEntryDraft]) {
        let (destination, content) = super::super::persist::read_follow_bound(path).unwrap();
        let mut doc = parse_config_document(&content, path).unwrap();
        let model_table = doc
            .entry("model")
            .or_insert(toml_edit::table())
            .as_table_mut()
            .unwrap();
        for entry in entries {
            assert!(
                !model_table.contains_key(entry.id.as_str()),
                "test precondition: id must be free"
            );
            model_table.insert(entry.id.as_str(), draft_to_item(entry));
        }
        super::super::persist::atomic_write_follow_bound(path, &destination, &doc.to_string())
            .unwrap();
    }

    #[test]
    fn round_trip_preserves_comments_and_siblings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "# my tuning\n[ui]\ncompact_mode = false\n\n[model.\"old/vendor\"]\nmodel = \"old\"\n",
        )
        .unwrap();

        write_at(
            &path,
            &[ModelEntryDraft {
                context_window: Some(1_048_576),
                max_completion_tokens: Some(131_072),
                supports_reasoning_effort: true,
                ..draft("glm", "glm-5.3-flash", ApiBackend::Messages)
            }],
        );

        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("# my tuning"), "comments must survive:\n{body}");
        assert!(body.contains("compact_mode"), "siblings must survive:\n{body}");
        assert!(body.contains("[model.\"old/vendor\"]"), "sibling model must survive:\n{body}");
        assert!(
            body.contains("api_backend = \"messages\""),
            "non-default backend must be written:\n{body}"
        );
        assert!(body.contains("context_window = 1048576"), "backfill must be written:\n{body}");
        assert!(body.contains("max_completion_tokens = 131072"), "backfill must be written:\n{body}");
        assert!(
            body.contains("supports_reasoning_effort = true"),
            "reasoning backfill must be written:\n{body}"
        );
        assert!(
            !body.contains("api_backend") || body.matches("api_backend").count() == 1,
            "default backend must be omitted:\n{body}"
        );

        // 写回结果必须能被正式 config 解析器读回。
        let parsed: toml::Value = toml::from_str(&body).unwrap();
        let entry = &parsed["model"]["glm"];
        assert_eq!(entry["model"].as_str(), Some("glm-5.3-flash"));
        assert_eq!(entry["context_window"].as_integer(), Some(1_048_576));
    }

    #[test]
    fn default_backend_omitted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_at(&path, &[draft("m", "m-1", ApiBackend::ChatCompletions)]);
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(!body.contains("api_backend"), "default backend must be omitted:\n{body}");
        assert!(
            !body.contains("supports_reasoning_effort"),
            "false reasoning flag must be omitted:\n{body}"
        );
    }

    #[test]
    fn slash_and_dot_ids_stay_single_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_at(
            &path,
            &[
                draft("deepseek/deepseek-v4-flash", "deepseek-v4-flash", ApiBackend::ChatCompletions),
                draft("a.b", "dot-model", ApiBackend::ChatCompletions),
            ],
        );
        let body = std::fs::read_to_string(&path).unwrap();
        let parsed: toml::Value = toml::from_str(&body).unwrap();
        let model = parsed["model"].as_table().unwrap();
        assert_eq!(model.len(), 2, "slash/dot ids must be single quoted keys:\n{body}");
        assert!(model.contains_key("deepseek/deepseek-v4-flash"));
        assert!(model.contains_key("a.b"));
    }

    #[test]
    fn debug_output_never_carries_the_key() {
        let rendered = format!("{:?}", draft("m", "m-1", ApiBackend::ChatCompletions));
        assert!(!rendered.contains("sk-test-secret"), "Debug must redact the key: {rendered}");
        assert!(rendered.contains("[redacted]"));
    }

    #[test]
    fn blank_file_writes_fresh_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "   \n").unwrap();
        write_at(&path, &[draft("m", "m-1", ApiBackend::ChatCompletions)]);
        let body = std::fs::read_to_string(&path).unwrap();
        let parsed: toml::Value = toml::from_str(&body).unwrap();
        assert_eq!(parsed["model"]["m"]["model"].as_str(), Some("m-1"));
    }

    #[test]
    fn missing_file_writes_with_parents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.toml");
        write_at(&path, &[draft("m", "m-1", ApiBackend::ChatCompletions)]);
        assert!(path.exists(), "missing file and parents must be created");
    }

    #[test]
    fn unparseable_file_is_refused_and_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let bad = "this is [not valid toml\n";
        std::fs::write(&path, bad).unwrap();
        let result = parse_config_document(bad, &path);
        assert!(result.is_err(), "non-blank unparseable must refuse");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), bad, "file must be untouched");
    }
}
