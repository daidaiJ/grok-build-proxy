# 上游同步记录：2026-09-15 … 2026-09-23（5 个快照）

> 分支 `sync/upstream-2026-09-23`，基线 `origin/main` = `9a211b70`（tag `v1.0.37-preview.11`）。
> 上游来源 = 公开镜像 `github.com/xai-org/grok-build` 的 `main`，本仓库内以 `refs/remotes/mirror/main`
> 保存（一次性 fetch，`git update-ref -d refs/remotes/mirror/main` 可删）。本 fork 上一条同步线
> `upstream` 分支停在 `37949780`（2026-09-09）。

## 合并范围

| 快照 | 日期 | 文件 | 行数 |
|---|---|---|---|
| `4827113` | 09-15 | 1650 | +122,644 / −49,180 |
| `a28ee2b` | 09-17 | 356 | +27,478 / −6,217 |
| `4247f66` | 09-19 | 403 | +24,000 / −6,610 |
| `07e35a3` | 09-22 | 419 | +25,267 / −6,269 |
| `f0e3be1` | 09-23 | 187 | +5,955 / −4,764 |
| **合计** | | **2003** | **+205,175 / −72,871** |

依赖变化（Cargo.lock）：`rmcp` 3.2.0→3.4.0、`rmcp-macros` 同步、`tree-sitter` 0.25.10→0.26.13、
`tikv-jemalloc*` 0.6.1→0.7.x、新增 `onig`/`onig_sys`、新增 workspace crate `xai-grok-file-lock`、
`fancy-regex` 0.16.2→0.14.0。工作区版本 `1.0.24`→`1.0.41`（release 由 tag 注入 `GROK_VERSION`，见
`.github/workflows/release.yml`）。

**本机偏离上游锁文件一处**：`tree-sitter 0.26.13` 要求 `cc ^1.2.48`，而上游锁的 `cc 1.2.48` 在
rustc 1.94 上编译失败（`cc/src/tempfile.rs` 把 `find_msvc_tools::windows_sys::FILE_ATTRIBUTE_TEMPORARY`
按 u32 传，实际是 i32）。本机 `cargo update -p cc` 解析到 `cc 1.5.1` + `find-msvc-tools 0.1.14`，
以此为本地锁文件基准；上游下次同步若带回旧组合需再确认。

## 冲突与裁定（52 文件）

裁定口径：**结构以上游为准**；`// LOCAL` 标注的语义补丁必须保留；纯文案本地化只在上游保留同一位置
时重放，上游删除/搬迁的站点记入下方"i18n 待补清单"，留待 i18n 收尾批次处理。

| 文件 | 裁定 |
|---|---|
| `Cargo.lock` | 取上游（构建时按需再生） |
| `xai-chat-state/src/commands.rs` | 取上游（`SamplingConfig` 显式字段列表已被 `..Default::default()` 取代） |
| `xai-grok-extra-ca/src/lib.rs` | 二者合并：保留本地进程级出口代理模块 + 上游的 ring/aws-lc 目标判定 |
| `xai-grok-pager/src/app/event_loop.rs` | 取上游（`defer_builtin_agent_profile` + `config_agent_is_explicit()`） |
| `xai-grok-pager/src/app/dispatch/dashboard.rs` | 二者合并：保留本地 `/lang` 切换后重建 triggers + 上游新字段 |
| `xai-grok-pager/src/app/app_view.rs` | 取上游结构；存活调用点补回本地 `quota_estimate` 参数 |
| `xai-grok-shared/src/clipboard.rs` | 取上游（`get_image` 改返回 `ClipboardImageRead`，带读取路径标记） |
| `xai-grok-sampler/src/metrics.rs` | 保留本地（TTFT 仍锚 `request_sent_at`；上游 `first()` 守卫与上方 `is_empty` 早退等价） |
| `xai-grok-sampler/{client.rs,actor/state.rs,tests/test_actor.rs}` | 取上游（显式字段列表 → `..Default::default()`） |
| `xai-grok-shell/src/agent/config.rs` | 二者合并：本地 `experimental` 与上游 `reasoning_summary` 双保留 |
| `xai-grok-shell/src/agent/config_tests.rs` | 取上游（字面量末尾已有 `..Default::default()`） |
| `xai-grok-shell/src/session/acp_session_impl/turn.rs` | 二者合并：本地 BeforeModelCall 钩子 + 上游 `requested_model` 快照 |
| `xai-grok-shell/src/session/acp_session_impl/spawn.rs` | 二者合并：上游 `spawn_step!` + 本地 minimal-style 注入 |
| `xai-grok-shell/src/session/acp_session_impl/slash_exec.rs` | 二者合并：保留本地 `SetMinimalStyle` 分支 + 上游注释 |
| `xai-grok-shell/src/session/acp_session_impl/model_switch.rs` | 取上游（重构为 `SessionModelSwitch`） |
| `xai-grok-shell/Cargo.toml` | 二者合并：本地两个 `[[test]]` 登记 + 上游 `tool_call_trace*` |
| `xai-grok-update/src/auto_update.rs` | 保留本地（自动更新默认关；丢弃上游 `unwrap_or(true)` + 首启落盘） |
| `xai-grok-version/Cargo.toml` | 取上游（`1.0.41`） |
| `xai-grok-workspace/src/permission/types.rs` | 取上游工具臂 + 保留本地 `ExpandOutput` 读权限臂 |
| `xai-grok-sampling-types/src/types.rs` | 手工补 `impl Default for SamplingConfig` 的本地 `experimental` 字段（上游新增该 impl） |
| `xai-grok-shell` 测试夹具（8 文件） | 本地夹具字段保留；上游已把 `repo_status_prefetch` 改名 `vcs_root`，夹具按上游 `vcs_root: None` |
| `xai-grok-pager/src/views/subagent_catalog_pane.rs` | 接受上游删除（本地 i18n 文案随之作废） |

### 撤销的本地补丁

- **`respect_config_agent`（`feat/local-agent-config-respect`，050b1323）**：上游 `a28ee2b`
  以 `SessionFlags::defer_builtin_agent_profile` + `views::agents_modal::config_agent_is_explicit()`
  实现了同一语义（配置 `[agent] name` 显式时不再合成 plan/ask-user profile）。合并后本地补丁撤销，
  `PATCHES.md` 对应条目同步标记为已由上游覆盖。

## i18n 待补清单

> 合并把纯文案本地化让位给上游新结构：上游删除/搬迁的界面代码里，本地 `tr()` 包裹未随行迁移。
> 下列站点在中文模式下会回落到英文，等 i18n 收尾批次补键或按新落点重放。

完整清单（34 条）：[`sync-2026-09-23-i18n-lost.txt`](sync-2026-09-23-i18n-lost.txt)。集中在
`views/dashboard/{render,row}.rs`（标题栏与活动行搬到上游新模块 `row_title.rs` / `row_activity.rs`）、
`views/tasks_pane.rs`（活动文案搬进 `run.activity_label()`）、`views/memory_modal.rs`（相对时间格式上游改紧凑方案）、
`views/dock/mod.rs`（`Section::tab_hint` 上游删除）。

## 测试夹具取舍

上游把 `SamplerConfig`/`SamplingConfig` 字面量里的显式字段列表整段换成 `..Default::default()`，并把
`SessionActor.repo_status_prefetch` 改名为 `vcs_root: Option<PathBuf>`；夹具按上游新形态改写（本 fork 自有的
`output_style_applied` / `first_model_call_done` 字段保留）。逐条见 `logs/` 会话临时清单。

## 验证

| 项 | 结果 |
|---|---|
| `cargo check --workspace` | 通过（0 error） |
| `cargo check --workspace --all-targets` | 通过（0 error，0 warning） |
| `ctest.sh -p xai-grok-sampling-types --lib` | 303 passed |
| `ctest.sh -p xai-chat-state --lib` | 396 passed |
| `ctest.sh -p xai-grok-extra-ca --lib` | 15 passed |
| `ctest.sh -p xai-grok-status-line --lib` | 18 passed |
| `ctest.sh -p xai-grok-sampler --lib` | 289 passed |
| `ctest.sh -p xai-grok-pager --lib` | 10031 passed / 0 failed（43 skipped：新增 4 条环境族 + 既有 39 条） |
| 活回合（TUI 实跑） | 待做 |

编译期间有一次 `rustc` `STATUS_ACCESS_VIOLATION`（0xc0000005），按既有纪律
（`rm -rf target/debug/incremental` + `CARGO_INCREMENTAL=0`）复跑通过，非代码问题。
