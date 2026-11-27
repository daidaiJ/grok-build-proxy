# zcode v4.0.0 → grok-build 移植增补

> **状态：调研入库，未开工（2026-11-27）**。本文回答的问题是：zcode 从 v3.18.0 走到
> v4.0.0 后，CLI 侧有哪些机制值得本 fork 吸收。**本轮只入库文档，无代码改动。**
> 上游：[zai-org/ZCode](https://github.com/zai-org/ZCode)，本地克隆 `D:/CODE/ai/zcode`，
> 基准 `a4b4445`（v4.0.0，根 `package.json` 4.0.0，CLI 工作区 0.18.0，README 2026-11-23）。
>
> **事实源分工**：逐条机制、源码锚点与 fork 对照在
> [`agent-cli-tracking/notes/zcode.md`](../agent-cli-tracking/notes/zcode.md)「2026-11-27 刷新」节；
> 引入定级、防误报与负面清单在 [`agent-cli-tracking/ADOPTION.md`](../agent-cli-tracking/ADOPTION.md) §11；
> **本文 = 决策摘要 + 待办台账**，只给结论、落点与验收信号。
> v3.18.0 的 Prompt Snapshot / 停滞提醒 / 修复持久化三项已有专题
> [`agent-cli-refresh-2026-11/README.md`](../agent-cli-refresh-2026-11/README.md)，本文只写 v4 的
> 增补与收敛，不重复已定级内容。

## 0. 结论先行

- **P1 两项**：① 推理历史修复持久化（OpenAI encrypted reasoning + Anthropic signature，含
  `reasoning_reset` 标记与请求投影卫生）；② 工具结果 `is_error` 落到 provider 线与冷恢复，
  并用同一判定选择 `PostToolUseFailure`。
- **P2 四项**：③ 压缩后文件内容回补（≤5 文件 / 5k / 50k）；④ 后台 subagent 停滞提醒；
  ⑤ MCP 首提示等待收缩 + `search_tool` 按 server 报 pending/needs-auth/failed 事实；
  ⑥ `[skills]` 配置变化触发 `ReloadSkills`。
- **P2 小件**：⑦ Bash 前台长 sleep 拦截；⑧ 压缩后 skill 名单增量补发（先量 token 再决定）。
- **P3**：⑨ Skill/工具名 `Did you mean`；⑩ 回复语言附件；⑪ subagent 默认后台 /
  立即发送转后台 / SendMessage 恢复，作为独立 UX 决策，不随本轮开工。
- **经核实 fork 已有，不再立项**：skills 文件监听与增量公告、MCP failed/needs-auth 公告、
  压缩后运行中任务/子代理状态、subagent 崩溃对账、MCP 配置热重载、MCP 执行期握手等待、
  output-limit 续写提醒持久化、Prompt Snapshot 的主要缓存收益、配置深合并。v4 未改 TUI 包。

## 1. 同步与去伪

版本链：`a9fe401e`（v3.17.2，2026-11-03）→ `b95ebaf`（v3.18.0，2026-11-11）→
`c590468`（v3.18.1，2026-11-12）→ `fec9759`（v3.18.2，2026-11-12）→
`a4b4445`（v4.0.0，tag，2026-11-23）。

- 3.18.0 → 4.0.0：2,283 文件 / +155,247 / −25,117；其中 3.18.2 → 4.0.0 占 2,209 文件 /
  +149,753 / −24,801。**v4.0.0 是一次大版本，不是 3.18 的补丁尾巴。**
- 远端复核（2026-11-27）：经 `https://ghproxy.net/...` 执行 `git ls-remote`，`refs/heads/main`
  仍为 `a4b4445`，本轮结论对应当前上游 head。本机直连 github.com 的 `ls-remote` 超时
  （约 3 分钟无输出），远端查询与拉取走代理。
- 新增官方插件包：`design-plugin`（HTML 设计稿/原型/HTML deck 技能）、`documents-plugin`、
  `spreadsheets-plugin`、`pdf-plugin`、`presentations-plugin`（后四者绑 LibreOffice 预装与
  文档工具链）、`node-repl-host`。CLI 内核新增 `core/src/mcp/runtime/`、`core/src/skills/`、
  `core/src/tool/cmd-readonly/`、`wait-for-mcp-servers`、`levenshtein`、`mcp-diagnostics`；
  adapters 新增 `mcp/{config-loader,config-source,tool-catalog-cache,transport-errors}`、
  `watch/`、`skills/catalog`、`fs/symlink-guard`、`memory-diagnostics/`。
- **TUI 包无 v4 改动**；新面集中在 CLI 的 MCP、skills、推理回放、工具结果、压缩与子代理。
- 去伪：diff 规模大，但多数是文档、插件资源与桌面/远程形态；判定「某机制是否新增」仍用
  `git ls-tree -r <old> | grep <path>` 与 `<new>` 交叉核对，别按 A/D 标记估特性量。

## 2. 增量地图（按面）

| 面 | v4 带来什么 | fork 现状 | 判定 |
|---|---|---|---|
| 推理 / 协议 | OpenAI encrypted reasoning 被 400 拒后，请求级删 reasoning 重试一次；Anthropic thinking signature 同规则；成功后写 `signedReasoningReset` 标记并持久化，后续投影删标记前的签名 reasoning。另加投影卫生：跨模型/跨 provider 丢弃、`itemId` 无 encrypted content 丢弃、尾部 reasoning 丢弃 | ✗ 当前把 encrypted_content 错误当终态（`retry.rs:104`、`sampler_turn.rs:1443-1462`），没有修复重试与标记；reasoning 原样回放（`responses.rs:211-217`） | ⭐ **P1** |
| 工具结果 | provider-neutral tool result 带 `isError`，Anthropic 映射 `is_error`；completed part 持久化 `providerIsError`，冷恢复按它还原，与 live 同字节；Hook 用同一判定选 `PostToolUseFailure`（Bash 非零/超时/取消、MCP `isError`） | ✗ `ToolResultItem` 无 `is_error`（`conversation.rs:298-310`），Anthropic `ToolResult` 无该字段（`messages.rs:124-129`）；Hook 只把 MCP 错误路由到 Failure（`tool_calls.rs:40-45,1104`） | ⭐ **P1** |
| 压缩 | summary 后按 read state 重读最近 5 个文件（单文件 5k、总 50k token）补回正文；`task_status` 补运行中与 30 秒内结束的后台任务/agent | 部分：运行中任务/子代理/待办/计划循环已在压缩提醒里（`compaction.rs:1468-1525`、`compaction_context.rs:157-234`）；文件正文不回补，只有图片资产保留 | 🟢 **P1/P2**（文件回补）；task_status 已覆盖 |
| 子代理 | 后台 agent 无进展 15 分钟提醒，之后 30/60… 上限 4 小时；进展 = 模型请求/回复、工具开始/结束、审批结束；Bash 心跳不算。跨进程遗留 `running` agent 对账 | 部分：崩溃对账已有（`agent/subagent/mod.rs:2529-2675`），无停滞计时器 | 🟢 **P2**（仅停滞提醒） |
| MCP | 交互 session 首请求只读已发布快照、不等握手；`search_tool` 报 per-server `pending/needsAuth/failed` 事实；无匹配时最多等 5 秒；执行期按目标 server 等待（同一回复共用 10 秒）；工具定义冻结/离开 | 部分：配置热重载、失败/需要授权公告、执行期 `ensure_initialized` 握手等待已有；首提示前仍等最多 2 秒（`mcp_snapshot.rs:15`、`prompt_build.rs:629`）；`search_tool` 只有 `ready/partial` 与一句通用提示（`search_tool/mod.rs:318-341`） | 🟢 **P2** |
| Skills | 实时目录：监听 skill 根、增量列表指纹、冷恢复不改写、Skill 未命中先 flush 一次、`Did you mean` | 大部分已有：`SkillsFileWatcher` + `reload_skills_from_disk` + `SkillManager` 增量公告 + 已公告名单持久化；缺 `[skills]` 配置变化重载、未命中 flush、`Did you mean` | 🟢 **P2 小件 / P3** |
| Bash | 前台顶层 `sleep N`（N≥25 秒）在 hook/权限前拒绝，带后台 until-loop 指引；非零退出按语义判 `is_error`；超时转后台文案带 `timedOutAfterMs` | 部分：内部 `ToolOutput::is_error` 已按语义判定（`output.rs:619-651`）；无 sleep 拦截；`sleep 999` 会跑到超时（`bash/mod.rs:3174` 测试） | 🟢 **P2 小件** |
| 元上下文 | 回复语言附件（仅变化时追加、压缩后重述）；silent-turn 提醒改为 60 秒时间窗、无每段上限；total-tokens 提醒默认关闭 | 无回复语言；无 total-tokens 提醒（v4 默认关闭，维持不做）；silent-turn 机制不同 | 🟡 **P3**（回复语言）；其余不做 |
| TUI | 无包级改动 | — | — |

## 3. 移植拍板清单

口径沿用 [ADOPTION.md](../agent-cli-tracking/ADOPTION.md) §0：**协议接口兼容性、token 经济学、
TUI 便利性**，不碰架构，不引入内置模型目录/厂商 preset。

| # | 项 | 落点 | 估计 | 状态 |
|---|---|---|---|---|
| R1 | 推理修复持久化 + 投影卫生 | `xai-grok-sampler/src/retry.rs`、`actor/request_task.rs`、`xai-grok-sampling-types/src/error.rs`、`conversation.rs`、`conversation/responses.rs`、`conversation/messages.rs`、`xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs` | 2–3 人天 | ⏳ 待拍板 |
| R2 | 工具结果 `is_error` 上线 + 冷恢复 + Hook 失败路由 | `xai-grok-sampling-types/src/conversation.rs`、`messages.rs`、`conversation/messages.rs`、`xai-grok-shell/src/session/acp_session_impl/tool_calls.rs`、`xai-chat-state` JSONL 回放 | 1–2 人天 | ⏳ 待拍板 |
| R3 | 压缩后文件正文回补 | `xai-grok-shell/src/session/compaction.rs`、`helpers/compaction_context.rs`、新增 read ledger（可放 `xai-grok-shell/src/session/` 或 `xai-grok-tools`） | 2–3 人天 | ⏳ 待拍板 |
| R4 | 后台 subagent 停滞提醒 | `xai-grok-shell/src/agent/subagent/` + session 定时器/通知队列 | 2–3 人天 | ⏳ 待拍板 |
| R5 | MCP 首提示等待收缩 + `search_tool` 状态事实 | `xai-grok-shell/src/session/acp_session_impl/mcp_snapshot.rs`、`prompt_build.rs`、`session/tool_index.rs`、`xai-grok-tools/src/types/tool_index.rs`、`implementations/search_tool/mod.rs` | 1–2 人天 | ⏳ 待量测 |
| R6 | `[skills]` 配置变化触发重载 | `xai-grok-shell/src/agent/app.rs:1319-1324` → `reload_skills_all_sessions()` 或 internal `ReloadSkills` | 0.5 人天 | ⏳ 可随手做 |
| R7 | Bash 前台长 sleep 拦截 | `xai-grok-tools/src/implementations/grok_build/bash/mod.rs` 输入校验 | 0.5–1 人天 | ⏳ 可随手做 |
| R8 | 压缩后 skill 名单增量补发 | `xai-grok-tools/src/types/skill_discovery_tracker/listing.rs:605-615`、`compaction.rs:1465` | 1–2 人天 | ⏳ 先量测 |
| R9 | Skill/工具名 `Did you mean` | `xai-grok-tools/src/implementations/skills/`、`use_tool`/`search_tool` 未命中路径 | 0.5 人天 | ⏳ 可选 |
| R10 | 回复语言附件 | 配置读取 + 每轮输入附件 + `compaction.rs` 重述 | 1 人天 | ⏳ 可选 |
| R11 | subagent 默认后台 / 立即发送转后台 / SendMessage 恢复 | `xai-grok-shell/src/agent/subagent/`、cancel/turn steering | 3–5 人天 | ⏳ 独立决策 |

### 3.1 R1：推理修复持久化 + 投影卫生（P1）

**问题**：fork 在 `encrypted_content` 不匹配时直接给用户「另开 session」（`sampler_turn.rs:1443-1462`），
而 `retry.rs:104` 也把该错误定为 `EmitToSession`；换模型、网关换账号、会话闲置过期都会命中。
fork 已经在 Responses 请求上设 `store: false` 并注入 `include: reasoning.encrypted_content`
（`client.rs:1594-1615`），所以这条链路是活的。

**做法**（按 zcode v4 的两段式）：

1. **请求投影卫生**（低风险，先做）：跨 provider/model 丢弃 encrypted reasoning；`itemId`
   存在但 encrypted content 为空时丢弃；每条 assistant 消息里尾部无 text/tool call 的 reasoning
   丢弃。zcode 的判定在 `docs/openai-responses-tool-continuation-boundary.md:34-40`。
2. **一次性修复重试**：只对 HTTP 400 且命中 `invalid_encrypted_content` / “could not be verified”
   / “has expired” 的响应，删除本请求副本里的 encrypted reasoning，重试一次；Anthropic signature
   同规则（`anthropic-reasoning-history-replay.md:168-206`）。重试不占普通重试预算。
3. **成功后写标记**：在触发修复的 assistant 消息上写 `reasoning_reset: bool`（serde default），
   后续投影删除标记之前的签名/加密 reasoning（`anthropic-reasoning-history-replay.md:219-248`）。
   标记随 chat-state 持久化，冷恢复后仍生效。
4. **复用现有请求级改写路径**：`request_task.rs:381,414-450` 已有 `RetryWithImageStrip` 的
   请求副本改写先例，可新增 `RetryWithReasoningReset`，避免另造一套重试。

**验收**：同一会话第二次请求不再重复 400；修复后的请求与后续请求前缀一致；冷恢复后标记仍在；
普通 400（无关错误）不触发修复；Anthropic 与 OpenAI Responses 各一条夹具。

### 3.2 R2：工具结果 `is_error`（P1）

**问题**：`ToolResultItem` 只有 `tool_call_id/content/images`（`conversation.rs:298-310`），
Anthropic `ToolResult` 也没有 `is_error`（`messages.rs:124-129`，构造点 `conversation/messages.rs:325-329`）。
结果是：live 请求与冷恢复都靠正文里的 “Error:” 让模型猜；Anthropic provider 看不到协议级失败位；
`PostToolUseFailure` 只覆盖 MCP 错误（`tool_calls.rs:40-45,1104`），Bash 非零退出、Read 失败等
仍走 `PostToolUse`。

**做法**：

1. `ToolResultItem` 增 `#[serde(default)] is_error: bool`（如要省略 false，新增一个
   `is_false` helper）；构造函数 `tool_result` / `tool_result_with_images`
   增加错误版或 builder（`conversation.rs:1408-1429`）。
2. 中心成功路径 `tool_calls.rs:3135-3146` 用 `result.output.is_error()` 落位；错误路径
   `tool_calls.rs:2867`、`3539` 直接置 true。`ToolOutput::is_error` 已有完整语义
   （`output.rs:619-651`，Bash 非零、grep 只把 >1 当错）。
3. Anthropic `ContentBlock::ToolResult` 增 `is_error: Option<bool>`，只在该 provider 序列化；
   OpenAI Responses / Chat Completions 忽略（协议无此字段）。
4. `is_mcp_error_result` 改成 `output.is_error()`（或新增谓词），让 Hook 失败路由与 tool_result
   的 `is_error` 共用同一判定；`PostToolUseFailure` payload 的 `is_error` 语义与 zcode
   `hooks.md`「失败的定义」对齐（Bash 非零/超时/取消、MCP `isError`）。
5. 冷恢复：chat-state JSONL 直接 serde 新字段；旧会话缺字段默认 false，不按正文推断
   （与 zcode `session-persistence.md` 的 `providerIsError` 口径一致）。

**验收**：Bash `exit 1` 的 live 请求 Anthropic body 带 `is_error: true`；同一会话冷恢复后字节一致；
`grep` 无匹配（exit 1）不置错；旧 JSONL 载入不报错；Hook 收到 `PostToolUseFailure` 且
`PostToolUse` 不重复触发。

### 3.3 R3：压缩后文件正文回补（P1/P2）

fork 已有压缩后的状态提醒（运行中任务、子代理、待办、计划循环、技能名单、MCP、AGENTS.md、
计划模式），缺的是**最近读过的文件正文**。zcode v4 的规则在
`rewind-compact.md:141-148`：从 compact 前 read state 按时间取 5 个，重读磁盘当前内容；
单文件超 5k token 或超过 Read 上限只放引用提示；总量超 50k 丢弃；跳过 plan 文件、指令文件、
`.git/` 等；不走工具调用流程，只按读权限 deny 规则过滤。

**落点**：新增一个 session 级 read ledger（path、读取时间、内容 hash、是否部分视图），由
Read/Write/Edit 成功路径写入；压缩 summary 返回后选文件、读盘、拼成 user 侧
`<system-reminder>`，与现有 `to_system_reminder` 的附件顺序合并。fork 的
`xai-grok-workspace/src/session/file_state.rs:197-293` 已有 rewind 快照（含正文），但只在
写文件时由 checkpoint 采集，不能直接当 read ledger 用；可复用其 `FlexiblePath` 与快照结构。

**验收**：读 6 个文件后压缩，只补 5 个；大文件只出引用提示；总量超限丢弃；被删文件跳过；
不触发权限弹窗；补回内容随 compaction 持久化，冷恢复不再读盘；append/prune 模式下不重复
补回保留段里已有的文件。

### 3.4 R4：后台 subagent 停滞提醒（P2）

fork 已有崩溃对账（`agent/subagent/mod.rs:2529-2675`：`ORPHAN_RECONCILE_REASON`、
`running_orphan_meta`、`reconcile_orphaned_subagents_with_backend`），缺的是**运行中无进展**的提醒。
按 zcode `07-subagent.md:116-146`：

- 进展 = child 新模型请求/回复、工具开始/结束、审批结束；Bash 心跳与流式 token 不算；
- 15 分钟首次，之后 30/60… 上限 4 小时；有新进展清零；
- 投递前复验：agent 已结束、有新进展、在等审批、executionId 变了就丢弃；
- 提醒措辞避开 `<task-notification>`，停还是等由主代理决定。

**落点**：subagent registry 增加 `last_progress_at` / `in_flight_tools` / `waiting_for_permission`；
父 session 用一个分段定时器检查并注入 append-only reminder。prior 估算 2–3 人天
（`agent-cli-refresh-2026-11/README.md` §3），v4 只补了参数与投递复验细节。

**验收**：卡死 agent 只提醒一条；正常长工具不误报；提醒前 agent 已结束则不投递。

### 3.5 R5：MCP 首提示等待与状态事实（P2，先量测）

fork 的执行期握手等待已有（`servers.rs:3645 ensure_initialized`，`mcp.rs:1144 is_server_pending`），
失败/需要授权公告已有（`mcp_failed_reminder.rs`），缺的是：

- 首条用户消息前的 2 秒 grace（`mcp_snapshot.rs:15`、`prompt_build.rs:629`）。zcode v4 对交互
  session 改为只读已发布快照、不等握手（`mcp-async-tool-catalog-cache.md:5`）。
- `search_tool` 的 per-server 事实。现在只有 `ready/partial` 与 “Some MCP servers are still
  connecting”（`search_tool/mod.rs:318-341`）；`SearchSnapshot` 只有 `is_ready`
  （`tool_index.rs:32-38`），server 状态在 `McpServerStatusPayload`（`mcp_dispatcher.rs:48-77`）
  只发给客户端。zcode 在无匹配时附 pending/needs-auth/failed 三段
  （`mcp-async-tool-catalog-cache.md:167-181,226-241`）。

**做法**：先把三种状态塞进 `ToolMetadataSnapshot`/`SearchSnapshot`，`search_tool` 输出
`pending` / `needsAuth` / `failed` 数组与文字；再量测首提示前 grace 的实际命中率。
若 MCP 握手经常 >200ms，交互 session 改为 0；headless/automation 保留等待。
`WaitForMcpServers` 工具不单独引入，执行期等待已覆盖。

**验收**：有慢 server 时首提示 TTFT 不再固定吃满 2 秒；`search_tool` 能区分 pending 与
needs-auth；无 MCP 会话的 tools 段字节不变。

### 3.6 R6 / R7 / R8：低成本小件

- **R6**：`ConfigUpdate::Skills` 现在只打日志（`app.rs:1319-1324`），改 `[skills] paths`
  需重启或手动 `/reload`。文件监听器只覆盖 skill 目录，不覆盖 config.toml；补一条
  `reload_skills_all_sessions()` 即可。验收：改配置后一次重载，slash 列表与新目录一致。
- **R7**：在 Bash 输入校验阶段拒绝顶层 `sleep N`（N≥25 秒），放在 hook/权限之前；
  允许 `FOO=1 sleep 30`、`timeout 40 sleep 30`、`echo a; sleep 30`、`sleep 25s`；提示改用
  后台 until-loop 或 `run_in_background`。zcode 规则见 `04-bash.md:141-160`。
- **R8**：压缩提醒总是用 `format_compaction_skill_listing` 发全量名单
  （`listing.rs:605-615`）；append/prune 模式的保留段可能已含同一名单。先量一次实际重复
  token；若 >1k，按 zcode 的 `skillsListingDelta.fingerprints` 从保留段回放已发行，只补缺失项
  （`skills-live-catalog.md:82-90`）。

## 4. 不移植（负面清单增补）

- **文档/设计插件全家**：`documents-plugin`、`spreadsheets-plugin`、`pdf-plugin`、
  `presentations-plugin`、`design-plugin`。前四者绑 LibreOffice 预装、字体下载、转换器与
  平台包；`design-plugin` 是 HTML 设计稿/HTML deck 技能，与终端编码 harness 无关。
- **`node-repl-host`**：Worker/宿主形态，fork 无 node_repl。
- **browser-state / CUA broker / site plugin / draft model migration / memory-diagnostics**：
  绑桌面、Web、官方服务端或诊断管线；`--browser-state`、`--no-browser-state` 等新 flag 同族。
- **provider-regression 夹具、SEA 跨目标打包、i18n 界面语言铺开**：工程/发行侧资产，
  与 CLI 运行时优化无关。
- 维持 ADOPTION §7.2 / §8.2 / §9.3 全部负面清单（dynamic-workflow TS 工作流、
  model-option-map DSL、off-peak 票据、CUA/移动端/office viewer/Gen UI 等）。

## 5. 防误报基线（本轮实际核实）

| v4 看起来新增 | fork 现状 |
|---|---|
| Skills 实时目录 | ✅ 大部分已有：`SkillsFileWatcher`（`config/watcher.rs:674`，`agent/app.rs:164-176` 注入 internal `reload_skills`）、`reload_skills_from_disk`（`session_setup.rs:201`）、`SkillManager` 增量公告、已公告名单持久化（`builder.rs:96,1197`、`announcement_state.rs:20`）。缺 `[skills]` 配置臂与 `Did you mean`；Skill 未命中 flush 未核 |
| MCP failed/needs-auth 公告 | ✅ 已有：`mcp_failed_reminder.rs` 分集公告、`McpAnnounced` 去重、auth 升级重公告 |
| 压缩后 `task_status` | ✅ 已有：运行中后台任务、子代理、待办、计划循环、工作流都在压缩提醒里（`compaction.rs:1468-1525`、`compaction_context.rs:157-234`）。缺 30 秒内已结束任务的细节，影响面小 |
| subagent 跨进程崩溃对账 | ✅ 已有：`agent/subagent/mod.rs:2529-2675`，含 process-restart 原因、`running` 孤儿识别与一次性 `SubagentFinished` |
| MCP 配置热重载 | ✅ 已有：`agent/app.rs:1256-1280`，global 与 project 两路 |
| MCP 执行期等待 | ✅ 已有：`servers.rs:3645 ensure_initialized` 会唤醒 `Initializing` 上停等的调用方；`mcp.rs:1144` 有 pending 判定 |
| output-limit 续写提醒的缓存稳定性 | ✅ 已有：`push_system_reminder` 写入 chat-state（`reminders.rs:569,591-593`，`turn.rs:3800-3803`），冷恢复回放原文；zcode 的 hidden record 是同一目的的不同实现 |
| Prompt Snapshot / total-tokens | Prompt Snapshot 的缓存收益已由「顶层 resume 保留旧 head」覆盖（见 agent-cli-refresh §1）；fork 无 total-tokens 提醒，v4 默认关闭，维持不做 |
| MCP structured content 去重、ToolSearch deferred builtins、配置深合并 | ✅ 已有（`call_result.rs`、`search_tool`/`use_tool`、`deep_merge_toml`） |
| TUI | v4 无 TUI 包改动 |

## 6. 验证计划

| 项 | 最小验证（零/低编译优先） |
|---|---|
| R1 | 纯函数投影单测：跨模型、id-without-encrypted、尾部 reasoning；sampler 夹具 400 → 重试一次 → 第二次请求无该 reasoning；冷恢复标记仍在；无关 400 不修复 |
| R2 | `ToolOutput::is_error` → `ToolResultItem` 映射单测；Anthropic body snapshot；旧 JSONL 载入默认 false；Hook 路由测试（Bash 1 / grep 1 / MCP error） |
| R3 | 压缩测试：6 文件取 5、5k 引用提示、50k 丢弃、删除文件跳过、deny 规则跳过、持久化后冷恢复不读盘 |
| R4 | 注入时钟测 15m/30m/60m/4h；进展清零；结束/审批/executionId 变化不投递 |
| R5 | `search_tool` 三段事实快照；无 MCP 会话字节不变；慢 server 首提示 TTFT 对照（改动前后各一次） |
| R6 | config watcher 单测：改 `[skills] paths` → 只发一次 reload；slash 列表更新 |
| R7 | 校验单测：`sleep 25` 拒、`sleep 24` 放、赋值前缀放、`timeout 40 sleep 30` 放、`echo a; sleep 30` 放、`sleep 25s` 放、background 放 |
| R8 | append/prune 压缩前后 skill 段 token 计数；保留段已有名单时不重复 |

验证入口按 AGENTS.md：`scripts-local/ctest.sh -p <pkg> --lib`；先用 `cargo check -p <pkg>`
做类型验证，行为验证按模块过滤，禁止裸跑全量。R1/R2 会动 `xai-grok-sampling-types` 与
`xai-grok-sampler`，先确认改动面再决定编译批次。

## 7. 待办台账

- [ ] **R1** 推理投影卫生（零重试部分）先落地，再加 400 修复重试与 `reasoning_reset` 标记
- [ ] **R2** `ToolResultItem.is_error` + Anthropic wire + Hook Failure 路由，作为一个提交
- [ ] **R3** read ledger 与压缩后文件回补；先复用现有 compaction 附件顺序
- [ ] **R4** subagent `last_progress_at` + 分段定时器 + 投递复验
- [ ] **R5** 先量首提示 grace 命中率；再决定是否 0，以及 `search_tool` 状态事实的字段形态
- [ ] **R6 / R7** 可随手做，独立提交
- [ ] **R8** 先量 token，再决定是否做增量名单
- [ ] **R9 / R10 / R11** 等拍板，不随 P1/P2 批次
- [ ] 3.17 遗留 `hook onError` + interrupt-marker（ADOPTION §9.1.6）仍独立，勿与 R2 混在一起

## 8. 刷新方法与存疑

```bash
# 本机直连 github.com 的 ls-remote 超时；远端查询与 fetch 走代理：
git ls-remote https://ghproxy.net/https://github.com/zai-org/ZCode.git HEAD refs/heads/main
git -C D:/CODE/ai/zcode fetch --all --tags
git -C D:/CODE/ai/zcode log --oneline a9fe401e..HEAD
git -C D:/CODE/ai/zcode diff --stat b95ebaf..HEAD
# 新增资产判定必须双版本树核对：
git -C D:/CODE/ai/zcode ls-tree -r b95ebaf | grep <path>
git -C D:/CODE/ai/zcode ls-tree -r HEAD    | grep <path>
```

存疑：

- v4 的 reasoning 修复、MCP 动态目录、skills 实时目录的证据主要来自设计文档与仓库内单测；
  **未跑真实多网关 BYOK**，尤其 OpenAI-compatible 网关对 `include: reasoning.encrypted_content`
  与 `store:false` 的实际行为需 `[api_dump]` 现场取证。
- zcode 自报 skills watcher 在 Windows 网络共享路径上有过「绑定反复报错」回归
  （`skills-live-catalog.md:63-74`，1 秒绑定超时 + 2 次失败停用）。fork 的 watcher 目前只记日志，
  本机若出现同类挂起再考虑加超时/断路，不预先改。
- R5 的收益取决于本机 MCP 握手耗时；`codebase-memory-mcp` 与 `ssh-mcp` 的实测分布未采。
- R3 的 5k/50k 上限来自 zcode 的 token 估算口径，fork 的估算器与截断器不同，落地时按本仓
  `xai-token-estimation` 重新标定。
