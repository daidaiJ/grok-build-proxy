# kimicode（Kimi Code CLI）× 本仓库 特性移植预研

> **状态：选型完成（2026-10-02）。** P1 已出设计原型分文档
> [`rewind-branch-undo.md`](rewind-branch-undo.md)：fork 摸底推翻了本文的一个前提
> （持久层已是 append-only + `RewindMarker` 分支标记，落点不是 sqlite-journal），
> D1 推荐分支树路线，**2026-10-03 随移植路线图拍板为分支树路线**（排序①，并入
> step-code branch summarization 同题需求，见
> [`docs-local/port-roadmap.md`](../port-roadmap.md)）。P2 维持并入
> deferred-tool-exposure 定稿（路线图排序④）、P3 列为路线图随手件，均未开工。
> 调研快照：2026-10-01；kimi-code 基线 `MoonshotAI/kimi-code` main `21406fb`
> （2026-09-30，v2.1.1 之后）；本地 clone `D:\CODE\ai\kimi-code`。
> fork 基线：main `2b8adae`（2026-10-01）。

## 对象是什么

Kimi Code CLI = Moonshot AI 的开源（MIT）终端 coding agent，TypeScript pnpm
monorepo，替代已归档的 Python 版 `kimi-cli`。包地图：

| 包 | 角色 |
|---|---|
| `agent-core-v2` | 核心引擎，自称 "DI × Scope"（仿 VS Code 的 DI 容器 + app→session→agent 三层生命周期） |
| `kosong` | LLM 抽象层（anthropic/kimi/openai 双代/google-genai 五 provider + capability 体系） |
| `kaos` | 执行环境抽象（local + **SSH 真实现**，SDK 层可把工具执行/持久化指向远程主机） |
| `pi-tui` | **fork 自 earendil-works/pi 的 TUI**，同步点 `53816d7`（0.85.1+） |
| `minidb` | 自研嵌入式 KV（Redis AOF + SQLite WAL 混合思路，纯 Node 零依赖），只做会话索引不存正文 |
| `acp-server` / `klient` / `node-sdk` | ACP 编辑器协议 / 强类型客户端 SDK / 嵌入式宿主 |

会话事实 = 每 agent 一份 append-only `wire.jsonl`（事件溯源，折叠重建状态而非逐步重放）。

可信度分级：fork 侧事实 = **A**（本机 grep/读码，锚点随文）；kimi-code 侧 = **A⁻**
（委派 4 个只读 agent 深读源码，锚点随文但未逐条人工复核）；收益/人天估算 = **C**。

## TL;DR

1. **最值得移植的一件：`/rewind` 从"破坏性截断"升级为 wire 分支树 undo**。fork 已有
   `/rewind`（`actions/defaults.rs:425`，丢弃后续回合、文件不动），kimi 的 undo 在同样
   的 append-only journal 前提上做成了**非破坏分支**：旧分支永久保留可审计、undo 后把
   原 prompt 放回编辑器供修改重发、"能撤到哪"由 compaction 边界精确计算。这是 HN 上
   被 K3 发布帖点名的特性，且与本仓库 journal 确定性重放路线完全兼容。
2. **第二件：`select_tools` 延迟工具声明**——与 workflow-pi-port 已选型的 P2
   （`deferred-tool-exposure.md`）是同一道题的第二个独立实现，且比 pi 的方案多两个
   可吸收点：公告式增量流（`<tools_added>/<tools_removed>`）+ 历史 schema 持续剥离
   （`stripDynamicToolContext`）+ capability 门（`dynamically_loaded_tools`）。
   P2 施工前值得一并对照定稿。
3. **子 agent 体系两个小而硬的设计**：结果信封（`agent_id/status/stop_reason/
   summary/resume_hint/next_step`，stop reason 映射 next_step，失败可 resume 同会话）
   与委托图约束（能委托别人的 profile 不能被别人委托，从声明层保证委托链终止）。
   workflow 子 agent 的 resume 语义本 fork 已领先，这两个是纯增益。
4. **不做/已有的别动**：hooks（`xai-grok-hooks` 已有等价物且 fail-open 语义一致）、
   plugin marketplace（已有）、minidb/持久化四接口（sqlite-journal 已覆盖主场景）、
   ACP（`xai-acp-lib` 已有）、视频输入（无对应模型能力位，不做）。

## 对照矩阵

| kimi-code 特性 | fork 现状 | 判定 |
|---|---|---|
| wire 分支树 undo（非破坏 + prompt 放回 + compaction 边界） | `/rewind` 破坏性截断（`router.rs:1592` dispatch 族）；`xai-chat-state` 有快照原语（`types.rs:26`） | ⚠️ **升级候选 P1** |
| `select_tools` 延迟工具声明 + 公告流 + 历史剥离 | 无（workflow-pi-port P2 同题，已有设计原型） | ⚠️ **并入 P2 对照** |
| 子 agent 结果信封 + resume 同会话 | workflow 子 agent 有确定性 resume（领先）；无信封/next_step 映射 | ⚠️ 增益 P3 |
| 委托图约束（能委托者不被委托） | `xai-grok-subagent-resolution` 现状未核实 | 待核实后判定 |
| 输入队列 | `xai-prompt-queue`（排队 + 合并规则），回合结束发送 | ✅ 已有 |
| Ctrl-S steer（注入运行中回合） | `xai-interjection-core` 机制层已有回合内注入通道（Ctrl-S 键位入口未对照） | ✅ 机制已有（键位入口待对照，2026-10-02 订正，原判"无"有误） |
| 审批 "approve for session"（压入同一规则引擎的 session 作用域） | 审批/settings 面已有 allow 类选项，具体作用域模型未核实 | 待核实后判定 |
| lifecycle hooks（20 事件点，0/2/fail-open 退出码） | `xai-grok-hooks`：7 事件、pre_tool_use 可 allow/ask/deny + 改写入参、fail-open 一致 | ✅ 已有（可补 Pre/PostCompact、PermissionRequest/Result 等事件位） |
| `/mcp-config`（AI-native 对话式 MCP 配置 = 内置 SKILL.md） | `xai-grok-mcp` 传输/OAuth 全有；对话式配置无 | 思想可吸收（见决策 D4） |
| plugin marketplace + trust 前置披露 | `xai-grok-plugin-marketplace` 已有 | ✅ 已有 |
| kosong capability 冻结 UNKNOWN + 按能力裁剪请求 | `xai-grok-models` 现状未核实 | 待核实后判定 |
| 工具结果 >50k 字符外置 `output_path` 文件 | 有 tool-output-compression-plan（未落地） | 呼应已有计划 |
| 视频输入（三路降级 + 服务端代传） | 无对应能力位 | ❌ 不做 |
| minidb / 持久化四接口 / SSH kaos / ACP / node-sdk | sqlite-journal、xai-acp-lib 已覆盖 | ❌ 不做 |
| pi-tui 的 UPSTREAM.md intent-card fork 治理 | 上游同步靠人工裁定（sync-upstream-2026-09-23 踩过 52 文件冲突） | 💡 流程思想，可移植到同步纪律 |

## 候选特性关键设计详解

### P1：wire 分支树 undo（`/rewind` 升级）

kimi 实现：`agent-core-v2/src/agent/undo/undoService.ts` + `src/wire/`。

1. **撤销 = 分支，不删除**。wire.jsonl 追加 `SwitchEdge` 记录（`switchBranch({turns,
   fromTurnId})`，`wire/tree/fork.ts` 的 `computeForkLine`/`buildUndoSwitchRecords`），
   旧分支内容永久保留；`context.undo` 记录语义是声明性 retract。可审计、可撤回撤销。
2. **可撤销边界可精确计算**：`ForkLineError(reason: 'compaction_boundary' |
   'insufficient')`——undo 不允许越过 compaction 摘要边界（摘要已把旧 wire 折叠，
   撤过它状态就不再自洽）。fork 侧对应物：workflow journal 的重放起点天然就是边界，
   语义可直接映射。
3. **undo 后的"如何继续"**：TUI 把该 checkpoint 的**原始用户输入放回编辑器**
   （`tui/commands/undo.ts:193-208` `host.restoreInputText(choice.input)`）供修改重发，
   而不是让模型自己续跑。这是 HN 好评的核心体验。
4. 文件级快照是另一套（`src/features/fileHistory`，按 turn×phase 存 BlobStore，
   上限 4MB/文件）——kimi 自己的 `/undo` 也不回滚代码改动，与 fork `/rewind`
   "文件不动"语义一致，无需移植。

fork 落点（2026-10-02 摸底更正）：`updates.jsonl` 持久层的 `RewindMarker`（已等价于
kimi 的 SwitchEdge，但语义是单向折叠）+ pager `/rewind` picker 改造；
`xai-sqlite-journal`（SQLite 模式选择器）与 `xai-grok-session-events`（遥测）**不是**
会话历史 journal，原判有误。且 prompt 放回编辑器（下第 3 点）fork **已有完整等价物**
（`app/dispatch/rewind.rs:396`），实际差距只剩"分支可往返 + 旧分支点可见 + 边界预计算"。
详见 [`rewind-branch-undo.md`](rewind-branch-undo.md) §1/§3。风险：`xai-chat-state`
现有快照 rewind 与分支树两条路线需拍板取舍（见决策 D1）。

### P2：select_tools 延迟工具声明（并入 deferred-tool-exposure 选型）

kimi 实现：`agent/toolSelect/toolSelect.ts` + `toolSelectService.ts`。要点：

1. 工具默认 `deferred` 不进 system prompt，只在一条**公告消息**里列名字
   （`<tools_added>/<tools_removed>`，`dynamicTools.ts:88-110`）；模型调
   `select_tools(names)` 后完整 schema 下一轮才进上下文。
2. **历史剥离**：`stripDynamicToolContext` 把历史中已折叠的动态 schema 消息从上下文
   持续剥掉——省 token 是持续性的，不只是首屏。
3. **capability 门**：只有 `dynamically_loaded_tools === true && tool_use` 的模型才
   允许 message-level tools（`toolSelectService.ts:87-88`）——请求发出前按能力裁剪，
   而不是发出去等上游报错。
4. 与 fork 已有 P2 原型（exposure 三档 + BM25）的差异：kimi 无 BM25/搜索，靠公告 +
   按名展开；"公告是可折叠增量流、工具可中途移除"两点值得吸收进设计原型。

### P3：子 agent 结果信封 + 委托图约束

1. **结构化结果信封**（`agentTool.ts:756-774` `formatForegroundAgentSuccess`）：
   `agent_id / actual_subagent_type / status / stop_reason / [summary] / resume_hint /
   next_step`——每个 stop reason（max_tokens / max_steps / 用户取消 / repeat-breaker）
   映射到不同的 next_step 指令；失败时 `Agent(resume=agentId)` 复用同一子会话续跑
   （子 agent 会话是持久实体，跨重启可重建 `rebuildSubagent:439`），而非重派全新
   agent 白烧 token。
2. **final message 即唯一 handoff**：父只收最后一条 assistant 文本，中间过程留在子
   agent 自己的上下文里；后台任务输出持久化到文件（超 16MiB 直接杀并提示重定向），
   `TaskOutput` 只回 4KiB 尾部 preview。
3. **委托图约束**（`profile-shared.ts:75-94` `profileCanDelegate`/
   `withoutDelegationTargets`）：任何"能派 agent 的 profile"自动从别人的委托
   allowlist 中剔除——委托链终止性在声明层保证，比运行时深度计数干净。

### 小件清单（不单独立项，随相邻专题顺带）

- **Ctrl-S steer**：流式中 Enter=排队、Ctrl-S=注入运行中回合（`tui/utils/
  steer-input.ts`）；bash 类队列项不可 steer。fork 队列已有，缺 steer 通道。
- **审批 "approve for session"**：会话内批准过的调用模式压入与配置规则**同一套**
  pattern 匹配引擎的 session-runtime 作用域（`session-approval-history.ts`）——一次
  实现同时支持配置白名单/会话记忆/turn 级覆盖三种作用域。设计模式比功能本身值钱。
- **NotifyPanel 多通道通知**（`notify-panel.ts`）：按 agent 分通道 tab + 未读点 +
  回合结束折叠一行 stub，解决多后台 agent 通知噪音。
- **UPSTREAM.md intent-card fork 治理**（`packages/pi-tui/UPSTREAM.md`）：fork 的每
  个上游行为差异记成卡片（decision / why-not-in-app / keep|absorbed），同步时逐文件
  diff 核对、禁止静默携带。可直接套用到本仓库上游同步纪律（本次 52 文件冲突人工裁定
  的痛点）与 kimi-code 仓库本身的 pi-tui 同步。
- **"/mcp-config = 内置能力写成 SKILL.md"思想**（`features/skill/catalog/builtin/
  mcp-config.md`）：对话式配置不硬编码 UI，而是 prompt 文档驱动模型 + 每个 needs-auth
  server 自动生成 `mcp__<server>__authenticate` 工具。

## 决策记录（预研阶段）

- **D1（2026-10-03 拍板：分支树路线，随移植路线图排序①）— undo 路线**：分支树（journal 追加）vs 现有 chat-state 快照 rewind。
  2026-10-02 摸底后倾向明确为**分支树**：fork 的 updates.jsonl 已是 append-only + RewindMarker
  分支标记，升级 = SwitchEdge 化而非新建 journal；证据与分层清单见
  [`rewind-branch-undo.md`](rewind-branch-undo.md) §3/§4。
- **D2 — select_tools 并入 workflow-pi-port P2 施工**：同一道题两参照（pi 的
  search_tools/BM25 + kimi 的公告流/历史剥离），P2 开工前合并对照，不另立专题。
- **D3 — hooks/插件市场/持久化/ACP 不移植**：fork 已有等价物；kimi 多出的事件位
  （Pre/PostCompact、SessionHeartbeat、PermissionRequest/Result）登记为 `xai-grok-hooks`
  的候选增强项，不因本次调研单独立项。
- **D4 — /mcp-config 不照搬**：本 fork MCP server 数量级远小于 kimi 目标用户场景，
  对话式配置收益有限；只吸收"内置能力写成 prompt 文档"的思想（与 fork skills 体系对接）。
- **D5 — 视频输入不做**：依赖 provider 侧 `video_in` 能力位与 Kimi 服务端代传通道，
  fork 供应商面无对应物。

## 分期路线（拟，全部未立项）

| 期 | 内容 | 关系 | 规模（C 级粗估） |
|---|---|---|---|
| P1 | `/rewind` 分支树升级（RewindMarker SwitchEdge 化 + prompt 放回 + 边界计算） | 依赖 D1 拍板；落点为 updates.jsonl，非新 journal；设计原型见 [`rewind-branch-undo.md`](rewind-branch-undo.md)（T1–T3 合计 5–7 人天） | 净开发 5–7 人天 |
| P2' | deferred-tool-exposure 施工前对照 kimi 方案补强设计原型 | workflow-pi-port P2 的输入，不独立立项；另见 step-code-port survey P2/D2（branch summarization 属同题第三参照） | 0（并入 P2） |
| P3 | 子 agent 结果信封 + next_step 映射 + 委托图约束 | `xai-workflow` 子 agent 面 | 净开发 2–4 人天 |
| 随手 | steer、session-approval 作用域、NotifyPanel、intent-card 治理 | 各自挂靠相邻专题 | 各 ≤1 人天 |

## 来源

- 仓库：`D:\CODE\ai\kimi-code`（upstream `21406fb`，2026-09-30）；changelog
  `docs/en/release-notes/changelog.md`（0.36→2.1.1 特性时间线）。
- 关键源码锚点见各节；fork 侧锚点：`crates/codegen/xai-grok-pager/src/actions/defaults.rs:425`、
  `app/dispatch/router.rs:1592-1600`、`xai-chat-state/src/types.rs:26`、
  `xai-grok-hooks/src/lib.rs`、`xai-prompt-queue/src/lib.rs`。
- 官方文档：README Key Features、docs/en/{guides,customization,reference}。
- HN（Kimi K3-256k 帖）对 checkpoint/revert 体验的第三方佐证。
