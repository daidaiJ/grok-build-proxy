# LOCAL 补丁清单（上游同步 rebase 用）

> 约定：所有本地插入行带 `// LOCAL:` 注释；新配置字段一律 `#[serde(default)]` 且追加在
> struct 尾部；不新增 workspace member。每次上游同步后按本表逐条核对冲突。
> 本文件本身在 `docs-local/`（上游不存在此目录，永不冲突）。

## 设计约束来源（外部事故复盘）

| 来源 | 事故 | 本地设计对应 |
|---|---|---|
| headroom #746（closed，fix #753） | Claude Code 对非官方 base_url 关闭工具延迟加载，+25K 基线 | 压缩不碰工具 schema；proxy 不做外挂代理形态 |
| headroom #2186（closed） | CCR 主动回注旧摘要，token 反涨 | 压缩检索纯 pull 式（规划文档） |
| headroom #1869 / #987（open） | wrap 对 opencode 零节省 / 按协议漏压 | 进程内、结果层压缩 |
| qwen-code PR #10995 | `${session_id}` 模板头 | 同款模板语法进 extra_headers |
| 本机 rtk 实测 | 压缩层误报构建成功 | 无损字段白名单 |

## 一期补丁（按 crate）

### xai-chat-state（失败计数账本链）
- `src/usage.rs`：`UsageTotals.failed_model_calls` 字段 + fold + `UsageLedger::record_main_loop_failure`
- `src/commands.rs`：`RecordModelCallFailure` 变体（`// LOCAL:` 注释处）
- `src/handle.rs`：`record_model_call_failure()`
- `src/actor/mod.rs`：对应 match 臂
- `src/actor/mutations.rs`：`record_model_call_failure()`（只记 session 账本）

### xai-grok-status-line（payload 扩展，全部 additive + serde default）
- `src/context.rs`：`StatusLineSessionUsage.reasoning_tokens`；新 `StatusLineApiCalls`、
  `StatusLineTurnPerf`；`StatusLineContext.api_calls` / `.perf`
- 注意：上游文档 25-status-line.md 与类型有 doc-sync 测试，若上游改此文件需同步补表行

### xai-grok-shell
- `src/extensions/notification.rs`：`PromptUsageModel.failed_model_calls` +
  `From<&UsageTotals>` 穷尽解构接线（该处上游注释本就要求新字段显式接线）；
  `is_token_empty` / headless 投影 / per-model 行三处穷尽解构补 `failed_model_calls: _`
- `src/session/acp_session.rs`：`SessionActor.last_turn_api_duration_ms`（AtomicU64，
  LOCAL：上一调用 API 时长，供 TPS 分母，不动 chat-state 协议）
- `src/session/acp_session_impl/spawn.rs`：上述字段初始化
- `src/session/acp_session_impl/status_line.rs`：
  - `build_context_window` 填 `reasoning_tokens`
  - `build_status_context` 填 `api_calls` / `perf`
  - 新 `build_turn_perf()`（signals 会话均 TTFT + actor 上的 last api 时长 → TPS，零新埋点）
- `src/session/acp_session_impl/sampler_turn.rs`：
  - `expand_session_header_templates()` + 3 个单测；在 `reconstruct` 的
    `inject_url_derived_headers` 之后展开 `${session_id}`
  - `log_terminal_failure()` 开头挂 `record_model_call_failure(None)`（所有终态失败的单一收口）
  - `record_response_token_usage()` 写入 `last_turn_api_duration_ms`
- `src/agent/config.rs`：新 `NetworkConfig`（`[network]` 表）+ `Config.network` 字段 +
  `Config::default()` 补字段 + `resolve_runtime_fields` 开头调 `set_process_proxy`（配置加载时一次算好）

### xai-grok-config（Windows shell 后端可选）
- `src/shell.rs` LOCAL 段：`SHELL_OVERRIDE` OnceLock + `set_windows_shell_override()`；
  `detect_windows_shell` 的显式覆盖读取顺序改为 config 覆盖 → `GROK_SHELL` → 自动级联
- 默认级联不变：pwsh → powershell.exe → Git Bash → powershell.exe

### xai-grok-shell
- `src/agent/config.rs`：新 `ShellBackendConfig`（`[shell] backend` 表，值：
  `pwsh` | `powershell` | `bash`(=gitbash) | `cmd`）+ `Config.shell` 字段 +
  Default 补字段 + `resolve_runtime_fields` 里 `set_windows_shell_override`
  （`#[cfg(windows)]`，配置加载时一次算好）

### xai-grok-extra-ca（进程级出口代理）
- `src/lib.rs` LOCAL 段：`ProcessProxyRule` + `set_process_proxy` / `process_proxy_rule`
  （env 回退 `GROK_PROXY`、`GROK_PROXY_HOSTS`）+ `apply_process_proxy` /
  `apply_process_proxy_blocking`（`Proxy::custom` 按 host 后缀白名单路由，loopback
  永远直连）+ `process_proxy_tests`（4 个单测）
- 默认白名单 `["x.ai", "grok.com"]`；`proxy_hosts = []` = 全部 host

### xai-proto-build（Windows 构建修复，fork 必需）
- `src/lib.rs` `emit_rerun_if_changed`：`--dependency_out=/dev/stdout` 与
  `--descriptor_set_out=/dev/null` 是 Unix 路径，Windows 上 protoc 直接 panic。
  LOCAL 修复：Windows 下走临时文件 + `NUL`，依赖行回读同一条解析路径；
  反斜杠路径归一化后再做 well-known-include 过滤
- 配套环境：本机无 DotSlash，`bin/protoc` 是占位脚本。下载真实 protoc 到
  `D:\data\tools\protoc\bin`，编译时加 `PATH="/d/data/tools/protoc/bin:$PATH"`

## 已知问题（快照自带，非本地补丁引入）

`cargo test -p xai-grok-shell`（测试 profile）存在上游快照自带的编译错误，
`cargo check`（lib 本体）不受影响，全部通过：

- `src/leader/transport.rs:256`：`String + &String` 写法与当前 toolchain 不兼容
- `src/session/acp_session_tests/tool_layer_images_bridge_tests.rs:15`：base64 crate API 漂移
- `tests/common/mod.rs` 等：引用快照中不存在的 `reset_startup_settings_for_tests` 等函数
- `xai-grok-tools` `src/computer/local/terminal.rs:4884`：`parse_login_env_capture` 缺失，
  阻塞 tools 的 lib test 编译（故 CI 对 tools 只 check 不跑测试）

这些阻塞了 shell 内联单测（含 `${session_id}` 模板测试）的运行；chat-state（362）、
status-line（17）、extra-ca（15+）单测全部通过。

## 用户侧配置示例（config.toml）

```toml
[network]
proxy = "http://127.0.0.1:7897"
# proxy_hosts = ["x.ai", "grok.com"]   # 缺省即此值；[] = 全部 host

[shell]
backend = "bash"   # pwsh | powershell | bash(=gitbash) | cmd；缺省自动级联（pwsh 优先）
```

环境变量等价物：`GROK_SHELL=bash`（config 覆盖优先于 env）。

模型侧（opencode Go 会话亲和头示例）：

```toml
[model_providers.ocgo]
base_url = "https://opencode.ai/zen/go/v1"
extra_headers = { "x-opencode-session" = "${session_id}" }
```

## 二期补丁（已完成）

### xai-grok-status-line（状态行 item 扩展，全部 additive）
- `src/config.rs`：`StatusLineItem` 新增 `ApiCalls` / `Perf` 变体（kebab-case：`api-calls`、`perf`）；
  两者 `varies_mid_turn() == true`（回合内计数与 TPS 会变，行需 tick 刷新）
- `src/context.rs`：`StatusLineApiCalls` 补 `Copy`（compose 消费用）

### xai-grok-pager（内置状态行渲染 + stats 子命令）
- `src/views/status_line/segments.rs`：`compose_builtin` 渲染两个新段——
  `✓ n`（有失败追加 `✗ m` 且整段 Warn 色调）、`ttft {ms}ms · {tps:.1} tok/s`（缺哪段省哪段）；
  字形走 `xai_grok_pager_render::glyphs::{check_mark, ballot_x}`（旧控制台回退）
- `src/views/status_line/segments_tests.rs`：两个新段的单测
- `docs/user-guide/25-status-line.md`：Set up 表补 `api-calls` / `perf` 行（doc-sync 测试要求）
- 新 `src/stats_cmd/`（`grok stats`，T3 独立 CLI，只读）：`--json`、`--days N`、`--limit N`、
  `--model <text>`（模型 id 大小写不敏感子串过滤，过滤后无匹配模型的会话整行消失）；
  遍历 `<grok-home>/sessions/**/usage.json`（≤4 层），按会话 / 本地日（`%Y-%m-%d`）/
  ISO 周（`%G-W%V`）聚合 turns（`ended_at` RFC3339）；三视图都带按模型拆分
  （JSON `models` 数组 + 人类输出 `By model` 表，最忙模型优先）；
  成本求和遇缺报 `+` 尾标；项目名经 `xai_grok_config::decode_cwd_from_dirname` 还原
- `src/app/cli.rs` + `src/lib.rs` + pager-bin `src/main.rs`：`Command::Stats` 接线
  （两处命令分类块 + 分发臂）

### xai-grok-pager（欢迎屏品牌定制：熊猫头 logo + 彩蛋副标题）
- `assets/logo/logo07.txt`（full tier，23x7）/ `logo05.txt`（compact tier，5 行）：
  Grok 字标换成熊猫头盲文点阵；`.gitattributes` 强制 `assets/logo/*.txt` LF
  （`include_str!` 原样嵌入，CRLF 会把 `\r` 带进二进制渲染成杂字形）
- `src/views/welcome/hero_box.rs`：`HERO_SUBTITLE` 改为
  `"Share code & cola with Panda — thanks for trying Grok Build! (/feedback)"`
- `src/app/mod.rs`：退出尾部（终端恢复后、`Ok(false)` 前）追加 `FAREWELL`
  常量打印（stderr）——hero 副标题会被 changelog/公告挤掉，退出告别语必打印，
  彩蛋稳定展示；`quit_for_update` / 模式 relaunch 路径先于打印 return，不会污染
- `tools/gen_panda_logo.py`：像素→盲文转换脚本（2x4 点/格，U+2800 空白格），
  生成两档 art 并保证 LF，留作他人定制参考
- `docs/user-guide/28-welcome-branding.md`：新增内置引导文档（`src/docs.rs`
  `USER_GUIDE` 注册），面向 AI agent 的 logo/副标题定制配方——启动时会解包到
  `<grok_home>/docs/user-guide/`，别人的 AI 可读到并自动复刻同类定制
- `docs/user-guide/29-local-enhancements.md`：新增内置引导文档，把本地增强
  （`[network]` 出口代理、`[shell]` 后端、状态行 `api-calls`/`perf`、
  `${session_id}` 会话亲和头、DeepSeek/GLM `reasoning_content` 思考回传）
  写成"作用 + 何时主动配置 + 示例"的 AI 引导表，含信号→配置对照表；
  供 AI 解包后读到并主动帮用户配置

### xai-grok-tools（LSP 诊断合并去抖）
- `src/implementations/lsp/mod.rs`：新 `DIAGNOSTICS_QUIET_WINDOW`（150ms）
- `src/implementations/lsp/manager.rs`：`take_answered_diagnostics` → `take_answered_items`
  （返回 per-file items，格式化收口到新 `summary_from`）；drain 循环改为把多批裁决
  累积进一个 `CollectedDiagnostics`——pending 未清前继续等整批，全答完后持 150ms
  静默窗合并迟到的推送；超时/静默放弃时已收集的照样发（原来丢弃）
- `src/reminders/lsp_diagnostics.rs`：注入级去抖——`DrainDebounce{last_inject}` 存
  `SharedResources`，注入后 750ms 内的编辑跳过 drain（下一轮 drain 一次报完整批），
  消除连续快速编辑的重复注入与重复阻塞
- 测试：`a_drain_merges_staggered_pushes_into_one_summary`（错峰 200ms 双推送合并）、
  `edits_inside_the_debounce_window_share_one_drain`（FakeBackend 计数）

## 发布

- `.github/workflows/release.yml`：推 `v*` tag 触发，构建 `xai-grok-pager`（grok CLI）
  release 二进制，仅 linux-amd64（tar.gz）与 windows-amd64（zip），附 sha256。
## 待办（下一期）

- 工具输出压缩：见 `tool-output-compression-plan.md`（原二期项，移入下一期）
- 排队时间戳 / wait 计算（`acp_session_impl/prompt_queue.rs`）：已从计划删除
- 排队卡视觉区分：已废弃
- T3 stats 后续可选项：把会话行接 `list_summaries` 拿标题、`--project` 过滤、
  TUI 内 `/stats` 斜杠命令复用 stats_cmd 聚合核心
