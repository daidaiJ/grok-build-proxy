# 上游同步记录：2026-09-23 … 2026-09-29（2 个快照）

> **状态：合并与验证完成，未合入 `main`**（分支 `sync/upstream-2026-09-29`，2026-10-19）。
> 合并提交 `a6bd1a8`（merge）+ 7 个收尾修复提交（`55d89e2` … `b102f90`）。
> 基线 `main` = `d567c07`（v1.0.63 文档收口态）；上游标记分支 `upstream` 已快进到
> `2bdd1d6a`（2026-09-29）。上游来源 = 公开镜像 `github.com/xai-org/grok-build`
> `main`，一次性 `git fetch`（未配置 remote）。

## 合并范围

| 快照 | 日期 | 文件 | 行数 |
|---|---|---|---|
| `97f190f` | 09-29 | 962 | +123,822 / −23,019 |
| `2bdd1d6a` | 09-29 | 68 | +6,183 / −5,368 |
| **累计（f0e3be1→2bdd1d6a）** | | **991** | **+129,569 / −27,951** |

两个快照同日落地，按用户拍板**一次合并到终态 `2bdd1d6a`**，冲突裁定一轮完成。
依赖变化：新增 workspace 依赖 `bstr` / `httparse` / `rstest`；`cc` 按本地偏离处理（见下）。

## 结构性变化（裁定前必须知道）

- **新增 5 个 crate**：`xai-grok-cloud-config`（收编 shell 的 `managed_config/*`
  与 `remote_config/*`，`cache_file` 迁入 `xai-grok-config`）、
  `xai-grok-permission-rules`（收编 `xai-grok-workspace/src/permission/*` 全树）、
  `xai-grok-egress-proxy`（sandbox 本地出口代理，与 fork 的进程级 `[network]`
  代理无关）、`xai-grok-external-agent-migration`、`xai-grok-lifecycle`。
- `EndpointsConfig` 从 shell `agent/config.rs` 迁入 `xai-grok-config/src/endpoints.rs`，
  shell 侧改为 `pub use` 重导出。
- `cache_file` 公开化：`xai_grok_config::cache_file::{read_capped, write_atomic}`，
  `write_atomic` 新增 `CacheFileMode`（`OwnerOnly` / `Default`）且返回 `Result`。
- 工具白名单原生实现：新 `xai-grok-agent/src/tool_list.rs`（`ListedTools` /
  `listed_tools`），原生覆盖 `SearchTool`/`UseTool` 保底、`Task` 依赖展开、
  未知项回退全量工具面——fork 旧 builder 内联 retain 的大部分条件被上游吸收。
- pager `acp/model_state.rs` 原生消费 `acceptsImages` / `inputModalities`
  （`current_model_accepts_images`），与 fork 视觉桥能力位同键同语义。
- `acp_model_meta()` 收编 shell 模型列表的 ACP meta 构造；rewind 预览收编为
  `session/helpers/rewind_preview.rs`。

## 冲突与裁定（36 文件）

裁定口径沿用上次同步：**结构以上游为准**；`// LOCAL` 语义补丁必须保留；纯文案
本地化只在上游保留同一位置时重放，上游重写的站点记入 i18n 待补清单。

| 文件 / 族 | 裁定 |
|---|---|
| `Cargo.lock` | 取上游，随后按本地偏离 `cargo update -p cc`（见下） |
| `xai-grok-pager/Cargo.toml` | 上游 `cloud-config` + `xai-grok-shell` path 依赖（`default-features = false`）；保留本地 `provider-usage` / `model-catalog` |
| shell `acp_session_tests/support.rs` + 6 测试文件 | 取上游 `test_compaction_config()` helper；LOCAL `idle_compact_in_flight` 补进 helper |
| shell `agent/config.rs` | 上游 imports/结构；LOCAL 六个配置节（network/provider_usage/model_catalog/model_groups/shell/notifications）整段保留；meta 内联块改 `acp_model_meta()` 并重放 `acceptsImages`；删除迁走的 `EndpointsConfig` 副本 |
| shell `agent/config_tests.rs` | 保留视觉桥 D1 四用例；上游新用例与 helper 全收，helper 补 LOCAL `experimental` 字段 |
| shell `agent/remote_config/{.rs,fetch.rs,manager/mod.rs}` | 上游搬迁形态（`mod cache` 移除、`xai_grok_cloud_config` 重导出）；`fetch.rs` 保留 `experimental`；manager 保留视觉桥两方法 + 上游测试 helper |
| shell `remote/client.rs` | 两者都保留：LOCAL `accepts_images` + 上游 `notice` |
| shell `extensions/model_catalog.rs` | `cache_file` 调用改指 `xai_grok_config`，`write_atomic` 按新签名传 `CacheFileMode::Default` 并记失败日志 |
| shell `benches/session_list.rs` | 上游 serde_json 夹具（`agent_name` 键与 fork `PersistedAgentSelection` 序列化兼容） |
| shell `session/acp_session_impl/rewind.rs` | 取上游 `rewind_prompt_preview`；删除重复的本地 `preview_of` |
| shell `session/acp_session_tests/rewind_cross_compaction_tests.rs` | 上游新用例补 LOCAL `RewindRequest.to_branch` |
| `xai-grok-agent/builder.rs` | 取上游 `listed_tools` 块；LOCAL CCR 条件补进 `tool_list.rs` |
| `xai-grok-hooks/dispatcher.rs` | 上游 gate 循环骨架（Defer/record_ask/block_verb）；LOCAL `updated_messages` 重放回 Allow/Ask/Blocked；两侧测试全保留 |
| `xai-grok-config/cache_file.rs` | 取上游（公开 API + `CacheFileMode`） |
| `xai-grok-workspace/permission/types.rs` | 删除随上游搬走的 `AccessKind` impl；LOCAL `ExpandOutput` 只读归类补进 `xai-grok-tools/permission_access.rs` |
| `xai-grok-tools/computer/local/terminal.rs` | 取上游返回 `Result` 的闭包；去掉 Windows 分支的 `mut` |
| pager `app/actions.rs` | 两者都保留：LOCAL `LspConfigureRequest` + 上游 `ModelChoice` |
| pager `app/agent_view/render.rs` | 上游 `footer_label` 形态；fallback 保留 `tr("unknown")` |
| pager `app/app_view.rs`、`views/dashboard/render.rs` | 上游 `PermissionLabel::display_name()` |
| pager `app/effects/mod.rs` | 上游 `fetch_rewind_points` helper；`parse_rewind_points_response` 补 LOCAL `abandoned` |
| pager `app/status_blocks.rs` | 上游 `display_kind()`（外层 `tr`）+ 新行格式/`format_by_model_row` |
| pager `slash/commands/model.rs` | 保留 LOCAL 目录详情用例 + 上游 window/effort 用例 |
| pager `slash/registry.rs` | 保留 fork `grok_builtin` 夹具（`/login`→`/grok` 改名） |
| pager `views/agents_modal.rs` | 上游 `RowTag`/`modal_list` 形态；三类标签重放 `tr()` |
| pager `views/import_claude_modal.rs` | 上游 `ImportRuleAction` 类型名 + 本地 `tr()` 文案 |
| pager `views/settings_modal/render.rs` | 上游 `render_section_header` + 本地 `tr(category.label())` |
| pager `views/tasks_pane.rs` | 上游 imports（去重）、`display_kind` 外层 `tr`、Workflow 标签 `tr` |
| pager `views/turn_status.rs` | 上游 imports/结构；LOCAL `turn_elapsed` 参数与 `tr` 前缀全保留 |
| pager `views/mod.rs`、`views/agents_modal.rs` 等视图族 | `lsp_panel` 与上游 `model_notice_banner` 并存 |

## LOCAL 补丁重放清单（同步后必须核对）

1. **`tool_list.rs`**：allowlist retain 增 CCR 条件
   `ccr_is_enabled() && id.ends_with(":expand_output")`（上游原生实现只缺这一条）。
2. **`permission_access.rs`**：`ToolInput::ExpandOutput(_)` 归 `AccessKind::Read(None)`。
3. **shell `agent/config.rs`**：`acp_model_meta()` 保留 `acceptsImages` 恒显；
   六个 LOCAL 配置节保留；`ExperimentalSamplingOptions` import 保留。
4. **`acp_session_tests/support.rs`**：`test_compaction_config()` 补
   `idle_compact_in_flight`。
5. **`effects/mod.rs`**：`parse_rewind_points_response` 补 `abandoned: r.abandoned_branches`。
6. **`model_catalog.rs`**：磁盘缓存原语迁 `xai_grok_config::cache_file`（PATCHES
   二十九/三十一期的 `pub(crate)` 提级已被上游公开 API 取代）。
7. **`xai-grok-test-support/env.rs`**：上游新用例 `matching_feature_stamp_still_asks_cargo`
   用 POSIX 脚本 + chmod，Windows 加 `#[cfg(unix)]`。
8. **`xai-grok-workspace/sandbox/tests.rs`**：上游新用例含 `std::os::unix::fs::symlink`，
   整个用例 `#[cfg(unix)]`。
9. **`win-skip.txt` 族Z**：上游 hooks 两个命令用例在 Windows Git Bash 失败
   （`${VAR:-D:\...}` 盘符冒号、`CLAUDE_PROJECT_DIR` 路径转换）。
10. **i18n 表**：合并产生的重复键（`enable/disable`、`running`、`failed`）已去重。
11. **`slash_commands.rs`**：`PAGER_COMMAND_KEYS` 补 `"lsp"`，防同名 skill 遮蔽。

## 本地锁文件偏离

上游 `Cargo.lock` 带回 `cc 1.2.48`，在 rustc 1.94 上 Windows 编译失败
（`cc/src/tempfile.rs` 把 `find_msvc_tools::windows_sys::FILE_ATTRIBUTE_TEMPORARY`
按 u32 传，实际是 i32）。本机 `cargo update -p cc` 解析到 **cc 1.9.0 +
find-msvc-tools 0.1.15**（上次同步用的 1.5.1，本次随 crates.io 最新兼容版）。
该偏离仅存在于本地锁文件；上游下次同步若再带回旧组合需重跑同一命令。

## 验证

| 项 | 结果 |
|---|---|
| `cargo check --workspace --all-targets` | **0 error**；27 warning 全部来自上游新增测试文件的 unix-only 导入/死代码（Windows 检查面独有，Linux CI 不触发） |
| `ctest.sh -p xai-grok-sampling-types --lib` | 314 passed |
| `ctest.sh -p xai-chat-state --lib` | 399 passed |
| `ctest.sh -p xai-grok-extra-ca --lib` | 15 passed |
| `ctest.sh -p xai-grok-status-line --lib` | 18 passed |
| `ctest.sh -p xai-compaction-transcript --lib` | 9 passed |
| `ctest.sh -p xai-grok-provider-usage --lib` | 63 passed |
| `ctest.sh -p xai-grok-model-catalog --lib` | 43 passed |
| `ctest.sh -p xai-grok-sampler --lib` | 294 passed |
| `ctest.sh -p xai-grok-pager --lib` | 10182 passed / 0 failed（65 filtered） |
| `GATE_FORCE=1 ctest.sh -p xai-grok-hooks --lib` | 274 passed（2 win-skip 环境族） |
| `GATE_FORCE=1 ctest.sh -p xai-grok-agent --lib tool_list` | 9 passed |
| i18n 扫描 | 通过（重复键已清；新扫描快照为上游新增界面文案基线，见下） |
| Linux CI | 待跑（合并分支推送后 build.yml 双 job） |

## i18n 待补（上游重写站点，非本轮回归）

- `PermissionLabel::display_name()`（"ask" / "auto-review" / "always-approve"）
  直接进 prompt flags，未过 `tr()`；旧键 "auto" 文案已不存在。
- `status_blocks.rs` 用量行与 `format_by_model_row`（"Input tokens"/"Output tokens"/
  "Total tokens"/"Model calls"/"Cost"/"By model"/"{tokens} Tokens"）改上游格式后
  未重放 `tr_str`。
- 其余为上游新功能界面文案（lifecycle / external-agent-migration / model notice 等），
  旧基线 `i18n-sweep-classified-2026-10-15.txt` 未覆盖，需要新一轮分类收尾。

## 遗留

- [ ] 合入 `main` + 推 CI（build.yml 双 job）后才算收口。
- [ ] 发版后活回合验证：合并触及的视觉桥（`acceptsImages` meta）、rewind 分支树、
      `/lsp` 面板、hooks `updated_messages` 路径。
- [ ] 本机仿真的上游新特性（cloud-config 搬迁、tool allowlist 原生实现）行为
      抽查：`/model` 选择器与子 agent 工具面。
