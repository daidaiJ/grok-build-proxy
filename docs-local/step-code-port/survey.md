# Step-Code（阶跃星辰 Step Code CLI）× 本仓库 特性移植预研

> **状态：选型定稿（2026-10-03，入 [`docs-local/port-roadmap.md`](../port-roadmap.md)）。**
> P1（bash AST 静态安全分析）排序②；P2（branch summarization）并入 kimicode-port P1
>（D2 落实，随路线图排序①施工）；P3（流恢复注入）列为路线图随手件；
> P4 待 fork spawn 链路核实后判；P5/P6 不立项。
> 本文保留调研全文，选型与施工顺序以路线图为准。
> 调研快照：2026-10-02；Step-Code 基线 `stepfun-ai/Step-Code` main `519e4de4`
> （2026-09-30，PR #204 merge）；本地 clone `D:\CODE\ai\Step-Code`（shallow）。
> fork 基线：main `c3e6c3c`（2026-10-02）。

## 对象是什么

Step Code = 阶跃星辰（StepFun）的开源（MIT）终端 coding agent，TypeScript pnpm
monorepo，573 星（2026-10-02 gh api），非测试 TS 源码约 5.0 万行 / 1434 文件。
2026-06 建仓，上游活跃（last push 2026-10-01）。自述亮点：token 效率、长程任务
委派（`/goal` + `/cron`）、静态站发布（steppage 插件）、Claude Code 插件/MCP
配置兼容。包地图：

| 包 | 角色 |
|---|---|
| `packages/agent-core` | 核心引擎（自称 step-harness）：agent loop、reducer、session、compaction、skill 发现 |
| `packages/coding-agent` | 产品层：core（session/tools/extensions）+ features（goal/cron/workflow/subagent/plan）+ step/（provider 专属：权限、MCP、遥测） |
| `packages/providers` | LLM 抽象层（多 provider + retry/stream 分类） |
| `packages/tui` | 自研 TUI（主屏 + 备用屏双布局模型，`tui-plan.md` 1001 行设计文档） |
| `packages/telemetry` / `contracts` / `config` | 遥测、契约、配置 |
| `plugins/playwright` / `steppage` | 官方插件（playwright 控制 / 静态站一键发布） |

工程治理面很有特色：`pnpm check` 挂了 15 个自写架构守卫脚本（layer-direction、
tui-no-ai、no-provider-dispatch、no-secret-leak、public-boundary 等）；
`docs/` 内多份"运行时契约文档"（goal-lifecycle / orchestration-lifecycle /
command-permissions）写得像规范文本，含与 Claude Code 的逐项差异表。

会话事实 = append-only session 文件 + **树状分支**（`branch()` API，见 P2）。

可信度分级：fork 侧事实 = **A**（本机 grep/读码，锚点随文，否定结论均有 grep 佐证）；
Step-Code 侧 = **A⁻**（逐文件读码，锚点随文已人工看过原文；未跑构建）；收益/人天估算 = **C**。

## TL;DR

1. **最值得移植的一件：bash 命令 AST 静态安全分析（三态判定 + 明确"分析不完备"态）**。
   Step-Code 用 pinned 的 `unbash` 解析器对命令做 typed AST 遍历（`shell-analysis.ts:75`
   `inspectShellScript`），区分 `bash -c` 内嵌命令、数组数据、算术展开、heredoc 数据
   （`command-policy.ts`），产出 hazardous / **analysisIncomplete** / ok 三态；"解析不了"
   不冒充"检测到危险"，且无审批通道时 deny 收口、`--non-interactive-denial continue`
   可选降级为失败工具结果让 agent 续跑。本 fork 的 bash 权限停在参数级 requirement
   表达式（`bash/mod.rs:1489`）+ hooks，无命令内容静态分析（全仓库 grep 0 命中）。
   这是 headless/无人值守场景的安全基座，也是 fork 现有审批 UX 的直接增强。
2. **第二件：会话分支树 + "离开分支时自动摘要"**。Step 的 session 是树
   （`session-manager.ts:1361` `branch()`、`:1261` `getBranch()`），且导航到另一分支时
   **给被离开的分支生成摘要**（`branch-summarization.ts`，提取 fileOps + 摘要随行），
   上下文不丢；摘要请求自身超限时按"工具调用配对原子组"降源重试
   （`summary-overflow.ts:14`）。与 kimicode-port P1（wire 分支树 undo）同题加强——
   kimi 解决"撤回"，Step 解决"切回来不丢上下文"，两条互补。fork 无分支（chat-state /
   session-events / sqlite-journal 全 grep 0 命中）。
3. **小而美：不完整流恢复注入**（`step-stream-recovery.ts:38`）。检测"stream ended
   before/without"类可重试错误后，下一轮注入一条 **projection-only**（不落盘、非合成
   用户消息）的恢复提示：指示模型改用更小响应、分块写大文件、勿重发大 payload。
   （`step-stream-recovery.ts:24-35`）
   fork 已有 Length 截断的 salvage（`xai-grok-sampler/src/client.rs:2438-2448`）与
   retry 分类，但"中断后轻量续作指令注入"没有对应物。≤1 人天。
4. **子 agent 进程面的两条防线**：① 一层 fan-out 用无条件环境标记收紧
   （`STEP_DISABLE_WORKFLOW/GOAL/CRON=1` + `STEP_CLI_SUBAGENT_CHILD=1`，声明在
   `buildSubagentChildEnv` 单点）；② 子进程 **输出驱动** idle watchdog（任何
   stdout/stderr 字节都续期，默认 30 min，`rpc-adapter.ts:137` 可调）防止哑子进程
   挂死父回合。fork 有隔离 spawn（`xai-grok-shell/src/agent/subagent/child_runtime.rs`）
   但未见等价的 env 标记契约与 idle watchdog（见未核实项）。
5. **不做/已有的别动**：goal 全家桶（fork 已有且更强——completion classifier、
   `no_progress_paused`、`token_budget`）、plan mode / todo / ask_user_question /
   MCP 导入 / secret 脱敏 / workflow token 预算 / steer 注入 / 插件市场 / 遥测
   全部已有；alt-screen 布局（fork 是 ratatui 原生全屏 TUI）、stdout takeover
   （Node 特有）、steppage（产品绑定）不做。

## 对照矩阵

| Step-Code 特性 | fork 现状 | 判定 |
|---|---|---|
| bash 命令 unbash-AST 安全分析 + 三态判定 + `analysisIncomplete` 显式化 | bash 权限 = 参数级 `ToolRequirement` 表达式（`bash/mod.rs:1489`）+ hooks allow/ask/deny；命令内容静态分析全仓库 grep 0 命中 | ⚠️ **移植候选 P1** |
| `--non-interactive-denial continue`（headless 审批阻塞降级为失败工具结果） | 未核实（headless 回合遇审批阻塞的行为未查） | 并入 P1 对照 |
| 会话树 `branch()` + 切分支 branch summarization + summary-overflow 降源 | 无分支树（chat-state/session-events/sqlite-journal grep "branch" 0 命中）；`/rewind` 为破坏性截断 | ⚠️ **并入 kimicode-port P1 对照定稿** |
| step-stream-recovery（不完整流 → projection-only 续作指令注入） | Length 截断有 salvage（`client.rs:2438`）；流中断走 retry 重发，无续作指令注入（`retry.rs:245-257` 仅文案） | ⚠️ 小件候选 P3 |
| 子 agent env 标记防递归 fan-out（`buildSubagentChildEnv` 单点契约） | 隔离 spawn 已有（`child_runtime.rs`）；等价 env 契约未核实（`GROK_*DISABLE*` grep 仅命中嵌入式搜索工具开关） | ⚠️ 待核实后判定 P4 |
| 子进程输出驱动 idle watchdog（30 min 默认、字节续期） | 未见（subagent 目录仅测试用 timeout；全目录 grep "watchdog/idle_timeout" 无产品命中） | ⚠️ 小件候选 P4 |
| `cron_create` 五字段 cron 表达式 + 投递 offset/DST + durable 项目级任务 | scheduler 已有（`grok_build/scheduler/`，`types.rs:311` 持久化）但是 **interval 型**（`intervalSecs`），grep 无 cron 表达式/DST | 差异点，看场景（P5，暂不立项） |
| ultraloop/ultracode 关键词工作流入口 + prompt 内 `+500k` 预算指令 | 无关键词入口（grep 0 命中）；workflow 工具 + token/agent 双预算已有（`xai-workflow/src/lib.rs:17-18`、`host.rs:54`） | 小件候选 P6（UX 入口） |
| workflow 前台工具行级进度投影（running/queued/cached/failed/花费列，`progress.ts:35`） | 有 tasks pane（`xai-grok-pager/src/views/tasks_pane.rs`）；逐 agent 行级投影细节未对照 | 待核实后判定 |
| HoH（Harness-of-Harness）`iterate()` 原语（`hoh.ts` 结构化 Planner/Developer 契约） | workflow 引擎已可表达编排原语（workflow-pi-port 结论）；HoH 是方法论包装 | 观察 |
| request-time context projection（salient-line 裁剪 + 标记，`harness/compaction/projection-*.ts`） | 多层 compaction 引擎已有（`xai-grok-compaction`）+ tool output compression 已有；等价面待对照 | 观察（避免重复建设） |
| `/goal` 目标生命周期（budget/pause/resume/clear 全语义） | **已有且更强**：`goal_enabled`/`goal_classifier_enabled`（`xai-grok-config-types/src/lib.rs:491-500`）、`GoalDisplayStatus`（`pager/app/agent.rs:345-353`）、`token_budget` + `no_progress_paused`（`shell/extensions/notification.rs:1033-1039`）；Step 自己把 completion classifier 列为 follow-up | ✅ 已有（fork 领先） |
| plan mode（enter/exit 工具 + 审阅对话框） | `EnterPlanModeTool`/`ExitPlanModeTool`（`xai-grok-agent/src/config.rs:148-149`） | ✅ 已有 |
| 澄清工具（clarify，多问题多选项 + freeform） | `grok_build/ask_user_question/` 已有 | ✅ 已有 |
| todo/task 列表 + 跨扩展导入协议 | `grok_build/todo/` + TodoWriteTool + TodoGate 催办（`system_reminder.rs:3-14`）已有 | ✅ 已有 |
| Claude/Codex MCP 配置自动导入 | `/import-claude` 已有（扫 `.claude/settings*.json`/`~/.claude.json`/`.mcp.json`，`slash/commands/import_claude.rs:1-8`） | ✅ 已有（fork 覆盖面更宽） |
| secret 脱敏（凭据形状值不过出站边界，1245 行） | `xai-grok-secrets/src/sanitizer.rs` 已有（PEM/AKIA/ghp_/xox-/AIza/Bearer 同族模式） | ✅ 已有（可对照补模式清单） |
| steer 注入运行中回合 | `xai-interjection-core`（`common/xai-interjection-core/src/lib.rs:1-7`）已有 | ✅ 已有（kimicode-port 矩阵该条建议订正） |
| 备用屏布局系统（滚动 transcript + 底部固定 dock） | fork 是 ratatui 原生全屏 TUI，天然具备；`tui-plan.md` 是 TS 侧补课 | ✅ 已有（架构不同） |
| stdout takeover/背压（output-guard.ts） | Node `process.stdout` 特有机制 | ❌ 不做 |
| steppage 静态站发布 | 无对应物且强绑定 StepFun 服务 | ❌ 不做 |
| MCP catalog / OAuth / 插件市场 / 遥测 | `xai-grok-mcp`（OAuth）、`xai-grok-plugin-marketplace`、`xai-grok-otel`+`xai-mixpanel` 已有 | ✅ 已有 |

## 候选特性关键设计详解

### P1：bash 命令 AST 静态安全分析

Step-Code 实现：`packages/coding-agent/src/step/shell-analysis.ts`（502 行，纯解析）+
`step/command-policy.ts`（509 行，策略解释）+ `step/permissions.ts`（挂在 `tool_call`
hook，前置到前台/后台执行之前）+ 契约文档 `docs/command-permissions.md`。

1. **typed AST 遍历而非正则**：pinned `unbash` 解析器产 typed AST，显式区分
   retained（简单命令/可执行替换）与 data（数组元素、算术操作数、case 模式、参数值、
   引号内词）；函数/复合体保守检查不执行控制流；heredoc 引号体是数据、未引号体查替换
   （docs/command-permissions.md）。
2. **包装命令显式拆解**：`bash -ec -- 'rm -rf ./build'` 解析到内层真实命令串；
   `timeout/nice/setsid/stdbuf` 等 wrapper 有显式操作数 arity，不支持项不猜；
   `find -exec`/`xargs`/字面 `eval`/`trap` 单独解释——目标是不让"没分析到"伪装成
   "安全"。
3. **三态判定，`analysisIncomplete` 是一等公民**：`StepToolDecision.analysisIncomplete`
   与 `hazardous` 显式区分（解析错误/未验证 heredoc 边界/检查预算耗尽 → 未决）；
   未决 ≠ 危险，但同样不得走自动放行——Ask/Bypass 下也要求逐次确认并说明不确定性。
4. **无人值守收口**：无审批通道时 deny 默认终止运行；opt-in
   `nonInteractiveDenial: "continue"` 把可恢复的审批阻塞转成失败工具结果让 agent
   换路续跑（不可恢复的执行环境故障仍终止）。
5. fork 落点：`xai-grok-tools` 的 bash 工具审批链（现 `requires_expr` 只看参数形态，
   `bash/mod.rs:1489`）+ `xai-grok-hooks` pre_tool_use 决策链。Rust 侧可用 `shlex`/
   `tree-sitter-bash` 复刻；三态枚举与"未决不冒充"的语义是比解析器本身更值钱的部分。

### P2：会话分支树 + branch summarization（并入 kimicode-port P1 对照）

Step-Code 实现：`packages/coding-agent/src/core/session-manager.ts` +
`core/compaction/branch-summarization.ts` + `summary-overflow.ts`。

1. **append-only 树**：`branch(branchFromId)`（`session-manager.ts:1361`）从更早 entry
   长出新分支，旧 entry 保留；`getBranch(fromId)`（`:1261`）取活动路径；entry 类型含
   `branch_summary`（`:83`）。
2. **切走时摘要被离开的分支**：`branch-summarization.ts:1-4` docstring 直说目的
   （"navigating to a different point … generates a summary of the branch being left
   so context isn't lost"）；摘要携带提取的 fileOps（read/modified files），恢复工作
   时知道哪些文件动过。
3. **摘要请求自身超限的降源**：`summary-overflow.ts:14-30` 把消息按"工具调用
   assistant↔result 配对完整"切成原子组，只丢最老的完整组，保留最新用户请求与末尾
   批次；配对不完整（挂起调用/重复 id）则放弃降源（不产出自相矛盾的输入）。
4. 与 kimicode-port P1 的关系：kimi 的 undo 解决"非破坏回退 + prompt 放回"；Step 补的
   是"多分支并存 + 切换上下文保全"。fork 若走分支树路线（决策 D1），branch
   summarization 应作为同一专题的需求项并入，不单独立项。

### P3：不完整流恢复注入

Step-Code 实现：`packages/coding-agent/src/features/step-stream-recovery.ts`（40 行）。

1. 原生 retry 会把失败响应从 agent 上下文移除但保留在会话历史；本扩展在 `context`
   事件读活动分支最新 entry，命中"可重试 assistant 错误 + errorMessage 匹配
   `stream ended (before|without)`"才注入（判定 `:11-22`）。
2. 注入物是 `display:false` 的 custom message：告知"上一次响应中断、其中的工具调用
   未执行、先前工具结果确认的工作仍有效"，并给出行为指令——**改用更小响应（约 50 行
   /几 KB）、大文件用 write/edit 分块增量构建、勿整文件重发**（`:29-34`）。
3. **projection-only**：`return { messages: [...event.messages, recovery] }`
   （`:38`），不落盘、非合成用户消息，重放/导航不会继承陈旧恢复态（状态从活动分支
   现算，不存内存标记）。
4. fork 对照：`xai-grok-sampler` 的 retry 只重发请求（`retry.rs:245-257` 中断文案），
   Length 截断有 salvage 完成分支（`client.rs:2438-2448`），但没有"中断→下一轮轻量
   续作策略注入"。落点在 sampler/agent 回合重组层，注意与 fork 现有 retry/_
   doom-loop 机制的边界：Step 版不拥有 retry，只改下一轮请求的输入投影。

### P4：子 agent 进程面两防线（env 标记 + idle watchdog）

Step-Code 实现：契约见 `docs/orchestration-lifecycle.md`（"Child fan-out and turn
settlement"节），代码在 `features/subagent/`（`rpc-adapter.ts:137`
`STEP_SUBAGENT_TURN_IDLE_TIMEOUT_MS`）与 `buildSubagentChildEnv` 单点。

1. **无条件 env 标记**：每个 harness 派生的子进程带
   `STEP_CLI_SUBAGENT_CHILD=1 + STEP_DISABLE_WORKFLOW/CRON/GOAL=1`——"被 harness
   派生即已在别人 fan-out 内，故不得再 fan-out、也不持有调度权"。文档特别记录了
   反例教训：曾用 workflow ACL 推导该门，导致 `subagent→workflow` 漏开、并发流冲爆
   账号限流——**终止性保证必须是单点无条件契约，不能从可选 ACL 推导**。
2. **输出驱动 idle watchdog**：子进程任何 stdout/stderr 字节都重置计时（模型
   `message_update` 增量也转发续期），默认 30 min（刻意宽裕，因为长 `run_command`
   静默运行）；到点把该回合按失败结算并结束子进程。是"哑子进程挂死父回合"的
   兜底，与 fork 的 crash-handler/interjection 体系正交。

### P5（差异点，暂不立项）：cron 表达式与投递窗口

fork scheduler（`grok_build/scheduler/`，interval 秒数 + occurrence journal + 持久化
`types.rs:311`）覆盖了"定时提示"主场景。Step 的 `step-cron.ts`（984 行）多出：
五字段本地时 cron 表达式（拒秒/时区字段）、投递 offset（recurring 延迟 0-30min 封顶
半间隔、整点 one-shot 提前 ≤90s 防整点拥挤）、DST 双实例可调度（366 天搜索窗）、
host 空闲才投递 + `agent_settled` 再排空、不追欠（missed 不爆发补跑）。除非用户
场景需要日历型调度，否则维持现状。

### P6（小件）：ultraloop 关键词工作流入口

`features/workflow/ultraloop-opt-in.ts`：prompt 含 `ultraloop`/`ultracode` 关键词
（或 `/ultraloop on` 会话常开）即给该 prompt 附 `ultraloop-opt-in` system-reminder
授权 workflow 编排，prompt 内 `+500k`/`+1.5m` 指令提供默认 token 预算；选择权是
"模型引导 + journal 记录"契约而非硬权限门。fork 的 workflow 工具/预算已有
（`xai-workflow/src/lib.rs:17-18`、`host.rs:54-55`），缺的只是这个低摩擦入口；
若引入，预算记账沿用 fork 现有 BudgetState。

## 决策记录（预研阶段）

- **D1（2026-10-03 立项：移植路线图排序②）— P1 shell 静态分析**：Rust 解析器选型（tree-sitter-bash vs 手写）与
  `analysisIncomplete` 语义放进 `xai-grok-tools` 还是独立 crate 需先定。
- **D2（2026-10-03 落实，随路线图排序①施工）— 分支树需求并入 kimicode-port P1**：kimi（undo/prompt 放回/compaction 边界）
  与 Step（branch summarization/summary-overflow）两参照合并成一份设计原型后再动工，
  不另立 step-code 专题。
- **D3 — P3 流恢复注入**：先在 `xai-grok-sampler` 确认不完整流的现有分类
  （`stream_classify.rs`）是否已含"流中断"判别式；若有，注入只是回合重组层小改。
- **D4 — P4 待 fork 侧核实后再判**：`xai-grok-shell/src/agent/subagent/` 的 spawn
  链路是否已有等价 env 隔离/idle 兜底（本次只做了关键词 grep，未通读 spawn 链路）。
- **D5 — P5/P6 不立项**：cron 表达式与 ultraloop 入口按需随手，不因本次调研动工。

## 未核实项清单

- fork headless 模式遇审批阻塞的现有行为（P1 的 `nonInteractiveDenial` 对照面）。
- `xai-grok-shell` 子 agent spawn 链路是否已有 env 级防递归与 idle 兜底（D4）。
- fork workflow 前台渲染的逐 agent 行级进度细节 vs Step `progress.ts` 投影。
- `xai-grok-compaction` 的 intra/inter 分层与 Step request-time projection 的
  功能重叠度（决定"观察"是否转"不做"）。
- Step-Code 侧：各特性仅静态读码，未实跑；`providers` 包的 retry/流分类细节
  未深读（P3 的错误判别式来自 `step-stream-recovery.ts` 引用，未到 provider 层复核）。
- 星数/规模为 2026-10-02 快照（gh api + 本地统计），会漂移。

## 来源

- 仓库：`D:\CODE\ai\Step-Code`（upstream `519e4de4`，2026-09-30）；README、
  `docs/goal-lifecycle.md`、`docs/orchestration-lifecycle.md`、
  `docs/command-permissions.md`、`docs/plan-loop-cron-workflow-hoh.md`、
  `tui-plan.md`、根 `AGENTS.md`（提交署名纪律）。
- 关键源码锚点见各节；fork 侧锚点：`crates/codegen/xai-grok-tools/src/implementations/
  grok_build/{bash/mod.rs:1489, scheduler/types.rs:311}`、`crates/codegen/xai-grok-agent/
  src/config.rs:148-149`、`crates/codegen/xai-grok-config-types/src/lib.rs:491-500`、
  `crates/codegen/xai-grok-pager/src/app/agent.rs:345-353`、
  `crates/codegen/xai-grok-shell/src/extensions/notification.rs:1033-1039`、
  `crates/codegen/xai-grok-sampler/src/client.rs:2438-2448`、
  `crates/codegen/xai-workflow/src/{lib.rs:17-18, host.rs:54-55}`、
  `crates/common/xai-interjection-core/src/lib.rs:1-7`、
  `crates/codegen/xai-grok-secrets/src/sanitizer.rs:1-30`、
  `crates/codegen/xai-grok-pager/src/slash/commands/import_claude.rs:1-8`。
- 姊妹调研：`docs-local/kimicode-port/README.md`（P1/D1 联动）、
  `docs-local/workflow-pi-port/`（P2 对照）。
