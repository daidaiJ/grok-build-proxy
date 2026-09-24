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
- 2026-09-19 语义扩展（test_context）：`cfg!(test)` 英文旁路升级为 `test_context()` =
  `cfg!(test) || env CARGO 存在`——集成测试把 pager lib 当普通依赖链接（无 cfg(test)），
  但 cargo 跑测试时子进程带 `CARGO` 环境变量（生产二进制没有），借此让上游集成测试
  （settings_e2e 等英文断言）默认英文，否则撞中文渲染。副作用：`cargo run` 开发态
  也默认英文；显式 `GROK_LANG=zh` 永远优先。三处判定位：`seed_lang`/`tr`/`tr_str`
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

## 八期补丁（2026-09-18：release 构建告警清理）

> v1.0.31 win/linux release 构建日志中的 rustc 告警清零，无行为变更；上游同步冲掉后按本条重放。

### xai-grok-shared
- `src/clipboard.rs` `get_text`/`get_image`：`arboard_error` 由"先初始化 None 再 match 赋值"
  改为按 match 臂直接产出不可变绑定（Windows 下 Ok(None) 提前返回导致初始化值永不被读，
  `unused_assignments` 告警 ×2）；错误在尾部的 `if let Some(error)` 统一上抛，语义不变

### xai-grok-pager-render
- `src/terminal/probe.rs`：`use std::time::Duration` 加 `#[cfg(unix)]`（仅 unix 门控的
  `LATE_REPLY_GRACE`/`read_tty_reply` 使用，Windows 侧 unused import）

### xai-grok-hooks
- `src/runner/mod.rs`：`gate_outcome`（文档注明 legacy test surface）加 `#[cfg(test)]`，
  release 构建不再编译（unused fn 告警）
- `src/runner/command.rs`：`gate_outcome` 导入拆分为 `#[cfg(test)] use super::gate_outcome;`
- `src/runner/command.rs` 测试模块：`make_scoped_ctx` 加 `#[cfg(unix)]`（仅 unix 门控的
  进程组测试使用；Windows 测试构建 dead_code 告警，构建日志不显示但 `--all-targets` 可见）

## Windows 测试环境族：分级屏蔽策略（2026-09-19 定稿；体系全貌见 docs-local/WIN-TEST-GATE.md）

> 本表 T0–T3 是溢出族的处理细则，已并入 WIN-TEST-GATE 分级体系（T1→L1、T2→L3）；
> 入口门控/静态扫描/抽样分诊的完整流水线、白名单晋升与 WSL 配方以 WIN-TEST-GATE.md 为准。
> 背景：上游基线是 Linux CI；本机 Windows 的测试线程默认栈仅 1MB（Linux 8MB），
> 上游深结构测试（subagent wake 族等）按 `STATUS_STACK_OVERFLOW`（0xc00000fd）
> 成族崩溃。根因是环境差异，不是产品 bug——按本表分级处理，禁止逐个排查。

| 级 | 手段 | 成本 | 适用 |
|---|---|---|---|
| T0 | **单测试 A/B 归因**：`git stash push -- <crate路径>` 只跑疑似溢出的那一个测试对比 HEAD；秒级，不动全量 | 极低 | 判定"我的改动还是既有问题"——必须最先做 |
| T1 | **`RUST_MIN_STACK=33554432` 抬栈**（32MB）：env 级、零代码改动、零重编译，已验证消掉整个溢出族 | 低 | 一切 `0xc00000fd` 溢出。**默认先行**：标准测试命令统一带此前缀 |
| T2 | **代码门控**：个别测试在 T1 下仍溢出（无界递归类）→ 该测试加 `#[cfg(windows)] #[ignore = "win 1MiB test-thread stack; see docs-local/PATCHES.md"]` LOCAL 补丁 | 中 | T1 无效的孤例 |
| T3 | 改造测试 harness / 构建脚本 | 高 | 目前不需要 |

**标准测试命令**（本机一律走门控入口；裸跑时才手动带前缀）：
```
scripts-local/ctest.sh -p <pkg> --lib          # 自动注入 RUST_MIN_STACK/TMP 并拼 win-skip.txt
# 裸跑等价形态：
TMP='D:\cargo-tmp' TEMP='D:\cargo-tmp' RUST_MIN_STACK=33554432 cargo test -p <pkg> --lib
```

**纪律**：T1 生效就不打 T2 补丁；T2 补丁必须登记本表便于同步重放；禁止在没有 T0 归因前
直接修产品代码。

## 九期补丁（2026-09-19：Windows 路径语义修复——门控测试复测产物）

> 背景：全量测试复测后按"重要性 × 修复成本"处置门控测试。以下四处修改的都是
> 上游同步文件，上游同步冲掉后按本节重放。Linux 无影响：全部为守卫式修改
> （MAIN_SEPARATOR/is_absolute≡has_root 于 POSIX 恒等/CARGO env 仅测试进程存在）。

### xai-grok-pager
- `src/git_info.rs` `collapse_home_path`：分隔符归一从 `~/` 折叠分支扩展到**所有分支**
  （不在 HOME 下的路径走 `path.display()` 回退会产出 `\`，与 libgit2 的 `/` 混用，
  dashboard 行/会话存储/测试助手 `collapsed_path_display` 要求可比较）。守卫
  `MAIN_SEPARATOR == '\'`。解锁 git_info ×2 + dashboard_open_detects_standalone ×1
- `src/app/workspace_sync.rs` `is_adoptable`：cwd 资格校验 `is_absolute()` 补
  `has_root()`——Windows 上 `/tmp/x` 无盘符 rooted 路径被误拒。解锁 workspace_sync ×4
- `src/app/foreign_sessions.rs` 测试闭包：`async_gate_supports_bundled_and_user_skill_locations`
  的子串匹配前归一分隔符（join 在 Windows 产出 `\`）。解锁 ×1
- `src/slash/i18n.rs`（LOCAL 文件，无冲突）：`test_context()` = `cfg!(test) || CARGO env`，
  英文旁路扩展到集成测试构建（见四期段 2026-09-19 语义扩展）。解锁 settings_e2e ×2

### xai-grok-dashboard-store
- `src/types.rs` `validated`：cwd 校验 `is_absolute()` 补 `has_root()`（同上语义）。
  解锁本 crate store 测试 ×19（此前在 Linux CI 应为绿的同一批用例）

### 效果
- pager lib：9566→9583 过（裸跑），门控清单 54→37 条（-17）
- 门控只留四类"确认不修"：doctor SSH-wrap（产品平台门控+用户决策无感知不修）、
  Tab 补全（上游 cfg!(not(windows)) 有意禁用）、scrollback 链接扫描（夹具 POSIX
  假设，Windows 真实场景不受影响）、其余路径散例（中等重要性低于修复线）
- 已知产品缺口（记录不修）：osc8 自由文本扫描正则仅认 `/` 分隔符
  （xai-grok-pager-render osc8.rs:216），Windows 反斜杠路径文本不会被链接化——
  修复涉及上游数百个精确断言，留作将来特性
## 九期补丁（2026-09-18：界面文案中文化 P1 常用弹窗与帮助）

> 方案见 `docs-local/ui-i18n-plan.md`（P1 = 常用弹窗与帮助，5 个模块）。
> 翻译表 449 → 917 组；施工规约与术语表以方案文档为准。

### xai-grok-pager（P1：设置弹窗/快捷键速查表/用量弹窗/MCP 弹窗/教程）
- `src/slash/i18n.rs`：翻译表追加 P1 段 430 组，分节与代码注释一一对应：
  settings 字面量/页脚 rest 键/分组标题、registry meta.label+description、枚举
  display+description（含 STT 语言名中文化）、shortcuts 分类/页脚/伪行/多行 long_help
  常量/ActionRegistry short_help+long_help、usage 标签页+页脚 rest 键+allowance+会话
  信息字段、mcps 分组模板+状态徽章、tutorial 引导语+主题 title/blurb+go_deeper 指南页标题
- `src/views/settings_modal/render.rs`：面包屑/分组标题/行标签/行值徽章/展开描述与锁定
  原因/Tip/过滤空态（`tr("No matches for ")` 译文自身参与宽度计算，布局与绘制同源）/
  编辑器占位符与校验错误（`Unknown model: "{}"` 模板键 strip 前后缀）/枚举选择器
  display+description 渲染出口统一 tr/tr_str；三处 `row_layout` 标签宽度同步传译文
- `src/views/shortcuts_help.rs`：ActionRegistry 渲染链路本次接线——`entry_display`
  （hint 说明+分类标题）、`CheatsheetRows::build`（折叠标题+内联帮助）、`render_detail`
  （详情页 title/body 渲染出口 tr_str，state 存英文不变）、`render_detail_body` 变灰
  注记、页脚与 3 处弹窗标题；搜索过滤 `filter_entries` 仍按英文匹配（中文模式下用
  英文词搜索，如需中文搜索需单独翻译匹配层）；`src/app/modals.rs` 仅 2 处
  "Keyboard Shortcuts" 标题接线
- `src/views/usage_modal.rs`：三个标签页标题（modal_window 渲染出口查表）、错误/空态/
  加载中、allowance 区（`Usage: ${used} / ${cap} per month` 命名占位符 replace）、
  会话信息字段标签屏显出口 tr；剪贴板复制串拆开保持英文（复制内容偏数据，且 dispatch
  测试对复制文本有英文断言）
- `src/views/mcps_modal.rs`：分组标题模板整键（`"Managed by grok.com ({})"` replace
  计数；插件分组 `"Plugin: "` 前缀键+动态名留 format! 参数）、Managed 说明行、6 个状态
  徽章 label()（已核实全部消费点为展示，无比较键）
- `src/views/tutorial.rs`：INTRO_LINES 渲染出口逐条 tr、列表行 title/blurb tr、两页页脚
  （`{}/{} explored` 双占位 replacen）；`tutorial_docs.rs` 零改动——title/blurb 是
  static 不进 state 也不被比较，弹窗标题走 docs 查看器中央 tr_str；go_deeper 的
  `find_doc(title)` 索引键保持英文
- `src/views/modal.rs`：reset 确认插值补 `tr_str(&meta.label)` 与默认值展示 tr_str
  （与 settings 弹窗行标签译文对齐）
- 主题专名（Grok Night/Tokyo Night 等）、"ZDR"、模型名不译（与 `/theme <name>` 用法
  一致）；settings 页脚图例 "type to filter" 译文呈 "type 以过滤"（shortcut_label_i18n
  固定保留键位 token，属机制限制，后续如需整句成键要改 modal_window 渲染函数）

## 十期补丁（2026-09-18：界面文案中文化 P2 集成管理弹窗）

> 方案见 `docs-local/ui-i18n-plan.md`（P2 = 集成管理弹窗，5 个模块）。
> 翻译表 917 → 1138 组（+221）；施工规约与术语表以方案文档为准。

### xai-grok-pager（P2：扩展/记忆/反馈/导入 Claude 五弹窗）

- `src/slash/i18n.rs`：翻译表追加 P2 段共 221 组，分节与代码注释一一对应：
  import_claude（标题/类型分组头/范围头/Enter 确认模板/页脚 rest 键）、memory（节头/占位符/
  空态/页脚/相对时间）、feedback（移出通知/trace 问句/标签行/空态/存储长句/taxonomy 枚举
  label/标题标签页）、extensions（分组头/徽章/计数模板/表单/页脚动作词）、modals.rs 侧
  （确认问句前缀/后缀键与静态提示）。合并时 "Name"/" cancel"/"Hooks"/"navigate"/"toggle"/
  "cancel"/"search"/"Import Claude settings" 等与既有条目同键同译，按既有条目去重；
  查重脚本按 translations() 全表解析断言无重复键（九期的临时脚本已清理，重放时按本条
  描述重建即可）
- `src/views/extensions_modal.rs` + `extensions_modal/workflows_picker_rows.rs`：
  6 个标签页名渲染出口 tr；分组头新增 `tr_group_label`（`Plugin: {name}`/`Custom: {path}`
  运行时拼接串按既有 `Plugin: ` 前缀键拆分，其余整串 tr_str；分组英文标签是折叠 state 键
  保持英文）；计数模板整键 + replace（`{n} plugins`/`{n} skills`/`{n} tools ({m} enabled)`
  等，单复数中文合并）；`post_select_row_hint` 整句模板键 + `{noun}`(tr_str)/`{verb}`(tr)
  注入（disable/enable 成对）；徽章 [policy]/[disabled]/[installed]/[error]/[update available]；
  展开字段标签；`Error: {msg}` 拆为 `format!("{}: {msg}", tr("Error"))`；modal_message
  渲染出口 tr_str（类型 `(&str, Color)` → `(String, Color)`）；result_notice/pending 徽章/
  表单标签与占位符渲染出口 tr_str；install_status 屏显值补 `not_installed`/`update_available`
- `src/views/memory_modal.rs`：节头 Global/Workspace/Sessions 存 state 英文（compute_filtered
  做 contains 过滤）→ 渲染出口 tr_str；页脚 13 条 tr；format_modified 相对时间模板键
  （`{mins}m ago` 复用既有键，`{hours}h`/`{days}d` 新增）；删除确认行内提示键含前导空格，
  对齐宽度 `len()` → `width()`（英文行为不变，中文译文修正右对齐）
- `src/views/feedback_modal/{mod,render,enum_picker}.rs`（drafts.rs 零改动，其字符串全部
  是存 state 的英文键）：7 条移出通知 notice() tr；trace 选项/确认问句/空态/删除确认/
  composer 占位符 tr；error 渲染出口 3 处 tr_str 覆盖 drafts.rs 全部存储键（多条多行长句
  整段成键，与源码逐字符核对含分号与 U+2026）；enum_picker 行 `tr(labels[variant])`，
  type-to-filter 过滤比较键保持英文；xai_grok_feedback taxonomy 枚举 label 渲染出口查表
  并补 25 键（Bug/Shell 保留英文不入表）
- `src/views/import_claude_modal.rs`：类型分组头 `tr(kind.label())`；Global 范围头整键存
  state → render_header_line 出口 tr_str；Project 范围头 `tr("Project  ")` 前缀键保宽；
  `"Enter import {}"` 模板键 + replace（shortcut_label_i18n 再拆时译文透传）；页脚
  navigate/toggle/fold/all/none/cancel/search 走 rest 键机制（部分 P0/P1 已入表）
- `src/app/agent_view/modals.rs`：feedback 移出通知配套两句 tr（与 notice() 同一条系统
  通知）；extensions 确认问句带动态名的用前缀键（`format!("{}\"{name}\"?",
  tr("Remove MCP server "))` 式，英文输出与原 format! 逐字一致，测试构建旁路不受影响）；
  pending_action 静态值（Reloading.../Processing.../adding.../Adding source.../
  Uninstalling.../Installing...）只补表，渲染出口已接 tr_str

### 已知余留

- `Authenticating {server}...`（modals.rs 存储期插值）无渲染侧模板可拆，中文模式显示英文；
- "Dropped {n} invalid image(s)." 采用存储期模板键 + replace（渲染侧拿不到计数），/lang
  切换不追溯已存文案；
- tests 断言的 state 值全部保持英文（测试构建查表整体旁路），tests 模块零改动。

## 十一期补丁（2026-09-18：界面文案中文化 P3 仪表盘与代理面板）

> 方案见 `docs-local/ui-i18n-plan.md`（P3 = 仪表盘与代理面板，5 个模块）。
> 翻译表 1138 → 1338 组（+200）；施工规约与术语表以方案文档为准。
> 同键异译仲裁：Failed→失败（dashboard/goal_detail/agent_status 三方统一）、
> Paused (error)→已暂停（出错）；confirm delete 与 P0 既有键撞键删重。

### xai-grok-pager（P3：dashboard 生产区 / tasks_pane / agent 页脚+agent_status / goal_detail / workflows）

- `src/slash/i18n.rs`：翻译表追加 P3 段 200 组，分节与代码注释一一对应：
  dashboard chrome/row/render/peek、goal_detail 状态与字段前缀、agent 页脚 hint 词族、
  agent_status chip、workflows 模板与状态说明、tasks_pane 调度后缀
- `src/views/dashboard/chrome.rs`：chip 绘制侧 tr(label)（hit-test id 保留英文）、
  Choose、+ New Agent( in Worktree)、Worktree/Disable Worktree 按钮
- `src/views/dashboard/row.rs`：逐帧重建处成键（`{tools} tools · {toks} tok ·
  {turns} turns`、`… {} more` 复用既有键）；整串状态词（Working/Awaiting your
  input/Loading…/Pending: question）仍存英文由 render.rs 出口 tr_str；
  AgentCommand::display_name()（app/agent.rs 五值）组合处 tr
- `src/views/dashboard/render.rs`：**方案文档"生产区仅 1–1103 行"有误**——实际
  `#[cfg(test)] mod tests` 在 3640 行起（render_tests.rs），1104/1950 只是两个 6 行
  cfg(test) 辅助函数；本次按真实边界接线整个生产区：banner 复数中文合并、空态/过滤、
  分组头（Pinned + `tr(rs.group_label())`）、Idle overflow、位置选择器、模式旗标
  （plan/auto/always-approve）、搜索与派发占位符、`rename: ` 前缀收敛 rename_prefix()
  保证绘制与宽度计算同源、页脚全部 hint（含 state.rs 的 label()/confirmation_label()/
  group_label()/focused_action_label() 调用点 tr）、覆盖层兜底与 [Dashboard]
- `src/views/dashboard/peek.rs`：response_type 存英文比较键（`== "Working"` 不动）→
  展示出口 tr_str（Thinking/Thought/Response/Read/Edit/… 17 词）；问题选项 label
  出口 tr_str；block_short_text 9 个括注（(thinking)/(tool call)/…）成键
- `src/views/dashboard/peek_tail.rs`：核查生产路径无说明性文案，零改动
- `src/views/goal_detail.rs`：状态行/字段行前缀（尾随空格是键的一部分）/分节头/
  事件值侧/verdict 标签/页脚接线；事件 match 键（goal_created 等）与 `d != "user"`
  比较键不动；相对时间族复用 {mins}m/{hours}h/{days}d ago/just now 既有键，新增
  {months}mo/{years}y ago；active_phase_label 已在 agent_status 出口翻译，本文件
  仅 tr_str 透传兜底（施工期临时 tr_phase_text 双保险已简化移除）
- `src/views/agent_status.rs`：goal_phase_label 5 个 pause 分支 tr(pause_label()) +
  Failed/Interrupted/Budget/Done；active_phase_label 的 `Verifying ({})` 模板键 +
  replace；goal_status_line 计数整键 replacen（{} tokens/{}/{} tokens）；chip_name
  = tr("Goal")。pause_label()（app/agent.rs:364）核实纯展示无比较消费，接线安全
- `src/views/agent.rs`：**零改动**——页脚 hint 集中翻译出口已在 shortcuts_bar.rs
  （P0 接线），本次只补 10 组表键（hide done/show done/reorder/page/queue/newline/
  accept suggestion/next/prev/turn/expand thinking）；HintItem label 存英文经
  ShortcutsBar::render 统一查表，就地再包会双重翻译
- `src/views/workflows.rs`：页脚 9 快捷键、标题 Workflow Runs、空态两行、Phases
  分节头；agents_meta() 按 total==1 选英文单复数键、中文合并；plural() 重接整键
  模板（noun 参数当前恒为 "agent"，`let _ = noun` 注释保留签名）；预算/failed 状态
  说明 4 条整键（含 {n} 模板）；rail 阶段名与 roster 标题渲染口 tr_str（数据侧
  phase_hits/selected_phase_name/比较逻辑存英文）；`{used} / {total} context` 模板；
  run.status.replace('_'," ") 协议词保留英文（tasks_pane 侧对去下划线值 tr_str
  查表，命中 complete/cancelled/interrupted/paused/budget limited/failed 则译）
- `src/views/tasks_pane.rs`：分组头 group.label() tr（search_text 过滤匹配键仍
  英文）；调度后缀整键含前导空格（" (next in {})"（2 处）/" (due now)"/
  " (running)"/" (starting)"，label 与 styled 同源）；空态三段 Span 两段成键；
  "Task " 前缀复用 Task 键显式补空格（避免尾随空格近重复键）；`1 agent`/
  `{n} agents` 复用既有模板键；"killing… " 覆盖层整键

### 仲裁与取舍（P3）

- `[worktree:on]`/`[worktree:off]` 徽标不译：宽度按 ASCII `len()` 预算且
  `find("on")` 字面定位高亮重绘，译文同时破坏宽度与定位
- dashboard 行年龄列 format_time_ago（"2m"/"just now"）不译：`{age:>6}` 按字符数
  填充对齐，CJK 破宽；util 出口跨视图属后续范围
- 5s/3m/2h 紧凑时长单位保留（agent_status chip 宽度预算）
- app 层动态串透传英文：format_activity_label（"Running: cargo test"）、
  format_subagent_label、format_context_badge——整串含运行时数据无法成键，
  app 层文案如需中文化另行立项
- tasks_pane 行 label 构造期翻译（entries 每次 sync 重建）：中文模式下行过滤匹配
  中文片段，与 dashboard/render.rs 同款既定取舍
- 占位符 `<query>` 随正文意译为 `<查询>`（斜杠命令用法串 [文件] 先例）；
  "esc close"（小写）与既有 "Esc close" 分键并存，译文风格一致
- workflows rail 宽度预估按未译 title 计算，中文标题略宽由 truncate_to_width 兜底，
  布局数学未动
- tests 断言的 state 值全部保持英文（测试构建查表整体旁路），tests 模块与
  *_tests.rs 零改动；查重脚本（translations() 全表解析断言无重复键无同键异译）
  重放时从本文件十期条目描述重建（Python，Rust \u{...} 转义需自行解码）

## 十二期补丁（2026-09-18：界面文案中文化 P4 低感知扫尾，i18n 计划全部完工）

> 方案见 `docs-local/ui-i18n-plan.md`（P4 = 低感知扫尾，约 20 个小文件）。
> 翻译表 1338 → 1417 组（+79）；施工规约与术语表以方案文档为准。
> 施工方式：3 个并发子代理分批接线（面板浮层 / 弹窗 dock 状态条 / 列表命令层），
> 主会话合并键值 + 补 rewind.rs 两处清单外漏网。

### xai-grok-pager（P4：jump/queue/todo/subagent_catalog/btw/location/session_title/
### new_worktree/managed_connectors_wait/hero_box/workspace_mode/dock/credit_bar/
### list_pane/block_viewer/picker/theme/debug/mode_support + rewind 补漏）

- `src/slash/i18n.rs`：翻译表追加 P4 段 79 组，分节与文件一一对应（jump/rewind、
  queue/todo/subagent_catalog、btw/location/session_title、new_worktree/
  managed_connectors、hero/workspace_mode、dock、credit_bar、list_pane、
  block_viewer/picker、theme/debug、mode_support 拒绝模板）
- `src/views/jump.rs`：浮层标题 "Jump to which turn?"、"(no preview)" 兜底 tr
- `src/views/rewind.rs`（清单外补漏，与 jump 共用键）：标题 "Rewind to which
  turn?"、"Loading rewind points..."、"(no preview)" 三处 tr
- `src/views/queue_pane.rs`：多行后缀 " (+1 line)"/" (+{n} lines)" 整键（宽度按
  实际译文动态算，无对齐破坏）
- `src/views/todo_pane.rs`：空态/完成态 4 条（含 {c}/{d} 计数模板整键 + replace）
- `src/views/subagent_catalog_pane.rs`：分组头 tr_str(owned_name)（构造期英文、
  显示出口翻译，search_text 过滤键不动）、空态；"Roles" 新键，Personas/Agents
  复用既有键
- `src/views/btw_overlay.rs`：Loading 态 "Answering…"（键含 U+2026）
- `src/views/location.rs`：detached→分离头指针、" (worktree of {repo})" 前导空格
  整键（测试构建英文输出逐字节不变）
- `src/views/session_title.rs`：合成回退标题 "session {id}"、"loading..."、相对
  时间 "now"/"{secs}s ago"（{mins}m/{hours}h/{days}d ago 复用既有键只接线）
- `src/views/new_worktree_dialog.rs`：标题/Esc 提示/字段前缀（尾随空格在键内）/
  " = create   "/" = cancel"，宽度计算与渲染同源（tr(LABEL_PREFIX).width()）
- `src/views/managed_connectors_wait.rs`：[copied]/[copy the url] 按钮（命中矩形
  由实际绘制串宽度推导）+ 两行说明
- `src/views/welcome/hero_box.rs`：HERO_SUBTITLE 整段一键（译文 57 列短于原文
  74 列不撑爆右栏）；Changelog 复用既有键
- `src/views/welcome/workspace_mode.rs`：status_label() 三分支 tr（唯一生产出口
  是状态条绘制，无比较消费）；"Workspace  " 前进量由硬编码 11 改显示宽度（英文
  等值）；选项 label() 在渲染处 tr（方法本身进日志/测试不动）；trailing 右对齐
  由字节 len 改显示宽度（中文按 3 字节会错位的必要伴随修正）
- `src/views/dock/mod.rs`：分组表头 tr(section.label())、"show {n} more" 整键、
  tab_hint() 与 kill_label()（[stop]）在方法内包 tr（命中矩形由绘制串宽度推导）；
  小写 subagents/tasks/watchers/queued 分键；layout.rs 无文案零改动
- `src/views/context_bar.rs`：**零改动**——"MAX %" 有 PCT_WIDTH=5 固定宽度契约
  （hover 进度条宽度由它反推 + 测试断言 len==5），无等宽中文等价物，豁免
- `src/views/credit_bar.rs`：usage_label() 出口 tr、标签冒号前缀全部重构为
  "{}: {}"（英文逐字节不变）、PAYG 双插值模板 ${used}/${cap} 整键、
  tr_str(&format!("{label} left")) 动态键三态。事实结论：credit_bar_line 系列
  生产无调用点（状态条不渲染它，真正在用的是 usage_warning/format_usage_summary），
  键入表备用
- `src/views/list_pane/render.rs`：" Copied!" toast（铺写改 set_string + 逐格还原
  bg，宽字符不错位；位置 min() 防越界）、输入条 4 前缀、matcher 模式词复用既有键；
  3 处宽度由 len 改 UnicodeWidthStr::width
- `src/views/block_viewer/mod.rs`：shortcuts_hints 12 处 hint 标签纯接线（键均在
  表）、"limit: "、result 计数两条（单复数拆键）、"Sources ({})"
- `src/views/picker.rs`：SEARCH_BAR_LABEL 四处出口 tr（布局宽度改显示列宽）、
  " / to search"、"Loading…"、"No matches"；picker_shortcuts 等 hint 纯接线；
  注意 picker_shortcuts 是 LazyLock——译文首次调用固化，语言随启动固定故无影响
- `src/slash/commands/theme.rs`：suggest_args 的 "auto (follow system)" 与
  " (active)" 后缀 tr（description/usage 由 slash_meta! 宏统一包 tr 无需重复）
- `src/slash/commands/debug.rs`：suggest_args 出口 tr_str；两条新键，第三条
  scroll-diagnostics 已在表纯接线
- `src/slash/mode_support.rs`：refusal() 三个模板整键 + {why}/{instead} 插值处
  tr；6 条 why（定义分散在 jump/dashboard/theme/find/timeline/tutorial 各命令
  文件）与 1 条 instead 在消费点统一查表，各定义文件零改动

### 仲裁与取舍（P4）

- `[cancel]`/`[Send now]`/`[edit]`（queue_pane 按钮组）不译：宽度按 ASCII
  `label.len()` 预算，三按钮 flush 链与窄面板丢按钮顺序被测试按 ASCII 宽断言
- `worktree ` 徽标（location.rs）不译：状态栏孪生实现（agent_view/render.rs）
  按 `"worktree ".width()` 预算路径热区偏移，两侧必须同进退
- `[Esc]`（btw_overlay）不译：键名徽标，参与右对齐宽度预留/命中矩形
- 近重复键并存：`"New Worktree"`（本文件原文大写）与既有 `"New worktree"` 分键
  （原文不可改）；`" search: "`（picker 布局 pad）与 `"search: "`（list_pane
  输入条）分键；`"queued"` 小写与 P3 `"Queued"` 分键
- 宽度语义伴随修正（英文逐值不变，中文才生效）：list_pane toast/输入条/status、
  picker 搜索条、workspace_mode trailing/Workspace 前进量共 6+ 处 byte len →
  UnicodeWidthStr::width，属"译文保宽"铁律的成对调整
- 模式名 minimal/fullscreen（refusal 模板 {current} 运行时值）未译：模式/命令
  标识符，与既有表正文用极简/全屏、命令名保留原文的译法并存
- screen_mode_switch.rs 零改动：计划所记"漏译一条"前提不成立（两键均在表且已包
  tr）；expand.rs 零改动：UseInstead 提示在 mode_support 消费点统一查表
- credit_bar 结论性豁免记录：状态条渲染路径当前未接（死代码），键入表备用，
  未来接线即生效

### 已知 Windows 环境族测试失败（与 P4 无关，A/B 定责留档）

P4 后跑 `cargo test --lib views::` 为 2766 通过 / 2 失败；两失败在 main 基线
（0878bc8d，P4 之前）同样失败，且 git -S 考古确认缺陷逻辑均来自上游提交
（c68e39f6 首发 / a5589e95 同步），LOCAL 各期未触碰：

- `views::extensions_modal::tests::handle_key_tab_completes_single_field_path`：
  `tab_complete_path()`（extensions_modal.rs ~1620）父目录回拼只认 `/`
  （`expanded.contains('/')` → `rsplit_once('/')`），Windows 路径是 `\` 分隔，
  parent_str 得空串 → Tab 补全只剩基名。上游 Linux CI 不撞的 Windows 真缺陷
  （同步文件，修复须登记重放），另行立项
- `views::btw_overlay::tests::done_state_scans_file_paths_like_scrollback`：
  测试用 POSIX 路径 `/Users/...`，扫描与解析链路（osc8.rs pass 2 →
  resolve_tool_path_target）全通，最后 `file_path_to_url` 的
  `Url::from_file_path` 在 Windows 拒绝无盘符路径 → 无 osc8_url。POSIX 语义
  假设的平台差异，非产品缺陷（Windows 真实路径带盘符，走 Prefix 分支正常）

## 十二期后：i18n 分级施工计划（P0–P4）全部完工

后续新增 UI 文案随写随补键即可；doctor 诊断与 tips 面板、markdown 正文仍是
范围外（见方案文档"砍掉/决策项"）。

## 十三期补丁（2026-09-18：会话默认 agent 尊重 `[agent] name` 配置）

### 问题

上游默认 `plan_mode`/`ask_user`/`subagents` 全开（`app.plan_mode = !args.no_plan`，
event_loop.rs:1241-1243，无正向 `--plan` 旗标、无法表达"用户显式选择"），TUI 创建
会话时 `SessionFlags::agent_profile()`（app/effects/helpers.rs）按旗标合成
`_meta.agentProfile = "grok-build-plan"`。shell 解析链（xai-grok-shell
agent_ops.rs `resolve_agent_definition`）中 ACP agentProfile（第 2 步）优先级
高于 config `[agent] name`（第 5 步）→ 用户配置的默认 agent 被静默覆盖，
新会话永远是 grok-build-plan。

### 改动（respect_config_agent 闸门）

- `src/app/effects/helpers.rs`：`SessionFlags` 增 `respect_config_agent: bool`；
  新增关联函数 `config_default_agent_name()`（读生效配置的 `[agent] name`）；
  `to_meta()` 的 agentProfile 合成分支加 `!respect_config_agent` 前置条件
- `src/app/event_loop.rs`：`session_flags_for_effects` 构造点：
  `respect_config_agent = app.agent_override.is_none() && config_default_agent_name().is_some()`
- `src/app/effects/tests.rs`：新增
  `respect_config_agent_suppresses_synthesized_profile`（闸门只抑制 profile，
  不影响 yoloMode 等其余 meta）

### 行为矩阵

- 配置无 `[agent] name`：行为与上游一致（合成 profile）
- 配置有 `[agent] name` 且无 `--agent`/`GROK_AGENT`：不发 agentProfile →
  配置 agent（grok-build-concise）生效；plan/ask-user 其余 meta 不受影响
- `--agent` / `GROK_AGENT`：显式选择，优先级照旧，闸门自动让位
- 每次 create-session 读一次配置（用户手动触发，非热路径）

### 重放注意

- 三处均为同步文件；闸门是纯前置条件，不改变 agent_profile() 本身的映射表
- 已知边界：配置 agent 后 plan 模式不再自动带 plan 工具集（enter/exit_plan_mode
  随 plan 定义走）——需要 plan 工作流时显式 `grok2 --agent grok-build-plan`

## 十五期补丁（2026-09-18：极简风格与 agent 变体解耦，`/style` 正交覆盖层）

> 背景：十三期的极简模式把风格规则绑死在 `grok-build-concise` 变体里（换载体=换工具
> 集+丢 AGENTS.md/MCP/子代理），用户要的是"风格约束叠加到任何 agent"。
> 本期把两者正交化：风格 = 可持久化开关的覆盖层；concise 变体回归上游本义
> （瘦工具集 + COMPACT_SYSTEM_PROMPT + agents_md:false，不再内嵌规则）。

### 语义（用户定义）

- `/style`（无参数）= 在开/关间切换；`/style minimal` 强制开；`/style default`
  强制关
- 切换**持久化**（`<grok_home>/output_style.json`，容错解析：缺文件/损坏=关）
- 每次 session spawn（主会话与子代理同一条路径）读一次持久化状态，把规则叠加到
  系统提示 → 下一次会话启动必然生效
- **首次模型调用前**切换：同时改写当前会话的 System 头（`replace_system_head`）
  并更新 actor 上的 live 标志 → 本会话立即生效
- 首次模型调用后切换：只持久化，**当前会话提示词不动**（不改写已建立的会话历史，
  无缓存抖动）；live 标志不再翻转
- 判定闸门 = actor 的 `first_model_call_done`（sampler_turn 的
  `record_response_token_usage` 与 `log_terminal_failure` 两个收口置位——成功与
  终态失败都算"发生过模型请求"）
- agent/model 切换重建提示词时按 live 标志重放覆盖层（幂等 strip+append），风格
  跨载体切换保持

### 改动（按 crate）

- `xai-grok-agent/src/prompt/template.rs`：`LOCAL_CONCISE_RULES` 更名
  `LOCAL_MINIMAL_STYLE_RULES`；新 `apply_minimal_style(base, enabled)`（按
  `# Output style: minimal` 头截断剥离 + 尾部追加，双向幂等）+ 单测
- `xai-grok-agent/src/config.rs`：`grok_build_concise()` 的 system_prompt 回归
  纯 `COMPACT_SYSTEM_PROMPT`（规则注入移除）
- `xai-grok-shell/src/agent/output_style.rs`（新）：持久化读/写
  `<grok_home>/output_style.json`（`grok_home()` 同款路径；容错解析）+ 单测
- `xai-grok-shell/src/session/acp_session.rs`：SessionActor 新增
  `output_style_applied: AtomicBool`（live 标志）与 `first_model_call_done:
  AtomicBool`
- `xai-grok-shell/src/session/acp_session_impl/spawn.rs`：bootstrap 处
  `apply_minimal_style(agent.system_prompt(), 持久化状态)`（主/子代理共享路径，
  全 agent 覆盖）；actor 初始化两个字段
- `xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`：两个收口点置位
  `first_model_call_done`
- `xai-grok-shell/src/session/acp_session_impl/model_switch.rs`：
  `handle_set_session_model` 的 concise 特例分支删除（`use_concise` 参数退化为
  `_use_concise`），改写统一走 `apply_minimal_style(agent.system_prompt(),
  output_style_applied)`；`handle_rebuild_agent_for_definition` 重建提示词同样
  按 live 标志重放
- `xai-grok-shell/src/session/slash_commands.rs`：`BuiltinCommand "style"` 注册
  （`BuiltinGate::AlwaysOn` 自动进保留名单）+ `BuiltinAction::SetMinimalStyle
  { enabled: Option<bool> }`（None=切换）+ `command_name`/`args_provided` 臂
- `xai-grok-shell/src/session/acp_session_impl/slash_exec.rs`：执行器（持久化 +
  首次调用前 `replace_system_head` 重写 + 翻转 live 标志；失败仅告警不回滚）
- `xai-grok-pager/src/slash/i18n.rs`：命令描述中文键 1 组（描述经
  acp_command 的 tr_str 通道自动翻译；argument_hint 与 always-approve 的
  "on|off" 同例不译）

### 决策记录 / 重放注意

- 覆盖层是**替换式**（strip 头截断 + append）：若自定义 agent 提示词正文恰好含
  `# Output style: minimal` 头会被截断——自定义提示词避免使用该头
- 持久化是进程外文件而非 config.toml：避免程序化改写用户手编配置；与
  announcements.json 同款小状态文件模式
- 压缩不重建系统头（`COMPACT_SYSTEM_PROMPT` 仅 concise 定义与
  `compact_system_prompt()` 访问器用，后者无生产调用方）——压缩后风格保持，无需
  额外接点
- 十三期的"concise = compact 基座 + 规则节"语义由本期取代：两者现在可独立组合
  （concise 载体 + `/style` 开 = 等价旧行为；默认载体 + `/style` 开 = 全工具集
  极简风格，MCP/子代理/AGENTS.md 不受影响）
- 测试PROFILE 注意：xai-grok-shell 测试 profile 有上游自带编译错误（见"已知
  问题"），本期 shell 侧验证以 `cargo check` 为准，单测覆盖放在
  xai-grok-agent（apply_minimal_style）与 output_style.rs 模块内

### agent crate 测试基线归因（A/B 静态证明，与本期无关）

`cargo test -p xai-grok-agent --lib` 10 失败，全部与本期 diff 零文件交集（diff
仅触及 template.rs/config.rs + shell/pager 10 文件，失败测试的输入在分支与
main 逐字节相同，属 main 既有 Windows 环境族）：

- `prompt::template::test_encrypted_templates_not_stale`：`.gitattributes` 对
  `templates/` 无 LF 规则，Windows CRLF checkout 使 include_bytes! 读到 CRLF，
  与 LF 生成的加密常量不匹配（上游 `Synced from monorepo` 自带）
- plugins::local_refresh / install_registry 7 例：symlink / 文件锁 / 临时路径
  家族
- prompt::skills 2 例：目录扫描家族

本期触及面全部通过：`minimal_style_overlay_appends_strips_and_is_idempotent`、
`test_mid_session_switch_concise_to_full`、template 组 31/32（唯一失败即上述
既有项）。

## 十四期补丁（2026-09-18：国模适配——think 标记泄漏 + ChatCompletions 网关 quirk）

> 背景：DeepSeek V4 Flash 经 OpenAI 兼容端点接入时，`</think>` 结束标记被当作正文
> 泄漏到 UI（推理本体走 `reasoning_content` 字段，网关在 reasoning→正文边界把孤立的
> `</think>` 作为单独 content chunk 下发；opencode issue #34126 同款流形态）。
> 调研结论（opencode #1325/#34698/#15389、vercel/ai extractReasoningMiddleware、
> qwen-code taggedThinkingParser.ts、crush/fantasy、vLLM ReasoningParser）：朴素
> 字符串替换必败于跨 chunk 拆分（LiteLLM ollama 反面教材），正确做法是跨 chunk
> 流式状态机；存储层保持"正文已剥离 + reasoning 单独成项"（opencode 保留原文的
> 哲学被否：grok 的 content_acc 会原样回传下一轮，标记必须不进存储正文）。

### xai-grok-sampler
- 新 `src/stream/think_split.rs`：`ThinkTagSplitter` 两相状态机（text/think），
  喂 content delta 产出 (text, reasoning) 二元组。要点：
  - 双标记集 `<think>|<thinking>` / `</think>|</thinking>`（qwen-code 同款）；
  - 跨 chunk holdback：buffer 整体或最长后缀是任一标记的真前缀时扣留
    （vercel/ai getPotentialStartIndex 同款；上限 11 字节）；
  - 边界孤儿标记：字段 reasoning 已见 + 正文未开始（纯空白不算）时，孤立或
    前导 `</think>` 丢弃——正文开始后同串原样保留（opencode #34698 的
    窄语义 + no-regression 两条测试同款）；
  - EOF flush 不丢字节：未闭合 think 块归 reasoning，扣留的半截标记归正文
    （qwen-code `final` 语义，修 vercel/ai TransformStream 无 flush 的坑）；
  - 17 个单测（含 #34126 流形态、跨 chunk 拆分、`<think></think>` 空块、
    先正文后 think、多 think 块、`value < than` 误报保护等）。
- `src/stream/mod.rs`：`mod think_split;`（LOCAL 注释处）
- `src/stream/chat_completions.rs`：
  - reasoning 分支提到 content 分支前（混合 delta 时先武装边界规则），
    `reasoning_content.or(reasoning).or(reasoning_text)` 归一化（GLM/vLLM 的
    `reasoning`、Kimi 的 `reasoning_text` 别名字段上同一通道）；
  - content 分支过 `think_splitter.feed()`，reasoning 产出与字段产出同路：
    FirstToken/chunk_index/reasoning_acc/ChannelToken::Reasoning；text 产出照旧
    （无标记流逐字节不变，存量测试零改动通过）；
  - 流尾 `finish()` flush 尾巴（Partial marker/未闭合块）；
  - 工具调用 id 缺失时合成 `call_{index}`（部分网关从不发 id，结果无法配对）；
  - 测试追加 6 个：#34126 边界流形态（断言零 Text token）、内联 think 跨 chunk、
    别名字段、未知 finish_reason serde、DeepSeek 缓存 token、空工具 id 合成。
- `src/stream_classify.rs`：`chat_chunk_has_content` 解构补 `reasoning`/`reasoning_text`
  并计入 content 判定（TTFT 门控对别名字段不失效）

### xai-grok-sampling-types
- `src/types.rs`：
  - `ChatChunkDelta` 尾部追加 `reasoning: Option<String>` /
    `reasoning_text: Option<String>`（serde default + skip_serializing_if）
  - `FinishReason` 追加 `#[serde(other)] Other` 变体——DeepSeek 专有
    `insufficient_system_resources` 等未知 finish_reason 会让整个 chunk 反序列化
    失败杀掉整条流，此变体兜住
  - `Usage` 尾部追加 `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`
    （DeepSeek 把缓存命中报成扁平 usage 字段而非
    `prompt_tokens_details.cached_tokens`）
- `src/conversation.rs`：
  - `From<FinishReason> for StopReason`：`Other => StopReason::Stop`（最诚实的
    近似映射）
  - `From<Usage> for TokenUsage`：`cached_prompt_tokens` 取
    `details.cached_tokens.max(prompt_cache_hit_tokens)`（两套字段并存时取大）

### 决策记录
- Messages（Anthropic 兼容）后端不改：思考走结构化 thinking block
  （`ThinkingDelta` 已处理），DeepSeek Anthropic 端点不会内联标记进 text
- Responses 后端不改：结构化 reasoning item，同理免疫（opencode PR #34698
  描述中同样结论）
- 多轮回传不额外剥历史：流层修复后新 content 不再含标记；存量会话含标记的
  按 opencode 争议决策保留原样（改动面大、收益不确定）
- 不加配置开关：无标记流逐字节直通（仅尾部 `<` 类前缀的跨 chunk 扣留，下一
  chunk 或 EOF 必然归还），常开零风险
- 版本：v1.0.35

## 趣味等待文案（witty loading phrases，2026-09-20）

移植 qwen-code 等待模型响应时的随机趣味文案（Apache-2.0；词表与轮换机制见
`packages/cli/src/ui/hooks/usePhraseCycler.ts`、`packages/web-shell/client/constants/loadingPhrases.ts`、
`packages/cli/src/i18n/locales/zh.js` 的 `WITTY_LOADING_PHRASES`）。分支
`feat/local-witty-loading-phrases`。

### xai-grok-pager
- `src/views/witty_phrases.rs`（新增）：EN/ZH 全量词表原文迁入 +
  `witty_phrase(turn_elapsed)`——按「本轮已进行秒数 / 15」时间桶 + 进程种子
  FNV-1a 取词（确定性伪随机，无 RNG 依赖），语言取 `slash::i18n::current_lang()`
- `src/views/mod.rs`：注册 `witty_phrases` 模块
- `src/views/turn_status.rs`：`compute_activity` 增参 `turn_elapsed`；
  `Thinking` / `Responding` 两臂的状态行文案从固定 "Thinking…" / "Responding…"
  改为趣味轮换词；其余（工具运行/重试/Compacting/Waiting 家族）不动
  （`Waiting(Model)` 的 "Waiting for response…" 有 pty e2e 依赖，保留）
- 测试：`views::witty_phrases` 4 个纯单测 + `turn_status` 既有用例改为断言词表成员
## /stats 模型用量聚合（model usage ledger，2026-09-20）

`/stats` 新增按 5h/一周/一月 × model id 聚合的用量/性能报表（累计输入/输出/缓存读/
缓存写 token、平均缓存命中率、TTFT 与 TPS 的 p50/p90），压缩账本段保留在其后。
分支 `feat/local-model-usage-stats`。

### xai-grok-tools
- `src/model_usage_ledger.rs`（新增）：调用粒度 JSONL 账本
  `grok_home/cache/model-usage.jsonl`——shell 每次成功推理追加一条采样
  （ts/model_id/四类 token/ttft/tps/duration），文件超 2MB 按保留窗口（35 天）重写剪裁；
  `aggregate(samples, window_ms, now)` 纯函数聚合 + 最近邻秩百分位

### xai-grok-shell
- `src/session/acp_session_impl/turn.rs`：`ModelResponseReceived` 事件落盘后
  追加一条 `model_usage_ledger` 采样（model_id 改为 clone 传入）

### xai-grok-pager
- `src/slash/commands/stats.rs`：重写为「用量聚合报表 + 压缩账本」双段；
  `format_usage_report(samples, now)` 纯函数可测
- `src/slash/i18n.rs`：命令描述换键 + 13 个报表词条

### 已知边界
- 采样只覆盖主循环推理路径（subagent 的模型调用不经 turn.rs，暂不计入）
- 历史数据不回填：账本从合入后新产生的调用开始累积

## 本文件修复记录（2026-09-20）
- 此前一次合并冲突解决经 PowerShell 默认编码往返，本文件中文被按 GBK 误读再编码，
  随 16f167b7 推送损坏；本次重建为 research/main 两侧账本的无损并集。
- 研究分支（research/agent-cli-tracking）的流层实现（thinking_scrub/实验开关）与
  main 的 deepseek-compat（think_split 常开）取并集：保留 think_split 常开语义与
  dialect 学习/回放，移除实验开关 `thinking_tag_scrub`（行为由 think_split 全量承载）。

## Windows all-targets 修复批次（2026-09-20，feat/local-win-build-hygiene）

首次在本机跑 `cargo check --workspace --all-targets`（上游只在 Linux 编测试/bench/example），
暴露两类问题并就地修复：环境族（unix 专属测试代码在 Windows 上的噪声）与真实损坏
（research 分支/本地提交给 `SessionActor` 前后加过两个字段，xai-grok-shell 的测试
从没在本机编过，断链未被发现）。全部为最小 diff，Linux 侧行为不变。

### 环境族（cfg 门控 / 本地桩）
- `xai-grok-pager-pty-harness/src/pty.rs`：`process_has_exited_without_reap` 的
  import 加 `#[cfg(unix)]`（上游 bug：lib 内全部调用点已 unix 门控，仅 import 漏了）
- `xai-grok-pager-pty-harness/benches/paste_latency.rs`：本地补 `Surface::as_str`
  （strum 0.27 的 `AsRefStr` 只生成 `AsRef<str>`；经 `IntoStaticStr` 桥接）
- `xai-grok-pager-pty-harness/src/scenarios/mod.rs`：同因补 `pub fn Scenario::as_str`
- `xai-grok-mermaid/src/{mmdc,subprocess}.rs`：测试 mod 里仅被 `#[cfg(unix)]`
  用例使用的 `Instant`/`Stdio`/`detached` 加门控
- `xai-grok-sandbox/examples/sandbox_smoke_test.rs`：非 unix 平台提供
  no-op `main`/`test_read`/`test_write` 桩（Landlock/Seatbelt 本就 unix 专属）

### 真实损坏（本机/上游 CI 均不编译该 crate 测试导致积压）
- `xai-grok-shell` 测试：11 处 `SessionActor {` 字面量补
  `output_style_applied` / `first_model_call_done`（AtomicBool::new(false)，
  与 spawn.rs 生产路径默认一致）；`src/agent/output_style.rs` 测试里
  `Instant::timestamp_nanos_opt`（用错类型，应为 `SystemTime`）改
  `duration_since(UNIX_EPOCH).as_nanos()`；`benches/session_list.rs` 去重
  `agent_id`/`attempt_id`（research 合并残留）
- `xai-grok-shell/Cargo.toml`：补 `[[test]] test_startup_prefetch_repair_overlap`
  条目 `required-features = ["test-support"]`（兄弟 5 个条目都有，该文件漏登记）
- `xai-grok-shell/tests/{acp_harness/mod.rs,common/mod.rs}`：
  `reset_startup_settings_for_tests` / `clear_startup_profile_for_tests` 调用点
  加 `#[cfg(feature = "test-support")]`（lib 侧函数本就带该门控）
- `xai-grok-workspace/src/workspace_ops.rs`：漂移守卫
  `hook_event_name_wire_covers_all_upstream_variants` 补
  `BeforeModelCall => Unknown("before_model_call")` 映射并加入轮询列表
  （本地 phase-3 给 `HookEventName` 加变体后守卫未跟上）

## 增量缓存 0xc0000005 处置（2026-09-20）

本机 rustc 反复 `STATUS_ACCESS_VIOLATION (0xc0000005)`，清
`target/debug/incremental` + `CARGO_INCREMENTAL=0` 后同命令全绿，确认是
Windows 增量缓存损坏（上游已知家族，rust-lang/rust #134119/#144652/#148067）。
自延续机制：崩溃/杀进程留下半写缓存，后续每次增量运行加载即崩，直到手清。

处置：不全局关增量（全量重编更贵），`ctest.sh` 执行尾部加自愈——检测
`0xc0000005`/`STATUS_ACCESS_VIOLATION` 自动清增量缓存重试一次，平时零开销。
配套措施（建议但不入库）：把仓库目录、`D:\cargo-tmp`、`~/.cargo`、`~/.rustup`
加入 Defender 排除（本机注册表确认当前无任何排除项）；`rustup update` 跟进
stable 修复。暂不建议 Dev Drive/ReFS（#151181 在其上有增量回归）。

## CI release 缓存投毒：陈旧 rmeta 导致 E0609（2026-09-20，feat/local-release-cache-fix）

现象：tag `v1.0.37-preview.1` 两次 release 构建在 linux/windows 同报
`error[E0609]: no field reasoning_text on type ChatChunkDelta`，但该字段在
tag 源码里存在、本地 `cargo check --release -p xai-grok-sampler` 全绿。

根因（CI 日志证据：全程无 `Compiling xai-grok-sampling-types` 行，sampler
却链接到了它的产物）：release.yml 开了
`cache-workspace-crates: "true"`（背景见 `docs-local/ci-cache-incident.md`
"v1.0.29" 一节），叠加 `git-restore-mtime`。cargo 按 **mtime** 做指纹，
restore-mtime 把 research 合并改过的 `types.rs` 回拨到其最后提交时刻，与
缓存 blob 里记录的指纹时间一致 → cargo 判定源码未变 → 复用合入前编译的
旧 rmeta。`cache-on-failure: true` 又把带毒产物存回同一 key，重打 tag 全部复现。
这正是 rust-cache 默认不缓存 workspace crate（README: "generally not
effective"）要防的场景——默认值被关掉才踩的坑。

修复（`a895e0ca` / main `40f330d0`）：

- release.yml 两个 job：`cache-workspace-crates` 恢复默认 `"false"`；
  workspace crate 每次 tag 重编（registry deps 仍走缓存）。
- `cache-scope` 改名 `release-2` / `release-xwin-2`，带毒 v1 blob 永不复用
  （一次性冷重建）。

对交接文档 `docs-local/ci-failures-2026-09-20.md` 的对账：其"修复 1"假设
源码不一致（建议重排 `.or()` 链）不成立——三字段在 tag 源码中齐备且无 cfg，
无需改 sampler；其"修复 2"（SessionActor 字面量 + output_style Instant）
已随 `1815e39b` 修复。

## README 改为 fork 版（2026-09-21）

`README.md`（上游同步文件）整篇重写为社区 fork 说明：仓库定位与二进制名
（`grok2`，与官方 `grok` 并存、自动更新默认关）、快速开始、**第三方模型接入**
（机制 + 最小配置 + 验证命令 + 6 条常见坑 + `model_providers`/`auth_provider`/
`endpoints.models_base_url`/`preferred_method` 进阶）、本地新特性分主题清单
（模型与网关 / 网络与代理 / Windows / 上下文与成本 / 交互与界面 / 扩展与钩子）、
配置速查表、**默认行为差异**（中文界面、状态行开启、自动更新关、工具输出压缩关）、
内置文档与 skill 指引、构建、开发约定（分支纪律 + ctest 门控 + PATCHES 重放）、
来源与许可。

重放注意：上游同步会整体覆盖 `README.md`。同步后**不要逐行合并**——直接从本
仓库历史取回 fork 版 README，再按上游本次同步的变更逐条回填（上游 README 主要
变动点是安装方式、crate 布局表、文档索引）。`SOURCE_REV` 与构建命令段需按同步
后的实际情况核对。

特性事实的唯一台账仍是 `docs-local/PATCHES.md`（本文件）与内置文档
`docs/user-guide/29-local-enhancements.md`；README 只做汇总索引，新增特性时同步
补一节，不在这里复述实现细节。

## think_split 多字节字符 panic 修复（2026-09-20，v1.0.37-preview.2）

现象：流式正文里出现 CJK / 制表符时**整个进程 panic 崩溃**（会话直接死掉，TUI
里只留一行 panic 头）。当日两次现场：

```
thread 'ses-01a0be8c' (16248) panicked at crates/codegen/xai-grok-sampler/src/stream/think_split.rs:160:66:
byte index 1 is not a char boundary; it is inside '这' (bytes 0..3) of `这篇`

thread 'ses-01a0be97' (17856) panicked at .../think_split.rs:160:66:
byte index 2 is not a char boundary; it is inside '│' (bytes 1..4) of ` │ ✓`
```

两例同为 step-3.7-flash / grok-build-plan，且都崩在**首个内容 delta**；
01a0be8c 的 `unified.jsonl` 尾部（`loop_index: 2` inference_start →
`waiting_model→thinking` phase transition）与 `first_token` 事件时间戳对齐，
可确认崩溃点在流式接收路径。

根因：`ThinkTagSplitter::emit_all_but_partial_marker` 的 holdback 窗口扫描按
**字节**枚举后缀起点（`for n in (1..MAX_TAG_LEN).rev()` → `start = buf_len - n`），
把不落在字符边界上的偏移直接喂给 `&self.buffer[start..]`。delta 尾部是非 ASCII
时（中文回答、表格/清单里的 `│ ✓`），`buf_len - n` 就会落进某个多字节码点内部
→ 切片 panic。触发面与内容语言强相关：只要尾部多字节字符落在 1..11 字节探测窗口内
即崩，故中文场景近乎必现。注意这不是"标记跨 chunk 拆分"的边界语义问题，
而是纯越界切片——无标记的普通正文一样会崩。

修复：扫描前加 `is_char_boundary` 守卫，跳过码点内部偏移（标记恒以 ASCII `<`
开头，这类偏移本就不可能起标记，跳过不改变任何 holdback 语义）。同 crate
`doom_loop_recovery.rs` 的同类截断早就是 `while cut > 0 && !text.is_char_boundary(cut)`，
本次是 think_split 漏了这个守卫。


验证：`think_split.rs` 追加 2 例（纯 CJK delta `这篇`；标记前后夹中文
`回答<thi|nk>想一下</think>|结束`），修前两例均在 160:66 panic，修后
`ctest.sh -p xai-grok-sampler --lib` 263 例全过。开发构建取栈（release 二进制
在 TUI 下只打出 `stack backtrace:` 头行 + note，无可用帧）：

## /stats 窗口化：三时间窗标签页 + 参数 + i18n（2026-09-20，feat/local-stats-modal-i18n）

`/stats` 从「回滚缓冲纯文本」升级为模态窗口，标签栏即时间窗，视觉与交互
对齐 `/usage`（同一 `modal_window` 框架：边框、标签栏、页脚快捷键、滚动）。
月窗按需求移除，改为 **5h / day / week** 三个窗口。

已完成：T1（参数 + day 窗）与 T2（模态窗口）。T3（全仓 i18n 扫尾）**未做**，
本批只覆盖 `/stats` 自身新增文案。TODO 与验收标准见
`docs-local/stats-modal-todo.md`。

### xai-grok-tools
- `src/model_usage_ledger.rs`：常量组改为 `WINDOW_5H_MS` / `WINDOW_DAY_MS`（新增）/
  `WINDOW_WEEK_MS`，**删除 `WINDOW_MONTH_MS`**；新增 `Window` 枚举
  （`ALL` / `len_ms` / `arg` / `from_arg` / `label` / `index` / `from_index`），
  供命令参数解析与标签页复用；`retain` 剪裁窗口与报表窗口解耦（原注释写"保证月窗
  完整"，月窗已无，保留窗口只剩"限制账本无界增长"的作用，注释同步订正）；
  新增 `fmt_tokens` / `fmt_ms_pair` / `fmt_tps_pair`（从 pager 下沉，使文本路径与
  模态窗口共用同一份数字格式化）
- 测试：day 窗边界（`now - WINDOW_DAY_MS - 1` 排除）、`Window` 参数解析与索引
  往返/越界钳制、`fmt_*` 边界，各拆独立用例

### xai-grok-pager
- `src/views/stats_modal.rs`（新增）：`StatsModalState` + `StatsTab`
  （三个时间窗 + 压缩账本共 4 标签页）+ `route_stats_modal_key/mouse` +
  `render_stats_modal`。每个 model 一张卡片（id + 调用数，token 与性能两组指标行，
  数值右对齐，面板过窄自动退化为每行一个指标）；超过 8 张卡片折叠为
  `… +N more` 一行，`a` 展开/收起；`y` 复制整份报表；`1`–`4` 直跳标签页。
  骨架（标签步进/滚动钳制/Outcome 枚举）与 `usage_modal` 同形，不塞进
  `usage_modal`（后者的标签页语义是会话用量/限额，与本地历史聚合不同源）
- `src/slash/commands/stats.rs`：`takes_args: true`，`usage` 改
  `/stats [5h|day|week]`，加 `suggest_args`；未知参数报
  `Unknown argument: {arg}. Use /stats [5h|day|week]`（对齐 `/usage` 文案风格）。
  全屏模式返回 `CommandResult::Action(Action::ShowStats { window })`；
  **保留极简模式文本路径**（极简无浮动窗口，仍走回滚缓冲全窗口报表）
- `src/app/actions.rs` / `dispatch/router.rs` / `dispatch/status.rs`：新增
  `Action::ShowStats { window }` 与 `dispatch_show_stats`（本地同步读取
  model-usage.jsonl + 压缩账本，**无** `fetch_nonce` 异步那套；已开窗口则只
  重新定位标签页）
- `src/views/modal.rs`：`ActiveModal::StatsInfo` 变体 + 标题映射（`Model usage`）
- `src/app/modals.rs` / `agent_view/panes.rs`：键盘、鼠标、滚轮路由与渲染分支
- `src/slash/i18n.rs`：新增/替换报表词条（窗口标签、卡片指标、页脚说明片段），
  删除已无用的 `last 5h` / `last week` / `last month` 键

### 已知边界
- 极简模式的文本路径与模态窗口共用 `fmt_*` 与聚合函数，但文本路径仍渲染全部
  三窗（与改造前的"全窗口"输出保持一致），窗口分节按 `Window::ALL` 顺序
- `Window::label()` 返回的英文串同时是 i18n 键（如 `Last 5h`），改文案须同改表

## CI 缓存清理脚本修复：过宽前缀会误删在用 scope（2026-09-20）

`scripts-local/purge-gh-caches.py` 的 `DELETE_PREFIXES` 用了 `v1-rust-release-`
与 `v1-rust-release-xwin-`，而**这两个串分别是当前在用的
`v1-rust-release-2-…` / `v1-rust-release-xwin-2-…` 的前缀**——按原清单执行会把
活着的 release 缓存一并删掉（preview 打 tag 后下次构建退化为冷编译）。
改为以 `-<os>-` 收尾的精确 scope 前缀（`v1-rust-release-Linux-x64-`、
`v1-rust-release-xwin-Linux-x64-`），并把当前 scope 与 build 条目移入
`KEEP_PREFIXES`。build 两条尾部 hash 不同、无法从 key 反推哪条是活的，
且各仅 ~290 MiB，一并保留（宁可留，不误删）。


验证：`think_split.rs` 追加 2 例（纯 CJK delta `这篇`；标记前后夹中文
`回答<thi|nk>想一下</think>|结束`），修前两例均在 160:66 panic，修后
`ctest.sh -p xai-grok-sampler --lib` 263 例全过。开发构建取栈（release 二进制
在 TUI 下只打出 `stack backtrace:` 头行 + note，无可用帧）：

```
6: xai_grok_sampler::stream::think_split::impl$0::emit_all_but_partial_marker::closure$1  at .\src\stream\think_split.rs:160
8: xai_grok_sampler::stream::think_split::ThinkTagSplitter::emit_all_but_partial_marker at .\src\stream\think_split.rs:160
9: xai_grok_sampler::stream::think_split::ThinkTagSplitter::feed                        at .\src\stream\think_split.rs:109
```

## 内置命令清单同步：`available_commands` 期望列表补 `/style`（2026-09-21）

**同步文件**（上游 `Synced from monorepo` 自带）：
`crates/codegen/xai-grok-shell/src/session/slash_commands_tests.rs`

十五期把 `BuiltinCommand "style"` 插在 `BUILTIN_COMMANDS` 的 `always-approve`
之后（`slash_commands.rs:88`），但本同步文件的
`available_commands_orders_builtins_first` 期望列表没跟着加，导致 main 的
build 工作流 linux job 在 `Test shell (in-process harness)` 步骤挂 1 例
（6657 passed / 1 failed，run 35521509986）：

```
left:  [..., "always-approve", "style", "flush", ...]   // 实际（含本地新增命令）
right: [..., "always-approve", "flush", ...]            // 期望（未同步）
```

补丁：期望列表在 `"always-approve"` 后插入 `"style"`，并加
`// LOCAL(minimal-style)` 注释说明它必须在 `flush` 之前、与
`BUILTIN_COMMANDS` 顺序一致。重放方式：同步冲掉后，把 `"style"` 重新插回该位置。

**注意**：此不一致自 `676ae80`（/style 合入 main，2026-09-18）起就存在，但那次
push **没有触发 build 工作流**（main 上 9-18 `f50f0f9e` 之后直接跳到 9-20
`be1ce7f`），所以直到 9-20 的 `/stats` 合入才暴露。割 tag 发预览版时 release
工作流只 build 不跑 shell 套件，同样拦不住这类同步漂移。

验证：静态对账（期望列表逐项比对 CI 日志里的 `left` 数组，22 项全等）；
本机 shell 套件因上游自带测试 profile 编译错误无法运行（见"已知问题"），
最终以 Linux CI build job 为准。

## `/stats` 卡片性能指标对齐（2026-09-21，feat/local-stats-card-align）

`/stats` 卡片里 ttft/tps 与 token 段对不齐，根因两条：
1. 性能段用复合标签 `ttft p50/p90`（12 列）比 token 段最宽的标签
   （`cache write`，11 列）宽，值 `4536/12324`（10 列）又超出固定数值列
   `VALUE_WIDTH = 8`——`saturating_sub` 只会把标签后间隔压到 1 列，数值列不再成立；
2. token 段与性能段各自调用 `metric_rows`，各自算自己的标签列宽，两段数值右边缘
   本来就不在一批列上（宽面板下 token 挤 4 列、性能才 2 列，错位最明显）。

窄面板下还会溢出：内容宽 44 列时性能行实宽 47 列，数值被顶出边框。

改法：p50/p90 各拆成一格（`ttft p50` / `ttft p90` / `tps p50` / `tps p90`），
标签不再比 token 段宽；列几何升为卡片级共享（`MetricGrid`：`label_w` / `value_w`
取自卡片内全部指标，`VALUE_WIDTH` 退化为数值列下限，数值更宽时自动放宽），
token 段与性能段只是分段换行；性能段列数上限固定 2（`PERF_COLS`），ttft 一行、tps 一行。

### xai-grok-tools
- `src/model_usage_ledger.rs`：新增单值格式化 `fmt_ms` / `fmt_tps`（与
  `fmt_ms_pair` / `fmt_tps_pair` 数值同口径；成对形式留给极简模式文本报表）；+1 单测

### xai-grok-pager
- `src/views/stats_modal.rs`：`metric_rows` → `MetricGrid`（列几何卡片级共享 +
  数值列按实际内容放宽 + 单段列数上限）；卡片性能段从 2 个复合格改为 4 个独立指标格；
  复制文本（`model_card_text`）同步为同一份数字口径
- 测试：+3 用例——真实账本数据（ttft 4536/12324、tps 98.8/230.4）的黄金串对齐、
  四个宽度档不越界（含 9 列宽数值 `10000.00m` 撑开数值列）、缺 ttft/tps 采样时占位
  `n/a`；`y_copies_every_section` 补复制文本口径断言

### 已知边界
- 极简模式文本报表仍是 `ttft p50/p90 4536/12324` 成对形式（滚动缓冲单行更紧凑），
  数字口径与卡片一致，仅排版不同
- 性能段标签（`ttft p50` 等）仍是英文字面量，未进 i18n 表——属 T3 全仓英文扫尾范围

## `/stats` 单次调用的缓存命中率显示 `n/a`（2026-09-21，feat/local-stats-card-align）

背景：`glm-5.3-flash`（火山 Ark）与 `step-3.5-flash`（StepFun）在 `/stats` 里显示
`cache hit 0.0%`，被读成"这两个 model 不支持缓存"。排查结论见
`docs-local/prompt-cache-notes.md`：**命中率数字本身没错**，但单次调用必然冷启动
（`step-3.5-flash` 在账本里只有一条样本），0.0% 会误导；另外 Ark 的缓存前缀包含
tools 块，MCP 工具集变动会整段打断复用，跨会话一次性试模型必然 0。

改动：命中率显示收进共享格式化 `fmt_hit_rate(calls, rate, na)`——`calls > 1` 才算
百分比，`calls == 1`（及无样本）返回 `na`。

### xai-grok-tools
- `src/model_usage_ledger.rs`：新增 `fmt_hit_rate`（+1 单测：多调用 98.6% / 单次 `n/a`）

### xai-grok-pager
- `src/views/stats_modal.rs`：卡片与复制文本改用 `fmt_hit_rate`
- `src/slash/commands/stats.rs`：极简模式文本报表同口径（三处显示一致，避免同一份
  聚合数据在不同界面给出互相矛盾的数字）
- 测试：+1 用例（单次调用的卡片行与复制文本都出 `n/a`）；文本报表用例补
  `cache hit n/a` 断言


## 状态行 ttft/tps 长时间缺失（2026-09-21，feat/local-perf-ttft-output-streams）

背景：用户在 v1.0.37-preview.3 上反馈状态行 TPS"丢失"。排查结论：代码无回归
（TPS 链路自 v1.0.36 起 byte 级不变，release 二进制确证构建自标签 commit）。真因是
数据门槛：`build_turn_perf()` 要求 signals 会话均 TTFT > 0 才显示 ttft，tps 公式又
依赖 ttft，而 sampler 三个流里只有**文本 delta** 推 `chunk_timestamps`——
reasoning delta（`reasoning_content` / `<think>` / ThinkingDelta）与工具调用参数
delta 只设 `chunk_has_content`（空闲超时用）。纯思考+工具调用的 agentic 开场连续
多个请求不产生任何 TTFT 样本，要等第一次流出可见文本，ttft/tps 才一起出现。

语义修正：TTFT 改为"首个流式输出 token（文本 / reasoning / 工具参数）"。

### xai-grok-sampler
- `src/stream/chat_completions.rs`：reasoning 字段臂、think_split 的 reasoning 臂、
  tool_calls 臂补 `chunk_timestamps.push(Instant::now())`
- `src/stream/messages.rs`：ThinkingDelta 臂、InputJsonDelta 臂同款补齐
- `src/stream/responses.rs`：ReasoningSummaryTextDelta / ReasoningTextDelta /
  FunctionCallArgumentsDelta 臂同款补齐
- `src/metrics.rs`：`time_to_first_token_ms` 与 `from_timestamps` 文档注释同步语义
- 影响：signals 的 ITL / 均值 TTFT 样本从首个输出 token 开始计；`/stats` 的 ttft
  分位数在工具-only 回合也有样本（此前整回合缺席）
## 状态行 ttft/tps 基本不显示：0ms 伪样本污染 + 窗口错配（2026-09-22，fix/local-perf-ttft-zerosample）

## 压缩请求半答并行 tool call 回填（2026-09-23，feat/local-compaction-toolpair-repair）

> 全量证据链（会话工件、事件流、上游复现探针）见
> `docs-local/compaction-dangling-toolcall-400.md`。

### 问题

`/compact` 在 DeepSeek 上游 400：`An assistant message with 'tool_calls' must be followed by
tool messages responding to each 'tool_call_id'. (insufficient tool messages following
tool_calls message)`。成因：回合在「并行工具仍有一个在飞」时中止，历史留下
「assistant 带 2 个 tool_calls + 只有 1 个 ToolResult」的半答 run；普通模型请求会在
`BuildConversationRequest` 里回填（`ensure_conversation_integrity`），压缩走 `GetConversation`
纯读不回填，而 prep 里的 `truncate_trailing_incomplete_tool_call` 只认「末项是 assistant」。
同一个上游对 MiMo 放行、对 DeepSeek 拒绝，故表现为"某模型压缩必挂"。

### 改动（同步文件，重放用）

- `crates/codegen/xai-chat-state/src/compaction_utils.rs`
  - 导入 `DanglingToolCallReason` / `repair_dangling_tool_calls`（`xai_grok_sampling_types`）
  - `prepare_conversation_for_verbatim_summarization`：`truncate_trailing_incomplete_tool_call`
    之后追加一次 `repair_dangling_tool_calls(&mut conversation, HarnessHalted { class:
    "compaction" })`——按调用顺序为未答的 tool_calls 插入合成 `tool_result`
- `crates/codegen/xai-chat-state/src/compaction_utils_tests.rs`
  - `verbatim_backfills_unanswered_call_in_partial_parallel_run`（半答并行 run：真实结果保留、
    合成结果指名工具、`has_dangling_tool_calls` 为假）
  - `verbatim_backfill_skips_complete_runs_and_is_idempotent`（完整 run 不回填、二次调用不变）

### 覆盖面

`session/compaction.rs` 三处请求组装（941 verbatim、256/347 two-pass、1165 阶梯降级）与
`helpers/session_recap.rs` 全经此函数，一处修复全覆盖；lossy 路径本就剥掉全部工具消息，免疫。

### 重放注意

- 只动这一个 `pub fn` 的尾部三行逻辑，上游若自行在 prep 里加了回填（如改用
  `repair_dangling_tool_calls`）则本补丁作废，直接删
- 用户侧绕过（旧二进制）：`features.compaction_verbatim_input = false` 走 lossy 路径
- 未处理反方向（孤儿 `ToolResult`），该形态由 `/repair` 与会话加载覆盖

## think_split 把正文里被反引号括起来的 `<think>` 字面量当控制标记（2026-09-24，fix/local-think-split-code-span）

现象：TUI 里可见回答**恰好在某个落单反引号处截断**，其后的内容（连同 `##` 标题、表格、
清单）整段出现于一个折叠的 “Thought for Xs” 思考块。用户先后以 `'` 与反引号描述，实为
同一处——折叠前最后一个可见字符就是那个反引号。

根因（`stream/think_split.rs`，LOCAL 独有文件）：`find_earliest(&buffer, &OPEN_TAGS)` 在正文
**任意位置**匹配 `<think>`/`<thinking>`，不看 markdown 上下文。模型在回答里*引用*协议时写的
行内代码跨度 `` `<think>` `` 被当成真标记：标记本身被 drain（两个通道都不出现），跨度首反引号
留在正文、第二个反引号连同其后**全部剩余 token** 走 reasoning（永不闭合 → `finish()` 按
qwen-code `final` 语义整块 flush 成 reasoning，字节不丢）。下游逐级放大：
`ChannelToken{Reasoning}` → `AgentThoughtChunk` → pager `RenderBlock::Thinking` → 回合结束
`finished_display_mode() = Collapsed`（`blocks/thinking.rs:553`）。

证据：扫本机全部 76 份会话 `updates.jsonl`，同形态 **6 处 / 3 会话**（`01a0c38e`×4、
`01a0c849`×1、`01a0d161`×1）。判定签名：正文 chunk 以反引号结尾、紧随的 thought chunk 以
反引号开头、且 `<think>` 字面量两个通道都缺——message/thought 两条 chunk 通道均不跨类型合并
（`update_chunk_merge.rs`、`persistence.rs::maybe_merge_notification`），故该缺字只可能由
splitter 吞掉。样本 `01a0d161` 第 42/43 条：原文 “强制每轮以 `` `<think>` `` 开头，修复流式
空响应”，其后续 1125 字符（含 `## 四`、`## 五` 两节）全部落入思考块。

修复：`think_split.rs` 新增 `CodeScan`，按**可见文本通道**增量跟踪行内代码跨度与反引号/波浪号
围栏；`in_code()` 为真的候选标记按普通文本发回正文（不再 drain），其余行为不变。要点：
- 跨度按行跟踪：未闭合的反引号只致盲本行，换行即清空，不连累后文；
- 围栏要 3+ 同字符且是行首内容（≤3 空格缩进）才开，闭合需同字符且长度 ≥ 开围栏长度；
- 跨 delta 的 run 用 pending 状态承载（` ``` ` 被拆成 `"``"`+`"`"` 也能正确开围栏）；
  `code_after_run` 与 `flush_run` 共用判据，避免“未 flush 的 run”被误判为非代码；
- 只喂正文通道（reasoning 不喂）；holdback 扣留的尾巴在真正发出时才喂，顺序不颠倒。

验证：`think_split.rs` 追加 8 例（本会话原文样本、标记跨 chunk 拆在跨度内、双跨度形态、
围栏内标记、围栏自身反引号 run 跨 delta 拆分、代码之后的真标记仍生效、跨度不跨行、
4 反引号围栏内 3 反引号不算闭合）；
`chat_completions.rs` 追加 1 例线级回归 `quoted_marker_in_answer_stays_text`（断言零
Reasoning token + `assistant_text()` 逐字还原 + 无 reasoning item）。
`bash scripts-local/ctest.sh -p xai-grok-sampler --lib` 272 例全过（原 263 例语义零改动）。
反证：把 `in_code()` 临时改成恒 `false` 后，上述 5 例跨度/围栏用例立刻转红（20 passed /
5 failed），确认守卫是承重的。

重放注意：`think_split.rs` 上游不存在，无同步冲突面；本修复只落该文件 + `chat_completions.rs`
测试块。残留边界（有意不覆盖）：其它引用形态（`"<think>"`、`**<think>**`）仍按标记处理。
