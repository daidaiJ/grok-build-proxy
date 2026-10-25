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
- 上游核实（2026-09-23，对 upstream main `07e35a3d`，2026-09-22 同步）：上游**没有**配置文件层面的
  shell 选择——`shell.rs` 只有 `GROK_SHELL` 环境变量（Windows 四取值：pwsh/powershell/bash/cmd；
  Unix 当绝对路径用，且要求文件名匹配 bash/zsh）与 `$SHELL`；`26-config-reference.md` 无 `shell`
  表段，配置解析层（含远程 `/v1/settings`）也没有 shell 字段。上游 main 的 `shell.rs` 与本仓库快照
  除本补丁段外逐行一致，故不存在"上游已提供、可撤销本补丁"的路径，补丁继续保留。
- 文档落点：内置 `29-local-enhancements.md` Shell backend 段（取值表 / 优先级 / 生效时机 /
  Windows-Unix 分工 / 各 shell 行为差异）+ 本文件下方"用户侧配置示例" +
  README 的 "Windows 可用性" 小节与配置速查行。

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

环境变量等价物：`GROK_SHELL=bash`（config 覆盖优先于 env；两者都没有才走自动级联）。
仅 Windows 生效（Unix 忽略此表，看 `$SHELL` / `GROK_SHELL` 绝对路径）；取值在进程内首次使用时
固定，改完需重启 `grok2`。

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
  见 `archive/ci-cache-incident.md`。
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

> **2026-09-28 已由上游覆盖、补丁撤销**：上游 `a28ee2b`（同步快照 2026-09-17）以
> `SessionFlags::defer_builtin_agent_profile` + `views::agents_modal::config_agent_is_explicit()`
> 实现了同一语义（配置显式设置 `[agent]` 时不再合成 plan/ask-user `agentProfile`）。
> 合并 `sync/upstream-2026-09-23` 时采用上游实现，本节的 `respect_config_agent` 闸门、
> `config_default_agent_name()` 与配套测试一并移除。以下为撤销前的历史记录。

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
`cache-workspace-crates: "true"`（背景见 `docs-local/archive/ci-cache-incident.md`
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

对交接文档 `docs-local/archive/ci-failures-2026-09-20.md` 的对账：其"修复 1"假设
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

- signals：0ms TTFT（上游/代理整体吐包的缓冲伪影）不计入 avg/min/max/count、不置末次样本；
  `SessionSignals.last_time_to_first_token_ms` 暴露末次实测值
- status_line：perf 段 ttft 改用末次实测调用的 TTFT 配同调用 API 时长（tps 窗口自洽）
- 实证：unified.jsonl 1096 条 inference_done 中 82% 采样为 None/0
- （2026-09-26 注：当时"整体吐包"的解读对 Ark 场景不准确——0ms 的主因是
  **晚响应头**（首 token 就绪才发 headers），见下文 request-anchor 节与
  `docs-local/status-line-perf-wiki.md`）

## 压缩请求半答并行 tool call 回填（2026-09-23，feat/local-compaction-toolpair-repair）

> 全量证据链（会话工件、事件流、上游复现探针）见
> `docs-local/archive/compaction-dangling-toolcall-400.md`。

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
（诊断早期的快照；全量重扫为 14 处，多出的 8 处同属 `01a0d161` 后续回合——该会话客户端是
`1.0.37-preview.4` 且期间无重启，故都在预览.4 上产生。）

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

现场验证（`v1.0.37-preview.6` release 产物，端点 `cc-deepseek-v4.1-flash`）：让模型用行内代码
引用标记的回合，正文完整（562 字符，含 `## 四`/`## 五`），落盘扫描 0 命中；围栏代码块内标记
同样留在正文；裸 `<think>…</think>` 真标记仍整块改道 reasoning（未误伤）；TUI 轮（`--minimal`）
正文 398 字符完整、折叠思考块内只有规划。对照 `v1.0.37-preview.5` 产物同提示词复现旧签名
（命中 1、466 字符改道）。R2/R5 印证残留边界：`"<think>"` 与 `**<think>**` 仍折叠。七轮对照表
见 `.handoff/think-split-quoted-marker-fold.md` §13。

重放注意：`think_split.rs` 上游不存在，无同步冲突面；本修复只落该文件 + `chat_completions.rs`
测试块。残留边界（有意不覆盖）：其它引用形态（`"<think>"`、`**<think>**`）仍按标记处理。

## 状态行 model.display_name 按会话 catalog key 覆盖（2026-09-25，feat/local-statusline-catalog-model-name）

现象：配置里多个 `[model.*]` 条目共用同一个 wire model id（如 `[model.gpt-6-luna]` 与
`[model.sub2-gpt-6-luna]` 都发 `model = "gpt-6-luna"`，name 不同、供应商不同）时，切到
sub2 的条目，状态行仍显示 `gpt-6-luna (token-unlimited)`——与选择器、`/session-info`、
dashboard 徽标（全部走 client 侧 catalog key）显示的供应商对不上。

根因（shell 侧 `acp_session_impl/status_line.rs:118-133`）：状态行快照的
`display_name` 用 `models_manager.display_name(SamplerConfig.model)` 解析，即拿 **wire id
当 catalog key 查** `info.name`。多个条目共享 wire id 时查到的是恰好以该 id 为 key 的那个
条目（通常配置序靠前、先写的那家），与会话实际所 run 的条目无关。live 实测（4 条目复刻
配置 + ACP stdio driver）：`-m sub2-gpt-6-luna` → 快照显示 `gpt-6-luna (token-unlimited)`
（错）；`sub2-gpt-6-sol` → 裸 `gpt-6-sol`；`gpt-5.6-luna` → `gpt-5.6-sol (token-unlimited)`
（用户 config 把 luna 条目命名成 sol）。非回退：404 `model_not_found` 现场确认请求带
`sk-sub2`、`{"model":"gpt-6-luna"}` 直发，无 `model_auto_switched`，且错误的名字在任何
请求发出前（`total_api_duration_ms: 0`）就已存在于快照。

修复（pager 侧覆盖，一处入口同时管 builtin 行与脚本 JSON）：`status_line.rs` 新增
`ClientOwnedFields.model_display_name`；`status_line_policy.rs::shell_status_context()` 用
`agent.session.models.current_model_name()`（client 侧按 catalog key 解析的名字，无 name
时回退 key 本身）覆盖 `ctx.model.display_name`。`model.id` 语义不动（仍为 wire id，脚本
按它 key 的不受影响）；shell 查不到时快照本就回退 wire id，覆盖是幂等的。
有意不做 shell 侧修复：session actor 不持有 catalog key（`spawn_session_on_thread` 60+
个位置参数，同步风险高），且 adjacent bug（actor 把 wire id 持久化为
`summary.json.current_model_id`）另案。

测试：`app/dispatch/tests/status_line.rs` 追加 3 例——builtin 行显示会话实际条目的名字
（真实载荷夹具复现 sub2→token-unlimited）、脚本载荷同名且 `model.id` 保持 wire id、
key 不在 client catalog 时回退显示 key。文档 `25-status-line.md` 的
`model.id`/`display_name` 行补 LOCAL 说明。

## wake 测试 rejected_deferred_start 的 summary 字节级比较改为身份字段比较（2026-09-25，fix/local-wake-summary-settle）

现象：Linux CI build 的 linux job 偶发挂一例——`xai-grok-shell` 的
`agent::subagent::tests::wake::rejected_deferred_start_restores_prior_without_publication`
（`summary.json` 恢复后与 spawn 刚结束时逐字节不等，`num_messages` 3→4，两次写入相差
176 ms）。近期 main 历史首次失败；本机（Windows）复现为确定性必挂，且滞后可达秒级。

根因：被测产品路径的 durable append 是"假等待"——`emit_turn_completed`（Durable 档）里
`AppendUpdateDurablyAndAck` 的 `respond_to` 直接丢弃（`acp_session_impl/updates.rs`，
persistence actor 注释自证"A dropped receiver is a fire-and-forget durable append"）。
因此 spawn 返回后仍有多笔尽力而为的滞后写入会合法地改写子会话 `summary.json`：
① 回合自身的 `TurnCompleted` 终态（patch `num_messages`/`updated_at`）；
② 被拒 deferred start 的取消终态（`stop_reason:"cancelled"` + `cancelTrigger:"shutdown"`，
`MidTurnAbort`）——本机实测有时滞后 ~2 s，有时 10 s 内不落盘。测试在 spawn 返回后立刻
读快照、恢复后再逐字节比较，比较的是"滞后中的磁盘"而非不变量，平台时序决定成败。

修复（仅改测试，`agent/subagent/tests/wake.rs`，LOCAL）：放弃字节级比较，改为解析
`Summary` 后比较身份字段——`attempt_id`（不许重试）、`agent_id`、`created_at`、
`current_model_id`、`next_trace_turn`、`session_kind`。这些才是"拒绝的 wake 不得篡改
先前工件"真正要守的不变量；`updated_at`/`num_messages` 允许因滞后的合法追加而前进。
与上游同文件兄弟用例 `started_wake_with_failed_metadata_write...`（只比
attempt_id/next_trace_turn）的模式一致。曾尝试"锚定终态行 + 计数自洽"的 settle 轮询
（三版），均因取消终态本身时序不定而放弃——fire-and-forget 通道的静默无法从文件状态证明。

验证：本机修复前该用例 4/4 必挂（含确定性复现 CI 形态），修复后 4/4 过（连跑 3 轮稳定）。
Linux 侧由 build.yml 全量 shell 测试回归（CI 不受滞后影响，字段比较严格弱于原断言的
通过面，不会引入新失败）。

重放注意：只动这一个测试函数内的两处读法与断言块；上游若把 Durable 档改为真等待 ack，
字节比较即恢复可行，本补丁可撤销。

## 状态行 TTFT 参考点前移到请求发起（2026-09-26，feat/local-perf-ttft-request-anchor）

> 接续 `4e27104`（reasoning/工具 delta 补时间戳）与 `8e049b6`（0ms 过滤）：两者之后火山 Ark
> 会话的状态行 ttft/tps 仍整段缺席，且被误判为"model id 显示修复（7804d5e）引入的回归"。

现象与取证（详见 `docs-local/status-line-perf-wiki.md`）：同一 preview.6 二进制、同一天内，
Command Code `deepseek/deepseek-v4.1-flash` 会话 799/807 次 ttft>0，火山 Ark
`glm-5.3-flash` 会话 302/303 次 ttft=0（09-23 起 preview.4/6 皆然，早于嫌疑提交两天）。
日志时间线还原（`01a0ddae` loop 31）：inference_start 37.223 → waiting_model phase
4731ms → thinking 42.010 → inference_done 50.611（model_elapsed 13386）。用户感知首
token 延迟 4.7s，而 ttft_ms=0——根因是 **metrics 层参考点在响应头之后**（`stream_start`
取于 L2 流首次 poll，poll 前 `conversation_stream().await` 已等完响应头），而 Ark 是
"首 token 就绪才发响应头、headers 与首 SSE 同 flush"：头后段恒 <1ms → 0ms → 被 8e049b6
门槛过滤 → ttft 无样本 → tps（依赖同调用 ttft）一并隐藏。Command Code 先发响应头，
头后段即真实值，故正常。8e049b6 的"整体吐包"解读对 Ark 场景修正为"晚响应头"。

改动（全部同步文件，重放用）：

- `crates/codegen/xai-grok-sampler/src/metrics.rs`：`from_timestamps` 增首参
  `request_sent_at`——ttft = 首 chunk − 请求发起；ttlb/ITL 维持 stream_start 参考
  （投递窗口口径连续，ttft 可大于 ttlb，无不变量约束）。+1 语义测试
  `ttft_anchors_to_request_sent_at_not_stream_start`（锚点 400ms/首 token 500ms/end
  1200ms → ttft 500、ttlb 800，两参考点独立断言）。
- `src/stream/{chat_completions,messages,responses}.rs`：三流函数尾参
  `request_sent_at: Instant` 透传（`stream_responses` 经 `stream_responses_tracked`）。
- `src/actor/request_task.rs` `run_one_attempt` 顶部取锚（retry 循环内每次物理尝试一次，
  失败尝试样本随其丢弃）；`src/client.rs` `conversation_collect_with_idle_timeout`
  同款。锚点含请求序列化与 bearer 准备（1–5ms 量级，可忽略）；不含 sampling gate 等待
  （在 submit 之前，与 shell model_timer 同界排除）。
- 消费侧零改动：signals 0ms 过滤保留（仍挡真伪影）；状态行 ttft=末样本、
  tps=output/(last_api−ttft) 配对维持 8e049b6 约束；`/stats` 与 turn-delta 的 ttft
  自动同步转为请求锚定口径。

验证：`cargo check -p xai-grok-sampler` 干净；`ctest.sh -p xai-grok-sampler --lib`
278/278（原 277 +1）；`cargo check -p xai-grok-shell` 干净。

重放注意：三流函数签名变化（尾参），上游若重构流入口按参数语义对齐；span 层
`http.*.ttft` 有意不前移（保持头后段，配 `response_headers_ms` 可拼装全窗口），
两口径并存见 wiki §1。

## /usage 周额度估算（2026-09-27，`feat/local-usage-quota-estimate`）

调研与验收标准见 `docs-local/usage-quota-estimate-todo.md`；他机对账背景见
`docs-local/quota-pct-jump-audit.md`。T1/T2/T3 一次落地。

### xai-grok-tools（新增两模块，`lib.rs` 注册）
- `src/billing_samples.rs`：`BillingSample`（ts/pct/period_start/period_type/tier/
  history_len，serde camelCase）+ `append_sample`（同 pct 60s 去重；32 MiB 触发
  35d 保留窗剪裁；`GROK_BILLING_SAMPLES=0` 关闭；进程内 Mutex 与账本同款）+
  `load_samples`。落盘路径 `grok_home/cache/billing-samples.jsonl`。
- `src/quota_estimate.rs`：反推内核 `estimate(samples, rows, is_xai_direct)`——
  周期切片（只配对最后一条采样的 period_start）、max-over-pairs 下界
  （Q̂=Δtokens_xai/Δpct×100）、质量门（≥3 采样、≥2 个不同 pct、Δpct≥2 整点、
  分子>0）、Tight/Loose 按 Δpct≥10 分档；`NoData(InsufficientSamples|NoLocalUsage)`。
  `load_model_base_urls` 手工解析 config.toml `[model."id"]` 的 base_url
  （子表/[models] 拒绝；漏配方向 = 当第三方 = 低估，安全侧）+
  `is_xai_direct`（缺省或 *.x.ai 官方）+ `xai_direct_predicate`（账本里查不到的
  model id = 默认后端 = 直连）。

### xai-grok-shell（T1 采样点）
- `src/extensions/billing.rs` `handle_get_billing` 成功路径（unified log 之后）：
  `credit_usage_percent` 有值即落一条采样；失败路径不落。

### xai-grok-pager（T3 UI 接线）
- `src/app/actions.rs`：`Effect::FetchQuotaEstimate { agent_id: Option<AgentId> }`、
  `TaskResult::QuotaEstimateComputed { agent_id, estimate }`。
- `src/app/dispatch/billing.rs` `handle_billing_fetched`：仅模态打开的拉取
  （`nonce != 0`）追加 emit（turn 结束静默刷新不重算）；返回 `effects`。
- `src/app/dispatch/task_result.rs`：新臂落双镜像（app 级 + 发起 agent 级）；
  `AppBillingFetched` 臂同款 nonce 门控（仪表盘路径 agent_id=None）。
- `src/app/effects/mod.rs` + `helpers.rs`：`compute_quota_estimate`（spawn_blocking）
  读三个本地文件估算，无采样返回 `None`。
- `src/app/app_view.rs` / `src/app/agent_view/{mod,session}.rs`：`quota_estimate`
  镜像字段（与 `credit_balance` 同模式）。
- `src/views/usage_modal.rs`：`render_usage_modal`/`tab_content`/`usage_limit_lines`/
  `allowance_lines` 加参；额度条下渲染估算块（下界值 + 依据窗口 + 多设备提示；
  NoData 显示原因；无采样镜像静默跳过）。
- `src/views/dashboard/render.rs` `render_dashboard`：加参透传。
- `src/slash/i18n.rs`：8 条中英文案。

验证：`cargo check -p xai-grok-tools/-shell/-pager` 干净；
`ctest.sh -p xai-grok-tools --lib`（GATE_FORCE=1，该 crate 不在白名单；全量 3114 过/75 挂，
挂的全为既有 computer::local::terminal 环境族；本模块 billing_samples 2/2 +
quota_estimate 8/8 过）；`ctest.sh -p xai-grok-pager --lib` **9614 过/0 挂**。

顺手修（同分支，非本特性引入）：
- `views/witty_phrases.rs`：`phrase_always_resolves_within_list` 与
  `phrase_follows_current_lang` 加 `#[serial_test::serial(WITTY_LANG)]`——前者读全局
  lang、后者 set_lang，并行跑互踩（同一 suite 两次运行一挂一过，实锤 flaky）。
- `docs-local/win-skip.txt` 登记 `python_command_line_with_a_drive_path_runs`
  （status_line spawn 路径 × 门控 env 探针退出码 2；手动直跑同脚本 rc=0；main 同源
  复现，与 LOCAL 改动无关，环境族不修）。

重放注意：`render_usage_modal`/`render_dashboard` 签名变化（新插参），上游若重构
调用点按参数语义对齐；billing-samples.jsonl 为新缓存文件，不随会话走。

## OpenCode Go 网关 UA 标识（2026-09-27，`feat/local-opencode-go-ua-marking`）

要求背景与配置启用见 `docs-local/opencode-go-gateway.md`。OpenCode Go 官方网关强制
请求携带应用标识 UA 与会话 ID 头（部分适配端点豁免）。

- `xai-grok-sampler/src/client.rs`：`SamplingClient::new` 中默认 User-Agent 的写入
  从 `extra_headers` 应用**之后**移到**之前**——per-model `[model.<id>].extra_headers`
  里的 `User-Agent` 项现在可覆盖默认 `grok-shell/<版本>` UA（其余头语义不变，
  `x-grok-*` 标识头仍在 UA 之后写入、不受 extra_headers 影响）。
- 既有配套：`${session_id}` 模板展开（`sampler_turn.rs` `expand_session_header_templates`，
  上期已实现），覆盖 `x-opencode-session` 的"同会话稳定、跨会话不同"要求。
- 新增回归测试 `extra_headers_can_override_user_agent`。
- 文档：`docs-local/opencode-go-gateway.md`（配置示例 + 要求对照表）。

验证：`cargo check -p xai-grok-sampler` 干净；`ctest.sh -p xai-grok-sampler --lib`
279/279（原 278 +1）。

## 上游同步 2026-09-15…09-23 收尾补丁（2026-09-28，分支 `sync/upstream-2026-09-23`）

上游这 5 个快照把若干本地/上游共享结构收敛或搬迁，合并后为保住本机（Windows）
`cargo check --workspace --all-targets` 与 lib 测试的基线，追加以下 LOCAL 改动：

- `xai-grok-shell/src/session/persistence.rs`：上游把 `Summary.agent_name`（原来是 `pub`）
  收敛成 `pub(crate) agent: PersistedAgentSelection`，导致 `benches/session_list.rs`
  既无法命名类型、也无法构造字段。把 `PersistedAgentSelection` 与 `Summary.agent`
  可见性放开到 `pub`（`selected` 字段同样放开），并在 bench 里改用
  `PersistedAgentSelection { selected: Some(PersistedAgent::Named(..)) }`。
  纯可见性放宽，不改语义；上游下次同步若再收敛可见性需重放。
- `xai-grok-shell/src/session/helpers/session_compact_retain_session_asset_files_tests.rs`：
  `keeps_only_regular_files_inside_the_assets_dir` 用 `std::os::unix::fs::symlink`
  造软链（含软链目录逃逸用例），Windows 无此 API → `#[cfg(unix)]` 门控（同时登记
  `win-skip.txt`）。
- Windows 侧 lint 归零（上游 CI 是 Linux，这些告警只在 Windows 出现）：
  `xai-grok-shared/src/clipboard.rs`（`#[cfg_attr(windows, allow(unused_assignments))]`）、
  `xai-grok-agent/src/prompt/paths.rs`（unused import）、
  `xai-grok-agent/src/prompt/user_message.rs`（`jp` dead_code）、
  `xai-grok-config/src/fs_atomic.rs` 与 `fs_atomic_tests.rs`（`same_file_identity` /
  `assert_follow_refused` dead_code）。均为属性级允许，不改行为。
- `Cargo.lock` 本机偏离上游一处：`tree-sitter 0.26.13` 要求 `cc ^1.2.48`，上游锁的
  `cc 1.2.48` 在 rustc 1.94 上编译失败（`tempfile.rs` 里
  `find_msvc_tools::windows_sys::FILE_ATTRIBUTE_TEMPORARY` 类型不匹配）；
  本机解析到 `cc 1.5.1` + `find-msvc-tools 0.1.14` 通过。

## 上游同步 i18n 丢失站点补回（2026-10-02，分支 `feat/local-sync-i18n-restore`）

按 `docs-local/sync-2026-09-23-i18n-lost.txt` 的"搬迁"类条目，在 v1.0.37 同步后
的新落点重放 `tr()`（中文表键均已存在，零新增键）：

- `crates/codegen/xai-grok-pager/src/views/dashboard/row_title.rs`：`New session`
  回退标题（宽模式头部 + 窄模式新增同款检测）、`failed` 徽标（`FAILED_LABEL` 常量
  拆成 `FAILED_SEP` + `tr("failed")`，宽度计算同步改动态）、`bg` 回退 chip。
- `crates/codegen/xai-grok-pager/src/views/dashboard/row_activity.rs`：进行中命令
  `cmd.display_name()` 包 `tr()`。
- `crates/codegen/xai-grok-pager/src/views/scheduled_next.rs`：`(due now)` /
  `(next in {})`。
- `crates/codegen/xai-grok-pager/src/views/tasks_pane.rs`：`(running)` / `(starting)`。
- `crates/codegen/xai-grok-pager/src/views/workflows.rs`：`activity_label()` 的
  `{n} agent` / `{n} agents` / `running`，动态状态串走 `tr_str`。

删除类条目核实：站点已随上游移除（`Subagent not loaded`、`[Dashboard]` overlay、
memory_modal 相对时间、dock tab_hint、location `format_cwd_parts`、
`{tools} tools · {toks} tok · {turns} turns`、subagent worktree 后缀、
`… {} more` 溢出行、subagent_catalog_pane 整文件），无需补回；memory `t toggle`
上游已自行改键且调用点带 `tr()`。

验证：`ctest.sh -p xai-grok-pager --lib` 五过滤（dashboard::row_title 16 /
row_activity 8 / scheduled_next 7 / tasks_pane 59 / workflows 62）全绿，152 过 0 挂。

重放注意：全部为调用点包 `tr()` 级改动，上游若再重构这些渲染函数，按
`sync-2026-09-23-i18n-lost.txt`（已标注处置结果）逐条核对。

## serial 锁组竞态修复——consent 测试回归默认组（2026-10-02，分支 `fix/local-grok-home-serial-lock`）

现象：CI run 36960784460 偶发挂一例——`xai-grok-shell` 的
`util/config/consent_tests.rs::set_consent_answer_is_monotonic_per_account`
（`load_from_disk must see the consent persist wrote`，left `{}` right `{"tos": ...}`）。

根因：serial_test 的**命名组与默认组互不排斥**——`#[serial(GROK_HOME)]` 只与本组内
测试互斥，不挡默认组（`#[serial]` 无参）测试。该 consent 测试虽用命名组守着 GROK_HOME，
却与默认组约 69 个经 `EnvGuard::set("GROK_HOME", ...)` 翻转变量的测试并发执行，对方切到
自己的空 tempdir 时 `load_from_disk` 读到空目录 → 断言左空右有。crate 既定约定是
env-var setter 一律住默认组（同 env 变量的守卫与 setter 必须同组才互斥）。

改动（两处均为同步文件）：

- `crates/codegen/xai-grok-shell/src/util/config/consent_tests.rs`：该测试属性
  `#[serial_test::serial(GROK_HOME)]` 改回 `#[serial_test::serial]`（默认组），
  并加 3 行注释说明"env guard 必须默认组"的不变量与本次 CI 事故出处。
- `crates/codegen/xai-grok-test-support/src/env.rs`：`EnvGuard` 文档注释补 4 行，
  点名调用方必须位于**默认** serial 组（命名组不排斥默认组，命名键 guard 仍会与
  同变量的默认组 setter 竞态）。

重放注意：上游同步冲掉后，若该测试属性又被改回 `serial(GROK_HOME)` 命名组，按本条重放。

已知后续项（不在本补丁）：`xai-grok-pager` crate 内 `serial(GROK_HOME)` 命名组用法
约 40 处，若其与同 crate 默认组 GROK_HOME setter 混用则存在同类碎片化，需单独审计。

## 十六期补丁（2026-10-02：TUI 英文残留 i18n 计划外扫尾，T3）

> **计划外扫尾**：stats-modal-todo.md 的 T3（全仓 i18n 扫尾），不在 P0–P4 计划内，
> 是 P0–P4 完工后的漏网复核。分支 `feat/local-stats-modal-i18n`（与 T1/T2 同分支）。

### 方法（先扫描后动手，零编译静态扫描）

- 新工具 `scripts-local/i18n_sweep.py`：注释剥离 + 字符串掩码做括号/宏结构扫描，
  按宏上下文预分类（plain/fmt/cmp/log/id/test/wrapped），输出按文件分组清单。
  陷阱留档：对字符串做空白化后再数括号会被 `"}"`、`\u{2026}` 等字面量破坏配对
  （permission_view.rs 的 `#[cfg(test)]` 区间因此漏判）——结构扫描必须用掩码跳过字面量内部。
- 复扫产物：`docs-local/i18n-sweep-classified-2026-10-02.txt`（施工前分类清单）、
  `docs-local/i18n-sweep-rescan-2026-10-02.txt`（施工后复核）。
- 施工流程沿用 ui-i18n-plan.md 规约 8：分批派遣子代理（每批 ≤3~4 视图文件）只改视图文件
  并回报 (en, zh)，主会话单写者统一合并进 `i18n.rs` → 查重 → 一轮编译。

### 施工面与结果

- **翻译表 +157 组**（1417 → 1574 组），覆盖 22 个文件、约 180 处渲染点接线。
- 主要补线区域：
  - **modal 页脚 shortcut rest 键**：Shortcut 标签由 `modal_window.rs:638
    shortcut_label_i18n()` 集中翻译（首空格切分、键帽保留英文、rest 查表）——
    本次补齐 rest 键缺口（switch/scroll/all models/copy session ID/copy all/
    edit field/$EDITOR/scope/prev/next agent/dashboard/fullscreen/pause/play/
    close this session/new in worktree/Release Notes 等）。
  - **turn_status**：活动名词（command/monitor/loop/subagent/workflow/task）经
    渲染出口 tr_str 查表；中文命中（≠原文键）即跳过复数 "s"（中文无复数）。
    状态词族（Cancelling…/Verifying…/Compacting…/Running…/Waiting…/Starting session…）
    与排队提示整键。
  - **rewind 对话确认**、**session_picker 展开卡片字段标签**（第二渲染路径）、
    **welcome 窗口提示/占位符**、**dashboard chrome 按钮/徽章**、
    **extensions 分组头**（出口 tr_group_label 已接线、键缺失）、
    **plan_approval formatter 固定段**、**privacy_banner 按钮+LEGAL 分段**
    （热区宽度改按译文 `UnicodeWidthStr::width`，与 LEGAL 分段同一约束），
    **prompt_widget 粘贴/图片提示**（具名占位符 `{n}`/`{size}` 沿用 turn_status 先例）、
    **announcements 隐藏按钮**（paint 出口加参数统一译文与预留宽度）、
    **app 层 chrome**（status_blocks 用量块/任务队列、agent_view 状态徽章与媒体按钮、
    app_view 浮层标题兜底）。
- **关键机制发现**：大量扫描 plain 命中实为「存英文键 + 渲染出口 tr/tr_str」的
  规约 1 正确模式（agent.rs 页脚 HintItem→shortcuts_bar.rs:291、dashboard 状态词→
  render.rs 出口、goal_detail 状态族、session_info_fields 单源双消费等），本次逐一
  核实出口后豁免，未双重包裹。

### 书面豁免台账（按口径不译，复扫残留的主体）

| 类别 | 代表 | 依据 |
|---|---|---|
| 一次性命令结果/错误 | toast、`No active session`、`couldn't …`、effects 层 ~70 条 | ui-i18n-plan 口径 |
| 协议/内部键 | `x.ai/*` 方法名、json! 键、serde rename、`allow-always-command`、`completed` 比较、TOML 键 | 翻译破坏匹配 |
| 协议文本匹配器 | `sanitize_user_error` 改写表、磁盘满标记 | 匹配入站英文错误 |
| 键帽/符号 | `Esc`/`Ctrl+N`/`↑/↓`、`●`、`[worktree:on]`（`.len()` 计宽 ASCII 假设） | 口径 + 布局约束 |
| 调试 HUD | fps_hud、scroll_debug_hud | 口径 |
| 技术缩写/极窄槽位 | `MAX %`（5 列固定槽）、`ttft`/`ctx`（状态行紧凑段）、`ZDR`、`camelCase` | 宽度/术语 |
| 双词表设计 | witty_phrases（EN/ZH 各自成表，非查表翻译） | W-Z 专题设计 |
| 品牌/命令/URL | `Grok Build`、`/resume`、`grok.com`、env 名 | 口径 |

### 已知风格残留（记录不阻塞）

- `Verifying`/`Compacting`（无省略号，历史键）与新键 `Verifying…`/`Compacting…`
  并存，中文分别为「验证中」与「正在验证…」——下次触碰对应文件时可统一。
- status_blocks 的 `{status:<9}` 列按字符数填充，中文显示更宽有轻微漂移（纯 scrollback 文本，可接受）。

### 验证

- `cargo check -p xai-grok-pager --all-targets`：exit 0。**教训**：前两轮只跑
  `cargo check`（不编译 `#[cfg(test)]` 区），agent 在测试区的一次宽度改动
  （render.rs `bw` 计算混 usize/u16）漏检，被 ctest 的测试二进制编译拦下——
  凡改动可能波及测试区（rustfmt 递归、测试模块内接线），验证一律加 `--all-targets`。
- 复扫：plain 2578 → 2460、wrapped 657 → 829（+172 处包裹；其中 ~40 条 fmt 模板
  迁入 wrapped）；剩余 plain 均落在上表豁免类。
- 定点 rustfmt 21+1 文件（`workflows_picker_rows.rs` 顺带 4 行格式修复）。
- `ctest.sh -p xai-grok-pager --lib slash::`：**405 passed / 0 failed**（含翻译表
  纯数据测试 `translations_table_covers_known_keys_without_duplicates` 的键唯一性断言）。

### 后记：rebase 到 2026-10-02 main（集成验证）

分支基点是 09-20 的 main，与发版线差 81 个提交（含 v1.0.37 上游同步、usage-quota、
perf-ttft 等大批合并）。合入改为 rebase 重放，冲突裁定原则 = **以 main 演进版为基准、
扫尾改动作薄层重放**：

- `stats_modal.rs`：main 的 perf-ttft 已把性能指标拆成 p50/p90 四行（组合键
  "ttft p50/p90"/"tps p50/p90" 失效）——拆分键 "ttft p50"/"ttft p90"/"tps p50"/
  "tps p90" 取代（首字延迟/生成速度 p50、p90），复制行改由标签键组合。
- `app_view.rs`：main 侧结构性重写，整取 main 后按复扫补回 6 处接线
  （PromptFlag plan/auto、(session) 兜底、两个 toast、Session closed 模板拆分）；
  其余 plain 命中核实为存键+集中出口（shortcuts_bar/modal title/picker）。
- `agent_view/render.rs`：4 个结构冲突取 main（上游 spinner 边界检查重写），
  Loading... 接线重放；其余提示词词族核实键全在表（存键+出口译）。
- `extensions_modal.rs`：main 的 `hooks.get(hi)` 边界检查与本批 tr_hook_row_label 合并。
- `turn_status.rs` / `session_picker.rs` / `i18n.rs`：格式级冲突，取 main 结构 + 本批
  tr/宽度修正重放；i18n.rs 保留 main 的 usage-quota 键块（并集）。

复扫（rebase 后）：plain 2506 / wrapped 821。较 rebase 前 +46 条，全部来自 main
81 个提交的新代码（dashboard/row_title、slash/commands/memory_ops、
agent_view/subagent_takeover 等新文件），**非本批回归**——main 自身的未接线存量，
留待下批扫尾。

集成后全量验证补记：`ctest.sh -p xai-grok-pager --lib` 首轮 10030/2 failed——
status_blocks 用量块的两个快照失配，根因是整串格式里的**对齐填充在成键时被丢**
（"  Input tokens:   {} ({} cached)" 的三格列对齐），W5-N 拆成了 label+值两段。
修复 = 恢复 main 原始整串为键（`{}`→具名占位符在 replace 后不可见，测试构建输出
字节级还原）+ 中文译文按 18 列标签宽补空格。终轮 **10032 passed / 0 failed**。
教训：带列对齐的格式串必须整键保留填充（规约 5 的变体）；快照测试是这类回归的兜底。

## Responses 流事件方言归一化（2026-10-02，分支 `fix/local-responses-event-dialect`）

> 背景：[`upstream-responses-event-compat.md`](upstream-responses-event-compat.md) 修复选项 C。
> Command Code 网关对 reasoning 增量发 `response.reasoning.delta`（规范名
> `response.reasoning_text.delta`），撞 async-openai `ResponseStreamEvent` 严格枚举 →
> 解析中止、带推理请求整轮失败且不重试（`request_task.rs:808` 刻意不重试）。

### 改动面（本仓库内，不动 async-openai fork）

- `xai-grok-sampler/src/client.rs`：新增 `RESPONSE_EVENT_DIALECT_ALIASES` 方言表 +
  `rewrite_dialect_event_name()`；接进 `deserialize_response_event()` 现有 sanitize
  重试分支（先归一化事件名 → 再走原有未知工具剥离 → `from_value` 严格反序列化）。
- 设计约束（遵守 compat 文档「只做事件名归一化」）：① 方言表是封闭常量，未知事件名
  仍走原 `Serialization(first_err)` 失败路径（测试锁定）；② 载荷字段不放宽，归一化后
  反序列化失败照样报错；③ 归一化命中打 `tracing::warn!`（含原始事件名），新方言可从
  日志发现后进表。
- 单测 2 条：`deserialize_response_event_normalizes_gateway_reasoning_delta_dialect`
  （Command Code 抓包原载荷）、`deserialize_response_event_unknown_event_name_still_fails`。

### 验证与遗留

- `ctest.sh -p xai-grok-sampler --lib`：291 passed / 0 failed。
- 遗留（= compat 文档「未验证项」顺延）：活体验证需把某个受影响模型切回
  `api_backend = "responses"` 实跑一轮带推理请求（本机当前已统一 chat_completions，
  未切回）；Command Code 是否还发其他非标事件名未穷举，命中 warn 日志可发现。

## rewind 分支树 undo（2026-10-03，分支 `feat/local-rewind-branch-undo`，进行中）

> 背景：[`kimicode-port/rewind-branch-undo.md`](kimicode-port/rewind-branch-undo.md)
> 设计原型 + [`port-roadmap.md`](port-roadmap.md) 任务①。kimi `/rewind` 升级为
> append-only 分支树 undo（SwitchEdge 化），T1/T2a/T3 本机完成，T2b/活回合验证待续。

### 改动面（多属上游同步线，上游同步后必须重放本节）

- **T1 持久语义**（storage 层核心）：
  - `extensions/notification.rs`：`RewindMarker` 增 `to_branch: Option<u64>`
    （serde default + skip_serializing_if，旧格式可读；None=向前 rewind，
    Some(b)=切回弃分支 b 的 redo）。
  - `session/storage/mod.rs`：`filter_rewind_by` 泛化为 `fold_branch_timeline`
    （标记序列折叠成分支树，branch id = marker 序数；`BranchPointer`
    Final/FinalCut/At 三态）。坐标系定案：fork target 记录在**写入时活跃时间线**
    的连续编号上，链上切点从叶子向根递归传播（keep_own = k − t_b，
    k ← min(k, t_b)）——首版按父分支自身编号切被 `filter_rewind_double_rewind`
    与 `test_multiple_rewind_markers` 两用例抓出回归后修正。
    `assign_branches` 构建阶段与 fold 分离共享。**所有消费面
    （replay/raw-filter/typed-filter/fork copy/tail 分页/搜索索引/prompt 提取）
    走同一 fold，旧格式输出逐条兼容（既有测试全过为门）。**
  - `PromptExtractEvent::RewindTo` 改结构变体（target + to_branch）；
    `collect_prompts_from_events` 缓冲后走同一 fold。
  - `session/helpers/replay.rs`：`updates_have_backward_markers` 预扫 → 命中
    backward 标记走两阶段路径（否则保持原流式，零内存增量）；`finish_replay`
    抽取共享尾部；新增 `replay_to_prompt_pointer`（At 指针）。
- **T2a redo 通道**：
  - storage：`RewindFace`/`collect_rewind_face`（分支树 + 各分支 prompt）、
    `timeline_prompts_at`（redo 落点时间线）、`active_chain_slices`（时间线点→
    分支坐标）、`collect_branch_checkpoints`（边界预计算数据）。
  - `acp_types.rs`：`RewindRequest.to_branch`（serde default）；
    `RewindPointInfo.branch/boundary`；`RewindPointsResponse.abandoned_branches`
    （+ `RewindBranchInfo`）。
  - `acp_session_impl/rewind.rs`：`handle_rewind_to_branch`——face 校验 →
    先追加 redo marker → 分支感知 replay 重建（preamble splice/清理与跨
    compaction 路径一致）→ 快照 `prompt_texts` 从 journal 面重推导（§7-3）。
    **redo 强制 conversation-only（文件半边不动，design §6-3）**。
    `get_rewind_points` 加分支坐标映射 + 缺 checkpoint 文件的边界标注 +
    弃分支列表（0..=tip）。
  - `extensions/rewind.rs`：`x.ai/rewind/execute` 透传 `to_branch`
    （wire 名 `toBranch`/`to_branch` 双收）。
- **T3 pager**（views/rewind.rs、agent_view/rewind.rs、actions.rs、router.rs、
  dispatch/rewind.rs、task_result.rs、effects/mod.rs）：
  - `RewindTarget { prompt_index, to_branch }` 贯穿 PickerSelect/Confirm/
    ConfirmNeverAsk/Effect::RewindExecute/`rewind_execute_params`。
  - picker 渲染：弃分支行 `↩` 前缀、boundary 点 `⚠` 前缀 + 灰化；
    弃分支点合入扁平列表（active 降序在后）。
  - i18n 债：boundary 文案由 shell 下发英文（未进 tr() 体系），记欠账。

### 验证与遗留

- 本机：`cargo check -p xai-grok-shell/-p xai-grok-pager --all-targets` 全绿；
  `ctest.sh -p xai-grok-pager --lib rewind` 63/0；shell 侧 rewind 过滤 58 过
  （25 挂全为 `support.rs:287` AbsPathBuf("/tmp") Windows 环境族——该 crate 不在
  白名单的既有原因，非本改引入）。
- 遗留：① T2b（向前 rewind 统一走 replay 重建）未做，需 Linux CI/专用会话；
  ② redo 端到端 + pty 回归（`rewind_after_compaction_with_missing_checkpoint`）
  待活回合/CI；③ 活回合 TUI 抽查（redo → 编辑重发 → regeneration 遥测）；
  ④ boundary 文案 i18n。

## 十八期：headless non-interactive-denial continue（2026-10-03，分支 feat/local-shell-command-analysis，提交 6b1bc29）

Step-Code P1 勘误后的唯一真实缺口（fork 已有 tree-sitter-bash 命令内容静态分析全套，
见 step-code-port survey 勘误节）：无人值守（headless 非交互）会话遇权限提示时，
客户端无审批通道 → 提示一律 Cancelled → 回合终止。新增 opt-in continue 模式：
把可恢复的审批阻塞转成 PolicyDeny（失败工具结果），agent 看到失败原因后换路续跑。
对齐 Step-Code `--non-interactive-denial continue` 语义；执行环境故障不经权限决策，
两种模式都照常终止。

### 改动面（上游同步后必须重放）

1. `xai-grok-shell/src/session/acp_types.rs`：`StartupHints.non_interactive_denial`
   （serde `nonInteractiveDenial`，default None；未知值不破坏解析、回落 stop）+
   `continue_on_unattended_denial()`。
2. `xai-grok-shell/src/session/acp_session_impl/tool_calls.rs`：新纯函数
   `resolve_unattended_denial(decision, continue_mode)`（Cancelled+continue →
   `Decision::PolicyDeny("no approval channel available (unattended \`continue\` denial mode)")`），
   在 permission resolution 返回后调用；PolicyDeny 既有管线（telemetry / PermissionDenied
   hook / handle_tool_not_executed / ToolLoop::Continue）全复用，无新增分支。
3. `xai-grok-shell/src/session/handle.rs`：`SessionHandle.continue_on_unattended_denial`
   （spawn.rs 从 hints 读取；`spawn.rs:1404`）。
4. subagent 继承链：`SubagentSpawnContext.parent_continue_on_denial`（mod.rs）←
   `subagent_spawn.rs` 读 parent handle → `handle_request.rs` 写子会话
   `StartupHints.non_interactive_denial = Some("continue")`。
5. `xai-grok-pager/src/headless.rs`：`HeadlessOptions.non_interactive_denial` +
   `build_headless_init_request(…, continue_on_denial)` 注入 initialize
   `startupHints.nonInteractiveDenial`。
6. `xai-grok-pager/src/app/cli.rs`：`--non-interactive-denial <continue|stop>`
   （hide，PossibleValuesParser）；`xai-grok-pager-bin/main.rs` 透传。

### 测试与接线

- 新测试：tool_calls `unattended_denial_tests` 3 条、acp_types
  `non_interactive_denial_tests` 3 条（continue 解析 / 缺省+stop / 未知值回落）、
  headless `headless_init_request_carries_continue_denial_hint_only_when_enabled`。
- 夹具补字段：`isolated_spawn_e2e.rs`、`lsp_runtime.rs` ctx_with_toggle、
  `subagent/tests/mod.rs` SessionHandle 字面量、`mvp_agent/tests.rs` 同。
- 验证：`cargo check -p xai-grok-shell -p xai-grok-pager -p xai-grok-pager-bin` 0 error；
  shell 3+3、pager 1 全绿（GATE_FORCE=1 窄过滤）。

### 遗留

- fork 路径（`--fork-session` / worktree 复用）的子会话经 ext `x.ai/session/fork`
  创建，不走 initialize startupHints，continue 语义不覆盖（登记为已知边界）。
- 活回合验证：headless 非交互跑一个会触发审批的命令，对比 stop/continue 两档行为。
## 十九期：workflow canonical context edit（2026-10-03，分支 feat/local-workflow-context-edit，提交 904dc9a）

pi ContextEditEntry 同思想移植（workflow-pi-port P1）：journal 历史只增不改，可见面
= 原始结果 + 按 seq 应用编辑序列。解决迭代式 workflow 的 prompt 线性膨胀：脚本可把
上一轮子代理结果替换为摘要占位符，token 不再随轮数线性涨。

### 改动面（上游同步后必须重放）

1. `xai-workflow/src/host.rs`：`ContextEdit`（target/action/digest/needle）+
   `placeholder()` + `apply_visible_edits()`；`AgentOpts.context_edits`
   （`skip_serializing_if = "Vec::is_empty"`——**兼容关键**：空时不序列化，旧 journal
   spawn hash 不漂移）。
2. `xai-workflow/src/journal.rs`：`spawn_output_text(seq)`（目标输出文本访问器）+
   `resolve_context_edits(upto)`（按 seq 顺序解析，first-edit-wins，结构化输出
   不可替换 → 丢弃该编辑；纯 journal 派生 → 重放逐字节一致）。
3. `xai-workflow/src/engine.rs`：
   - 新 Rhai 函数 `replace_visible(seq, digest)` / `hide_visible(seq)`，
     `context_edit_call` journal 直记（deviation：设计原型说走 host_call 通道，
     施工改为 journal 直记——seq 分配/hash/重放/divergence 语义等价，无需 host 回路；
     seq 无条件消耗，重放命中也不能省，否则后续 spawn 会踩进编辑的 seq 位）；
   - `spawn_agent_call`：payload 哈希前强制 `opts.context_edits = journal.resolve(upto)`
     （覆盖脚本手填）；返回值注入 `r.seq`（引擎侧注入，journal 记录不含）；
   - `parallel()`：batch 前统一 resolve 注入；`PendingAgent::Replayed` 携带 seq，
     resolved 三元组，结果统一注入 seq。
4. `xai-grok-shell/src/session/workflow/host_service.rs`：spawn prompt（含 contract
   包装后）经 `apply_visible_edits` 替换 needle → 占位符；needle 不出现为 no-op。

### 测试

- 引擎 4 用例：替换进 later spawn payload（含 needle/digest 断言）、hide 占位符、
  编辑序列变化 → divergence、同脚本重放不重跑且结果一致。
- journal resolve 单测（first-wins + 结构化丢弃 + 边界）+ apply_visible_edits 单测。
- `ctest -p xai-workflow --lib` 66/0；shell `workflow::host` 过滤 4/0。

### 遗留

- 结构化输出（非字符串）暂不可替换（needle 无法预测脚本嵌入形态），文档登记。
- 占位符呈现格式为设计 §5-1 的首版定案（纯文本摘要行），等真实 workflow 场景校准。
- 活回合验证：跑一个 3+ 轮评审 workflow 观察/token 曲线（验收 1 的曲线留档）。

## 二十期：workflow 子代理工具面 allowlist（deferred exposure T1）（2026-10-03，分支 feat/local-deferred-tool-exposure）

deferred-tool-exposure 设计（workflow-pi-port P2）第一切片：`AgentOpts.tools` 显式
工具面白名单。fan-out 场景每个子代理默认全量工具面（内置 + MCP），工具面 token
成本 = agent 数 × 工具面大小；白名单让 workflow 作者按子代理角色裁剪声明面。
**T2（defer_tools + search_tools 延迟发现）未做**，见遗留。

### 改动面（上游同步后必须重放）

1. `xai-grok-tools/.../task/types.rs`：`SubagentRuntimeOverrides.allowed_tools:
   Option<Vec<String>>`（canonical tool id，可裸名 `read_file` 或全限定
   `GrokBuild:read_file`，大小写不敏感）。
2. `xai-grok-subagent-resolution/src/types.rs` + `overrides.rs`：
   `EffectiveRuntimeConfig.allowed_tools` 透传（v1 runtime-only，role/persona
   不可设）。
3. `xai-grok-subagent-resolution/src/definition.rs`：`apply_child_tool_policy`
   新参 `allowed_tools: Option<&[String]>`——在 capability 过滤与 workflow 工具
   剥离**之后** retain，故只能裁不能加（read-only 档下白名单也无法复活写工具）；
   裁剪后重跑 `prune_orphaned_background_task_tools`。
4. `xai-workflow/src/host.rs`：`AgentOpts.tools`（`skip_serializing_if =
   "Option::is_none"`——旧 journal 重放 hash 零漂移；进 payload → 白名单变化
   自然 divergence）。
5. `xai-grok-shell/src/session/workflow/host_service.rs`：spawn 处
   `allowed_tools: opts.tools.clone()` 下发到 SubagentRuntimeOverrides。
6. `xai-grok-shell/src/agent/subagent/handle_request.rs`：`run_shell_child` 调
   `apply_child_tool_policy` 补传第 4 参 `effective_runtime.allowed_tools.as_deref()`
   （shell 子代理 spawn 链路接线；T1 验证时留在工作区未提交，合 main 发版前补线
   `607fb6e`）。
7. `xai-grok-shell/src/session/agent_rebuild_tests.rs`：同函数测试调用点补 `None`。

### 测试

- resolution 2 用例：白名单裁剪面（含裸名匹配断言）、白名单无法放大
  capability 过滤（read-only + bash 白名单 → 仍无 bash）。全 suite 95/0。
- 引擎 1 用例：tools 进 opts payload + 改动 → divergence。xai-workflow 67/0。

### 遗留（T2：deferred 发现层，另切片）

- `defer_tools: bool` + `search_tools(query)` 工具（对 deferred 工具名+description
  简单打分，§5-1 允许首版降级 BM25）；命中者声明给下一轮；声明记录在子 agent
  会话内（不碰 workflow 重放语义）。
- **国模能力门（路线图要求）**：动态声明工具依赖模型支持轮间工具面变化
  （Anthropic `dynamically_loaded_tools` 类能力位）；GLM/DeepSeek/Qwen 等国模
  对中途改声明面的支持未核实，T2 施工前必须先加能力门（不支持者 defer_tools
  拒绝或降级为纯 allowlist），未验证能力门不准默认启用。
- resume 原子性（设计 §2.1-3）：声明态与工具面同批重建，T2 验收用例必须覆盖。

## 二十一期：不完整流恢复注入（stream recovery note）（2026-10-04，分支 feat/local-stream-recovery-injection）

随手件-P3（step-code-port 调研，Step-Code `step-stream-recovery.ts` 的 fork 移植）：
回合以不完整流错误收尾时，失败响应被丢弃，模型下一轮不知道自己的工作"消失"了，
容易整文件重发。本补丁复用 fork 既有的一次性 interrupt envelope 管道，在下一轮
真实用户 prompt 前置一条恢复提示（轻量、按次消费）。

### 改动面（上游同步后必须重放）

1. `xai-grok-session-events/src/tracker.rs`：`EventTracker` 新增一次性标志
   `pending_stream_recovery: Cell<bool>` + `set_pending_stream_recovery` /
   `take_pending_stream_recovery`（语义同 `pending_interrupt_reminder`：
   `begin_turn` 不清除，活到下一次真实用户 prompt）。
2. `xai-grok-shell/src/sampling/error.rs`：
   - 新增 `is_incomplete_stream_error(&acp::Error)`——kind 命中
     `Http | IdleTimeout | EmptyResponse`（`EventStreamError` 在采样器侧折入
     `Http`；`StreamError` 折入 `Api`，wire 层不可达，恢复提示降级为缺席）。
   - `terminal_error_data` 盖章集合扩为 `MaxTokensTruncation | IdleTimeout |
     Http | EmptyResponse`（原先只盖前两者；新 kind 对旧读者经 parse fallback
     优雅降级为无类型）。
3. `xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs`（经由
   `handle_sampling_failure` 终点漏斗生效，无需改动）。
4. `xai-grok-shell/src/session/acp_session_impl/turn.rs`：采样失败出口
   （`Err(error)` 前一步）按 `is_incomplete_stream_error` 置位；
   `PromptOrigin::User` 分支在 interrupt envelope 之外前置
   `maybe_apply_stream_recovery_note`（verbatim prompt 消费标志但保持原文）。
5. `xai-grok-shell/src/session/acp_session_impl/reminders.rs`：
   `STREAM_RECOVERY_NOTE` 常量 + `maybe_apply_stream_recovery_note`。

### 验证

- `ctest.sh -p xai-grok-session-events --lib`（GATE_FORCE，本地自有 crate）
  16/16；`ctest.sh -p xai-grok-shell --lib sampling::error`（GATE_FORCE 过滤子集）
  29/29（新增 `incomplete_stream_error_classifies_by_kind`）。
- 活体行为（真流中断 → 下一轮提示出现）随客户端 1.0.41+ 活回合抽查，未验证。

### 设计取舍

- 与 Step 的差异：Step 从会话历史读失败 entry（持久化失败态）；fork 失败响应
  完全不落盘，故用内存一次性标志（进程重启即失，可接受——重连后的首轮本就无
  陈旧恢复态可继承）。
- 投影纯度：提示文本只折叠进下一轮用户消息，不注入合成历史条目，重放/rewind
  不继承。
## 二十二期：rewind T2b——replay 重建统一双路径（2026-10-04，分支 feat/local-rewind-branch-undo）

rewind-branch-undo 设计 §4-L2 遗留件：`handle_rewind` 原为双路径（无 compaction 时
内存截断全保真 / 跨 compaction 时 replay 文本重建），本补丁统一为**journal replay
单路径**，并把 replay 状态机补到全保真，使重建结果与 resume 重建
（`chat_history.jsonl` = ChatReducer 输出）对齐。

### 改动面（上游同步后必须重放）

1. `xai-grok-shell/src/session/helpers/replay.rs`（ReplayState 全保真化）：
   - 新增 `pending_tool_calls` / `tool_args` / `emitted_tool_results`，处理
     `ToolCall` / `ToolCallUpdate`：助手 item 携带本步工具调用（合并 text +
     calls），完成更新产出 `ToolResult` item（先 flush 助手）——与
     ChatReducer 同构；原文本-only 折叠两者皆丢。
   - `current_user_text: String` → `current_user_parts: Vec<ContentPart>`，
     Image 块进 parts（相邻 text 块合并，保持与 live drain 同形）；interjection
     仍取纯文本。
   - rewind marker / checkpoint 前置路径同步清理工具缓冲（`clear_partial_tools`）。
   - 新增测试 `replay_keeps_tool_calls_and_results_full_fidelity` /
     `replay_rewind_marker_discards_partial_tool_step`。
2. `xai-grok-shell/src/session/storage/mod.rs`：`chat_rebuild::extract_tool_result_text`
   提为 `pub(crate)`（replay 共享同一提取器，两条重建线产出同形 tool-result）。
3. `xai-grok-shell/src/session/acp_session_impl/rewind.rs`：
   `handle_rewind` 删除 `needs_compaction_replay` 分支与内存截断路径，一律
   spawn_blocking replay 重建（splicing/marker 重算逻辑保留）；
   `needs_compaction_replay` 函数删除。

### 语义变化（登记为有意行为变化）

- rewind 目标在 compaction 点之上时，重建基准从"当前内存对话截断"变为
  "checkpoint blob + raw updates 重放"——与 resume 加载同源；失败不再回退
  截断（构造性保证重建语义唯一）。
- 全量 updates.jsonl 扫描成本随总行数（非活跃分支长度）增长，checkpoint base
  天然压制（设计 §7-2 已记）。
- Reasoning siblings 仍不进重建（与 resume 侧 ChatReducer 一致——上游同步
  线本来就不持久化 thought chunk 进 chat_history）。

### 验证

- 本机：`ctest.sh -p xai-grok-shell --lib session::helpers::replay`（GATE_FORCE
  过滤子集）+ `cargo check --all-targets`；acp_session rewind 族本机 win-skip
  （族R），行为回归依赖 Linux CI（rewind_cross_compaction / rewind_synthetic_turn）。


## 二十三期：模型级出口代理启用配置（2026-10-09，分支 feat/local-model-proxy-toggle）

### 需求与语义

`[model.<id>] use_proxy = true` 让该模型的采样请求走进程级出口代理
（`[network] proxy_url` / `GROK_PROXY`）；**默认 false 恒直连**——即使配置了
进程代理，未显式启用的模型也不走。与旧版「白名单 host 默认过代理」语义不同，
升级注意：依赖旧行为的配置需补 `use_proxy = true`。没配代理 / URL 非法：
规则丢弃 + warn 一次 + 直连；代理运行中故障走既有重试，不做请求级自动绕过
（避免静默改道掩盖真实故障）。

### 改动点

1. `xai-grok-extra-ca/src/lib.rs`：`build_reqwest_client_no_proxy()`（内嵌
   `build_reqwest_client_inner(configure, apply_proxy)` 重构）；
   `set_process_proxy` 与 `GROK_PROXY` env 回退路径加 `reqwest::Url::parse`
   校验，非法丢弃 + warn。
2. `xai-grok-sampler`：`SamplerConfig.use_proxy`（serde default false）；
   `shared_http.rs` 增共享直连 client 双胞胎（SHARED_H2_NO_PROXY /
   SHARED_HTTP1_NO_PROXY）；`client.rs` 客户端选择按 `use_proxy`（且规则存在）
   分流；mTLS 路径行为不变（仍走进程代理匹配）。
3. `xai-grok-sampling-types/src/types.rs`：`SamplingConfig.use_proxy`（随会话
   持久化；旧会话反序列化默认 false）。
4. `xai-grok-shell`：`ModelEntryConfig` / `ConfigModelOverride` / `ModelInfo`
   加字段（override `apply` 装配）；六处 SamplerConfig/SamplingConfig 装配点接线
   （config.rs `sampling_config_for_model` / sampler_turn `reconstruct_full_config`
   / subagent 继承 / spawn 与 model_switch 持久化转换 / tools web-search 直连）；
   远程目录模型（remote/client.rs）写死 `use_proxy: false`——授代理只认本地
   config.toml。

### 验证

- `cargo check -p xai-grok-shell --all-targets` 0 error。
- `ctest.sh -p xai-grok-sampler --lib` 291/291、`xai-grok-extra-ca --lib` 15/15、
  `xai-grok-sampling-types --lib` 303/303。
- 新增 shell `agent::config::tests` 2 例（`sampling_config_carries_per_model_use_proxy`
  / `model_override_applies_use_proxy`，GATE_FORCE 过滤跑）通过。
- 活回合（真实代理环境）验证未做，待发版后实测。

## 二十四期：出口代理自动探测（2026-10-09，分支 feat/local-model-proxy-auto-detect）

对齐 `D:\pro\websearch-mcpserver` 的 `pkg/proxy` 自动代理识别。`process_proxy_rule()`
解析链扩为三级：① `[network] proxy_url`（config 加载）→ ② `GROK_PROXY` /
`GROK_PROXY_HOSTS` env → ③ 自动探测：`HTTPS_PROXY`/`https_proxy`/`HTTP_PROXY`/
`http_proxy`/`ALL_PROXY`/`all_proxy` env，再 Windows 系统代理（HKCU Internet
Settings 的 `ProxyEnable` + `ProxyServer`，`https=`/`http=` 分项优先于裸
`host:port`，`<local>` 忽略，无 scheme 补 `http://`）；全空直连。自动探测到的代理
只影响 `use_proxy = true` 的模型。同时 `build_reqwest_client_no_proxy` 落
`.no_proxy()`：默认路径即使系统/env 有代理也真直连。未移植 websearch 的后台轮询
热切换（grok2 共享 client 进程内 latched，代理切换需重启）。

改动：`xai-grok-extra-ca/src/lib.rs`（`detect_system_proxy` + `wininet_system_proxy`
经 `windows` crate 读注册表；workspace windows dep 增 `Win32_System_Registry`
feature）、`Cargo.toml`。验证：`cargo check -p xai-grok-extra-ca` 0 error、
extra-ca 15/15 + sampler 291/291；活回合验证随 v1.0.44。

## 二十五期：状态行影子键冻结修复（2026-10-10，分支 fix/local-status-line-shadow-key）

现象：上游网关 429/400 终止性失败后，状态行冻在 `✓ 0 ✗ m`（tokens/cache/cache-misses/think
四段整段消失），此后无论多少轮都不刷新。根因（issue
[`docs-local/issues/2026-10-11-status-line-stale-after-429-400.md`](issues/2026-10-11-status-line-stale-after-429-400.md)）：
账本键分裂——成功调用按「上游回显 model id」记账（`glm-5-3-flash`），
终止性失败按「配置 id」记账（`glm-5.3-flash`）；两种拼写不同时，显示侧
`scoped_usage` 精确命中那条只有 ✗ 的影子条目（`model_calls == 0`），
`window_totals` 的 `model_calls > 0` 过滤又把四段全滤掉，行从此冻死。

改动（全部带 `// LOCAL:`）：

1. `xai-chat-state`
   - `src/actor/state.rs`：`ChatState.last_echoed_model: Option<(String, String)>`
     （配置 id → 上游回显 id）；唯一构造点 `state.rs:237` 初始化，会话账本本就不持久化。
   - `src/actor/mutations.rs`：`record_model_call_usage` 成功路径记该配对；
     `record_model_call_failure(None)` 兜底键改为「仍与当前 `sampling_config.model` 配对的回显 id」
     （换过模型不吃陈旧回显），从未回显过才退回配置 id。唯一调用点
     `xai-grok-shell/…/sampler_turn.rs` 的 `log_terminal_failure` 语义随之变化（注释同步）。
   - `src/actor/tests.rs`：三例（回显优先 / 从未回显退回配置 / 换模型不吃陈旧回显）。
2. `xai-grok-shell`
   - `src/session/acp_session_impl/status_line.rs`：`scoped_usage` 加影子键守卫——
     候选条目 `model_calls == 0` 且全会话有已完成调用时不采用，继续试 provider-qualified
     后缀候选，都不中才退回全会话合计；采用候选时把被跳过的精确影子的
     `failed_model_calls` 叠回（✗ = 会话级 endpoint health，不因换取数键而丢）。
     守卫逐候选生效，避免精确影子短路掉 `c66d29e` 的 qualified 命中。
   - `src/session/acp_session_impl/status_line_tests.rs`：新增
     `status_usage_shadow_key_does_not_freeze_the_row`（现场账本夹具）、
     `status_usage_prefers_the_qualified_echo_over_a_shadow_config_key`；
     `status_usage_model_known_only_through_failures_reads_zeroed_calls` 按新语义改写。

验证（本机 Windows 门控）：`GATE_FORCE=1 ctest.sh -p xai-grok-shell --lib status_line`
18/0（含 `b68a036`、`c66d29e` 两个历史修复用例）；`ctest.sh -p xai-chat-state --lib` 399/0。
顺带按 win-skip 族R 格式登记 `session::acp_session::subagent_usage_fold_tests::`
（同一条 `/tmp` 夹具环境族，非本补丁引入）。活体 TUI 复现未做（发版后新客户端实跑）。

发版：合 main `350874c`（merge 提交），tag **`v1.0.45`**（轻量 tag，指向该 merge）。
build run 38037822957（Linux `cargo test -p xai-grok-shell` 全套 + Windows chat-state）与
release run 38039766302（Linux 原生 + Windows 交叉编译）双双 job success，
release 页 4 资产（`grok2-v1.0.45-x86_64-{linux.tar.gz,windows.zip}` 各带 sha256），
产物实测 `grok2.exe --version` = `grok 1.0.45 (350874c36b81)`。活体 TUI 复现仍未做。

## 二十六期：供应商套餐用量上 /usage 面板（2026-10-10，分支 feat/local-provider-usage-display）

当模型 `base_url` host 匹配到「推理同一把 SK 可查套餐用量」的供应商时，Usage limit
标签页改显该供应商的套餐窗口用量并标注来源，替代 SuperGrok 额度块；失败回退
SuperGrok，不静默互换。方案与验收见
[`usage/provider-quota-display-todo.md`](usage/provider-quota-display-todo.md)，端点/
口径依据 [`usage/quota-endpoints-and-credentials.md`](usage/quota-endpoints-and-credentials.md)，
解析参照 cc-switch `coding_plan.rs` + yetone/magpie `planquota.go`/`commandcode_plan.go`。
供应商判定 = base_url host 匹配（用户拍板，无显式配置）；首批五家：OpenCode Go /
Command Code / 智谱 GLM / Kimi / MiniMax（GLM 裸 key→`code:1001` 信封再换 Bearer、
MiniMax 新旧双路径 404 切换）；火山 Ark / 百炼 / Cline / Token Unlimited 不做
（百炼 2026-10-10 复核官方 CLI 源码：单 API key 仍查不了套餐余量，三域分离）。

改动（均带 `// LOCAL:`）：

1. 新 crate `xai-grok-provider-usage`（纯函数适配层，一家一文件）：
   `types.rs`（`PlanUsageSnapshot`/`PlanUsageWindow`/`UsageProviderId`/`UsageWindowKind`）、
   `requests.rs`（`PreparedRequest` 按序尝试表）、`parse.rs`（状态码分层 + 时间戳宽容
   解析 秒/毫秒/RFC3339）、`base_url_match.rs`（`provider_for_base_url`）、
   `providers/{opencode_go,commandcode,glm,kimi,minimax}.rs`（百分比语义归一为已用：
   OpenCode/GLM 原生、Command Code 美元 used/cap 换算 + `exceeded`/`limited` 进 note、
   Kimi 计数换算、MiniMax 剩余反转 + status 2=用尽/3=无此限）。
2. `xai-grok-shell`：`extensions/provider_usage.rs`（`x.ai/providerUsage` 扩展，
   acp_agent.rs 注册）——base_url 匹配 + `own_credential` 取 key + 按序尝试 +
   进程级 TTL 缓存（`[provider_usage] cache_minutes`，默认 5，clamp 1–120，拍板 4）
   + `use_proxy` 代理语义与采样路径一致 + unified.jsonl 成败各记一条。
   `agent/config.rs` 增 `ProviderUsageConfig` 配置节。
3. `xai-grok-pager`：`Effect::FetchProviderUsage`（usage 模态打开时触发，与 billing
   共用 fetch generation）+ `TaskResult::ProviderUsageFetched`（nonce 守卫只 settle
   自己这代模态）+ `usage_modal.rs` `usage_limit_lines` 供应商分流渲染（快照→套餐块
   替代 SuperGrok；失败→错误行+回退 SuperGrok；5h/周在时月度裁剪；`usage_bar_line`
   提取两处共用）+ i18n 五组新键。

验证：`cargo check` 三 crate（provider-usage --all-targets / shell / pager）0 error；
`ctest.sh -p xai-grok-provider-usage --lib` 54/54（夹具单测：各家窗口归一/百分比反转/
`code:1001` 信封/无订阅 403/双路径/配置值轮转）；`ctest.sh -p xai-grok-pager --lib
usage_modal` 37/37（含 替代/裁剪/回退/loading 四个新用例）；i18n 扫描器分类一致。
活体验证（面板 vs `scripts-local/commandcode_usage.py` 对账）未做，待新客户端实跑。

发版：合 main `afd4058`（merge 提交，含 `3df24b7` 索引文档），tag **`v1.0.46`**
（轻量 tag）。build run 38065337768（Windows job 先绿；Linux `cargo test -p …
-p xai-grok-shell` 全套，本分支的 shell 扩展单测由该套件覆盖）与 release run
38067181846 双双 job success，release 页 4 资产
（`grok2-v1.0.46-x86_64-{linux.tar.gz,windows.zip}` 各带 sha256，zip 本机下载
sha256 实测一致）。产物已部署本机 `D:\tool-cli\grok2\grok2.exe`，实测
`grok2.exe --version` = `grok 1.0.46 (afd4058c1992)`；旧 v1.0.40 留
`grok2.exe.v1.0.40.bak` 供回滚。面板活体核验由用户人工进行。

## 二十七期：套餐取数修复 + 单飞/缓存/显示分离（2026-10-11，分支 fix/local-provider-usage-fetch）

v1.0.46 活体核验发现 Command Code 套餐取数失败：`/alpha/billing/credits` 端点
**源站延迟尾部超 5s**（响应头 `Server-Timing: total` 自报 0.7–3s+，复现探针实测
尖刺 >5s），单请求 5s 总超时撞线；且 body 读取失败被 `text().unwrap_or_default()`
**吞成空串**，日志伪装成 `invalid JSON: EOF`，真实原因（超时/流中断）不可见。
探针矩阵排除 UA、h2、代理、直连四嫌疑（h1-only 同样超时；httpx h2 成功；
直连/代理都通）——失败只与「5s 预算 vs 源站尾部」相关。协议 A/B（Python httpx
冷连接交错 5×h1/5×h2）：h1 mean 1749ms / h2 mean 1428ms，10/10 成功，延迟由源站
主导——**协议不是耗时因素，保持 reqwest 默认，不加 `http1_only`**。

改动（均带 `// LOCAL:`，请求/响应缓存/显示三层分离）：

1. shell `extensions/provider_usage.rs`：单请求超时 5s→15s（`REQUEST_TIMEOUT`）；
   body 读取失败与传输层失败同待遇（真实上报 + 可换下一个 PreparedRequest），
   `err_chain` 展开 reqwest source 链保留根因；缓存与单飞改按 **(provider, SK 指纹)**
   键控（`key_fingerprint` 64 位哈希，不落密钥明文）——同一把 SK 跨模型共享缓存，
   同账号并发至多一个在途请求（per-key tokio Mutex 单飞锁，等锁后复检缓存直接复用）；
   缓存命中回包提取 `cached_response` 共用。
2. pager 触发/落库（用户拍板「请求单发，不允许上个没到响应就再发；请求和响应缓存
   和显示分离」）：`Effect::FetchProviderUsage` 去 nonce、`TaskResult::ProviderUsageFetched`
   带 `model_id`；触发端在途守卫（`agent.provider_usage_loading` 期间重开面板不再发
   新请求）；回包**无条件落 agent 级缓存**（`AgentView.provider_usage_cache`
   `ProviderUsageCache{model_id, response, fetched_at_ms}`，模态关闭不丢、换模型显示
   过滤），失败回包不覆盖已有成功快照，与 billing nonce 解耦。
3. pager 渲染（用户拍板「TTL 到期先保证有显示，再等更新」）：stale-while-revalidate
   —— 有旧快照时刷新中/刷新失败都继续显示旧数据并附**「查询于 HH:MM」**（时间戳取
   快照 `fetched_at_ms`，shell TTL 缓存命中时即真实上游取数时刻；在途时加
   「刷新中…」）；无任何缓存才显示加载行/错误行。i18n 新增两组键
   （`Queried at {time}` / `Refreshing…`）。

验证：`ctest.sh -p xai-grok-pager --lib usage_modal` 38/38（新增
stale_cache_keeps_showing_with_timestamp_and_refresh_marker）；
`GATE_FORCE ctest.sh -p xai-grok-shell --lib provider_usage` 5/5（新增单飞锁键共享 +
指纹确定性两用例）；`cargo check -p xai-grok-pager --all-targets` 0 error；
i18n 扫描 0 挂。

发版：合 main `f91592d`，tag **`v1.0.47`**（轻量 tag，修复直接进本 tag，无版本
号分叉）。build run 38078649203 与 release run 38078649198 双双 job success
（约 27 分钟），release 页 4 资产（非草稿），Windows zip 本机下载 sha256
实测一致（`966a73aa…`）。产物已部署本机 `D:\tool-cli\grok2\grok2.exe`，实测
`grok2.exe --version` = `grok 1.0.47 (f91592d5cf5d)`；旧 v1.0.46 留
`grok2.exe.v1.0.46.bak`（更早的 `.v1.0.40.bak` 仍在，待用户核验后自行清理）。
面板活体核验由用户人工继续。

## 二十八期：T0——/login→/grok 硬改名 + 零配置首启 BYOK 引导（2026-10-11，分支 feat/local-auth-onboarding）

> /auth 供应商配置面板 + /stats API 成本立项的分期首件（设计文档
> `docs-local/provider-onboarding-api-cost-todo.md`，交接卡
> `.handoff/provider-onboarding-api-cost.md`）。上游同步重放时按本节整节重放。

1. **改名**：`slash/commands/login.rs` → `grok.rs`（`LoginCommand`→`GrokCommand`，
   name/usage/description 同步），注册点 `slash/commands/mod.rs`；shell 保留名单
   `session/slash_commands.rs` "login"→"grok"；用户可见文案 6 处换 `/grok`
   （`xai-grok-login/error.rs` provider_login_message ×2、`manager/remedy.rs`
   SelfHealing advice、shell `session/compaction.rs` 抑制通知 ×2、pager
   `effects/session_list.rs` NoOauth toast、`scrollback/blocks/session_event.rs`
   ReAuthRequired）；i18n 表描述键换新（"Log in or re-authenticate with your Grok
   account"/"登录或重新验证 Grok 账号"）；测试夹具与断言全量跟改（registry/mod/
   acp_command/inspect/slash_commands_tests/prompt/task_result/session_events/
   compaction_inline 等；`/log` 前缀用例改 `/gr`）。ACP 通告的 method id
   `grok.com` 与 authenticate 流不动。
2. **零配置首启 BYOK 引导**：shell `AuthMethodsBuildInputs` 新增
   `byok_recommended`（`agent/auth_method.rs`），unpinned 路径把它写进 grok.com
   method 的 `meta.byok_recommended`（pinned 路径恒 false）；调用点
   `agent/mvp_agent/acp_agent.rs` 计算（unpinned ∧ 非 disable_api_key_auth ∧
   无外部 SK ∧ 无 cached token ∧ 无企业 OIDC ∧ 无 auth_provider_command）。
   pager `acp/mod.rs` `startup_auth_metadata` 扩五元组提取该 meta →
   `AcpConnection.byok_recommended` → `AppView.byok_recommended`（event_loop
   仅 `connection.needs_login` 分支写入，mid-session 登录/切号不受影响）→
   `WelcomeRenderParams`。welcome Pending 首屏（`views/welcome/mod.rs`）：
   `byok_recommended ∧ 无 auth 错误` 时消息位显示 BYOK 配置指引两行（新增 i18n
   键 3 组：No model provider configured yet / Add a [model.<id>] entry…/
   Sign in with your Grok subscription），菜单 `l` 项改「使用 Grok 订阅登录」；
   `render_welcome_blocked` 消息参数扩为多行切片。T2 面板落地后引导位升级为进面板。

验证：见 port-roadmap.md §6 2026-10-11 T0 完成条目（三 crate check 0 error；
pager 六滤全绿 3 535 例含 7 个新用例；shell/login GATE_FORCE 过滤全绿；
i18n 0 挂）。win-skip 新登记族S/族T。

## 二十九期：模型目录取数 + /stats「缓存写」→ API 成本列（2026-10-11，分支 feat/local-model-catalog）

> provider-onboarding-api-cost 分期 T1 + 特性 B（设计文档
> `docs-local/provider-onboarding-api-cost-todo.md`，拍板六轮：TTL 1 天起步可配、
> 只留官方价格、OpenRouter 主 / models.dev 兜底）。上游同步重放时按本节整节重放。

1. **新纯函数 crate `xai-grok-model-catalog`**（`crates/codegen/`，与
   xai-grok-provider-usage 同分层：端点组装 + 解析 + 匹配纯函数，HTTP 归调用方）：
   - `requests.rs`：OpenRouter `/api/v1/models` 主 + models.dev `/api.json` 兜底，
     免鉴权带 UA（modelq 先例）。
   - `openrouter.rs` / `modelsdev.rs`：响应解析归一为 per-1M USD
     （OpenRouter 字符串价 ×1e6，`"0"`=免费 `Some(0.0)`、空/负/非数=未列价）；
     models.dev `limit`（context/output）+ `cost` 直取。
   - `match_model.rs`：置信匹配移植 modelq `match.go`（100 exact / 95 `:variant`
     剥离 / 86 vendor 别名折叠 / 85 canonical / 84 归一化，平分主源优先）；
     fuzzy 子串=噪声不富化（拍板沿用）。
   - `cost.rs`：`cost_usd`——OpenAI 兼容口径 `cached_read ⊆ prompt`（计费输入
     =prompt−cached）；`cache_creation>0` 仅 Anthropic 风格上报（prompt 已排除
     缓存读，不减）；任一「量>0 成分」缺价 → `None`（整条无价，宁 – 不低估）。
2. **shell 扩展 `extensions/model_catalog.rs`（`x.ai/modelCatalog`）**：请求
   `{models:[...]}` → 按序双源取数（首个成功者生效）→ 进程级单条目 TTL 缓存 +
   全局单飞锁（目录与账号无关）→ 匹配请求 id 回 `{entries: {id: entry|null}}`。
   取数失败回退过期目录（stale，价格面宁旧勿空），全部失败条目全 null。
   配置 `[model_catalog] cache_minutes` 缺省 1440，clamp 5–20160。路由点
   `acp_agent.rs`。目录请求走 `build_reqwest_client`（用户 proxy_hosts 显式
   收录的目录 host 可走隧道，缺省 allowlist 外恒直连）。
3. **pager 成本列**：进程级 store `app/model_catalog_store.rs`（增量并入 + 在途
   标志 + 6h 重取窗 + `aggregate_cost`/`UNPRICED_LABEL`）。`Effect::
   FetchModelCatalog`（`dispatch_show_stats` 触发，账本内 model id 去重为取数
   目标；回包 `TaskResult::ModelCatalogFetched` 落 store 并即时刷新已打开面板）。
   渲染面三处「缓存写」列替换为「成本」：模态卡片 `model_card_lines`、复制文本
   `model_card_text`、极简文本 `format_usage_row`——有价 `fmt_cost`（<1 美分
   4 位小数，否则 2 位），无价/未取到显示 `–`；模态卡片列表尾新增合计行
   （`cost total` 求和 + `unpriced` 条数标注，无价剔除出合计）。i18n：
   `("cache write","缓存写")` 键删除，新增 cost / cost total / unpriced 三键；
   `xai-grok-tools::model_usage_ledger` 增 `fmt_cost`。

验证：catalog 29/29；shell GATE_FORCE extensions::model_catalog 5/5；pager 门控
stats 过滤 36 例 + catalog 过滤 30 例、tools GATE_FORCE fmt_cost 过滤 6 例全绿；
三 crate `cargo check --all-targets` 0 error/0 warning；i18n 扫描 cache write
键 0 残留。

**补记（二十八期，2026-10-11 晚）**：T0 改名漏一处集成测试断言——
`xai-grok-shell/tests/external_auth_expired_credential.rs`（`#![cfg(unix)]`，本机
Windows 编译为 0 用例，当班 lib 过滤覆盖不到）仍断言 `Run /login`，v1.0.48 的
main build run `38115162565` Linux 红灯暴露（正文实际已输出 `/grok`）。修复 =
该文件三处字面量（doc 注释 + 正反断言）改 `/grok`，随 feat/local-model-catalog
分支合入。另：同 run 首挂的 lib 用例
`reset_title_to_auto_adopts_in_flight_generation_as_auto`（8 秒墙钟轮询）在
failed-job 重跑中通过（7096/0），判定为计时 flake，非 T0 回归。

## 三十期：T2/T3——/auth 供应商配置面板 + 目录回填 + config 写回（2026-10-11，分支 feat/local-auth-panel）

> provider-onboarding-api-cost 分期 T2 + T3（设计文档
> `docs-local/provider-onboarding-api-cost-todo.md`，拍板七轮：SK 明文 api_key
> 直写、条目 id 用户缩写唯一、toml_edit 定点写回）。上游同步重放时按本节整节重放。

1. **shell 写回 `util/config/model_entry_writes.rs`**：`ModelEntryDraft`（id/model/
   base_url/api_key/api_backend/context_window/max_completion_tokens；手写 Debug
   打码）+ `write_model_entries`——`lock_config_writes`（SAVE_LOCK ⊃ init flock）
   守卫下 toml_edit DocumentMut 定点编辑 `[model.<id>]`：注释/未知段/格式全保留；
   全批原子（任一 id 冲突整批拒绝、文件不动），已有同 id 一律拒绝不覆盖；
   非空白不可解析文件拒绝写回。`ChatCompletions` 为 serde 缺省省略不写；
   `context_window`/`max_completion_tokens` 由调用方按「与兜底不同才写」预筛
   （回填语义见设计文档）。空 api_key 省略字段。
2. **shell 扩展 `extensions/provider_models.rs`（`x.ai/providerModels`）**：参数
   `{base_url, api_key?}` → `GET {base}/models`（15s 超时；Bearer 仅当有 key；
   `build_reqwest_client` 同款客户端纪律）→ OpenAI/Anthropic `data[]` 与裸数组
   三形态解析为 `{id, owned_by?}` 清单；body 读错真实上报（不吞成空串）；
   错误进回包体不进 ACP Err。SK 只进 Authorization 头，不进日志/错误/Debug。
   路由点 `acp_agent.rs` + `extensions/mod.rs`。
3. **pager 面板 `views/auth_modal`**：分步向导 Protocol→BaseUrl→ApiKey（掩码
   回显，Ctrl+U 清空）→Models（`GET /models` 拉取 + 关键词过滤 + 空格多选 +
   `a` 手填 slug 回退 + `r` 重试；手填项重取不丢）→Confirm（每行 id 可编辑 `e`、
   冲突标红阻断、回填值展示）→Writing（Esc 吞掉）→Done。目录回填走 T1 store
   （`AuthFetchCatalogBackfill` → `ids_needing_fetch` 去重 + `begin_loading`
   单发 → 复用 `Effect::FetchModelCatalog`；回包刷新确认行）。`SecretKey`
   newtype 手写 Debug 打码 + PartialEq。
4. **Effect/TaskResult**：`Effect::FetchProviderModels`（ACP ext 通道，陈旧回包按
   requested base_url 丢弃）+ `Effect::PersistModelEntries`（pager 同进程直调
   `write_model_entries`，persist_setting 先例）；`TaskResult::ProviderModelsFetched`
   / `ModelEntriesPersisted` 路由到 AuthPanel 两宿主。
5. **双宿主接线**：会话内 `ActiveModal::AuthPanel`（AgentView active_modal，
   `/auth` 命令触发）；welcome 门 `AppView::welcome_auth_panel`（零配置 BYOK
   用户被困门后是主场景）：Pending 菜单增 `a` 项（byok_guidance 时 3 项，索引
   映射 `dispatch_pending_menu_action` 同步）+ 键 `a` 直开面板；welcome 两渲染
   路径 + 输入路由 + 粘贴 + scroll-blocking 门控 + link_spans 清除均接线。
6. **注册与保留字**：`slash/commands/auth.rs`（`/auth` = OpenAuthPanel）注册；
   shell `PAGER_COMMAND_KEYS` 增 `auth`（防 skill 影子化）；welcome BYOK 引导
   文案改指 `a`/`/auth`（T0 占位文案升级）；pager 依赖补
   `xai-grok-sampling-types`（原 optional 死条目转正常依赖，ApiBackend 需要）。
   i18n：新增 auth_modal + welcome 引导键（zh 翻译齐），扫描 0 挂 0 重复。

验证：pager 门控 lib auth_modal 13 / welcome 226 / slash 568 / dispatch::tests
1732 全绿；shell GATE_FORCE 过滤 model_entry_writes 7 / provider_models 6 /
slash_commands 105 全绿；两 crate `cargo check --all-targets` 0 error/0 warning；
i18n 扫描 0 挂。活回合验证（真实 TUI 走五步 + 写回后 /model 热加载）待新客户端。

## 三十一期：目录裸 id 匹配 + 只留原厂 + prices.cache + /auth 面板两修复（2026-10-11，分支 fix/local-catalog-bare-ids-auth-panel）

> v1.0.50 活回合反馈三问题（issue
> `docs-local/issues/2026-10-11-catalog-bare-ids-and-auth-panel-fixes.md`，拍板二轮
> 8–13 见 design doc）：成本列半数模型无价（裸 id 撞上 OpenRouter 单源）、/auth
> 提示行悬空、写回后 /model 不刷新。上游同步重放按整节重放。

1. **纯函数层 `xai-grok-model-catalog`**：
   - **源换序**（`requests.rs`）：models.dev 主、OpenRouter API 兜底（修订拍板 4）。
   - **原厂过滤**（`modelsdev.rs` + `match_model::FIRST_PARTY_PROVIDERS` 21 项白名单，
     2026-10-11 线上数据逐项核对存在性与模型数、collapse 归一化零撞车）：models.dev
     解析只留白名单 provider（含 `openrouter` 官方牌价镜像），302ai 等 200+ 转售
     provider 整库不留；订阅 plan 条目价格恒 0 →「input/output 双 0 归无价」防护。
   - **匹配策略**（`match_model.rs`）：新增 84 档三路——vendor 不识别剥壳取 `xx/`
     后半段、纯 id 分隔符变体（`_`/`.` vs `-`）、部署戳尾巴容忍（6–8 位日期，
     4 位日期型型号如 `step-3.5-flash-2603` 不剥）；平局键升三元的
     (source, first_party, canonical 自证)——deepseek 自家 0.15/0.6 压过 alibaba-cn
     托管同 id 0.14/0.28。
2. **shell 扩展 `extensions/model_catalog.rs`**：`~/.grok/prices.cache` 磁盘缓存
   （`DiskCache{version, fetched_at_ms, catalog}` 原子写 tmp+rename，读 32MB 封顶；
   TTL 内跨进程复用回 `cache="disk"`；取数失败 stale 回退改进程内 → 磁盘任意龄期
   两级，宁旧勿空）。`remote_config::cache_file` 的 `read_capped`/`write_atomic`
   提为 `pub(crate)` 供复用。
3. **pager 写回热重载**：`Effect::ReloadAgentModels`（写回 Ok 后发
   `x.ai/internal/reload_models` 裸方法名，解析 `{models: n}` 回包）+
   `TaskResult::AgentModelsReloaded`（Err 落 AuthPanel 完成页 `reload_error`，
   灰字提示重启）。根因：embedded/headless 模式无 config 文件 watcher（仅
   `run_leader` 装配），v1.0.50 的 hot reload 文案实为空头支票。
4. **pager 面板布局**：按键提示从内容流改为固定渲染面板倒数第二内容行（Step x/5
   页脚之上），对齐全局面板贴底惯例；`step_hints` 按真实按键路由给文案（editing
   态 Enter/Esc 保存 id）；顺带补 `next`/`retry`/`add manually` 等漏登记 i18n 键。

验证：model-catalog lib 38/38；shell GATE_FORCE model_catalog 7/7 +
ext_method_reload_models 1/1（钉死裸名路由 + 回包形状）；pager GATE_FORCE
auth_modal 14/14（含提示行贴底渲染断言）+ dispatch::tests::auth 25/25（写回 Ok
→ ReloadAgentModels、Err 不触发、重载 Err 落完成页）；两 crate check --all-targets
0 error；i18n 扫描基线不变。匹配仿真（线上两源真实数据）：本机 10 个在用 model id
8 命中全为原厂/官方价，2 miss 均 sensenova（豁免）。活回合待新客户端。

## 三十二期：v1.0.51 复发二次修复——裸 id↔vendor/model 段命中 + 匹配索引化 + 缓存治理 + 启动预热（2026-10-11，分支 fix/local-catalog-bare-id-cache-dir）

> v1.0.51 活回合成本列依旧只剩 v4.1 flash（issue 文末「复发与二次修复」节）。
> 根因三件叠加：models.dev 主源瞬时失败只落 `tracing::debug!` 静默回退
> OpenRouter（458 条 pure open_router，unified.jsonl 零失败行）；匹配器裸 id
> 分支写死只认裸条目，`vendor/model` 目录形态下必失配；缓存误写 home 根。
> 拍板五轮 14–18 见 design doc；**用户拍板不切新 tag，v1.0.51 重指重发**。
> 上游同步重放按整节重放。

1. **纯函数层 `xai-grok-model-catalog`**：
   - **裸 id ↔ `vendor/model` 段命中**（`match_model.rs` 裸 id 分支）：84 档兜底从
     `!entry.id.contains('/')` 放宽为取目录条目 model 段（最后一个 `/` 后）容忍
     比较——OpenRouter 整库兜底时裸网关 id 也能命中；`:variant` 条目段键自带
     后缀不参与（`:free` 零价不冒充牌价），近似型号（`flashx`）不冒充。
   - **匹配索引化**（`types.rs` `CatalogIndex`）：`ModelCatalog::new` 构建时预建
     by_id（原样 id）/ by_canonical / by_vendor（collapse 别名键）/ by_segment
     （model 段 + 剥部署戳备选键）四张倒排，匹配从全库线性扫描改 O(1) 候选取回，
     `consider` 平局裁定语义不变；`entries` 收紧 `pub(crate)`（外部直改会使索引
     失效），serde Serialize 正常出条目、**手写 Deserialize 同形读入并重建索引**
     （磁盘缓存往返测试钉死该形状）。
   - **`CatalogEntry.reasoning` 位**：OpenRouter 取 `supported_parameters` 含
     `reasoning`（缺席 = None 未知）、models.dev 取 `reasoning` 布尔。
   - **models.dev provider 级提前过滤**：转售商整 provider 跳过、模型条目不物化
     （压低 5.4MB 级响应的解析内存峰值；行为与文末 retain 一致）。
2. **shell 扩展 `extensions/model_catalog.rs`**：
   - **可见性**：`fetch_one` 单源失败 warn 进 unified.jsonl（source/url/error），
     `fetched` 日志带 `source`（生效源）+ `trigger`（panel/startup）——回退一眼可见。
   - **缓存治理**：磁盘缓存迁 `~/.grok/cache/prices.cache`（version 2，读路径
     `write_atomic` 自带 create_dir_all），首次成功取数后清除 home 根旧文件。
   - **冷热分离**：进程缓存改「请求 id → 参数」薄表（`Cached.results`），全量
     目录只在取数/薄表补齐瞬间物化用完即弃；`completed_entries` 对缺失 id 从
     磁盘全量库临时物化重匹配（lazy）并并回薄表；启动预热不塞结果（空薄表 +
     磁盘热数据，首次查询 lazy 补齐），全量条目零常驻。
   - **启动预热** `spawn_startup_prefetch`（装配点 `MvpAgent::with_models`）：
     TTL 内磁盘新鲜直接空薄表入缓存；过期才后台取数一次/天；与面板触发共用
     单飞锁 + 同一份磁盘缓存天然去重；无 tokio runtime（测试）静默跳过。
   - **`CatalogMatch.reasoning`** 随匹配结果下发。
3. **/auth 回填**：`ModelEntryDraft.supports_reasoning_effort`（目录
   `reasoning = Some(true)` 才写 `supports_reasoning_effort = true`，接入既有
   effort 机制；None/false 不写，网关不支持不冒充）+ toml_edit 写回该键 +
   `ConfirmRow.supports_reasoning_effort`（catalog_backfill 三元化）+ 确认行
   `think=on` 标注（与 ctx=/max_out= 同风格）。

验证：model-catalog lib 41/41（新增裸 id↔段命中 / 双源优先级 / 戳容忍段命中 /
reasoning 解析 / Deserialize 重建索引）；shell GATE_FORCE model_catalog 8/8
（磁盘往返用例抓出手写 Deserialize 形状 bug，已修；含回包快照回归用例）+
model_entry 7/7；pager auth_modal 14/14（backfill 三元含思考位断言）+
dispatch::tests::auth 25/25 + stats_modal 17/17 + model_catalog_store 1/1；零
warning。**真实数据仿真（catalog-sim，读本机 v1.0.51 遗留 OpenRouter-only 458 条
缓存 = 最坏情形）**：config `[model.*]` 表键 8/8、路由 slug 8/8、账本回显 6/6
（并集 11 id）全命中原厂价。发版：**v1.0.51 tag 三次重指重发**（同一需求线，
用户拍板不切新 tag）：一发的匹配/缓存/预热、二发的 `priced` 运行时日志、三发的
回包快照补修，终版 merge `9ef52aa`；release run `38144699713` / build run
`38144694968` 双 job success，4 资产非草稿，本机部署 `grok 1.0.51 (9ef52aa1d712)`。
**活回合已验（用户确认 + unified 实证）**：/stats 成本列多模型显示正常；
`model catalog: priced` 日志 6 模型全 100 档命中（models.dev 主源裸条目）、
`unpriced: []`；models.dev 读体失败时 warn `source failed` 可见并回退 open_router。

**补修（同 tag 重发内，用户活回合复现「成本全 `–` 连 v4.1 flash 也无」）**：
`completed_entries` 回包用了调用方旧快照——首次 /stats 时薄表尚空（启动预热只放
空表），lazy 补齐算出的匹配只并回共享薄表、回包仍全 null，pager store 按「已知
无价」缓存 6h 不重问 → 全部模型显示 `–`。修：回包基于并回后的合并视图组包
（shell model_catalog 8/8，新增回归用例
`completed_entries_backfills_from_disk_and_returns_prices` 钉死）。**补运行时
价格日志**（用户要求）：每次取价回包打 unified `model catalog: priced`——
`priced: {id: {entry, score, in, out}}` + `unpriced: [...]` + `cache` 档位，
覆盖 hit/disk/fetch/stale/error 全部回包路径。

## 三十三期：套餐块被 xAI 账单门吞掉（API-key/BYOK 下 /usage 不显套餐用量）（2026-10-12，分支 fix/local-provider-usage-gate）

用户报告「/usage 不显示 Command Code 套餐余量」。排查用真客户端活体（ptyctl 驱动
`D:\tool\cli\grok2.exe` 1.0.51 + unified.jsonl 双向对照）定位：**取数成功、显示被门吞**。

证据链（同一时刻两条并存）：

1. shell 侧取数成功：`~/.grok/logs/unified.jsonl`
   `provider usage: fetched`（`provider=commandcode`、`modelId=cc-deepseek-v4.1-flash`、
   `windows=2`），当天 00:40 / 01:03 / 01:07 / 01:24 四次（末次即复现打开面板当刻）。
2. 面板同刻渲染「用量限制由你的团队管理」——ptyctl 驱动真 TUI 敲 `/usage` 抓屏得证。

根因：`xai-grok-pager/src/views/usage_modal.rs` 的 `usage_limit_lines` 分支顺序把
供应商套餐块排在 `!state.ctx.usage_visible` 之后。本机为纯 BYOK（无 `~/.grok/auth.json`，
状态行「已用 API key 登录」）→ `is_api_key_auth = true` → `consumer_account()` 假
→ `usage_visible = false` → 命中「团队管理」行即返回，套餐块永不可达。而取数触发
（`app/dispatch/status.rs:122`）不看 `usage_visible`，故出现「请求发了、数据回了、
面板不显示」的割裂。

改动：套餐块分支提到 `!usage_visible` / `billing_redirect_url` 两道 xAI 账单门之前
（`chat_kind` 网关会话仍整块跳过，拍板不变）。理由：套餐块数据由模型 `base_url`
命中决定、与 xAI 账单面无关，取数本就按模型触发；xAI 账单门继续管 SuperGrok 额度块、
重定向文案与额度告警，语义未动。

验证：`cargo check -p xai-grok-pager --all-targets` 0 error；
`ctest.sh -p xai-grok-pager --lib usage_modal::` 31/31（新增回归用例
`provider_block_shows_when_billing_surface_hidden`，钉死「`usage_visible=false` 有快照
必出套餐块 / 无快照仍走账单门文案」）。

活体复验（2026-10-12，本机 debug 构建 + ptyctl 驱动、同 BYOK 条件）：

- 修后（分支构建，状态行显示分支名）：`/usage` 首屏即出「套餐用量 — Command Code」
  + 5 小时窗口 `$0.35 / $14 · limited` 2% + 每周限额 `$7.12 / $35 · limited` 20%
  + 「查询于 10-12 10:29」，「团队管理」行消失。
- 同条件回归（切 `ark-glm-5.3-flash`，无供应商匹配）：面板仍是原「用量限制由你的
  团队管理」，与改前一致——非供应商模型零变化。
- 环境备注：debug 构建的 main 线程 1MB 栈会 `has overflowed its stack`（release 无
  此问题），临时用 PE 头 `SizeOfStackReserve` 1MB→64MB 补丁跑通，补丁只在
  `target/debug/xai-grok-pager.exe` 该产物上，重新构建即失效。

发版：合 main merge `1cada7bd`，tag **`v1.0.52`**（轻量 tag，与 v1.0.4x/5x 同制）。
release run `38196276842`（Linux + Windows 双 job success，约 30 分钟）与 build run
`38196272306`（双 job success）全绿；release 页 4 资产非草稿：
`grok2-v1.0.52-x86_64-linux.tar.gz(.sha256)`、`grok2-v1.0.52-x86_64-windows.zip(.sha256)`。

**发行产物活体复验（不是自制 debug 版）**：下载 Windows zip（sha256
`7d0f7766…` 与资产一致）解出的 `grok2.exe`（`--version` = `grok 1.0.52 (1cada7bd693f)`，
sha256 `5d585604…`）经 ptyctl 驱动同 BYOK 条件敲 `/usage`：面板出「套餐用量 —
Command Code」+ 5 小时窗口 `$0.45 / $14 · limited` 3% + 每周限额 `$7.21 / $35 ·
limited` 20% + 「查询于 10-12 11:09」。本机部署 `D:\tool\cli\grok2.exe` = v1.0.52
（客户端自更新已先一步装好，与本资产字节一致），回滚点 `grok2.exe.1.0.51`。
## 三十四期：`/lsp` workspace 级 LSP 工具启停（2026-10-12，分支 feat/local-lsp-workspace-toggle）

> ADOPTION §7.6.1 提案落地（2026-10-11 登记）。同类调研（crush / opencode /
> qwen-code 三家真实现深读；goose / kimi-code / zcode / Step-Code / minimax-code
> 两轮扫描 0 命中）与设计正文见
> [`docs-local/lsp-port/README.md`](lsp-port/README.md)。qwen-code 热重载设计
> 文档明说「启用旗标不做运行时切换」，与提案的会话边界拍板互为印证。

1. **shell 存储 `util/workspace_overrides.rs`（新）**：grok-home
   `feature-overrides/<encode_cwd_dirname(cwd)>.json` 平面键值（`{"lsp_tools":
   bool}`，键可扩展）；owner-only 写（`write_secure_file`）、clear 后无键删文件、
   未知键 fail-loud、非法 JSON 读作 None 回落全局；`_in` 变体注入 home 供测试
   （`grok_home()` OnceLock 进程缓存，测试内不可换 home）。
2. **shell 门控 `Config::resolve_lsp_tools_for_cwd` /
   `resolve_lsp_tools_with_override`（agent/config.rs）**：优先级
   **pin > env(GROK_LSP_TOOLS) > workspace 覆盖 > config > remote > default**
   （用户工作区显式选择压过环境默认；进程 env 与 MDM pin 保持最高）；决策核拆
   纯函数（覆盖值注入）供测试。消费点 agent_ops.rs LSP 注册块换走新门控，
   `tracing::debug` 记 tier+值；生效点 = session spawn 的 definition 构建，
   会话中途改覆盖不重塑在跑会话的工具清单。
3. **pager `/lsp`（slash/commands/lsp.rs 新）**：`on|off|status|clear`（中文别名
   开启/关闭）→ `Action::LspConfigure`；`LspConfigureRequest` 三态
   （Status/Set/Clear）。执行体 `dispatch_lsp_configure`（router.rs）：cwd 取
   会话 cwd（agent 视图）或 app cwd（dashboard/welcome，无门控）；写入过
   「零用户回合」新会话门（prompt_history 空 + Idle + 无在途/排队/持有 prompt
   + 历史加载完成；无 session_id 视为未建立放行），带历史拒绝并指向 /clear
   （/clear 即 /new 别名）；status 显示本会话注册态（`meta.tools` 广播的
   `available_tools` 含 `"lsp"`）+ 工作区覆盖 + 生效点说明；agent 视图回显走
   scrollback 系统块（同 /lang），dashboard/welcome 走 toast。
4. **i18n**：描述/usage 入翻译表（"Toggle LSP tools for this workspace" /
   "/lsp [on|off|status|clear]"）；运行时消息按 /lang 先例硬编码中文。
   不扩 `CommandExecCtx`（50 个字面构造点，cwd/门控信号在 dispatch 层取）。

验证：shell GATE_FORCE workspace_overrides 4/4 + lsp_tools 3/3；pager lsp 6/6
（命令解析 2 + dispatch 门控 4）+ translations_table 1/1；双 crate check
--all-targets 0 error 0 warning；i18n 扫描 exit 0。**待办**：活体 TUI 验证
（新客户端 `/lsp on` → /clear → 工具清单现 `lsp`；status 三态；带历史回合拒绝）。

## 三十五期：LSP 开箱即用——默认开 + builtin 检测（五语言）+ `/lsp status` 数据面 + 零回合热加载（2026-10-13，分支 feat/local-lsp-builtin-servers）

> 用户对 v1.0.52 的 /lsp 三条不满：① 必须显式开 `[features] lsp_tools`（且当时
> 默认关）② 没有 rust/go/python 的自动检测，必须手写 lsp.json ③ `/lsp status`
> 只报「工具注册态」，看不到 server 面。施工中追加两条拍板：④ 补 Java/C 覆盖
> 主流语言 ⑤ 零回合会话直接热加载，不逼用户 /clear（推翻三十四期的「会话边界
> 切换」语义——本会话还没发过消息就没有契约可污染，重建无代价）。

1. **默认开（config-types registry）**：`Feature::LspTools` `default_enabled:
   false → true`；registry 测试表 + off_reason 断言改挂 WebFetch 兜底；
   shell config 测试 `lsp_tools_workspace_override_enables_default_off` 重写为
   `…_disables_default_on`（off 覆盖生效且不泄漏全局）。关闭仍走
   `[features] lsp_tools = false` / `GROK_LSP_TOOLS=0` / `/lsp off`。
2. **builtin 检测（tools `implementations/lsp/builtin.rs` 新）**：静态槽位表
   rust-analyzer / gopls / pyright→basedpyright→pylsp（优先序三选一）/ jdtls /
   clangd（.c/.h/.cpp 一族，.h 映射 cpp 语言 id）；`probe_command` 只 stat PATH
   候选（Windows 走 PATHEXT 语义，绝不 spawn 二进制——qwen 教训：探针不执行
   命令）；`merge_builtin_servers` 双重 gap 语义：同名让位 config + 扩展已被
   任何 config server 认领则整语言让位。canonical 合并入口
   `load_servers_with_builtins_sourced`（config.rs），spawn / inspect /
   workspace handle / status 四处统一换用；`load_servers*` 纯 config 版保持
   原样（测试确定性，不被探测机的 PATH 污染）。
3. **`/lsp status` 数据面（shell `extensions/lsp_status.rs` 新 +
   `x.ai/lspStatus`）**：一次回包 = 门禁判定（pin/env/override/config/remote/
   default 六层来源标签）+ 工作区覆盖 + server 清单（名称/命令/args/来源
   display_label/裸命令 PATH 探测命中）+ 会话后端快照（启动阶段 + 运行中
   server）。后端快照走 `LspBackend::status_snapshot()`（trait 新方法，
   adapter 从 StartupState + mgr.clients 直读；test double 默认 None）。
   pager 渲染 `format_status`（zh，`/lang` 先例硬编码）；Status 臂改发
   `Effect::FetchLspStatus` 异步取数，回包 `TaskResult::LspStatusFetched` 落
   scrollback；解析/发送失败折进回包体 `error`（状态块总能落盘）。
4. **零回合热加载（shell + pager 全链）**：`SessionCommand::ReloadLspHarness`
   （新，commands.rs）→ run_loop 臂 → actor `handle_reload_lsp_harness`：
   definition 与 system prompt 不变，只换 builder 的 LSP 注入面，复用
   model-switch 零回合重建的全部簿记（bridge 重绑 / MCP rematerialize /
   零回合前缀改写 / `HarnessRebuild` 工具清单重广播——pager 镜像自动跟进）。
   `AgentRebuildSpec::build_agent_with_lsp`（agent_rebuild.rs，lsp_override
   参数穿透 build_agent_inner）。后端组装抽 `MvpAgent::build_lsp_backend`
   （spawn 块同步收敛到这一个入口）。执行面 `x.ai/lspApply`（extensions/
   lsp_apply.rs 新）：门禁重解 + turn_count>0 拒绝（signals 快照，与
   model-switch 同源）+ 现状比对（无变化不重建）+ 重建 oneshot 等待回包。
   pager Set/Clear 成功写覆盖后，零回合有 session_id 时追加发
   `Effect::ApplyLspTools`；`TaskResult::LspToolsApplied` 追加权威结果行。
   带历史回合的 Set/Clear 拒绝文案同步改口径（热加载会污染已定格的对话契约）。
5. **i18n / 文档**：user-guide 05-configuration（features 表默认值 + LSP servers
   节四源合并语义）+ 26-config-reference 默认值行；`/lsp` 运行时文案保持
   zh 硬编码（/lang 先例）。

验证：cargo check shell/pager(--all-targets)/tools/workspace 0 error；
tools lib builtin 4/4（probe 跨目录/路径形态/优先序、merge 同名/扩展 gap/补位）；
pager lsp 8/8（status effect 参数 + format_status 三态 + 门控四例）；
shell GATE_FORCE config lsp 3/3（默认开 off 覆盖 / pin 压覆盖 / 覆盖压 config）
+ workspace_overrides 4/4。**GATE_FORCE tools 全量 lsp 过滤附带实锤一个既有
Windows 环境族**：`implementations::lsp::tests` mock e2e 40 挂（mock server 是
Python 脚本直接当 server spawn，Windows `std::process::Command` 拉不起 .py →
`InitFailed("service stopped")`；70 过 40 挂 2 ignored，非本次回归——tools
crate 从未进 win-whitelist 即因于此，懒治理：e2e 覆盖靠 Linux CI 或后续把
mock 换 .bat 包装）。**待办**：活体 TUI 验证（rust 仓起会话看 `lsp` 工具自动
注册 + rust-analyzer 拉起；/lsp on 热加载零回合会话；带历史拒绝；status 面板）。

**补修（CI 一轮暴露，同 tag 内重指重发）**：v1.0.54 首发时 Linux build job 挂
13 例（全在 shell subagent 测试族）——根因：默认开后 `WorkspaceOps::for_test()`
等**同步测试夹具**构造 WorkspaceHandle → `WorkspaceHandle::build` 检测到 runner
PATH 上的 clangd → `ensure_started_background()` 裸 `tokio::spawn` 在无 runtime
上下文 panic（"there is no reactor running"）。生产路径全在 runtime 内不受影响；
修复 = `ensure_started_background` 改 `Handle::try_current()` fail-safe（无
runtime 跳过 eager start，首次 runtime 内使用经 `ensure_ready` 启动——同文件
Drop 的 try_current 先例），workspace handle / reminders 两个调用点随 trait 方法
一并覆盖。本机验证：tools check 0 error + 原 13 例同根的
`subagent_worktree_snapshot_gate` 4/4 绿（带 clangd PATH 重编复跑）。

**补修二（活体验证暴露的 v1 遗留 bug，同 tag 内）**：ptyctl 真客户端活体发现
`/lsp on|off` 在全新零回合会话上**恒被拒绝**（"已有历史回合"而 /session-info
轮次=0）——根因：v1 的 pager fresh 门把 `session.prompt_history.is_empty()`
当「零用户回合」判据，但该镜像是**跨会话持久的输入召回历史**（上箭头召回，
slash 命令也记录、/clear 不清空），生产环境永非空；单测全绿只因夹具是空历史。
修复 = 移除 pager 侧 fresh 门（覆盖文件写入无害恒放行），零回合权威判定收敛到
shell `x.ai/lspApply` 的 `turn_count`（signals 快照，与 model-switch 零回合重建
同源）；带回合的会话写完覆盖后 apply 回 `ok:false`「热加载跳过；改动对新会话
生效」，pager 照实渲染。两个拒绝测试改写为新城语义（带历史写入 + 发 apply /
运行中回合 Clear 照写）。

**补修三（热加载链路活体第二处，同 tag 内）**：摘除态下 `/lsp on` 被误判
「无变化」不重建——`x.ai/lspApply` 的现状比对读 `handle.tool_context.lsp`，
而该字段是 spawn 时的**快照**，`ReloadLspHarness` 重建后不回写（摘除态下快照
仍为 Some），「摘除→再启用」被吞。修复 = apply 移除现状比对、恒重建（零回合
重建廉价，保证 harness 与门禁一致）；旧 manager 进程组由 process_scope 会话
结束时统一回收。已知边界（记录不修）：status 的「本会话后端」段同样读该快照，
摘除态下显示旧后端就绪——以「LSP 工具：禁用」主行为准；handle 快照回写需
ToolContext 字段换共享槽（ArcSwap），结构侵入不值当。

## 三十六期：主题目录扩容——11 套社区色板移植（2026-10-13，分支 feat/local-theme-catalog-port）

> 需求：从父目录同类 coding agent 吸收差异化的社区好评主题配色，移植不缺失。
> 捐献者选定 **opencode 主题库**（`D:\CODE\ai\opencode\packages\tui\src\theme\assets\*.json`，
> 32 个 JSON 主题资产，社区经典最全）；qwen-code（dracula/atom-one-dark/ayu 等 TS 定义）、
> crush（Go styles 分散）、goose 等作对照未采用。

1. **移植清单（11 套，与既有 6 主题零重叠）**：Catppuccin Mocha（别名
   `catppuccin`/`mocha`）、Catppuccin Latte（`latte`，第二个浅色主题）、Dracula、
   Gruvbox Dark（`gruvbox`）、Nord、One Dark（`onedark`）、Everforest、Kanagawa、
   Vesper、Night Owl（`night-owl`）、Solarized Dark（`solarized`）。全部标
   `requires_truecolor = true`（256 色 picker 隐藏，同 TokyoNight/RosePineMoon 策略）。
2. **新文件 ×9**（`xai-grok-pager-render/src/theme/`）：catppuccin.rs（mocha+latte
   双极性同文件）、dracula.rs、gruvbox.rs、nord.rs、onedark.rs、everforest.rs、
   kanagawa.rs、vesper.rs、nightowl.rs、solarized.rs——每文件自带 palette 常量
   （十六进制注释）+ `impl Theme` const fn + 签名测试。**不缺失口径**：~60 个
   颜色槽位逐一映射（struct 完整性由编译器强制）；donor 有定义的槽位（背景/边框/
   diff 底色/fg、markdown 语义、diffLineNumber/textMuted）原值直采，donor 未定义的
   （bg_hover/bg_visual、边框细档、plan 金、skill 青绿、md 六级标题梯、灰阶三档）
   按 TokyoNight 先例在主题自身色系内手调派生，文件头注明推导；donor 设计原貌
   （Nord/Solarized 双 diff 同底、Vesper 双高亮复用、Solarized base0 正文）保真并注释。
3. **注册接线 ×8 处**：`theme/mod.rs`（ThemeKind 变体 7..17、ALL 目录序、
   display_name、aliases、requires_truecolor、display_name_for_canonical 补
   oscura 缺项、`current()` 分发、滚动条对比度测试分发）；`theme/cache.rs`
   （`theme_kind_from_u8` 解码臂——漏加会静默回落 GrokNight）；`syntax.rs`
   `get_syntect`（深色系并 grok-night.tmTheme 臂、Latte 并 grok-day 臂）；pager
   `settings/defs.rs` 两 catalog（THEME_CHOICES + CONCRETE_THEME_CHOICES 各
   +11）；`slash/i18n.rs`（11 条中文描述）；`views/settings_modal/tests.rs`
   raw-theme 分发臂；`scrollback/blocks/mermaid_content.rs` `theme_is_dark`
   纳入 CatppuccinLatte（浅色）+ 测试双列表扩容；README / user-guide
   06-theming（表 6→17 行）/ 29-local-enhancements 新节。
4. **语法高亮基底**：11 套不新增 .tmTheme——深色 10 套并 `grok-night.tmTheme`
   臂（RosePineMoon/Oscura 同 precedent），Latte 并 `grok-day.tmTheme`；token
   色随后由 `syntect_rgb_to_fg` 按活动主题重映射，基底只须槽位完整。
5. **测试**：每文件签名测试（bg_base/accent_user 原值、is_dark 极性、别名/显示名、
   Latte 滚动条浅底 ΔΣ 方向）；既有泛化测试自动收编全部新主题（mod.rs 滚动条
   对比度遍历 ALL、主题 picker e2e 遍历 catalog）。

   **补修一轮（同分支，2026-10-13 测试轮暴露两件）**：
   - **doctor JSON 夹具漏改**：`totalThemes` 两处夹具（empty/contract）仍写 6，
     代码已正确报 17（= `selectable().len()`）→ 夹具改 17；顺带实证 256 色环境
     下 11 套新主题正确隐藏（availableThemes 仅 groknight/grokday/terminal）。
   - **测试隔离缺陷（预存类，本次实锤）**：`theme` 过滤多线程下
     `info_line_chrome_is_muted_on_terminal_theme` 拿到 Oscura 样式——机制 =
     `reset_for_test()` 置 `LOADED=false` 后，任何未预置主题状态的无守卫测试
     首次 `current_kind()` 走 `load_from_disk()` 读入**开发者真实
     `~/.grok/config.toml`**（本机 = `theme = "oscura-midnight"`）写进全局
     cache，打断并发持锁测试的断言窗口。修复 = `load_from_disk` 双门控
     `#[cfg(any(test, feature = "test-support"))]` 返回 None（测试构建永不读
     开发者 config；生产启动路径不变），单线程/多线程双验证。副产物：win-skip
     登记**族U**（color_support 探测断言依赖宿主 TTY，GATE_FORCE 首跑暴露）。

## 三十七期：`/lsp select`——server 勾选面板（自动检测 + 用户勾选 + 确认生效）（2026-10-14，分支 feat/local-lsp-server-picker）

> 需求：三十五期 builtin 检测默认把 PATH 上命中的五语言 server 全部装配，但
> 用户当前项目未必全都要（非 Java 项目不想要 jdtls 等）。新增「检测 → 勾选 →
> 确认 → 热加载」交互：`/lsp select` 列出检测/配置到的全部 server，空格勾选本
> 工作区要启用的，Enter 确认；取消勾选的进本工作区禁用清单。

1. **持久化（shell `util/workspace_overrides`）**：feature-overrides 文件增
   `lsp_disabled_servers: Vec<String>`——**禁用清单语义**（只记排除项；空/缺省 =
   清除覆盖、恢复全量跟随检测与配置）。读写 API `read_lsp_disabled_servers(_in)`
   / `write_lsp_disabled_servers(_in)`；写入前去重排序（同选幂等、面板重复确认零
   写放大）；与 `lsp_tools` bool 键同文件互不覆盖，文件无活跃键时整删（沿既有
   惯例，read 侧对 JSON 损坏容错回退 None）。
2. **装配过滤（shell）**：`MvpAgent::build_lsp_backend`（session spawn 与
   `x.ai/lspApply` 热加载共用单入口）在 folder-trust 过滤后应用禁用清单——被禁
   server 无论来源（config/builtin/plugin）一律不装配，debug 日志留痕。
   `x.ai/lspStatus` 回包每 server 带 `disabled` 标志（面板初始勾选与 status
   展示同源；serde default 向后兼容）。
3. **pager**：`/lsp select`（别名 选择/勾选/勾选面板）→ `Effect::FetchLspStatus`
   （新 `select` 位）→ 新 `views/lsp_server_picker` 模态（↑/↓/j/k 移动、空格
   勾选、a 全选/全不选、Enter 确认、Esc 取消；无 server 时给空态文案）。确认 →
   `Action::LspServersConfirm{cwd, session_id, disabled}` → 写覆盖 → 零回合活动
   会话发 `x.ai/lspApply` 热加载（带回合由 shell turn_count 权威拒，改动对新
   会话生效）；无会话时写覆盖 + 提示「definition 构建时生效」；welcome/dashboard
   发起则提示先进会话。`/lsp status` 增「（已禁用 — /lsp select 勾选恢复）」标注
   与 select 提示行。
4. **i18n**：面板文案走 tr()（新键 4 条 + `toggle all`），hint 键（move/toggle/
   confirm/cancel）复用既有表项；扫描器对新文件的 plain 项 = hint 元组键（与
   auth_modal step_hints 同款已包裹口径，非裸文案）。

**验证**：cargo check shell/pager --lib 0 error（首查遇 rustc 0xc0000005 增量
缓存损坏，清 incremental + CARGO_INCREMENTAL=0 重查通过）；GATE_FORCE
shell `workspace_overrides` 6/6（含新禁用清单往返/去重幂等/与 bool 键互存两例）、
pager `lsp` 过滤 14/14（含 select 取数旗标、无会话提示、确认写清单 + 热加载
effect + 写后自清回收、勾选面板 toggle/边界/确认语义三例）；i18n 扫描 exit 0。
手工项（真机 TUI 面板开合 + 确认后热加载回路）列入遗留。

## 三十八期：`/lsp` 面板化——统一操作面板（状态显示 + 启用关闭 + 自动发现 + 勾选确认）（2026-10-14，分支 feat/local-lsp-panel）

> 需求：三十七期的 `/lsp select` 与文本 status、on/off 子命令割裂，用户要求把
> `/lsp` 收敛为面板操作：面板内完成启用/关闭、状态显示、自动发现（启用时
> builtin PATH 检测）与手动勾选确认。

1. **命令层（pager `slash/commands/lsp.rs`）**：裸 `/lsp`（及 `panel`/`面板`）
   改发 `LspConfigureRequest::Panel`（原 `Select` 改名并收编 select/选择/勾选
   为兼容别名）——回包走面板而非文本；`status|状态` 保留文本回显，`on|off|
   clear` 保留直接子命令。`enabled_source_label` / `startup_label` 抽成
   pub(crate) 共享（status 文本与面板渲染同源）。usage 改
   `/lsp [on|off|panel|status|clear]`。
2. **面板视图（新 `views/lsp_panel.rs`，替代 lsp_server_picker）**：
   `LspPanelState` 持有整份 `LspStatusResponse` 快照（门禁 + 来源 + 工作区
   覆盖 + server 清单 + 会话后端）。渲染 = 状态头（LSP 工具 启用/禁用 + 来源
   + 覆盖行；禁用时 warning 色提示「按 e 启用，自动检测已安装的语言服务器」）
   + server 勾选清单（[x]/[ ] + 来源 + 命令探测 + 运行态）+ 会话后端行 +
   六键 hint（e/↑↓/space/a/Enter/Esc）。按键路由增 `e|t` →
   `LspPanelOutcome::ToggleEnabled`（本地态先翻转，面板保持打开）；Enter 确认
   语义不变（全集 − 勾选集 = 禁用清单）。
3. **接线**：`ActiveModal::LspServerPicker` → `LspPanel`（改名，键路由/绘制
   分发/unreachable 组三处）；`FetchLspStatus`/`TaskResult::LspStatusFetched`
   字段 `select` → `panel`；router 增 Panel 臂（需会话内，welcome 提示语改
   「/lsp 面板需要在会话内使用」）；modals `apply_lsp_panel_outcome`：Toggle
   发 `Action::LspConfigure(Set(新值))`（dispatch 写 workspace 覆盖 +
   `x.ai/lspApply` 热加载，面板不关）、Confirm 走既有
   `Action::LspServersConfirm`。shell 侧零改动（数据面/写面/热加载面三期已齐）。
4. **i18n**：面板新键 16 条（状态头/覆盖三态/禁用提示/会话后端/运行态/启动
   阶段/enable-disable）；三十七期两条遗留键标注存档保留。

**验证**：cargo check pager --lib 0 error 0 warning（中途 E0425 导入缺失 +
E0716 tr 临时值借用 + 测试用例缺 TaskComplete 包装三处小修）；GATE_FORCE
pager `lsp` 过滤 16/16（原 14 例改名迁移 + 新增 panel 取数旗标/无会话提示/
回包开面板三例中的面板打开例并入计数）；i18n 扫描 exit 0（lsp_panel 无挂）。
遗留：真机 TUI 活体验证（面板开合、e 翻转后 status 同步、勾选确认热加载）。

### 三十八期补：面板二次收敛（usage 同款 chrome + 子命令全撤 + Clear 全量重置）（2026-10-14 同日，同分支 feat/local-lsp-panel；v1.0.57 tag 重指重发）

> 用户反馈三：面板太小、风格与 usage modal 不对齐、option 子命令残留。三点全收：
> v1.0.57 tag 不新增 v1.0.58，修复后原地重指重发（沿 v1.0.39 / v1.0.51 先例）。

1. **公共 chrome（views/modal_window）**：`render_lsp_panel` 改走
   `mw::render_modal_window`（与 usage/stats 同款 `ModalSizing`：width_pct 0.65 /
   max 100 / v_margin 2 / footer_lines 3，compact 模式同款收缩），标题入边框、
   七个快捷键入页脚（e/c/↑↓/space/a/Enter/Esc），`ModalWindowState` 挂进
   `LspPanelState`（关闭按钮命中区随 chrome 承担）；内容超长随光标滚动。
   自绘 Clear+Block 小窗与 hint_line 移除。
2. **子命令全撤**：`/lsp` 只开面板——`takes_args: false`，任何非空参数（含
   on/off/status/clear/select）一律 Error 并提示面板内完成；`status` 文本回显与
   `format_status`、`LspConfigureRequest::Status` 整体移除（enabled_source_label /
   startup_label 迁入 lsp_panel.rs）；`FetchLspStatus`/`TaskResult::LspStatusFetched`
   的 `panel` 旗标随之删除（回包恒开面板）。
3. **`c` 清覆盖（面板内新键）**：`LspConfigureRequest::Clear` 语义升级为全量
   重置——`write_workspace_override(None)` + `write_lsp_disabled_servers(None)`
   串联（恢复跟随全局配置与自动检测），回显与热加载路径复用。
4. **i18n**：新 description/usage 键 2 条（旧 on|off|status|clear usage 键删除），
   面板文案键沿用前版。

**验证**：cargo check pager --lib 0 error 0 warning（本轮三连 rustc 0xc0000005
分别打在 update/config-types/shell-terminal 三个未触碰 crate，清 incremental +
CARGO_INCREMENTAL=0 + 限 -j 4 后通过）；GATE_FORCE pager `lsp` 过滤 14/14
（status 文本 2 例与 panel 旗标例随面删除，新增 Clear 重置 outcome 与拒绝
子命令 2 例）；i18n 扫描 exit 0。
## 三十九期：Codex 品牌主题——OpenAI Codex CLI 真实 UI 配色移植（2026-10-14，分支 feat/local-codex-theme）

> 需求：引入 Codex 主题配色。捐献者 = 本地 clone `D:\CODE\ai\codex`（codex-rs/tui），
> 取其**真实 UI 配色体系**（style.rs / color.rs / markdown_render.rs / 内置 tmTheme）。
> **用户拍板：无障碍配色不要**——6 个内置 `.tmTheme`（ada/babbage/curie/cushman/dali/davinci）
> 是 256 色无障碍变体（金/橙 `codex.accent`、`#FF5F5F` 主前景），整族弃用；品牌识别
> 以 style.rs 的 ChatGPT 蓝体系为准。

1. **donor 色源（真实 UI，非无障碍板）**：`UI_ACCENT = CHATGPT_BLUE_200 #63A8F8`
   （picker 选中/正文强调）、`CHATGPT_BLUE_100 #A4CDFB`（选中填充）、
   `warning_notice` 深底琥珀 `(196,167,103)`、`secondary_text_style`（fg/bg 60%
   blend → 近黑底上 #999999）、`user_message_bg`（暗底白 12% 叠底）、tmTheme 暗底
   前景 `#D0D0D0`、comment `#808080` / punctuation `#BCBCBC`、`markup.inserted/
   deleted`（#00AF00/#FF5F5F）。codex 默认语法主题是 Catppuccin Mocha（fork 已有
   独立主题），不重复吸收。
2. **新文件** `theme/codex.rs`：~60 槽位完整映射（编译器强制）。直采槽位如上；
   donor 未定义的（bg 深/浅梯度、hover、边框细档、verify、md 六级标题梯）按 Nord
   先例在「ChatGPT 蓝 + 中性灰」色系内手调派生并文件头注明；md 标题 modifiers
   对齐 codex markdown_render（h1 bold+underlined、h3 bold+italic）。底色取
   `#141414` 近黑中性（codex 跟随终端底色的品牌观感），`bg_light = #303030`
   即 codex 用户消息块白 12% 叠底的实算值。签名测试（bg_base/accent_user/
   bg_light 原值 + is_dark + display_name）。
3. **注册接线（同三十六期八处，+1 变体 `Codex = 18`）**：`theme/mod.rs`（mod 声明、
   ThemeKind、ALL、display_name、aliases 空、requires_truecolor truecolor 门控、
   `current()` 分发、display_name_for_canonical）；`theme/cache.rs` 解码臂；
   `syntax.rs` 深色臂并 grok-night.tmTheme；pager `settings/defs.rs` 两 catalog
   +1；`slash/i18n.rs` 中文描述 +1；`views/settings_modal/tests.rs` 分发臂 +1；
   doctor 夹具 17→18（doctor_cmd/tests.rs 三处、diagnostics/doctor_format_tests.rs
   一处）；user-guide 06-theming 表 +1 行（17→18）。mermaid `theme_is_dark` 是
   浅色负名单制，Codex 为深色，零改动。
4. **语义对照**：`theme_is_dark`/量化/Windows 对比度增强/ANSI16 覆盖等泛化路径
   自动收编（遍历 ALL 的既有测试自动覆盖新变体）。

验证：`cargo check -p xai-grok-pager-render -p xai-grok-pager --all-targets`
0 error 0 warning；GATE_FORCE 合批测试（render theme 179/0 + pager theme/settings/
doctor/mermaid 526/0；settings 单独 380/380 对齐三十六期基线）；i18n 扫描新增
描述已带 zh 对，0 新挂。**运行备注**：合批首轮暴露 `clamped_preview_renders_
note_below_content` 一挂（note fg 拿到 Reset）——复判为既有全局主题测试隔离
flake（`info_line_chrome_is_muted_on_terminal_theme` 的 native-lock 窗口与
settings preview 并行互踩；三十六期按过滤分开跑故未共现），同命令重跑 526/0
零改动自愈，与 Codex 主题无关。活体 ptyctl 抽查随新客户端。

## 四十期：Ayu 主题——移植自 qwen-code 主题定义（2026-10-14，分支 feat/local-ayu-theme）

> 需求：qwen-code 侧调研可引入主题。捐献者 = 本地 clone
> `D:\CODE\ai\qwen-code`（`packages/cli/src/ui/themes/ayu.ts`，社区 Ayu dark）。
> **用户拍板「避免同质化」**：该库候选逐一对照现有 19 主题筛查——`qwen-dark`
> 是 ayu 的同源克隆（采样 8 色中 7 色相同，仅一强调色之差）、`shades-of-purple`
> 将成第三紫调（Dracula/OscuraMidnight 之后）、`github-dark` 与 One Dark/Codex
> 同族、`googlecode`/`xcode`/各 `*-light` 浅色已饱和（GrokDay/Latte 两个）、
> `ansi`/`no-color` 被 `terminal` 覆盖——**唯一非同质化引入 = Ayu**，填
> 「暖橙日落深色」空位（Vesper 的琥珀极简之外的首个全彩暖橙主题）。

1. **新文件** `theme/ayu.rs`：donor 直采——Foreground `#AEACA6`→text_primary、
   LightBlue `#59C2FF`→accent_user、AccentBlue `#39BAE6`→assistant/running/link、
   AccentPurple `#D2A6FF`→verify/model、AccentCyan `#95E6CB`→skill/md_code、
   AccentGreen `#AAD94C`→success/diff_insert_fg、AccentYellow `#FFB454`→
   command/path/warning/plan（暖橙辨识位）、AccentRed `#F26D78`→error/
   diff_delete_fg、Comment `#646A71`→gray、DiffAdded/Removed `#293022`/
   `#3D1215`→diff 底色；底色梯度/边框细档/md 标题梯按 Nord 先例蓝黑色系内
   手调派生（行内注明偏移比例）。签名测试（bg_base/command 原值 + is_dark +
   display_name）。
2. **接线（同三十九期八处 +1 变体 `Ayu = 19`）**：mod.rs（含滚动条对比度测试
   分发臂）、cache.rs 解码臂、syntax.rs 深色臂、settings defs 两 catalog、
   slash i18n 中文、settings_modal tests 分发、doctor 夹具 18→19（4 处）、
   user-guide 表 19 行；mermaid `theme_is_dark` 浅色负名单制零改动。别名
   `ayu-dark`。

验证：cargo check render/pager --all-targets 0 error 0 warning；GATE_FORCE 合批
测试（render theme 180/0 含 ayu 签名测试 + pager theme/settings/doctor/mermaid
526/0；settings 单独 380/380 对齐基线）；i18n 扫描新增描述已带 zh 对，0 新挂。
**附带根除既有 flake**：合批首两轮复现三十九期记过的 `clamped_preview_renders_
note_below_content` 竞态（三十九期「重跑自愈」判断不准——ayu 分支连挂两次），
根修 = settings_modal 59 个「渲染后对照 Theme::current()」测试统一补裸
`test_lock()` 守卫（只串行化不改状态，已持锁的 2 个 pin_theme 用例跳过），
读侧与全部改状态者共享同一把锁后竞窗结构性消除；fix 提交先行（f1de484），
feat 随后。活体 ptyctl 抽查随新客户端。合并在 codex 主题（三十九期）之后——
分支叠建，接线点编号顺延无冲突。

## 四十一期：模型组（跨供应商降级链）+ subagent 模型/effort 配置（2026-10-14，分支 feat/local-subagent-model-group）

> 需求：内置 subagent 可配 model 与推理深度；同一 model id 来自多供应商，要能精确
> 指定供应商条目并做降级链（A 家 flash 不可用换 B 家）。设计文档
> `docs-local/subagent-model-group-todo.md`（六拍板点按建议值落地）。Ayu 主题分支
> （三十九/四十期）按用户指示随本分支一起合并发版。

1. **配置层**：新顶层 `[model_groups.<name>]`（`ModelGroupConfig`：`entries` 有序
   降级链引用 `[model.<id>]` 条目 id、条目级 `effort` 覆盖、`sticky`（缺省 true，
   v1 语义=组解析跳过冷却条目）、`cooldown_minutes` 缺省 10）；`[subagents.model_groups]`
   （agent 名 → 组名，与 `[subagents.models]` 同 agent 并存时 pin 赢 + load 时 warn）、
   `[subagents.efforts]`（agent 名 → effort）。`SubagentsConfig::resolve` + Config
   字段（`#[serde(skip)]` 三张表 + `model_groups` 反序列化）。
2. **解析链**：`resolve_subagent_sampling_config` 优先级扩为 spawn runtime override >
   `[subagents.models]` pin > `[subagents.model_groups]` 组链 > `AgentDefinition.model` >
   继承父会话；组分支按序跳过冷却条目（进程级 `ModelGroupRuntime` OnceLock 冷却表，
   懒回收过期）与不可解析条目，全不可用 fall through + warn；返回值增
   `ModelGroupBinding{group, entry_idx, cooldown_minutes}`。组条目 `effort` 应用带
   能力门（`model_supports_reasoning_effort`）与 variants 路由（`model_for_effort`），
   低于 spawn runtime/`[subagents.efforts]`（后者在 `run_shell_child` 里
   fill-if-none 接到 `effective_runtime.reasoning_effort` 链，优先级 spawn > role/persona
   > config efforts > 全局默认）。
3. **降级运行时（v1 = spawn 级）**：子任务尝试失败且错误串命中 provider-unavailable
   保守签名（429/rate limit/5xx/connection/timeout/quota；400/401/403 一律不触发）
   → 用过的组条目进进程冷却（`unified_log::warn "subagent model group: entry marked
   cooling"` 带组名/下标/错误预览），后续 spawn 解析自动顺延下一可用条目（非首选
   条目命中时 `unified_log::warn "resolved past the first entry"`）。会话中途换条目
   （首字节前 failover）与跨会话粘性留 v2；设计文档已记实况偏差。

**验证**：`cargo check -p xai-grok-shell --all-targets` 0 error；新增
`model_group_tests` 5 例（候选序/全冷却/冷却互不串组/重复标记/分类器正反例）+
`config/tests` 2 例（三张新表解析 + `[model_groups]` 形状/缺省）+
`tests/rest.rs` 组链优先级 1 例（首选→冷却跳条目→全冷却 fall through 到
AgentDefinition）；既有 `runtime_override_wins_over_subagents_models_pin` 等
5 处解构随三元组返回适配。GATE_FORCE `--lib group` 23/24——唯一挂
`auth_error_no_retry_tests::model_switch_preserves_existing_conversation_group`
为既有环境族（夹具写死 /tmp，NotAbsolute panic，与本改动无关），登记
win-skip **族V** 后 23/23 全绿（含既有 conversation_group/auth 用例）。

## 四十二期：Deep Dark 主题——Claude Code 泄露源码 darkTheme 移植（2026-10-15，分支 feat/local-claude-theme-port）

**来源与判定**：Claude Code v2.1.88 经 npm `cli.js.map` 意外泄露（2026-03-31），
主题定义取自公开存档仓 `qw0258/claude-code-v2.1.88` 的
`src/utils/theme.ts`（`darkTheme`）。六内置主题四档判定：dark ✓（Claude 赤陶橙
`#D77757` 与现有 20 主题零撞车）、light ✓ 次选（本轮未做）、daltonized 两档可选
（success→蓝/diffAdded→深蓝的色盲安全变体）、ansi 两档 ✗（`Theme::quantized`
自动降档已覆盖，同 Codex 期弃 256 色板先例）。预研记录
`docs-local/claude-theme-port.md`（含 light/daltonized 的后续映射草案）。

- **`theme/deep_dark.rs` 新文件**：~64 槽完整映射（编译器强制）。直采槽：
  claude→accent_assistant、briefLabelYou→accent_user、claudeBlue（系统
  spinner）→accent_running/accent_system、permission→accent_model/path/
  fuzzy_accent/accent_remember（donor remember 同值）、planMode→accent_plan、
  merged→accent_verify、background→accent_skill/md_code、success/error/
  warning 语义三件套、bashBorder→command（Claude 的 bash 消息边框粉）、
  ide→link_fg、diffAdded/Removed + Word 对 → diff 四槽、userMessageBackground
  (+Hover)→bg_light/bg_hover、messageActionsBackground→bg_highlight、
  memoryBackground→bg_visual、selectionBg→selection_border、promptBorder/
  inactive/subtle/promptBorderShimmer→灰阶四件。donor 无终端背景槽（跟随终端），
  bg_base 取 #141414（拍板默认）；deep_dark 的 `running` 槽用 claudeShimmer
  （运转中=品牌色闪烁语义）；md 六级标题梯/边框细档在品牌橙+蓝紫色系内派生。
- **命名**：用户拍板 `deep-dark`（`ThemeKind::DeepDark = 20`），别名
  `claude-dark` / `cc-dark`。
- **注册接线九处**（同 Ayu 期清单）：mod.rs（模块声明/枚举/ALL/display_name/
  truecolor 门/aliases/canonical 显示名/apply_kind/测试分发）、cache.rs 解码臂、
  syntax.rs 深色臂、settings defs 两 catalog、slash i18n 中英文、settings_modal
  测试分发、doctor 夹具 19→20（3 处）、user-guide 06-theming 表 14 行；
  mermaid theme_is_dark 负名单制零改动（深色默认收编）。

**验证**：cargo check render/pager --all-targets 0 error 0 warning（首查 pager
挂 i18n 表项嵌套括号，E0308 修正后绿）；GATE_FORCE render theme 175/0、
pager theme 76/0、settings 380/0（对齐 Ayu 基线）、doctor 42/0（首跑两轮
rustc 0xc0000005 增量缓存损坏，清 incremental + CARGO_INCREMENTAL=0 后绿）；
i18n 扫描 exit 0。

## 四十三期：空闲时自动压缩主上下文——实验特性 `[features] idle_auto_compact`，默认关（2026-10-15，分支 feat/local-idle-auto-compact）

**需求原话**：实现空闲时自动压缩主上下文，默认不启用，用户可通过配置文件开启
这个实验性特性（参考 Codex / Claude 的 auto-compact 设计）。

**现状与定位**：fork 已有 inline auto-compact（回合内 pre-sampling 阈值检查 →
`run_compact_only`，含两段式/prefire、抑制分级、cancel gate）。本特性把**同一套**
压缩挪到回合间隙的空闲点提前跑：上一回合结束时上下文已过阈值，就在空闲期把
摘要做完，用户下一条 prompt 不再内联付摘要延迟。inline 检查原样保留作 backstop。

- **配置**：`[features] idle_auto_compact = true` / env `GROK_IDLE_AUTO_COMPACT`
  （env > config > 默认 false）。键登记 `UNMIRRORED_BOOLEAN_FEATURES`（typo 检查）；
  config_docs `unmirrored_requirements` 映射「—」；user-guide 26 补行。
- **运行态**：`CompactionConfig.idle_compact_in_flight: AtomicBool`（12 处构造点
  同步）；`check_auto_compact_needed` / `check_preflight_overflow` 置位时早退
  （inline 与 idle 双压缩互斥）。
- **触发**：run_loop 回合完成后 `turn_ran && turn_succeeded` 门（同 turn-summary
  成功门：cancel/失败回合后不发模型调用）+ 非 subagent 会话，spawn_local
  `maybe_run_idle_auto_compact`（`session/acp_session_impl/idle_compact.rs` 新模块）。
- **防抖与中止**：3s 宽限轮询共享 idle 谓词（`is_session_idle_for_injection`）；
  达阈值（复用既有 `check_auto_compact_needed`——is_flushing/抑制/force_compact
  语义全继承）才发 `run_compact_only`；压缩期间 select! 持续轮询 busy，**一忙即弃
  未来**——RAII cancel scope 回卷，与用户 stop 同一取消语义，并发送
  `AutoCompactCancelled` 清 pager 活动位。不引入任何新的用户可见开关面。
- **可观测**：unified 事件 `shell.idle_compact.fired/.aborted/.skipped`。

**验证**：cargo check shell --all-targets 0 error 0 warning；新测 5 例
（fires/skips/below_threshold/disabled/in-flight 互斥）落在既有
`compaction_inline_auto_compact_flow_tests` 模块（win-skip 族 /tmp 夹具，本机跳过，
Linux CI 跑）；config_docs 行注册测试绿（--features config-docs）。

**CI 修复四轮（2026-10-16）**：build `38768559297` Linux 6 挂——① idle 自锁：
idle 任务置 `idle_compact_in_flight` 后调 `check_auto_compact_needed` 被自己的
守卫拦住，拆 `check_auto_compact_needed_inner` 让 owner 走 inner 绕开守卫
（`60da10b`）；② 中止测试信号修正：`state_is_busy` 不含
`notifications_suppressed`，原 abort 信号打不进中止臂，改推 pending_inputs
（`c335b26`）。build `38773838791` 2 挂（其一为 `reset_title_to_auto…` 既有计时
flake 重跑过）——③ fires 测试：`replace_conversation` 按消息字节估算
`reseed_total_tokens` 把 180k 种子抹掉致阈值检查永不过，replace 后
`record_token_usage(180_000)` 重播种（`1064109`，绝对置位 mutations.rs:438）。
build `38823680111` 仍挂 fires "got 4 items"——④ 真根因是断言语义错：
`build_compacted_history`（xai-chat-state compaction_utils.rs:936）无条件产出
[System, user_meta(prefix), 重挂 last query, summary] 恰 4 条，`len < 4` 恒假
且无法区分「未压缩/压缩成功」两态；分支采样诊断（Started+Completed+checkpoint
全到、cancelled=false）证实压缩全程成功，断言改摘要文本语义（mock 摘要必须出现
在对话中）。**夹具副产**：`create_test_actor` 的 cwd 写死 `/tmp` 被 Windows
AbsPathBuf 拒绝（族S 根因），改 `cfg!(windows)` 分支用 `env::temp_dir`，模块
本机 GATE_SKIP=0 全跑 42/42 绿，win-skip 族S 撤销
`session::acp_session::compaction::inline_auto_compact_flow_tests::` 条目
（其余两族条目保留）。build `38894033624` 双 job success（`e63a954`）。

## 四十四期：视觉桥 T1/T2/T3 + D8——纯文本主模型经识图模型转写用户图/工具图（2026-10-15，分支 feat/local-vision-bridge）

**需求**：`docs-local/vision-bridge/plan.md`（D1–D9 拍板定案）整期落地：部分文本
模型不能读图，用一个识图模型转写，原图不进文本 provider 的请求。D9 二级
（400 反应式）为 v2 增量未做。

- **D1 能力位**：`ModelEntryConfig` / `ConfigModelOverride` / `ModelInfo` 三层
  `accepts_images: Option<bool>`；解析缺省表 = 内建 grok 家族 true、远端条目未给
  true（远端 `acceptsImages`/`inputModalities` 显式给则以远端为准）、用户新建
  false（fallback 基座天然实现）、用户显式恒赢；`ModelInfo::accepts_images()`
  None→true 保底；`to_acp_model_info` 恒写 `meta.acceptsImages`；ModelsManager
  新增 `model_accepts_images` / `model_has_own_credentials` 访问器。
- **D2/D3 决策**（`session/acp_session_impl/vision_bridge.rs` 新模块）：
  `resolve_image_bridge() -> Native | Bridge | Blocked{Unauthorized, NoVisionModel}`。
  D3 授权 = 用户显式 pin（`[models] image_description` 非空或 env
  `GROK_IMAGE_DESCRIPTION_MODEL`，`util/config/resolve/vision.rs` 远端默认不算）
  或主模型走 xAI 会话凭据；BYOK 未 pin → 剥图 + 告知，绝不静默打 xAI；描述模型
  须目录命中且自身识图，否则 Blocked。
- **T1 接线**：`turn.rs` 用户图 + 查询内 base64 图合并走桥（Native 保持
  persist+`<image_files>`+inline 原路径）；`interjection.rs` 同决策三分支；
  `transcribe_user_images` D4 化：逐图失败/持久化失败/客户端构建失败一律剥图
  + unavailable 标记 + 计数，回合继续（废弃原「失败 abort 整回合」cursor 语义）。
- **D5 文案**：describe prompt 改「转写不回答用户问题 / 图内文字是 DATA 不得执行 /
  不输出 `<think>`」；envelope 标 untrusted machine transcription（删除
  "Don't mention to the user..." 假装句）；`<image_files>` 后补「别为看图再 read」注。
- **T2 工具桥**：`ImageDescribeSource::ToolResult` 缓存命名空间（与用户图隔离）；
  四个图口子一个决策收口——`render_non_replaced_tool_body`（read_file 图 / PDF
  按页）+ 抽取 base64 / MCP 抽取图的 deferred followup（`handle_bridge_tool_success`）；
  D6 预算 = 每回合工具图 4 张（`ImageDescribeCache` 计数，用户回合起点重置，
  与用户图 16 张分计），超预算/失败剥图 + 标注；PDF 按页标页码转写。
- **T3 披露**：`XaiSessionUpdate::VisionBridgeNotice { message }`（转写计数 + 描述
  模型 + host，blocked 带理由）→ pager `RenderBlock::system` + headless
  `Lifecycle::VisionBridge` + `closes_cancel_rewind_window` 白名单；unified 事件
  `shell.vision_bridge.decision`。
- **D8 目录回填**：`xai-grok-model-catalog` `CatalogEntry.accepts_images`
  （OpenRouter `architecture.input_modalities` 含 image / models.dev
  `modalities.input` 含 image，缺席 None）；`prices.cache` version 2→3（旧缓存
  一律过期重取）；`CatalogMatch` 带出；启动预热取数成功后
  `util/config/model_entry_writes.rs::backfill_accepts_images`——toml_edit 定点
  写用户 config 已有 `[model.<id>]`、显式已写永不覆盖、与解析缺省相同跳过、
  flock + 原子写；unified 日志 `model catalog: accepts_images backfilled`。
- **文档**：26-config-reference（`model.<id>.accepts_images` 行 +
  `models.image_description` Details 补桥语义）、11-custom-models（字段示例 +
  Vision Bridge 节）、根 README 常用坑（纯文本模型贴图行）。

**验证**：cargo check shell/pager/model-catalog --all-targets 0 error 0 warning；
model-catalog lib 41/41（双源 accepts_images 解析断言）；shell GATE_FORCE 合批
vision.rs 3/3 + D1 缺省表 4/4 + D5 envelope/image_describe 存量绿 + config_docs
行校验 2/2；pager 合批 36/36（model_catalog_store/session_notification/
auth_modal/stats_modal）；i18n 扫描 0 挂。win-skip 新登记族W（vision_bridge
决策测试 /tmp 夹具）、族X（image_describe 路径分隔符，上游既有）、族Y（上游
per-crate AGENTS.md 缺席）——三者均 Linux CI 验证。

**CI 修复（2026-10-16，build `38768559297`）**：vision 决策测试 3 挂——
`create_test_actor` 内部 `spawn_local` 需 LocalSet 上下文，`decision_for` 包
`tokio::task::LocalSet::run_until` 修复（`4a2ddbf`）；二轮起全绿。族W/X 本机
跳过项经 Linux CI 验证绿。

## 四十五期：`/stats` 压缩标签页换月度统计——4 时间窗（5h/day/week/近一月）（2026-10-17，分支 feat/local-stats-month-tab）

**需求**：用户口述「/stats 面板的压缩换成月度统计」——工具输出压缩账本标签页
退场，第 4 标签页改为月度用量统计。恢复 df81acf 原始月窗（滚动 30 天，
`WINDOW_MONTH_MS`；2e35532 面板化时按需求移除），`RETAIN_MS=35` 天本就是为
月窗预留的剪裁窗口（比月窗长 5 天，窗沿不因剪裁抖动丢数据）。压缩账本数据面
不动：headless `grok stats --json` 的 `tool_output_compression` 字段与
`stats_cmd/display.rs` 渲染保留（独立机器面）。

- **model_usage_ledger**：`Window` 加 `Month`（arg `month`、label `Last month`、
  `WINDOW_MONTH_MS = 30 天`滚动窗）；`ALL` 3→4；月窗边界聚合用例 + 保留窗口
  覆盖断言补齐。
- **stats_modal**：删 `StatsTab` 包装枚举（Compression 退场后唯一使命消失），
  `active_tab` 直用 `Window`；`StatsModalState` 丢 `compression` 字段；
  页脚 `a all models` 快捷键恒显（原 Compression 页不折叠才隐藏）；月窗与其它
  时间窗共用同一套卡片/合计渲染。
- **dispatch_show_stats**：不再物化压缩账本行，`StatsModalState::new(tab, samples)`
  双参。
- **slash/commands/stats.rs**：usage 与未知参数报错改 `[5h|day|week|month]`；
  参数补全加 month（`Last 30 days`/近 30 天）；极简文本报表 = 全 4 窗去压缩段；
  命令描述去「压缩收益」（`Show model usage stats`/查看模型用量统计）。
- **i18n**：+`Last month`/近一月、+`Last 30 days`/近 30 天；描述词条改写；
  −`Compression` 词条（唯一消费方已删）。
- **README**：/stats 交互节加「月」档；本地命令列表去「压缩收益」。

**验证**：cargo check -p xai-grok-pager --all-targets 0 error 0 warning；
GATE_FORCE `ctest.sh -p xai-grok-tools --lib model_usage_ledger` 13/13；
门控 `ctest.sh -p xai-grok-pager --lib stats` 38/38（stats_modal 17 +
slash::commands::stats 8 + stats_cmd 9 + 同名杂项 4）；i18n 扫描 exit 0
（报告 i18n-sweep-classified-2026-10-17.txt）。

## 四十六期：/model picker 右侧目录详情列——模态/上下文/思考/价格（2026-10-17，分支 feat/local-model-picker-catalog-details）

**需求**：用户口述「/model 选模型时，优化下右边的空白区域，显示模态上下文，
是否支持思考」「还有定价」——模型行右侧空白列（原 right_label=description，
模型条目常为空）改显目录四字段。

- **model_catalog_store**：新 `picker_detail_label(meta, matched)` 组合器 +
  `fmt_tokens`（999/256k/1M）+ `fmt_price`（≤2 位小数去尾零）。token 串
  「模态 上下文 思考 价格」单空格拼接（`文本 256k reasoning $3/$15`），未知段
  逐段省略；模态=本地 meta `acceptsImages`（视觉桥 D1）优先、目录兜底；上下文
  本地 `totalContextTokens` 恒在恒优先（拍板 3：本地为准）；思考=本地
  `supportsReasoningEffort` 或目录 `reasoning==Some(true)`，只标支持；价格只来
  自目录，已知无命中（负缓存）显式 `–`（与 /stats 成本列同口径），本进程未取到
  先不显示——回包落地后下一帧自动补显（渲染时查进程级 store，无需状态接线）。
- **modals.rs ArgPicker 渲染**：`command=model|m && args_query 空`（模型阶段）
  时按行 `match_text`（=info.name）映射回 `models.available` 取 id，查 store
  组装 right_label；effort 阶段与其他 ArgPicker（theme 等）原样。
- **取数触发**：新 `Action::FetchModelCatalogForPicker`（actions/router 接线），
  两条打开路径（input.rs ModelPicker 快捷键、modals.rs /model 输入流）开 modal
  后返回该 Action（FetchSessionList 先例：modal 已开、Action 只补取数副作用）；
  router 对当前 agent 全部模型 id 过 `ids_needing_fetch`，单飞+负缓存由 store
  把关，复用既有 `Effect::FetchModelCatalog`。
- **description 注入 + 启动预取（活体验证补强）**：`build_model_items` 把详情
  token 串拼进 ArgItem.description（`{detail} · {原描述}`）——dashboard `/model`
  下拉与会话内下拉（`SuggestionRow::from_arg` 复制 description）即见四字段；
  ArgPicker 渲染时查 store 保持更鲜活。pager 侧进程 store 由「模型列表到达即
  预取」加热：`prefetch_effect()`（store 负缓存+单飞+6h 新鲜度把门）接三处
  lifecycle/load 模型安装点 + `x.ai/models/update` 通知路径（`pending_effects`
  通道）——补齐此前只有 /stats、/auth、ArgPicker 三个触发点、dashboard 下拉
  永远冷 store 的缺口（shell 启动预热的 store 在 shell 侧，pager 读不到）。
- **i18n**：+`vision`/视觉、`text`/文本（`reasoning`/推理 复用既有键）；
  组合器内字面量直进 `tr()`（扫描器可识别形态）。
- **文档**：user-guide 04-slash-commands /model 节补详情列说明；
  model-picker-catalog-details-todo.md 补施工实况（T1 落地，T2 剩余窄终端主动
  折叠与 stale 标注未见需要）。

**验证**：cargo check -p xai-grok-pager -p xai-grok-sampler -p xai-grok-sampling-types
-p xai-chat-state --all-targets 0 error；GATE_FORCE 四 crate lib 批量过滤 65/65
（pager 36 含 model_catalog_store 9、sampler 26、sampling-types 3）+ 增项
build_model_items 注入 1/1；ptyctl 活体见 roadmap §6；i18n 扫描 exit 0
（新键全 wrapped）。

## 四十七期：视觉桥 D9 tier-2——400 反应式摘图转桥（2026-10-17，分支 feat/local-vision-bridge-d9）

**范围**：v1.0.60 视觉桥（四十四期）遗留的 D9 第二级——能力位不可判定（目录
miss + 用户未写，走了缺省 true）时请求照发，上游 400 且签名可高置信归因
「不支持图像输入」→ 本回合摘图转写重发；置信不足的 400 一律不动。三级拓扑的
一级（前置判定）与三级（终态人工告知）四十四期已覆盖。

- **sampling-types**：① `SamplingError::is_image_input_unsupported_error()`——
  Api 400/415 且消息同时命中图像 token（image/multimodal/多模态/图片/图像）与
  不支持 token（not support/unsupported/不支持），中英网关方言归一；StreamError
  显式 false（置信不足不动）。② `ConversationRequest.image_bridge:
  Option<ImageBridgeHook>`（`Arc<dyn Fn(urls) -> BoxFuture<notes>>`，手动
  Debug/Clone）+ `image_urls()`（无突变收集）+
  `strip_images_with_transcriptions()`——user 图原位换 envelope 文本（无 note
  保持既有 IMAGE_STRIP_PLACEHOLDER），tool 图丢弃、note 追加进 tool 文本
  （front path T2 同款 render_image_description_block 语义）。
- **sampler**：`classify_error` 新签名 → 既有 `RetryWithImageStrip`；
  `strip_reason_for_image_error` 归 `ServerRejected`（服务器确认级）。重试臂：
  ServerRejected 且 hook 在场时 `image_urls()` → hook 转写（`tokio::select!`
  与取消令牌互斥）→ 转写注入后重发；PayloadHeuristic（413/连接重置）维持纯剥图。
- **shell**：`vision_bridge.rs` 新 `resolve_reactive_image_bridge(_with)`——
  eligibility 与前置门同构（只挂 `accepts_images==true` 的模型 + D3 授权 +
  描述模型自身识图；Bridge/Blocked 永不挂），D3 explicit 位可注入（测试不读真实
  config）。hook 执行体：data URL 解码（空 mime 回落 image/png）→ 共享
  `ImageDescribeCache::get_or_describe`（UserAttachment 命名空间，与 front path
  同图共享缓存）→ D5 envelope；失败（解码/描述）返回 None（D4：占位继续，
  provider 错误只进日志）。成功发 transient `VisionBridgeNotice`
  （rejected → transcribed with {model} ({host})）+ unified 日志
  `shell.vision_bridge.reactive`。turn.rs 主回合请求逐循环装配。
- **chat-state**：request_builder 请求体补 `image_bridge: None`（chat-state 恒不
  置钩子，shell 装配）。
- **持久化语义**：ServerRejected 在 Completed 后仍从存储历史剥图 + 既有提示
  （不变）；转写文本只进本回合重试请求（D9「重发本回合」字面范围），存储历史
  不留转写——已知取舍：历史留像素会让后续每请求重吃 400。
- **win-skip 族W**：`recap_display_only_tests::`（strip_reason 过滤词首跑暴露的
  /tmp 夹具环境族，与本次改动无关）。

**验证**：cargo check 四 crate --all-targets 0 error；sampling-types lib 3/3
（签名表 12 断言 + strip/notes 两用例）；sampler lib 26/26（classify/reason 归类）
+ 集成 test_actor 2/2（假服务器全链路：签名 400 → hook 转写 → 重试体含
envelope 不含像素；既有 invalid_image 回归绿）；shell vision_bridge 模块
GATE_SKIP=0 直跑 6/12——6 纯函数全绿（含新 decode_data_url），6 决策矩阵/
reactive eligibility 为已登记 /tmp 夹具环境族（族W 同根因，Linux CI 验证）。

## 四十八期：命令提示跟随实际二进制名——resume/leader kill 两处（2026-10-17，分支 feat/local-resume-hint-bin-name）

**需求**：用户口述「`grok --resume <id>` 这里的 grok 没有和进程二进制名同步」，
要求「用户任意修改二进制名也能正确适配」。发行名是 `grok2`，用户面提示里写死
`grok` 的命令示例照抄即错。

- **helper**：`app::cli::current_binary_name()`——argv[0] 文件名（`.exe` 后缀
  剥除、空缺回退 `grok`），加 `display_name_from_argv0` 纯函数 5 断言单测。
- **session_title_resolve**：多会话同名匹配报错里的
  `Resume by session id instead: grok --resume <session-id>` 改用 helper。
- **startup_failure**：启动失败屏 `NextStep::RestartSharedLeader` 的
  `grok leader kill` 改用 helper（`command()` 返回 `Option<String>`）。
- **遗留（open，见 issues/2026-10-17-binary-name-in-command-hints.md）**：
  diagnostics/fix.rs 的 `grok doctor fix` / `grok wrap ssh` 面板文案与 doctor
  写入 shell 配置的 alias 常量——别名参与幂等检测与配置产物，需「写入名 +
  检测容忍历史名」一起换算，单独立项（P3）。

**验证**：cargo check -p xai-grok-pager --all-targets 0 error；
ctest pager lib `display_name_from_argv0` 1/1。

## 四十九期：DeepSeek 官方 API 余额上 /usage 面板（2026-10-17，分支 feat/local-deepseek-balance-usage）

**需求**：用户点名「给 deepseek 官方 api 对应的 model id 集成余额查询」，官方
接口 https://api-docs.deepseek.com/zh-cn/api/get-user-balance。本机 config 唯一
命中 `deepseek/deepseek-v4.1-flash-1017`（base_url `https://api.deepseek.com/v1`）；
Ark / Command Code / ClinePass 的 deepseek 系列 id base_url 各指网关，host 匹配
天然不命中。调研文档 §「官方 API key 直连」当时已预留该落点（余额是金额，
不是百分比）。

- **provider-usage（第六家）**：`UsageProviderId::DeepSeek`（配置值 "deepseek"，
  显示名 DeepSeek）+ `UsageWindowKind::Balance`（rank 殿后）。`providers/deepseek.rs`：
  `GET /user/balance`（Bearer 同 key，origin 跟随 deepseek.com base_url host、
  推理路径 `/v1` 剥除）→ 每币种一条 Balance 窗口，`used_pct` 恒 None，金额进
  `note`（`granted_balance`>0 附 `· grants`；`is_available=false` 追加
  `· unavailable`，对齐 Command Code `limited` 先例；`topped_up_balance` =
  余额 − 赠金不重复展示）；`balance_infos` 缺失/空/无可用条目 = Parse 错误。
  错误信封 OpenAI 风格 `/error/message`（与 OpenCode Go 共臂）。
- **pager**：块标题供应商分流（DeepSeek → `tr("Balance")`「余额」，其余供应商
  仍「Plan usage」）+ 余额窗渲染（金额行 plain、无 bar、无 unknown 行；窗口
  裁剪规则不受影响——Balance ≠ Monthly 恒不裁）；i18n 增 `("Balance","余额")`。
- **shell 扩展零改动**：`x.ai/providerUsage` 通用管线直接吃第六家（判定/TTL
  缓存/(供应商, SK 指纹) 单飞/错误分层全部复用）。

**验证**：cargo check provider-usage/pager/shell --all-targets 0 error 0
warning（11m33s）；provider-usage lib 63/63（新增 9：官方文档逐字夹具/多币种/
unavailable/未知币种回退/空 infos/401 信封/500/origin 跟随/ALL 往返）；pager
usage_modal 过滤 25/25（新增 DeepSeek 渲染断言：余额标题 + 金额行 + 无 bar 无
unknown 行）；i18n 扫描 exit 0。**活体验证（ptyctl 真 TUI + 真 API，release
bin 核对 exe mtime）**：/model 切官方 1017 → /usage 面板「余额 — DeepSeek /
余额 / ¥99.89 / 查询于 10-17 19:50」；切回 Ark 回归 SuperGrok 周限额块零变化。

**发版补记（四十九期）**：四十六/四十七/四十八/四十九期四项同批随 tag
`v1.0.62` 发版（2026-10-17）：release run `39033051450` / main build run
`39030472409` 双 job success、4 资产非草稿；本机部署
`grok 1.0.62 (6d6161809c5c)`（D:\tool-cli\grok2，rename-aside 替换），回滚点
`grok2.exe.1.0.61-acc31ae.bak`。

## 五十期：DeepSeek hosted-search 供应商分流——api.deepseek.com 模型剥 x_search（2026-10-18，分支 feat/local-deepseek-hosted-search-gate）

**需求**：DeepSeek 内置 web_search 接线 handoff 任务单 ②（responses 通道，用户
拍板「分期：本轮 responses，anthropic 另立项」）。评估文档
`docs-local/deepseek-responses-builtin-tools-eval.md` D1/D2a 落地：builder 对
backend search 一向供应商盲双发 `WebSearch`+`XSearch`（`x_search` 是 xAI 专有
类型，DeepSeek 官方 tools 表对未知类型「忽略」——无害但脏），把声明面按供应商
对齐；启用位 `supports_backend_search` 语义保持「用户显式开启」，不自动。

- **落点（sampler / sampling-types / agent / pager 全零改动）**：
  `SessionActor` 增 `hosted_search_vendor: Cell<HostedSearchVendor>`
  （`Xai`（缺省，维持双发）｜`Deepseek`）；spawn 按 `sampling_config.base_url`
  host 判定、模型切换在 `supports_backend_search` 同点随采样配置更新（unified
  日志 `backend_search: model switch` 加 `hosted_search_vendor` 字段）；
  `resolve_hosted` 在 Deepseek 档剥 `XSearch`——该函数是全部上线路径的唯一漏斗
  （turn / side_call / recap / compaction 均经 `hosted_tools_for_turn`），
  主会话与子代理会话各自按自己的 base_url 过滤，天然模型切换正确。
- **host 判定复用先例**：`xai_grok_provider_usage::provider_for_base_url`
  == `Some(DeepSeek)`（四十九期 host 匹配，`api.deepseek.com`，大小写不敏感），
  单一事实源。
- **D2a**：不新增配置键；`supports_backend_search = true` 在 DeepSeek 模型上
  即「发 DeepSeek 服务端 web_search」。user-guide 26-config-reference 行 +
  11-custom-models 段随更（注明 responses 内置 web_search 仅 v4-pro 执行、
  flash 静默忽略）。

**验证**：cargo check shell --lib 0 error；ctest（GATE_FORCE）过滤
`hosted_search_vendor` 4/4（host 判定三方言 + Deepseek 剥 XSearch 保 WebSearch +
Xai 保持双发）；测试夹具 SessionActor 直构 11 处补 `hosted_search_vendor:
Default::default()`；合并树（双特性同 main）cargo check shell/model-catalog/pager
--lib 0 error。发版后活体验证项：v4-pro 条目 + `supports_backend_search = true`
实测全链路（搜索块呈现 + x_search 不再出现在请求）。

## 五十一期：目录思考位回填——reasoning_efforts 档位随目录自动补齐（2026-10-18，分支 feat/local-catalog-reasoning-backfill）

**需求**：handoff 任务单 ③。原设计（`model-picker-catalog-details-todo.md`
追加节）断言「目录只有思考布尔位、无档位列表」，静态档位表为可选项；施工前
现场重抓双源原始载荷推翻该结论——**OpenRouter `reasoning.supported_efforts`**
（`z-ai/glm-5.3-flash` → `['max','high','low']`，与端点实测方言一致）与
**models.dev `reasoning_options`**（原厂白名单条目，zai/deepseek 的 glm-5.3 系
→ `['low','high','max']`）都带档位数据。按用户指示升级为目录驱动零手写，
静态表方案作废。

- **model-catalog**：`CatalogEntry` 增 `reasoning_efforts: Option<Vec<String>>`
  （serde default，旧缓存兼容）；OpenRouter 解析 `reasoning` 对象（宽容布尔型，
  取 `supported_efforts`），models.dev 解析 `reasoning_options` 首个 effort 型
  条目 values；归一化 = 小写/去空白/去空/去重/保源序（`normalize_effort_levels`），
  未知值留给消费方按枚举过滤。`prices.cache` v3→v4（旧缓存视为过期重取）。
- **shell 回填**：`backfill_accepts_images` 泛化为 `backfill_model_capabilities`
  单遍历三字段（D8 accepts_images 语义逐字不变）。新字段红线（用户拍板）：
  **只在键缺失时回填**——条目已显式写 `supports_reasoning_effort`（含 false）或
  `reasoning_efforts`（含空表）一律不碰；目录 `reasoning == Some(true)` 才动；
  档位按 `ReasoningEffort` FromStr 过滤未知值，全未知不写。幂等（第二轮零写入），
  逐字段 tracing::info（`model catalog: supports_reasoning_effort/reasoning_efforts
  backfilled from catalog`），启动预热统一出口改记
  `model catalog: capabilities backfilled`。
- **hot 语义**：沿 D8 先例——回填写的是 config，会话内已加载的 ModelInfo 到
  下次启动生效（嵌入式无 watcher）；用户本机 glm-5.3-flash（Ark 条目）已按
  拍板手工补两行（`supports_reasoning_effort = true` +
  `reasoning_efforts = ["low","high","max"]`），另两条 glm 条目（cline-pass
  默认档 / z-ai 网关）随本特性下次启动自动补齐。
- **既有遗留补齐**：D8 「backfill 无单测」本期限定补齐——locked 实现参数化
  `_at(path)`（生产传用户 config 路径，测试传 tempdir）+ 7 用例
  （双写/显式 false 与既有列表不碰/显式空表保留/目录无 reasoning 不写/未知档位
  过滤与全未知跳写/幂等/accepts_images 语义回归）。

**验证**：cargo check model-catalog/shell --lib 0 error；model-catalog lib
43/43（新增 2：OpenRouter 归一化与布尔容忍 / models.dev effort 条目与 toggle
缺席）；ctest（GATE_FORCE）过滤 `backfill_capability` 7/7；合并树三 crate
--lib check 0 error。发版后活体验证项：删除测试条目的思考位后启动看回填日志
（unified `model catalog: capabilities backfilled`）与 /effort 菜单。

## 五十二期：上游 2bdd1d6a 同步裁定（2026-10-19，分支 sync/upstream-2026-09-29）

两个同日快照（`97f190f`、`2bdd1d6a`，991 文件 +129,569/−27,951）一次合并。完整
裁定表与验证记录见 [`sync-upstream-2026-09-29.md`](sync-upstream-2026-09-29.md)。
本期只记**同步后必须重放的 LOCAL 补丁点**与本地锁文件偏离：

1. `xai-grok-agent/src/tool_list.rs`：上游原生 allowlist（`ListedTools`）吸收 fork
   T1 的 `SearchTool`/`UseTool` 保底、Task 依赖与 unknown 回退；仅缺 CCR 条件——
   retain 增 `ccr_is_enabled() && id.ends_with(":expand_output")`（LOCAL）。
2. `xai-grok-tools/src/permission_access.rs`：`ToolInput::ExpandOutput(_)` 归
   `AccessKind::Read(None)`（上游把 `AccessKind` impl 从 workspace/permission/types.rs
   搬到这里）。
3. `xai-grok-shell/src/agent/config.rs`：六个 LOCAL 配置节（network / provider_usage /
   model_catalog / model_groups / shell / notifications）整段保留；
   `acp_model_meta()` 内保留 `acceptsImages` 恒显；`ExperimentalSamplingOptions`
   import 保留；上游迁走 `EndpointsConfig` 后删除本地副本。
4. `xai-grok-shell/.../acp_session_tests/support.rs`：`test_compaction_config()` 补
   LOCAL `idle_compact_in_flight`（上游把测试夹具收成 helper）。
5. `xai-grok-pager/src/app/effects/mod.rs`：`parse_rewind_points_response` 补
   `abandoned: r.abandoned_branches`（上游 helper 漏掉 fork 分支树字段）。
6. `xai-grok-shell/src/extensions/model_catalog.rs`：磁盘缓存原语改指
   `xai_grok_config::cache_file`；`write_atomic` 按上游新签名传
   `CacheFileMode::Default` 并记失败日志（原 `pub(crate)` 提级补丁已被上游公开 API 取代）。
7. Windows 编译门：`xai-grok-test-support/src/env.rs` 上游新用例
   `matching_feature_stamp_still_asks_cargo` 与 `xai-grok-workspace/src/sandbox/tests.rs`
   的 managed-config symlink 用例加 `#[cfg(unix)]`（POSIX-only）。
8. `docs-local/win-skip.txt` 族Z：上游 hooks `parameter_expansion_default_is_not_refused` /
   `project_dir_env_is_exported_to_the_child` 在 Windows Git Bash 环境族失败。
9. i18n：合并产生的重复键 `enable/disable` / `running` / `failed` 去重；
   `PAGER_COMMAND_KEYS` 补 `"lsp"`。

**本地锁文件偏离**：上游锁 `cc 1.2.48` 在 rustc 1.94 + Windows 编译失败（
`find_msvc_tools::windows_sys::FILE_ATTRIBUTE_TEMPORARY` u32/i32 不匹配），
`cargo update -p cc` 取 **cc 1.9.0 + find-msvc-tools 0.1.15**；上游下次同步若带回
旧组合需重跑同一命令（上次同步记的是 cc 1.5.1）。

**验证**：`cargo check --workspace --all-targets` 0 error（Windows 独有 27 条上游
测试文件 warning）；sampler 294 / pager 10182 / hooks 274 / agent tool_list 9 /
sampling-types 314 / chat-state 399 / extra-ca 15 / status-line 18 /
compaction-transcript 9 / provider-usage 63 / model-catalog 43 全绿；
两个新 win-skip 环境族已登记。

## 五十三期：Responses `keepalive` 传输心跳兼容——长推理不再被误判为解析失败（2026-10-21，分支 feat/local-responses-keepalive）

**需求**：`gpt-6-luna`（token-unlimited.com，`api_backend = "responses"`）在
长推理时稳定报 `unknown variant keepalive`；三次失败均在该次模型调用开始后
31.6–32.7 秒，短调用正常。`keepalive` 是 OpenAI/Azure Foundry Responses 的
传输层心跳，不在 `async-openai` 的严格枚举里。

- **落点（只动 sampler）**：
  `crates/codegen/xai-grok-sampler/src/client.rs`
  - 新 `RESPONSES_KEEPALIVE_EVENT` 常量 + `is_keepalive_event(event_name, data)`：
    SSE `event:` 名为 `keepalive`，或 `data` 顶层 `type == "keepalive"` 即命中；
    正常 delta 正文引用该词不误杀，畸形载荷仍交给严格解析器报错。
  - Layer-1 `scan` 的 `swallow` 条件先判 keepalive，命中后 `Some(None)` 丢弃，
    不进入 `deserialize_response_event`。
  - 不放宽通用未知事件：其他未知 `type` 继续 `SamplingError::Serialization`。
- **为什么不在 fork 加 variant**：keepalive 无 Response 状态，加进
  `async-openai` 枚举会让所有消费方都要处理；流层丢弃更小、更贴合传输层语义。
- **边界**：keepalive 不计入模型产出，Layer-2 `idle_timeout` 语义不变
  （心跳只证明连接活着，不证明模型在推进）。
- **测试**：`is_keepalive_event_matches_name_or_payload_type`（事件名 /
  payload type / 正文引用 / 畸形载荷）；`response_stream_skips_keepalive_heartbeat`
  （mock SSE keepalive → `response.completed`，断言流不报错）。
- **文档**：`docs-local/upstream-responses-event-compat.md` 追加 2026-10-21 节。

**验证**：`cargo check -p xai-grok-sampler --all-targets` 0 error；
`ctest.sh -p xai-grok-sampler --lib` **296 passed / 0 failed**（新增 2 例）；
`cargo fmt -p xai-grok-sampler -- --check` 新增段无 diff（crate 内既有文件
仍有历史格式差异，整仓 check 不干净）。

**发版（2026-10-21）**：合 main merge `6fc83fab`，tag **`v1.0.65`**；
release run `39556652542` 双 job success（windows 23m23s / linux 28m57s），
release 页 4 资产非草稿；Windows zip 本机下载 sha256 校验通过，
`grok2.exe --version` 报 `grok 1.0.65 (6fc83fab32cd)`。

## 五十四期：异常 LLM API 响应落盘 + 索引（`[api_dump]`）+ 压缩传输重试对齐（2026-10-22，分支 feat/local-llm-api-dump）

**需求**：把非 429 的异常响应按日期落到 `~/.grok/dump/`，供脚本归纳不同供应商的
兼容性错误并回查原始请求/响应；429 与限流是供应商策略，明确排除。同时把压缩路径
的传输类重试对齐到主回合（此前压缩 3 次重试复用同一客户端，HTTP/2 连接池中毒后
每次都同样失败）。

- **新模块 `crates/codegen/xai-grok-sampler/src/api_dump.rs`**（纯函数层 + 进程级
  设置）：
  - `Settings { enabled, dir, max_body_bytes(2 MiB), max_stream_bytes(256 KiB),
    max_files_per_day(200, 0=不限) }`；`configure()` 安装，env 覆盖：
    `GROK_API_DUMP=0|false|off` 强制关、`GROK_API_DUMP_DIR` 换目录。
  - 文件：`<dir>/<YYYY-MM-DD>/<YYYYMMDDTHHMMSS.mmmZ>-<kind>-<backend>-<mode>-<model>-<req>.json`；
    索引：`<dir>/index.ndjson`（16 MiB 轮转为 `index.ndjson.1`）。每日文件数上限
    进程内计数（跨日期自动重置）。
  - `kind` ∈ `http_error` / `transport` / `decode` / `stream_error` / `idle_timeout` /
    `auth`；`backend` ∈ `chat_completions` / `responses` / `messages`；`mode` ∈
    `stream` / `nonstream`。
  - 索引行字段：`ts, kind, status, backend, mode, model, provider(URL host),
    session_id, req_id, error_type, error_code, message(≤400), file(相对路径),
    request_bytes, stream_truncated`；文件内另含完整 `error`、`request`（method/url/
    headers/body/body_bytes/body_truncated）、`response`、流式失败的 `stream`
    （frames/frame_count/truncated）。
  - 脱敏：`authorization` / `api-key` / `api_key` / `apikey` / `token` / `secret` /
    `cookie` / `key` / `*_key` / `*-key` 头与敏感 query 参数写 `[REDACTED]`；
    URL 在 capture 与写盘两处各脱敏一次。正文按 UTF-8 边界截断。
  - `StreamRecorder::push_frame` 保留非 `message` 的 SSE `event:` 名（方言信号），
    达上限后置 `truncated` 继续计数帧数。
- **采样器接线 `client.rs`**：六个发送点（chat/responses/messages × stream/nonstream）
  全部接线。`build_json_request` 返回未压缩 JSON 副本（仅在 dump 开启时克隆，关闭时
  零额外序列化）；`handle_response` 增 `dump` 参数，401 / 非成功状态 / 反序列化失败
  三处落盘；流扫描闭包在 `try_parse_stream_error`、`Serialization`、
  `EventStreamError` 与 responses 的 `deserialize_response_event` 失败处落盘。
- **配置面 `agent/config.rs`**：新 `[api_dump]`（`enabled` 缺省 true、
  `max_body_bytes`、`max_stream_bytes`、`max_files_per_day`），
  `resolve_runtime_fields` 解析为 sampler 设置（目录 `$GROK_HOME/dump`）。
- **压缩重试对齐**：`session/helpers/session_compact.rs` 增 `is_transport_failure`
  （`Transient` 且 detail 含 `reqwest error stream: ` 或 `request error: `）；
  `ShellCompactionSampler` 客户端改 `Mutex<OaiCompatClient>`，首字节前传输失败后
  下一次尝试换 `force_http1 = true` 的新客户端（对齐主回合
  `RetryWithClientRebuild`）；`session/compaction.rs` 重试 3→5。
- **测试**：`api_dump` 模块 4 例（400 落盘 + 429 跳过 + 流帧 + 脱敏；流截断；
  正文 UTF-8 截断；索引/文件形状）；`client.rs` 线测
  `api_dump_records_bad_request_but_not_rate_limit`（mock axum 400 → 1 文件 + 1 索引行，
  429 → 不增）；`session_compact_classify_tests.rs` 新增
  `transport_failures_are_distinguished_from_other_transient_ones`；
  `config_tests.rs` 新增 `parses_api_dump_table_and_defaults_to_enabled`。
- **顺带修复**：`26-config-reference.md` 的 `features.idle_auto_compact` 行
  Managed 列由 `—` 改 `user`——`config_docs::managed_column_matches_resolver_metadata`
  在 `--features config-docs` 下一直红（既有缺口，非本次引入）。
- **文档**：`26-config-reference.md` 增 `api_dump` 表；
  `29-local-enhancements.md` 增「API failure dumps」节（布局、索引字段、jq 检索、
  脱敏与边界）+ 引导信号表一行；`11-custom-models.md` Troubleshooting 增
  「Provider Rejects a Request」。后者随 pager 启动的 `extract_user_guide_docs`
  落到 `~/.grok/docs/user-guide/`，引导 agent 按 `<user_guide>` 指令直接读取。

**验证（2026-10-22）**：`cargo check -p xai-grok-sampler --all-targets` /
`-p xai-grok-shell --lib` / `-p xai-grok-pager --lib` 0 error 0 warning；
`ctest.sh -p xai-grok-sampler --lib` **300 passed / 0 failed**（含新增 4 例）；
`GATE_FORCE ctest -p xai-grok-shell --lib session_compact` 49 passed / 1 ignored；
`GATE_FORCE ctest -p xai-grok-shell --features config-docs --lib config_docs`
10 passed / 0 failed；`ctest -p xai-grok-pager --lib docs::` 19 passed / 0 failed。
**活体实测（v1.0.66 release 产物 + 本机 mock 端点，独立 GROK_HOME）**：400 → 落盘
`dump/<date>/*.json` + `index.ndjson` 一行，`authorization` 为 `[REDACTED]`，
请求体为实际出站 JSON（含 tools）；429 → 无新文件无索引行；`enabled = false`
→ 不创建 dump 目录；`GROK_API_DUMP=0` → 同上；`GROK_API_DUMP_DIR` → 落点切换
且默认目录零新增。

**发版（2026-10-22）**：合 main merge `6054081`（PR #2），tag **`v1.0.66`**；
PR build `39699522376` 双 job success，release run `39702900602` 双 job success，
release 页 4 资产非草稿；Windows zip sha256 `cda253a9…` 与本机下载一致，
`grok2.exe --version` = `grok 1.0.66 (6054081530b1)`；已部署
`D:\tool-cli\grok2`，回滚点 `grok2.exe.1.0.65-6fc83fa.bak`；
用户 config 增 `[api_dump] enabled = true`（备份 `config.toml.bak-api-dump-*`）。

**根因记录（2026-10-22 补）**：压缩失败的两起独立故障（09-23 的半答 tool_calls 400、
10-19 的传输中断）已合并整理到
[`docs-local/compaction-failure-rca.md`](compaction-failure-rca.md)，含现场工件字段、
重试阶梯对照与遗留清单。本节只记「压缩重试对齐」的施工面，根因与取证细节看该文。

## 五十五期：嵌套派发深度剥离的 task 调用改为终态引导——子 agent 不再被「Tool not found」循环卡死（2026-10-23，分支 fix/local-nested-subagent-task-guidance）

**需求/现场**：主会话（grok 4.6）动态派发绑定第三方网关模型（gpt-6-luna）的 subagent
后，该 subagent 又尝试派发。默认 `subagents_max_depth = 1`，嵌套 task 调用本应快速
拒绝，但工具面剥离后的调用走 `Tool not found: task` 裸错误——模型拿不到「为什么被拒」，
弱模型/第三方网关模型会反复重试，subagent 回合原地打转，用户侧表现为卡死。

- **`xai-grok-tools/src/registry/types.rs`**：`call_streaming_with_context` 的
  prepare-dispatch 失败臂加拦截——调用名命中 `task`（大小写不敏感）且本会话
  `SubagentDepthCounter >= MaxSubagentDepth`（与 `apply_child_tool_policy` 的剥离
  条件同构）时，把通用 not-found 换成终态引导：「task 在本 subagent 不可用（已达
  深度上限 depth N of max M），不要再次调用 task，自己继续完成工作并在最终回复中
  汇报结果」。其余情况（未知工具名、深度未到限）保持原 `Tool not found` 不变；
  成功派发路径零新增开销（拦截只在失败臂）。
- **`xai-grok-tools/src/implementations/grok_build/task/mod.rs`**：Task 工具自身的
  深度超限错误（max_depth ≥ 2 显式放开时仍会走到）追加同样的不重试引导文案。
- **测试**：`registry::types::tests` 新增 3 例（深度剥离命中引导 / 根会话保持
  not-found / 非任务名不受影响）+ 既有 `depth_limit_exceeded` 回归绿。

**验证**：`cargo check -p xai-grok-tools --all-targets` 0 error（10 条 warning 均为
既有测试模块旧 import）；新 3 例 + 既有深度限制用例 GATE_FORCE 定向跑全绿。
两处落点均在上游同步面（registry/types.rs、task/mod.rs 曾被 `Synced from monorepo`
覆盖），下次上游同步需按本节重放。

**发版（2026-10-24）**：合 main merge `8019d08`，tag **`v1.0.67`**；build run
`40041666526` / release run `40041666192` 双 job success，release 页 4 资产非草稿；
Windows zip sha256 `ecf3a3c7…` 与发布 `.sha256` 一致，本机已部署
`grok 1.0.67 (8019d08fba1f)`（D:\tool-cli\grok2），回滚点
`grok2.exe.1.0.66-6054081.bak`。待活体验证：第三方网关模型 subagent 再次派发
时收到终态引导并正常收尾（不再 `Tool not found` 重试打转）。

## 五十六期：LSP 诊断按需 op（`diagnostics`/`workspaceDiagnostics`）+ 描述路由指南 + Windows 环境修复（2026-10-24，分支 feat/local-lsp-diagnostics-op）

> [`docs-local/lsp-port/enhancement-final-design.md`](lsp-port/enhancement-final-design.md)
> 定稿的首期施工：P1（T1）+ P0（随 T1 搭车）+ P5 之 Windows 检测回归用例
> （随 P1 搭车）。T1 原案（improvements-todo §2）之上按定稿 §3.2 加两点
> 设计补丁（pull-only 分流 / settle）。D3（limit 缺省 100）按默认执行。

1. **types.rs**：`LspOperation` 增 `Diagnostics`/`WorkspaceDiagnostics`
   （serde camelCase 序列化名 `diagnostics`/`workspaceDiagnostics`，加
   `PartialEq`/`Eq`）；`LspToolInput` 增可选 `limit`（workspaceDiagnostics
   截断，缺省 100 对齐 qwen）。
2. **dispatch.rs（核心）**：两 op 不发 LSP 请求，直读 per-server
   `DiagnosticsStore`。`Diagnostics` 单文件：auto-open → settle 等待 →
   过滤 ERROR/WARNING → `error L{行}:{列}: {message} (source)` 行格式，
   空结果按「已应答」/「未应答」分流（`clean (server=x)` vs
   `no diagnostics reported yet`）。两点补丁：
   - **settle（定稿 §3.2 补丁 2）**：版本感知等待——store 从未应答（首次
     打开）等 1s（对齐 reminder 路径）、已有旧 verdict（近期被编辑）等
     300ms、触发过 pull 等 2s（一次 pull 往返）；超时按现状返回并在尾注
     「编辑后可能尚未稳定，请重查」。等待持 clone handle（store/documents/
     pull 都是共享句柄），不占 manager 锁。
   - **pull-only 分流（补丁 1）**：`!server_publishes() && pull.support()
     == Asking`（即 pull.rs 既有 `worth_asking` 判定的外部复算）时
     `will_answer` 主动问一次再等——push 型 server 读 store；Roslyn 类
     只答不推的 server 不再从空 store 返回假 clean。实现走既有
     `will_answer` 而非裸 `request`（同路径 + confirm 逻辑免费复用）。
   `WorkspaceDiagnostics`：聚合全部 server 的 tracked 文档（不扫盘），
   按 server 分块 + limit 截断尾注（`Showing N of M (limit L)`）+
   诚实边界注（tracked-only，空结果不是全库无错）；从未应答的 pull-only
   server 标注「用 diagnostics op 按文件查」。
3. **grok_build/lsp/mod.rs（P0）**：description_template 增两 op 说明 +
   grep-vs-LSP 路由段（精确文本/字面量/错误信息 → grep；引用/调用链/
   影响面/大纲 → lsp）+ navigate-then-edit（导航后先读再改）+ 改完
   接口当回合 `diagnostics` 自验。
4. **builtin.rs（P5 搭车）**：五语言 PATH 检测回归用例（rust-analyzer/
   gopls/basedpyright 降级/jdtls/clangd 映射各一条）+ Windows PATHEXT
   语义用例（`gopls.exe` 命中、无扩展名文件不命中；返回路径带 PATHEXT
   大小写，断言 eq_ignore_ascii_case）。
5. **manager.rs**：测试分派 `dispatch_tool_typed` 增两 op 臂（复用同一
   store 读取函数，生产/测试路径不分叉）。

**Windows 环境修复（附带收获，lsp 套件本机 44 挂 → 0 挂）**：crate 此前
不在 win-whitelist，lsp 套件从未在本机跑过，本轮 GATE_FORCE 首跑 44 挂
分族后修掉三族——
- **mock server 五脚本 stdout 文本模式**：`\r\n` 被 Windows 翻译成
  `\r\r\n`，LSP 帧头解析全坏（mock 从未在 Windows 工作过）；一律加
  `sys.stdout.reconfigure(newline="\n")`。
- **两个 pull mock 的 marker 写错位置**：`uri[len("file://"):]` 在
  Windows 产出 `/D:/...`，`open` 落到当前盘 `\D:\` 目录；补盘符剥离。
- **`restart.rs replay_tracked_documents` 真 product bug**：
  `strip_prefix("file://")` 把 `file:///D:/...` 转成非法路径 `/D:/...`，
  读文件失败 → server 崩溃重启后 tracked docs **全部静默丢弃**（Linux
  CI 测不出）；改 `Url::parse + to_file_path` 平台映射。

**验证**：`cargo check -p xai-grok-tools --all-targets` 0 error（9 条
warning 均既有 grep/bash 测试文件旧 import）；GATE_FORCE
`ctest.sh -p xai-grok-tools --lib lsp::` **119 passed / 0 failed**
（2 ignored 既有；含新 8 例：两 op e2e ×4、序列化名 1、五语言检测 5、
PATHEXT 1）；i18n 扫描 exit 0。`watched_files
out_of_workspace_globs_never_match` 为既有 Windows 路径匹配问题
（工作区外绝对 baseUri 的 glob 误命中工作区文件，产品影响小），登记
win-skip 族AA 挂触发，不在本期修。

**遗留**：① 真实 server 手验（rust-analyzer 开本仓库造错 → `lsp
diagnostics` → 修复 → clean；多文件错误场景 `workspaceDiagnostics`）随
发版活体；② P0 路由指南实测（模型是否仍绕过 LSP，决定 hook 执法层是否
立项，D7）；③ 上游同步重放：`implementations/lsp/` 为 fork 重镇，本轮
落点 types.rs / dispatch.rs / format.rs / manager.rs / restart.rs /
builtin.rs / grok_build/lsp/mod.rs。

## 五十七期：LSP 懒启动——server 首次被触达才拉起 + spawn 失败负缓存（2026-10-24，分支 feat/local-lsp-lazy-start）

> enhancement-final-design.md P2（§3.3），从「挂触发」上调的中价值项：lsp_tools
> 默认开 + eager bootstrap 意味着每个会话 spawn 都付出全部检出 server 的启动
> 代价（rust-analyzer 大项目 1–2GB 内存），哪怕全程没用过一次 LSP；D6 按默认
> 倾向执行（缺省懒启动，显式启用走 eager）。

1. **manager.rs**：`ensure_initialized` 拆出 `ensure_server_started(name)` 按
   server 启动原语（返回是否需要崩溃监控）；eager 路径 = 逐 server 调它，
   两路共用同一启动序列。
2. **dispatch.rs**：`StartupCoordinator` 改 per-server 状态机（map 缺席 =
   未启动）。文件级 op（位置 op / `diagnostics`）只拉起 owner server；
   `workspaceSymbol` / `workspaceDiagnostics` 按需全拉（问的是全工作区）。
   `notify_file_event` = 读写触发点：先入队再拉起（防启动任务的 drain 超前
   消费）；owner 未就绪时事件进 pre-ready 队列，启动完成后按
   owner+ownerless 回放（ownerless 的 watched-path 事件由首个启动的 server
   消费，跨 server 时序以注释说明）。**负缓存**：spawn 失败记 60s TTL，期内
   同语言访问不再重试（防每文件白探测）；status 聚合 Starting>Failed>Ready。
3. **reminders/lsp_diagnostics.rs**：去掉无条件 `ensure_started_background()`
   （eager 泄漏点）——每个 `notify_file_event` 自带懒拉起。
4. **shell**：`build_lsp_backend(cwd, scope, eager)`——session spawn 传
   `false`（懒启动缺省路径），`x.ai/lspApply` 热加载传 `true`（/lsp 面板 e 键
   显式启用 = 跳过懒启动立即拉起，保留用户强制语义）；`/lsp status` 兼容：
   未拉起 server 不在 running_servers 出现，按配置清单显示为未运行。

验证：GATE_FORCE `ctest.sh -p xai-grok-tools --lib lsp::` **122/122**（新 3 例：
懒启动只拉 owner 且坏 server 不被拖累 / 编辑触发启动且回放首个变更 /
负缓存命中 + 健康 server 不受累）；`cargo check tools+shell --all-targets`
0 error。

遗留：真实会话活体（懒启动下 /lsp status 面板、首触达延迟、内存曲线）随
发版。

## 五十八期：LSP rename 写路径——独立 `lsp_rename` 工具（2026-10-24，分支 feat/local-lsp-rename）

> enhancement-final-design.md P4（§3.4）。D1（独立工具）与 D2（先全量读内存
> 再统一写、失败列已写）按默认倾向执行。

1. **implementations/lsp/rename.rs（新，纯函数层）**：`WorkspaceEdit` 扁平化
   （`documentChanges` 优先于 `changes`；`documentChanges` 内的文件资源操作
   create/rename/delete **拒绝执行**——符号 rename 不得带文件增删副作用）+
   `apply_text_edits`（LSP UTF-16 位置→字节偏移、文档序排序校验、越界/重叠
   即整批拒绝、自尾向前应用——qwen 教训：陈旧偏移绝不冒充成功）。
2. **`LspBackend::rename`（trait，默认 Err——测试替身免改）**：adapter 实现 =
   owner server 解析 + ensure_ready（与懒启动联动）+ socket_for_file 自动
   didOpen + `timed_request(textDocument/rename)` + **原子预检**：全部受影响
   文件读入内存、edits 全部成型后才返回给工具层，一个坏偏移整批拒绝。
3. **`grok_build/lsp_rename`（新工具，D1 独立写工具）**：`is_read_only: false`
   + `ToolScope::Write`，六导航 op 的免审批面零变化；落盘走 `FileSystem`
   资源（与 search_replace 同写语义）；写阶段失败按 D2 报错并列出已写文件
   （不回滚）；写后逐文件 `notify_file_event` 喂回 LSP（诊断回喂不漏）；
   new_name 非空/无空白/无路径分隔符校验。
4. **接线**：`ToolInput` 增 `LspRename` 变体（derive_more From 自动）+ registry
   `register::<LspRenameTool>()` + normalization 兜底臂。

**实现偏差（记录）**：设计要求「落盘走既有编辑工具事件链」（zcode 教训）——
本实现为诊断回喂等价直连（工具内 notify_file_event），未走 `ToolOutput` 事件
变体（避免动跨 crate 的 `ToolOutput` 枚举面）；reminder 的 drain 对 rename
结果与编辑同路径可见。代价：diff 面板/acp tracker 不展示 rename 改动（输出为
文本摘要），接入留 v2。

验证：GATE_FORCE `ctest.sh -p xai-grok-tools --lib lsp::` **130/130**（新 10
例：rename.rs 纯函数 6 + mock rename server e2e 2〔双文件 edits 全返回 /
STALE 陈旧偏移拒批〕+ 工具形态 2）；`cargo check tools+shell --all-targets`
0 error。

遗留：① rust-analyzer 真机手验（本仓库改 `LspOperation` 枚举变体名跑全量
rename）随发版活体；② 非 YOLO 模式的审批面（多文件改动的授权按工具调用整体
呈现，未见 sibiling 文件逐个审批——需活体确认体验）；③ diff 面板接入。

**发版记录（五十六/五十七/五十八期合并收口）**：三分支顺序叠放
（`feat/local-lsp-diagnostics-op` → `feat/local-lsp-lazy-start` →
`feat/local-lsp-rename`）按序 fast-forward 合 main（main `9987be0`，PR #3
为 CI 验证面），随 tag **`v1.0.68`** 发版（2026-10-24）：PR build run
`40080132346` / main build run `40081680780` / release run `40081685940`
双 job success、4 资产非草稿；Windows zip sha256 `58b75c66…` 核对一致，
发行产物 `grok 1.0.68 (9987be08d72e)`，本机已部署（回滚点
`grok2.exe.1.0.67-8019d08.bak`）。活体验证清单见
`.handoff/lsp-enhancements.md` §12（诊断主链路 / 懒启动 / rename 全量改
枚举名 / restart 回放）。

## 五十九期：dsh in-history 系统提示放置 + append 模式工具结果裁剪——per-model 缓存友好能力位（2026-10-24，分支 feat/local-dsh-in-history）

> 背景：cache-strategy-port 专题（调研 `docs-local/cache-strategy-port/README.md`）
> ——fork 独有的「历史中途改写」（工具结果裁剪原地替换 + 系统提示原地改写）是
> DeepSeek 前缀缓存断崖的结构性来源。拍板 D-a…D-d 按建议值落地；probe 门
> （硬前置）四象限通过后开工，probe 结果回填
> `docs-local/dsh-port/context-engineering-design.md` §3.2。

### 能力位（对齐 use_proxy 二十三期先例）
- `xai-grok-sampling-types/src/types.rs`：`SamplingConfig` 尾部追加
  `system_prompt_update: SystemPromptUpdate`（off/in-history，默认 off）+
  `prune_mode: PruneMode`（rewrite/append，默认 rewrite），全 `#[serde(default)]`
- `xai-grok-sampler/src/config.rs`：`SamplerConfig` 同名透传字段（sampler 本体
  不读，仅为到达 spawn/model_switch/sampler_turn/subagent 的 SamplingConfig 派生点）
- `xai-grok-shell/src/agent/config.rs`：`ModelEntryConfig`（TOML，`#[serde(default,
  skip_serializing_if = ...)]` + is_default_* 辅助）、`ModelInfo`（默认值 +
  from_config 拷贝）、`ConfigModelOverride`（Option 覆盖 + apply 显式值赢）
- 构造点补齐 6 处（config.rs ×3 / sampler_turn / remote/client / tools/config）+
  测试穷尽字面量（config_model_override_parse roundtrip guard）
- 派生点拷贝 4 处：spawn.rs / model_switch.rs / sampler_turn.rs / subagent/mod.rs

### P1 in-history 放置（`xai-chat-state/src/actor/`）
- `state.rs`：`InHistoryWireState { head, last_snapshot }` wire 层派生态
  （head = 冻结在 messages[0] 的字节，永不改写；last_snapshot = 最近追加快照）
- `request_builder.rs`：`apply_in_history_system_placement`（api_backend ==
  Messages 代码级拒绝——折叠转换层语义不符，dsh-port §3.2 放行矩阵）+
  纯函数 `apply_in_history_placement`：变更时恢复冻结头 + 新快照追加历史尾，
  此前追加的快照原位保留（wire 跨轮 append-only；中途踩过「base 单字段在
  变更后更新导致 position 0 换字节」的设计错误，测试逼出后改为双字段）
- `actor/mod.rs`：`UpdateSamplingConfig`（model switch）重置 wire 态
  （新模型 = 新缓存命名空间）；compaction/replace_system_head 不重置
  （派生逻辑自洽：前者整段重写本就断前缀，后者正是要保护的场景）

### P2 append 模式裁剪（`xai-chat-state/src/actor/request_builder.rs`）
- `prune_conversation_append`：标记文本 call-id 键控且**与裁剪类别无关**
  （软裁→硬清深化时标记不变，杜绝中途改写 wire 字节）；标记集只增不减
- `prune_items_for_turn_request` 分流 + `mutations.rs::prune_retained_conversation`
  append 模式跳过（保留会话层必须留原字节，否则标记失去指向对象）
- `updates.jsonl` 语义不动（两条路径均为 wire/派生层）

### 文档
- `xai-grok-pager/docs/user-guide/26-config-reference.md`：`model.<id>.
  system_prompt_update` / `model.<id>.prune_mode` 两行

验证：ctest 门控 chat-state lib **407/407**（新增 10 例：in-history 5 +
append 5，含「三轮连续变更 wire 逐字节 append-only」「标记文本与类别无关」
「turn-1 wire 是 turn-2 wire 的字节前缀」三个核心断言）；sampler **300/300**；
sampling-types **314/314**；pager docs **19/19**（config reference 行格式门）；
GATE_FORCE shell `config_model_override` **15/15**；shell `--all-targets`
check 0 error。

遗留：① limit-probe A/B 活体（触发点后首轮 cached_tokens/prompt_tokens，
off ≈0 → on ≈ 稳态）随新客户端（用户 config 已可对
`deepseek/deepseek-v4.1-flash-1017` 开两能力位实测）；② messages backend
in-history 需先改折叠转换层再重 probe；③ compaction 后 in-history 基点
未显式重置（派生逻辑自洽，活体观察）。

**发版记录**：PR #4（CI run `40097306847`，linux 首跑挂既有计时 flake
`reset_title_to_auto…`〔7172 通过仅此 1 挂，当日第 2 次〕重跑过，双 job
success）合 main（merge `e86015c`），随 tag **`v1.0.69`** 发版（2026-10-24）：
release run `40101135537` 双 job success、4 资产非草稿；Windows zip sha256
`094aede4…` 核对一致，发行产物 `grok 1.0.69 (e86015c19194)`，本机已部署
（D:\tool-cli\grok2，回滚点 `grok2.exe.1.0.68-9987be0.bak`）。main build
run `40101077754` 同 flake 再挂（当日第 3 次），重跑绿——**该 flake 复发
频率已到立案线，建议单独立项修 persistence 计时用例**。

## 六十期：persistence 计时 flake 根治——in-flight 标题用例去竞态化（2026-10-25，分支 fix/local-persistence-title-inflight-flake）

> 背景：`session::persistence::durable_update_tests::
> reset_title_to_auto_adopts_in_flight_generation_as_auto` 计时 flake 当日
> CI 三挂（PR #4 run `40097306847` / main run `40101077754` / 另见 v1.0.48、
> v1.0.58 轮次重跑记录），到立案线单独立项。**根因不是慢，是顺序不确定**——
> 测试设计缺陷，非产品 bug。

### 根因链（全部锚点核实）

- `persistence_tests.rs` 的 `test_actor_inner` 用 `SamplerConfig::default()`
  （`base_url` 空串、无 key）装配 `SummaryGenerator`；后台任务 T 的
  `conversation_collect` 在请求构造阶段即刻报错 → **T 首个 poll 内（微秒级）**
  落回 `title_fallback_from_user_text` 并把 `GeneratedTitle` 塞回 FIFO 通道
  （`session/summary.rs:79`）——「生成中」窗口实际只有微秒
- 测试随后 `storage.reset_title_to_auto(&info).await` 直接落盘 unpin，该写走
  `apply_summary_patch_reporting` → `tokio::task::spawn_blocking`
  （`storage/jsonl/mod.rs:1143`，真实 yield 点）；T 的 `GeneratedTitle` 处理
  （`set_generated_title_if_absent`）同样 spawn_blocking 抢同一把 summary
  sidecar 锁——两个阻塞闭包谁先拿锁取决于线程池调度，CI 满载时翻车
- 竞输形态：`if_absent` 先拿锁看到 MANUAL → `Ok(false)` 合法跳过（永不复用）；
  随后 unpin 刷空盘面；generator 已被 `ResetTitleToAuto` 置回 Idle 但再无
  chunk → 盘上标题永远空白 → 8s 轮询超时 panic（原 2091 行）

### 修法（纯测试面，零产品改动，`persistence_tests.rs` 单文件）

- unpin 落盘 + `flush_ack` 屏障之后**显式补发一条** `GeneratedTitle(expected)`
  （FIFO 保证严格后于 reset 处理）；两种调度结局收敛为同一确定值——后台 T
  的消息与显式消息都由 `title_fallback_from_user_text(CHUNK)` 求值，字符串
  相同，恰好一条被 `if_absent` 采纳、一条被跳过
- 删掉 8s 轮询循环，改为 `GeneratedTitle` + `flush_ack` 屏障后**单次读盘**
  `assert_eq!(display_title(), expected)`（比原断言更强：原只查非空 + 非
  MANUAL）
- 同文件排查结论：1887 行远端同步用例传 `mark_summary_done: true`（Done 态
  不 spawn，无竞态）；`non_resident_reset_then_load_regenerates` 在 reset
  落盘确认后才 spawn——**仅此一例有顺序竞态**

### 同步重放点（上游同步后必查）

`persistence_tests.rs` 是 `Synced from monorepo` 同步文件：上游若改动该用例
（或重排其消息序列），以本条为准重放「屏障后显式补发 + 单次读盘」结构；
核心不变量 = 不依赖 spawn_blocking 线程调度顺序。

验证：GATE_FORCE 门控 Windows 本机单测 ok（1.18s）；连跑 15 次全绿
（0.28–0.57s）；`title` 族批量过滤 93/94——唯一挂为族AB 环境族
（acp_session_tests support.rs /tmp 夹具 NotAbsolute，同族N/S/V/W 根因，
已登记 win-skip，与本次改动无关）；shell `--all-targets` check 0 error。
Linux CI 全量 shell 套件随 PR build 验证。

**发版收口：** PR #5 merge `ce2d88e`；PR #6 同批 merge `ac85985`。最终 main build `40151902095` Linux/Windows 双 job success；与 PR 验证 build `40149746244`、`40149884484` 均全绿。已将 `v1.0.69` 重指到 `ac8598508ca687dc8562e1eb8adc79a1db79c0c2`，release run `40154140290` 双 job success、4 资产非草稿。

## 六十一期：/stats 面板两修复——y 只复制当前标签页 + 面板期间释放鼠标捕获供原生框选（2026-10-25，分支 fix/local-stats-modal-copy-select）

> 背景：用户报告 /stats 模态两个使用问题：①面板里的用量数字无法用鼠标
> 框选一部分复制（TUI 捕获鼠标后终端原生选择失效）；②`y`（及页脚按钮）
> 复制的是全部四个时间窗拼接的整份报表，而不是当前标签页——用户摘录时
> 被迫粘出一大坨别的窗口的数字。
>
> 期号说明：本期挂在 main（止于五十九期）之上编号六十一，把六十让给
> `fix/local-persistence-title-inflight-flake`（六十期，未合 main），避免
> 两分支合并后期号撞车。

### 修复 1：`y` 只复制当前标签页（`xai-grok-pager/src/views/stats_modal.rs`）

- `StatsModalState::copy_all()`（遍历 `Window::ALL` 拼四段）改
  `copy_active_tab()`：只导出 `active_tab` 一页，段头 `== <label> ==`
  保留（粘出来的文本自带窗口上下文）；要看全部就逐页切过去各复制一份。
- 两个触发点同步：`y` 键（`handle_stats_modal_key`）与页脚
  `COPY_STATS_SHORTCUT` 按钮点击（`route_stats_modal_mouse`）。
- 用例：`y_copies_every_section` 重写为 `y_copies_only_the_active_tab`
  （5h 页不含其它段头/别窗样本 + 切日页后包含日窗样本、不含 5h 段头）；
  `single_call_rows_show_cache_hit_as_na` 跟随改名。

### 修复 2：/stats 面板 = 原生选择面（`xai-grok-pager/src/app/app_view.rs`）

- 复用 auth 原始 URL 面板的原生选择机制：`AppView::stats_modal_open()`
  谓词（`active_agent()` 的 `active_modal` 是 `StatsInfo`）并入
  `sync_native_selection_mouse()` 的 `want_off`——面板打开期间
  `DisableMouseCapture`（Windows 另启用 QuickEdit `win_native_selection`），
  终端自己接管拖选；关闭面板后仅在本次 hold 时恢复捕获（用户
  `/toggle-mouse-reporting` 关闭态不被打扰）。
- 谓词只认 `active_agent()`：dashboard/welcome 视角看不到 agent 的面板，
  不触发释放。
- 代价（有意）：面板打开期间滚轮滚动/标签点击/页脚点击让位给终端原生
  选择；键盘（Tab/1-4/hl 切页、jk/翻页滚动、a 展开、y 复制、Esc 关闭）
  仍覆盖全部操作。转义舞步与真实终端行为随活体验证（谓词用例零副作用，
  不碰真实 stderr/console）。
- 用例：`app_view_tests::stats_modal_counts_as_native_select_surface`
  （无模态 false / StatsInfo true / 关闭 false / dashboard 视角 false）。

### 验证与遗留

- `cargo check -p xai-grok-pager --all-targets` 0 error（8m22s）。
- ctest 门控过滤 `stats_modal` + 新谓词用例：**18 passed / 0 failed**
  （含重写的 `y_copies_only_the_active_tab` 与新
  `stats_modal_counts_as_native_select_surface`；10230 filtered out）。
- 遗留：活体验证随新客户端——真 TUI 开 /stats 后拖选框选 + `y` 粘贴
  只有当前页；Windows Terminal/conhost 两宿主下拖选行为抽查。

**发版收口：** PR #6 merge `ac85985`，最终 main build `40151902095` Linux/Windows 双 job success；PR build `40149884484` 双 job success。`v1.0.69` 标签已重指到该 merge commit；release run `40154140290` 双 job success，发布页为 4 个非草稿资产。Windows zip SHA-256：`d48bd970cac0b0557aadf506405cd042a43ffa17b3da477fef8bd7139b775ff6`；下载校验通过，运行显示 `grok 1.0.69 (ac8598508ca6)`。
