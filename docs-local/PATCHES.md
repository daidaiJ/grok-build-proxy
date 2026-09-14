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

## 待办（二期）

- 排队时间：`acp_session_impl/prompt_queue.rs` 的 `pending_inputs` 入队时打时间戳、
  提升为 running 时计算 wait（本轮未做，状态行 `perf` 已预留展示位）
- 内置状态行的 ✓/✗/TTFT/TPS 渲染（payload 已备齐，先走自定义 command 脚本）
- `/stats` 会话/天/周聚合（T3 独立 bin 优先）、LSP 诊断合并去抖、排队卡视觉区分
- 工具输出压缩：见 `tool-output-compression-plan.md`
