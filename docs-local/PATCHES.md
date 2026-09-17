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
  `"Code together, cola together — thanks for pairing with Panda! (/feedback)"`
  （初版措辞 "Share code & cola" 读起来像替别人备份代码，已按意图改为结对共写）
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

### 状态行原生默认（脚本退役：`tokens` / `cache` / `think` item + 默认开启）
- `xai-grok-status-line/src/config.rs`：`StatusLineItem` 新增 `Tokens` / `Cache` /
  `Think`（kebab-case：`tokens`、`cache`、`think`，`varies_mid_turn` 均 true）；
  `StatusLineType` 的 `#[default]` 从 `Disabled` 翻到 `Builtin`——缺省 section 即出行；
  `DEFAULT_ITEMS` 换成本地指标集 `[model, api-calls, tokens, cache, think, perf]`
  （对齐原 `~/.grok/statusline.py` 的指标集合；model 永远可得，放最左做行首锚点，
  避免开局行首是空段跳过的视觉抖动）
- `xai-grok-status-line/src/context.rs`：`StatusLineSessionUsage` 补 `Copy, Eq`
- `xai-grok-pager/src/views/status_line/segments.rs`：三个新段——
  `in 47k out 3.2k`（窗口总量缺省回退 usage 三桶和，k/M 一位小数去尾零）、
  `cache 95.7%`（cache_read / 会话输入总量）、`think 28.1%`（reasoning / 会话输出）；
  拿不到值整段隐藏（新会话只画 model，不画占位符）
- 消费方语义翻转后的测试同步：`config_tests.rs`（orphan/off 断言改显式 Disabled、
  无 type 载荷改"画默认行 + 仍报孤儿键"）、`metrics_tests.rs`（unset 记 true、
  不可画行改 Disabled）、dispatch `status_line.rs`（同）；新增
  `token_segments_mirror...` / `a_fresh_session_shows_only...` 单测；
  `docs/user-guide/25-status-line.md` Set up 表与默认值同步（doc-sync 测试强制）
- 用户脚本 `~/.grok/statusline.py` 退役可选：保留即覆盖默认（command 优先）

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

### xai-grok-pager（MCP 命名格式文档强化）
- `docs/user-guide/07-mcp-servers.md`（上游文件，4 处插入，均有 `<!-- LOCAL: -->` 或台账记录）：
  - Tool Naming 新增 "Server Name Format Requirements" 小节：`<server>__<tool>` 全名须匹配
    `^[a-zA-Z_][a-zA-Z0-9_-]{0,63}$`（`xai-grok-mcp/src/servers.rs` `validate_tool_name`）——
    server 名数字开头（`7zip`）会静默丢光工具，只留 `Skipping MCP tool with invalid name` 日志；
    `grok mcp add` 只查字符集不查首字符，是坑的隐蔽点
  - Troubleshooting 新增 "Server Connects but Its Tools Are Missing" 症状条目
  - Configuration 开头与 CLI Management breaking-changes 句各补一句警告 + `#tool-naming` 交叉引用
- 背景：上游镜像（xai-org/grok-build）关了 issue 区，此 bug 无人报过；文档先行，代码放宽待上游

### xai-grok-pager（agents 弹窗展示全部内置变体 + 隐藏模型/杀开关文档）
- `src/views/agents_modal.rs`：删 `user_visible_builtins()` 策展隐藏名单，`build_agent_list`
  改为遍历全部 `BuiltinAgentName::iter()`。背景：`[agent].name` / `GROK_AGENT` 可选任意
  内置变体（含 `grok-build-concise`），但弹窗只展示 5 个——`/agents` 里按 `s` 会把选中
  的 `grok-build` 写回 `[agent].name`，静默覆写用户配置且无 UI 可见（实际踩坑事故）。
  回归测试 `build_agent_list_lists_every_builtin_variant` 防新变体再被藏
- `docs/user-guide/26-config-reference.md`：
  - features 表补 `turn_transient_retry` 行（上游漏文档的回合瞬时重试杀开关）
  - models 表后补 "Hidden vs disabled" 散文段：hidden 模型 `-m` 可用、只有
    `disabled_models` 才移出目录、内置 plumbing 模型（web_search/image_description/
    子代理/次要模型）按设计隐藏

## 发布

- `.github/workflows/release.yml`：推 `v*` tag 触发，构建 `xai-grok-pager`（grok CLI）
  release 二进制，仅 linux-amd64（tar.gz）与 windows-amd64（zip），附 sha256。
- CI 缓存事故与最终方案（job 名撞车 → sccache GHA 碎片撑爆 10 GB → 改回隔离的 rust-cache）：
  见 `ci-cache-incident.md`。
## 待办（下一期）

- 工具输出压缩：一期已落地（简化 headroom 策略，见 `tool-output-compression-plan.md`）；
  二期低损/无损组合已定稿（2026-09-17，见同文档「二期方向」节）；only-cc-lite git
  依赖与 embedding 检索仍不做
- 排队时间戳 / wait 计算（`acp_session_impl/prompt_queue.rs`）：已从计划删除
- 排队卡视觉区分：已废弃
- T3 stats 后续可选项：把会话行接 `list_summaries` 拿标题、`--project` 过滤
- `/stats` 斜杠命令已落地（压缩正向/负向收益 + CCR I/O 延迟；`grok stats` 同步展示）

## 三期补丁（已完成，2026-09-15）

> 通用前置（延续）：实现前核查上游是否已有相仿/冲突机制，有则不做；
> 全部可配置启用/停用，默认不改变上游行为。
> 2026-09-15 核查：MCP 懒加载上游已内置且强制默认（见对照表），不做。
> 范围：极简 primary agent + BeforeModelCall hook + PostCompact 重注入 + 通知开箱。
> 说明：PreCompact/PostCompact 事件上游本就存在（compaction.rs 触发，Observe 型），
> 三期补的是 PostCompact 的 additionalContext 重注入通道。

### 已落地

- `BeforeModelCall` 消息变换 hook：每次 LLM 请求组装完成后、发送前改写消息列表
  （出去脱敏/回来还原，双向），session 记录永不修改——裁剪/脱敏/压缩类扩展的
  头号依赖面（DCP 4.2k★、vibeguard 均建立在此 hook 上）。接入锚点：
  `acp_session_impl/sampler_turn.rs` 请求组装边界（reconstruct 之后、采样调用之前，
  fork 的 `${session_id}` 模板展开已在同一 seam）；事件注册走 `hook_dispatch.rs`。
  它是「待办（下一期）」工具输出压缩的前置消费者，排期时一起评估
- 极简模式 primary agent（复用现成载体，不新增输出风格子系统）：上游已内置
  concise 变体——`BuiltinAgentName::GrokBuildConcise`（`xai-grok-agent/src/config.rs`
  `grok_build_concise()`：`COMPACT_SYSTEM_PROMPT` + 精简工具描述集
  `grok_build_concise_toolset` + `agents_md:false`），`--agent-profile
  grok-build-concise` / `agent.name` / `/config-agents` 均可选为主会话 agent，
  但其提示词只有两句话、零输出风格规则。fork 改造 = 给 concise 载体追加 LOCAL
  极简规则节：改 `xai-grok-agent/src/agent.rs` `system_prompt()` 的 concise 分支
  （`COMPACT_SYSTEM_PROMPT` 后拼接 `LOCAL_CONCISE_RULES` 常量），会话中切换路径
  `acp_session_impl/model_switch.rs` 同步拼接。规则融合三源：
  qwen Concise（答案先行、零旁白零复盘零寒暄、用户要解释时给全文、正确性>极简、
  冲突时本节胜出）+ caveman（去冠词/填充语/客套话/对冲、短同义词、箭头表因果、
  片段句可用、技术词精确、代码块与报错原文不动、安全警告/不可逆操作临时恢复
  完整表达）+ i-have-adhd 46k★（首行即下一步行动、多步编号且步数最少、每回合
  重述进度状态、结尾至多一个两分钟内可做的 next action、错误平铺直叙
  cause+fix、展示列表≤5 条且分析完整性不受限、完成的事说清"现在能用什么"）。
  二批候选：Learning 类交互风格、qwen 式 `keep-coding-instructions` 基础提示分节

- `PostCompact` 重注入（已落地）：上游事件本就存在（Observe 型，payload 仅
  `{source}`）；三期补 `additionalContext` 响应通道——收集文本在压缩重置后以
  单条 system item 重注入（`dispatch_post_compact_context` +
  `dispatch_post_compact_collect_context`），任务列表/记忆类扩展的
  状态恢复通道（rpiv-todo 月下载 14.8 万的核心卖点）
- 通知开箱化（已落地）：`[notifications]` 配置节（`desktop`/`sound`/
  `min-interval-secs`/`suppress-after-user-input-secs`，全部 opt-in 默认关）；
  发射走终端 OSC 9 + OSC 777 + BEL（零依赖，Windows Terminal/iTerm2/kitty/WezTerm）；
  聚焦抑制为启发式——用户最近 N 秒有输入则跳过；挂在 `dispatch_notification_hook`
  入口，与 hooks 完全独立

**行为变化（rebase 用）**：grok-build-concise 现为 strict harness（定制提示 +
精选工具集）——客户端 `_meta.agentProfile` 不能覆盖它（与 codex 同策略）；
`harnesses_are_compatible` 视其仅与自身兼容，切换到它需重建 harness（重建路径
`model_switch.rs` 会写入 compact+规则提示，语义一致）。相关上游测试已按此更新：
mvp_agent/tests.rs（兼容矩阵 + ACP profile 解析）、xai-grok-agent config.rs
（`expected_strict_harness` / 按名分类）。

**实现锚点（rebase 用）**：`xai-grok-hooks`（event.rs 事件/GateKind::ModelCall/payload、
runner/mod.rs `resolve_rewrites`+`gate_outcome(gate)`、dispatcher.rs
`MessageRewrite`/`dispatch_before_model_call`/`dispatch_post_compact_context`、
config.rs ModelCall 超时）；`xai-grok-agent`（template.rs `LOCAL_CONCISE_RULES`、
config.rs `grok_build_concise()` Custom 模板）；`xai-hooks-plugins-types`
（HookEvent::BeforeModelCall）；`xai-grok-shell`（turn.rs 采样前 seam + 失败归因、
hook_dispatch.rs `apply_before_model_call_hooks`/`trip_before_model_call_breaker`
(阈值 3)/`dispatch_post_compact_collect_context`、model_switch.rs 规则拼接、
updates.rs `emit_builtin_notification`、types.rs 活动时间戳静态、agent/config.rs
`NotificationsConfig`、compaction.rs PostCompact 重注入）。

## 上游已有能力对照（社区呼声 → 勿重复实现）

| 社区呼声（高 reaction/高星） | 上游 grok 现状 |
|---|---|
| `/context` 上下文分解 | 已有（细分到 tool defs / skills / MCP 成本） |
| `/goal` 持久目标 + token 预算 | 已有（`--budget` + 对抗验证） |
| `/btw` 侧问浮层 | 已有（`/aside`，含 minimal 面板） |
| 结构化提问工具 | 已有（内置 `ask_user_question`） |
| LSP 诊断回喂 | 已有（repo 级 server + 插件 LSP + `lsp` 工具；本地已做合并去抖增强） |
| subagent 编排 | 已有（personas + `send_subagent_message` + monitor/kill_task） |
| worktree 隔离 | 已有（内置） |
| 权限矩阵 / sandbox / headless / memory / hooks | 均有（hooks 含 `updatedInput` 改参、`updatedToolOutput` 替换、record 与 model 分离） |
| 交互式后台进程 | 已有（background tasks + ptyctl） |
| 通知 | 事件已有，缺开箱实现（已列三期 P1） |
| MCP 懒加载（pi-mcp-adapter 月下载 94 万） | 已有且强制默认：请求 tools 数组只含内置工具（`sampler_turn.rs` `prepare_tool_definitions_inner` → `tool_definitions_builtins_only`，注释 "tool search is always enabled"），MCP 工具走 `search_tool`（BM25 索引）→ `use_tool` 两段式；announcement 是变更时增量 `<system-reminder>`（指纹持久化 `announcement_state.json`，`MCP_REMINDER_MODE`=delta/full），不做全量 schema 常驻 |
| opencode primary 自定义 agent（Tab 切换 build/plan） | 已有：agent 定义 `.grok/agents/*.md` / `~/.grok/agents/`（作用域含主会话：model/tools/prompt body/skills），`/config-agents` 设默认 + 会话中切换激活，启动侧 `--agent-profile` / `GROK_AGENT` / `agent.name`；personas 是 subagent 专属行为叠加层 |
| 输出风格极简/详细（qwen `/output-style` 多风格选择器） | 不照搬（用户决策：只做极简一种）。极简主 agent 上游已有载体：内置 `grok-build-concise`（`--agent-profile`/`agent.name`/`/config-agents` 可选，`COMPACT_SYSTEM_PROMPT` + 精简工具集），但提示词无风格规则 → fork 注入三源融合规则节，已列三期 P0 |

## 四期补丁（TUI 界面文案双语）

### xai-grok-pager（斜杠命令描述/用法中英切换，默认中文）
- `src/slash/i18n.rs`（新文件）：`Lang`（Zh 默认 / En）+ 进程级 `AtomicU8` 全局状态
  （首读时 `GROK_LANG=en` 可改默认）+ `tr()`（英文原文 → 中文译文查表，无译文/英文模式
  原样透传）+ `translations()` 静态翻译表（约 90 组：全部内置命令 description/usage/
  arg_placeholder、effort 等级描述、voice/minimal/fullscreen 手写文案）+ `test_sync`
  测试串行锁
- `src/slash/command.rs`：`slash_meta!` 宏的 `description` / `usage` / `arg_placeholder`
  三个生成位包一层 `i18n::tr()`（LOCAL 注释处）；其余字段不动
- `src/slash/commands/voice.rs`、`screen_mode_switch.rs`：手写 `description()` 两处字面量
  过 `tr()`；`effort_levels.rs` `effort_description()` 各分支过 `tr()`
- `src/slash/commands/lang.rs`（新命令 `/lang`）：无参数在中英间切换，`zh|en|中文|english`
  显式指定，未知参数报错；切换后返回确认消息。已注册进 `commands/mod.rs` `builtin_commands()`
- `src/slash/registry.rs`：新增 `pub refresh_trigger_text()`（转发私有 `rebuild_triggers()`）
- `src/app/dispatch/prompt.rs` + `dashboard.rs`：两处命令分发点在执行前记录语言、执行后
  若语言变化则对各自 slash_controller 的 registry 调 `refresh_trigger_text()`，
  使斜杠菜单/命令面板/ghost 补全的描述文本立即换语言
- 边界：ACP/技能等运行时文案不翻译（表外透传）；语言不持久化到 config.toml，
  会话内有效，`GROK_LANG=en` 可固定英文
- 测试语义：`cfg!(test)` 构建下 tr() 默认英文（上游既有测试按英文文案断言），
  生产默认中文；`xai-grok-shell` `slash_commands.rs` `PAGER_COMMAND_KEYS` 追加
  `"lang"` 占位（防技能同名遮蔽，测试 `pager_builtin_triggers_are_reserved_in_shell` 强制）

## 五期补丁（2026-09-17：状态行数据修复 + i18n 覆盖扩展 + 自动更新默认关）

### xai-grok-shell（状态行 in 0 out 0 修复）
- `src/session/acp_session_impl/status_line.rs`：空账本（`model_calls == 0`）投影出的
  `UsageTotals` 全零，`session_input_tokens = Some(0)` 使 tokens 段从会话一开始就画出
  `in 0 out 0`（与"数据存在才绘制"的设计相悖）。修复：`build_status_context` 里给
  `build_context_window` 传 `window_totals = totals.filter(|t| t.model_calls > 0)`，
  无调用时 token 窗口整体缺席，行只画 model；`api_calls`/`cost` 仍用原始 totals
  （前者自带 `model_calls > 0 || failed > 0` 过滤）
- `src/session/acp_session_impl/sampler_turn.rs`：`record_response_token_usage` 记账后
  追加 `emit_status_snapshot_detached()`（LOCAL），每次模型响应立即刷新状态行，
  不再只等 turn-end 快照

### xai-grok-pager（i18n 覆盖扩展：快捷键栏 + shell ACP 命令）
- `src/slash/i18n.rs`：新增 `tr_str(&str) -> String`（动态字符串查表；英文模式/表外
  原样返回）；翻译表追加：快捷键提示栏全部标签（send/cancel/copy plan 等约 75 组，
  渲染时查表）+ "press again to" 前缀 + shell 内置命令描述/占位符（/memory /flush
  /dream /context /hooks-* /session-info /deep-research /goal /plugins 等约 30 组）
- `src/views/shortcuts_bar.rs`：bar 渲染处标签与 "press again to {label}" 前缀经
  `tr_str`/`tr` 查表，宽度按译文计
- `src/slash/acp_command.rs`：`AcpSlashCommand::from` 构造时对 ACP 下发的
  description / arg_hint 过 `tr_str`（覆盖 shell 端命令在斜杠菜单里的中文显示）

### xai-grok-update + xai-grok-pager-bin（自动更新默认关闭）
- 背景：官方安装器自动升级会把 fork 构建覆盖为官方二进制（2026-09-17 实证：本地
  fork 1.0.29 被覆盖为官方 1.0.34）
- `src/auto_update.rs`：`check_update_background` / `run_update_if_available` 的
  auto_update 门从 `== Some(false)` 拦截改为 `!= Some(true)` 拦截（None 默认关）；
  删除首写 `Some(true)` 的持久化；`UserCommand` 触发（手动 `grok update`）不受门限
- `xai-grok-pager-bin/src/main.rs`：leader 每小时 converge 的 auto_update 检查同步
  改为 `!= Some(true)` 拦截
- 恢复自动更新：config.toml 写 `[cli] auto_update = true`

## 六期补丁（实验性工具输出压缩）

> 默认关；`[tool_output_compression] enabled = true` 才改行为。未引入
> `only-cc-lite` git 依赖，策略按 headroom 简化：json / logs / search / diff /
> generic head+tail。`exit_code`/`stderr` 与 bash `exit: N` 头无损。
> 会话启动时把配置钉进 SharedResources；已写入对话的 tool_result 永不回写，
> 提示缓存前缀在整段会话内字节稳定。改配置只影响下一个新会话。

### 二期方向定稿（2026-09-17，摘要；全文见 `tool-output-compression-plan.md`「二期方向」）

目标升级为**低损/无损**，组合为三级瀑布：无损层（重复行折叠 xN、JSON 精确去重、
search 标题化保全行、diff index 剥离、ANSI 剥离；全部可逆 + 往返自校验失败退回
原文）→ 低损层（日志重要性打分限预算、JSON 膝点 adaptive-k）→ 有损兜底
（generic head/tail，新增 `max_lossy_ratio = 0.25` 上限，截断必写 CCR marker）。

事实依据（本地消融 `mech_ablation_report` 测试 + 四家社区风评）：

- 本地消融：重复型 JSON 一期基线省 92.7% 但**丢中间唯一项**，精确去重 91.3% 零丢失；
  日志模板折叠省 93.1% vs 基线 78.8% 且关键行全保留；search 每文件 3 条上限
  丢尾部命中（一期隐藏损失）；唯一型内容"低损=低省"是物理极限（全保留仅省 23-25%）。
- headroom 丢信息投诉：#3545 search 行熔接 → 行号↔内容假配对；#3580 代码被 ML
  通道删词；#3590 小结构化输出压残；#3625 截断未写 marker；#3544/#3560 有 marker
  无 retrieve；#3587 1886 请求零次检索（marker 成本白付）。无损正解在它的
  `lossless_compaction.py`（可逆 + 自校验）。
- 实测经济账：重写历史 → cache bust 123 vs 14，净省 ≈0（brandonbarker.me 对照
  实验）；headroom 自报 savings ~1.9x 高估（/stats 数字只当相对指标）；
  tsheadroom 保守档实测仅 ~40%（vs 宣传 60-95%）。
- DCP（模型主动压缩）不采用：摘要膨胀反烧 738k token（#573）、静默丢数据（#534）、
  原地替换破 cache（#604）、保护白名单 Windows 路径分隔符从未匹配（#592）。
- context-mode（事前沙箱）不采用：MCP 盲区 + FTS5 召回依赖模型写对脚本 +
  evict 排序 bug；本 fork 该场景由 rtk hook 覆盖。

### xai-grok-tools
- 新模块 `implementations/output_compression/`：检测、压缩、CCR 文件库、
  `expand_output` 工具、进程级 ledger
- `registry/types.rs` `finalize_output`：prompt 文本压缩（提醒之前）
- `ToolRegistryBuilder::new` 注册 `expand_output`（dispatch）；广告面由
  AgentBuilder 在 CCR 开启时注入

### xai-grok-shell
- `Config.tool_output_compression`（serde default，struct 尾部）
- `resolve_runtime_fields` 调用 `set_runtime`

### xai-grok-agent
- `builder.rs`：CCR 开启时把 `expand_output` 注入 toolset

### xai-grok-pager
- `/stats` 斜杠命令 + `grok stats` 段：正向 saved tokens/%，负向 expanded +
  retrieve 回灌 tokens，CCR 额外 I/O ops/ms
- `docs/user-guide/29-local-enhancements.md` 配置说明

## 七期补丁（2026-09-18：界面文案中文化 P0 交互必经）

> 方案见 `docs-local/ui-i18n-plan.md`（P0 = 每个会话都撞上的交互路径，8 个视图）。
> 翻译表 280 → 449 组；施工规约与术语表以方案文档为准（状态存英文键渲染出口翻译、
> 比较/匹配键不包翻译、测试构建查表旁路不变）。

### xai-grok-pager（P0：命令面板/启动屏/隐私横幅/会话选择/引导采集/权限·提问·计划审批）
- `src/slash/i18n.rs`：翻译表追加 P0 段 169 组（命令面板条目名与按钮、启动屏信任确认/
  认证流程/菜单/相对时间词族、隐私横幅分段文案、会话选择过滤徽章与加载头、引导采集
  标签与 URL 校验错误、权限模式编辑预览与页脚、提问占位符、计划审批状态标签与空计划
  占位段等）
- `src/views/modal.rs`：36 条命令面板条目在 `default_palette_entries` 构造处 tr（渲染
  出口在 app/modals.rs，属后续批次；已核实 label 无比较点，中文模式下按 shortcut 列
  仍可英文检索）；按钮 label()/面板标题问句/reset 确认拆片段/docs 选择器与查看器页脚
  图例等约 14 组
- `src/views/welcome/mod.rs`：trust 确认逐行成键、认证流程常量（AUTH_HEADER 等渲染处
  查表）、菜单项、Yes/No 确认、gate 屏、更新通知模板、相对时间族（"just now" 保留
  英文键，goal_detail.rs 的 `ago == "just now"` 比较不受影响）；3 处命中矩形/换行
  估算 `.len()` → `.width()`（测试构建走英文旁路，ASCII 宽度不变，断言不受影响）
- `src/views/privacy_banner.rs`：标题/说明段整段成键；LEGAL 链接逐段成键保分段数，
  热区宽度改按译文 `shown.width()` 计（三变体中文渲染宽度均 ≤ 英文，选档不变量保持）
- `src/views/session_picker.rs`：`SourceFilter::label()` 六个过滤徽章、"(no prompt)"/
  "(no summary)"、加载头（spinner 改 `"{} {}"` 拼接，英文输出逐字不变）、hidden 外部
  会话计数模板整键；`session_picker_surface.rs` 零改动（"Open session" 标题经
  modal_window 渲染出口 tr_str 命中，图例 nav/select/close/search 键已在表）
- `src/views/elicitation_view/{render,state}.rs`：标签/按钮/等待/滚动标记 render 处
  tr；3 个标题模板与 4 条 URL 校验错误构造处 tr 后 replace 占位（键含 {} 占位符）
- `src/views/permission_view.rs` + `question_view.rs` + `plan_approval_view.rs`：模式
  编辑 5 态预览行/页脚动作词/`"all tools from {}"` 拆片段；提问占位符与截断提示
  （question_view:834 "Other" 是 ACP 线上协议串，不译；可见行标签在 dashboard/peek.rs
  属 P3）；`plan_approval_status_label` 纯展示出口 tr
- `src/app/agent_view/plan.rs`：空计划占位段渲染出口 `tr_str(EMPTY_PLAN_PLACEHOLDER)`
  （常量本体保持英文，trim 判空逻辑不动）
- 遗留（后续批次处理）：session_picker 展开卡字段标签 ID/CWD/Created/… 因 picker.rs
  用 `{:<12}` 字符补齐 + `.len()` 布局暂不译，需先把该处改 unicode_width；命令面板
  渲染出口 app/modals.rs、peek.rs "Other" 行标签在 P3
