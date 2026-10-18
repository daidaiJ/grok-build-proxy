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

/// LOCAL（视觉桥 D8 + 目录思考位回填）：目录取数成功后，把目录判定的能力位
/// 回填进用户 `config.toml` **已有**的 `[model.<id>]` 条目。
///
/// 规则：
/// - 只碰用户文件里已存在的条目（`/auth` 新建语义不在这里发生）；
/// - **显式字段永远赢**：条目已写 `accepts_images` / `supports_reasoning_effort` /
///   `reasoning_efforts` 任一键（含显式 `false` / 空表）时该键跳过，永不覆盖；
/// - `accepts_images`：回填值与「无显式键时的解析缺省」相同则跳过（避免无谓的
///   config 重写）：条目 id 在内建/预取目录里（`catalog_keys` 命中）缺省 true，
///   否则（用户新建）缺省 false；
/// - `supports_reasoning_effort` / `reasoning_efforts`：目录 `reasoning` 为 true
///   才写；档位列表先按 `ReasoningEffort` 枚举过滤未知值，全未知不写（档位方言
///   留给用户钉，如 GLM 系 low/high/max、拒 medium）；
/// - 目录未命中或目录未给该字段 → 跳过。
///
/// 返回实际写入的条目数；写回失败只记日志（回填是尽力而为的增强，不阻塞取数）。
pub(crate) async fn backfill_model_capabilities(
    catalog: &xai_grok_model_catalog::ModelCatalog,
    catalog_keys: &[String],
) -> Result<usize> {
    let catalog = catalog.clone();
    let catalog_keys: std::collections::BTreeSet<String> =
        catalog_keys.iter().cloned().collect();
    let guard = super::persist::lock_config_writes()
        .await
        .map_err(|error| anyhow::anyhow!("lock config.toml for capability backfill: {error}"))?;
    guard
        .run_blocking(move || backfill_model_capabilities_locked(&catalog, &catalog_keys))
        .await
        .map_err(|error| anyhow::anyhow!("capability backfill task failed: {error}"))?
}

fn backfill_model_capabilities_locked(
    catalog: &xai_grok_model_catalog::ModelCatalog,
    catalog_keys: &std::collections::BTreeSet<String>,
) -> Result<usize> {
    backfill_model_capabilities_locked_at(
        catalog,
        catalog_keys,
        &super::mcp::user_config_path(),
    )
}

/// 测试与真实路径共用的实现体；`path` 由调用方给定（生产 = 用户 config）。
fn backfill_model_capabilities_locked_at(
    catalog: &xai_grok_model_catalog::ModelCatalog,
    catalog_keys: &std::collections::BTreeSet<String>,
    path: &std::path::Path,
) -> Result<usize> {
    let (destination, content) = super::persist::read_follow_bound(path).map_err(|error| {
        anyhow::anyhow!(
            "read {} for capability backfill: {error}",
            path.display()
        )
    })?;
    let mut doc = parse_config_document(&content, &path)?;
    let Some(model_table) = doc.get_mut("model").and_then(|i| i.as_table_mut()) else {
        return Ok(0);
    };
    // 先取键快照：匹配器要 `&str`，且借用不能横跨 `get_mut`。
    let keys: Vec<String> = model_table
        .iter()
        .map(|(key, _)| key.to_owned())
        .collect();
    let mut written = 0usize;
    for key in &keys {
        let Some(entry) = model_table
            .get_mut(key.as_str())
            .and_then(|i| i.as_table_mut())
        else {
            continue;
        };
        let matched = xai_grok_model_catalog::match_model(catalog, key);
        let mut changed = false;

        // accepts_images（视觉桥 D8，语义不变）：显式键永远赢。
        if !entry.contains_key("accepts_images") {
            if let Some(capability) = matched.as_ref().and_then(|m| m.entry.accepts_images) {
                // 无显式键时的解析缺省：覆盖内建/预取条目 → true；用户新建条目 → false。
                let would_be_default = catalog_keys.contains(key);
                if capability != would_be_default {
                    entry.insert("accepts_images", toml_edit::value(capability));
                    changed = true;
                    tracing::info!(
                        model_key = %key,
                        accepts_images = capability,
                        "model catalog: accepts_images backfilled from catalog (vision bridge D8)"
                    );
                }
            }
        }

        // 思考位 + 档位（目录思考位回填）：目录 reasoning 为 true 才动；显式键
        // （含显式 false / 空表）永远赢，档位按枚举过滤未知值、全未知不写。
        if matched.as_ref().is_some_and(|m| m.entry.reasoning == Some(true)) {
            if !entry.contains_key("supports_reasoning_effort") {
                entry.insert("supports_reasoning_effort", toml_edit::value(true));
                changed = true;
                tracing::info!(
                    model_key = %key,
                    "model catalog: supports_reasoning_effort backfilled from catalog"
                );
            }
            if !entry.contains_key("reasoning_efforts") {
                let levels = matched
                    .as_ref()
                    .and_then(|m| m.entry.reasoning_efforts.as_deref())
                    .and_then(enum_valid_efforts);
                if let Some(levels) = levels {
                    let mut efforts = toml_edit::Array::new();
                    for level in &levels {
                        efforts.push(level.as_str());
                    }
                    entry.insert("reasoning_efforts", toml_edit::value(efforts));
                    changed = true;
                    tracing::info!(
                        model_key = %key,
                        efforts = ?levels,
                        "model catalog: reasoning_efforts backfilled from catalog"
                    );
                }
            }
        }

        if changed {
            written += 1;
        }
    }
    if written == 0 {
        return Ok(0);
    }
    let content = doc.to_string();
    super::persist::atomic_write_follow_bound(path, &destination, &content).map_err(|error| {
        anyhow::anyhow!(
            "write {} for capability backfill: {error}",
            path.display()
        )
    })?;
    Ok(written)
}

/// 目录档位字符串按 `ReasoningEffort` 枚举过滤（小写归一在解析层已做）；
/// 全部未知 → `None`（不写空表）。
fn enum_valid_efforts(levels: &[String]) -> Option<Vec<String>> {
    let valid: Vec<String> = levels
        .iter()
        .filter(|level| level.parse::<xai_grok_sampling_types::ReasoningEffort>().is_ok())
        .cloned()
        .collect();
    (!valid.is_empty()).then_some(valid)
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

#[cfg(test)]
mod backfill_capability_tests {
    use super::*;
    use xai_grok_model_catalog::{CatalogEntry, CatalogSource, ModelCatalog, ModelPricing};

    /// 目录条目：id 与用户 config 键按 by_id 精确命中。
    fn catalog_entry(id: &str, reasoning: Option<bool>, efforts: Option<Vec<&str>>) -> CatalogEntry {
        CatalogEntry {
            id: id.to_string(),
            provider_id: "zai".to_string(),
            source: CatalogSource::ModelsDev,
            name: format!("name-of-{id}"),
            canonical_id: None,
            reasoning,
            reasoning_efforts: efforts.map(|levels| levels.into_iter().map(String::from).collect()),
            accepts_images: None,
            pricing: ModelPricing::default(),
            context_length: Some(128_000),
            max_completion_tokens: None,
        }
    }

    fn config_with_entry(id: &str, body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, body).unwrap();
        (dir, path)
    }

    fn model_entry_toml(id: &str, extra: &str) -> String {
        format!("[model.\"{id}\"]\nmodel = \"{id}\"\napi_key = \"sk-test-secret\"\n{extra}")
    }

    fn read(path: &std::path::Path) -> toml::Value {
        toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn reasoning_flag_and_levels_backfilled_when_keys_missing() {
        let catalog = ModelCatalog::new(vec![catalog_entry(
            "glm-5.3-flash",
            Some(true),
            Some(vec!["max", "high", "low"]),
        )]);
        let (_dir, path) = config_with_entry(
            "glm-5.3-flash",
            &model_entry_toml("glm-5.3-flash", "context_window = 1048576\n"),
        );
        let written = backfill_model_capabilities_locked_at(
            &catalog,
            &std::collections::BTreeSet::new(),
            &path,
        )
        .unwrap();
        assert_eq!(written, 1);
        let parsed = read(&path);
        let entry = &parsed["model"]["glm-5.3-flash"];
        assert_eq!(entry["supports_reasoning_effort"].as_bool(), Some(true));
        let efforts: Vec<&str> = entry["reasoning_efforts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(efforts, ["max", "high", "low"], "档位保目录源序");
        assert_eq!(entry["context_window"].as_integer(), Some(1_048_576));
    }

    #[test]
    fn explicit_false_and_explicit_levels_are_never_overwritten() {
        let catalog = ModelCatalog::new(vec![catalog_entry(
            "glm-5.3-flash",
            Some(true),
            Some(vec!["max", "high", "low"]),
        )]);
        let (_dir, path) = config_with_entry(
            "glm-5.3-flash",
            &model_entry_toml(
                "glm-5.3-flash",
                "supports_reasoning_effort = false\nreasoning_efforts = [\"high\"]\n",
            ),
        );
        let written = backfill_model_capabilities_locked_at(
            &catalog,
            &std::collections::BTreeSet::new(),
            &path,
        )
        .unwrap();
        assert_eq!(written, 0, "显式键（含 false / 既有列表）一律不碰");
        let parsed = read(&path);
        let entry = &parsed["model"]["glm-5.3-flash"];
        assert_eq!(entry["supports_reasoning_effort"].as_bool(), Some(false));
        assert_eq!(
            entry["reasoning_efforts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<&str>>(),
            ["high"]
        );
    }

    #[test]
    fn empty_levels_array_counts_as_explicit_and_is_kept() {
        let catalog = ModelCatalog::new(vec![catalog_entry(
            "glm-5.3-flash",
            Some(true),
            Some(vec!["max", "high", "low"]),
        )]);
        let (_dir, path) = config_with_entry(
            "glm-5.3-flash",
            &model_entry_toml("glm-5.3-flash", "reasoning_efforts = []\n"),
        );
        let written = backfill_model_capabilities_locked_at(
            &catalog,
            &std::collections::BTreeSet::new(),
            &path,
        )
        .unwrap();
        assert_eq!(written, 1, "布尔位缺失仍回填 true");
        let parsed = read(&path);
        let entry = &parsed["model"]["glm-5.3-flash"];
        assert_eq!(entry["supports_reasoning_effort"].as_bool(), Some(true));
        assert_eq!(
            entry["reasoning_efforts"].as_array().unwrap().len(),
            0,
            "显式空表 = 用户意图，不代写档位"
        );
    }

    #[test]
    fn catalog_without_reasoning_writes_nothing() {
        let catalog = ModelCatalog::new(vec![catalog_entry("glm-5.3-flash", Some(false), None)]);
        let (_dir, path) =
            config_with_entry("glm-5.3-flash", &model_entry_toml("glm-5.3-flash", ""));
        let written = backfill_model_capabilities_locked_at(
            &catalog,
            &std::collections::BTreeSet::new(),
            &path,
        )
        .unwrap();
        assert_eq!(written, 0);
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(!body.contains("supports_reasoning_effort"));
        assert!(!body.contains("reasoning_efforts"));
    }

    #[test]
    fn unknown_level_values_are_filtered_and_all_unknown_skips_write() {
        let catalog = ModelCatalog::new(vec![catalog_entry(
            "glm-5.3-flash",
            Some(true),
            Some(vec!["bogus", "nope"]),
        )]);
        let (_dir, path) =
            config_with_entry("glm-5.3-flash", &model_entry_toml("glm-5.3-flash", ""));
        let written = backfill_model_capabilities_locked_at(
            &catalog,
            &std::collections::BTreeSet::new(),
            &path,
        )
        .unwrap();
        assert_eq!(written, 1, "布尔位照常回填");
        let parsed = read(&path);
        let entry = &parsed["model"]["glm-5.3-flash"];
        assert_eq!(entry["supports_reasoning_effort"].as_bool(), Some(true));
        assert!(
            entry.get("reasoning_efforts").is_none(),
            "全未知档位不写空表"
        );
    }

    #[test]
    fn backfill_is_idempotent() {
        let catalog = ModelCatalog::new(vec![catalog_entry(
            "glm-5.3-flash",
            Some(true),
            Some(vec!["low", "high", "max"]),
        )]);
        let (_dir, path) =
            config_with_entry("glm-5.3-flash", &model_entry_toml("glm-5.3-flash", ""));
        let keys = std::collections::BTreeSet::new();
        assert_eq!(
            backfill_model_capabilities_locked_at(&catalog, &keys, &path).unwrap(),
            1
        );
        assert_eq!(
            backfill_model_capabilities_locked_at(&catalog, &keys, &path).unwrap(),
            0,
            "第二轮全部命中显式键，零写入"
        );
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body.matches("supports_reasoning_effort").count(), 1);
        assert_eq!(body.matches("reasoning_efforts").count(), 1);
    }

    #[test]
    fn accepts_images_semantics_unchanged_explicit_wins_and_default_skip() {
        // 本用例只验 accepts_images 语义；目录 reasoning 置 None 让思考位分支保持静默。
        let mut entry = catalog_entry("glm-5.3-flash", None, None);
        entry.accepts_images = Some(true);
        let catalog = ModelCatalog::new(vec![entry]);
        // 用户新建条目（不在 catalog_keys）：显式 false 保留。
        let (_dir, path) = config_with_entry(
            "glm-5.3-flash",
            &model_entry_toml("glm-5.3-flash", "accepts_images = false\n"),
        );
        let keys = std::collections::BTreeSet::new();
        assert_eq!(
            backfill_model_capabilities_locked_at(&catalog, &keys, &path).unwrap(),
            0
        );
        assert_eq!(
            read(&path)["model"]["glm-5.3-flash"]["accepts_images"].as_bool(),
            Some(false)
        );
        // 缺键且缺省相等（catalog_keys 命中 → 缺省 true = 目录 true）：跳过。
        let (_dir2, path2) =
            config_with_entry("glm-5.3-flash", &model_entry_toml("glm-5.3-flash", ""));
        let mut keys = std::collections::BTreeSet::new();
        keys.insert("glm-5.3-flash".to_string());
        assert_eq!(
            backfill_model_capabilities_locked_at(&catalog, &keys, &path2).unwrap(),
            0
        );
        assert!(read(&path2)["model"]["glm-5.3-flash"].get("accepts_images").is_none());
    }
}

