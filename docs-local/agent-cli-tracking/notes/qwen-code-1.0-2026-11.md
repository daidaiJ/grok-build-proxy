# qwen-code 1.0 / 0.26–0.27 增补评估（2026-11-13）

> 基准：本地克隆 `D:\pro\qwen-code` @ `84b5b53f`（origin/main，2026-11-13 10:19 +0800）。
> 官方口径：release `v1.0.0-preview`（Qoder harness 合并、原生安装包、Qwen/Qoder 账号）
> 与 `v0.26.1`–`v0.27.3`；文档在 `docs/users/features/`。
> 范围：插件 / 体验 / token 经济性三块；不重复 `notes/qwen-code.md`（09-14 基线）与
> `ADOPTION.md` 已评估项。对 fork 的每一条都给了源码现状锚点。

## 0. 1.0 这次到底多了什么

对外是"把 Qoder 的 harness 并进 Qwen Code"，实际与本仓库相关的增量只有两处：
插件侧多了一套进程内可编程中间件（Mods），体验侧把上下文成本做成了可查、可度量的
一等对象。原生安装包、headless 会话恢复、多账号这几项 fork 已有对等能力，不构成移植项。

## 1. token 经济性

### 1.1 `/context detail` + idle cost ⭐ P0（建议先做）

**qwen 的设计**（`docs/users/features/context-cost.md`、`packages/cli/src/ui/commands/contextCommand.ts`）：

- `/context` 只给分类行；`/context detail` 追加 **per-item** 明细——每个内置工具、每个 MCP
  工具、每个 context 文件（并标出所属 extension）、每个 skill 一行。
- 提出 **idle cost** 指标：一个只问一句、不调工具的会话的输入 token。它与模型/窗口无关，
  比"占窗口百分比"更能反映常驻开销（同样配置在 1M 模型显示 6.5%、在 128k 显示 37%）。
- 明确定位为**本地估算**，不与 API 回报的 used/total 混算（合计可以高于或低于请求总量）。
- 配套纪律：判断改动收益要"同时看账单和前缀"，因为砍掉 `grep`/`glob` 后模型可能改走
  shell，token 从声明移到对话里（qwen 文档自己列的 trap）。

**fork 现状**：

- `/context` 只渲染五类——system / messages / tool defs（仅总数+工具数）/ overhead / free
  （`crates/codegen/xai-grok-pager/src/scrollback/blocks/context_info.rs`）。
- 数据来自 `ContextInfo`（`xai-grok-shell/src/session/acp_session_impl/session_setup.rs:672`），
  字段是数字没有条目列表；逐项文本在 `context_snapshot.rs` 里已存在，但只为 telemetry 服务，
  且精确计数要走 `/tokenize-text`（需 telemetry 开关 + 凭证）。
- 本地估算函数已有：`xai_grok_tools::util::truncate::estimate_tokens`（chars/4）。

**移植**：`ContextInfo` 增加 `items: Vec<{kind, label, tokens, owner?}>`，在 session_setup
里用同一批文本（tool definitions、`agents_md_files`、skill listing、MCP announcement、
workflow listing、startup context）逐项估算；pager 加 `detail` 视图与 idle cost 行。

**价值**：后续"砍哪个工具/哪份 AGENTS.md/哪个 skill"的所有判断都靠它；没有它，
1.3 只能凭感觉。成本是一天量级，纯观测、无行为变更。

**注意**：估算口径要显式标注（qwen 用 `~` 前缀），避免与 API 计数对账时被当成误差。

### 1.2 microcompaction + `/compress-fast` 🟡 P1（ADOPTION §2.2 已有，本轮补规格）

**qwen 的实现**：

- `microcompactHistory(force)` 只清**老工具结果**（占位 `[Old tool result content cleared]`）与
  内联媒体，不动对话骨架；默认触发阈值是工具结果总字符数 `500_000`
  （`packages/core/src/config/clearContextDefaults.ts`）。
- `/compress-fast` = 强制 microcompact + 剥离 thinking part，**零模型调用**
  （`packages/cli/src/ui/commands/compressFastCommand.ts` → `LlmChat.compressFast`）。
- NOOP 判定用同一个 estimator 比对前后，不做无收益改写；token 计数以 API 的上次
  `promptTokens` 为基线、按估算差值调整，而不是整体替换。
- 压缩后必须做语义记账：撤销文件读缓存、标记记忆体被逐出（`disarmFileReadCacheAfterEviction`）。

**fork 现状**：只有 LLM 压缩（`xai-grok-agent/src/compaction.rs`：85% 阈值、`compact_model`、
two-pass 预摘要）与输出层压缩（`[tool_output_compression]`）。history 侧没有任何规则清理，
长会话只能等 85% 触发一次昂贵的摘要。

**移植**：策略函数放 `xai-grok-compaction`，改写走 context edit journal（`xai-workflow/src/engine.rs`
的 `replace_visible` / `hide_visible` 已是同语义落点）；`/compress-fast` 作手动入口，
自动档挂在 warn 阈值之下而非替代全量压缩。

**约束**：改写点之后前缀缓存必然失效，所以只在手动命令或接近阈值时跑，不做每轮自动；
fork 没有文件读缓存，qwen 那项 bookkeeping 不用带。验收：改写后 `promptTokens` 下降、
消息序列仍合法（连续 user 合并规则）、`/context` 反映新值。

### 1.3 `tools.eager`：内置工具也能 deferred 🟢 P3（先量后做）

**qwen 的设计**：所有工具仍注册，但初始请求只声明 eager 白名单；其余通过
`tool_search` → `tool_call` 桥按需取 schema 并调用，声明列表保持字节稳定，因此
`tools.toolSearch.threshold` 默认降为 0。逐项豁免清单、子代理拿到全量 schema、
"被 demote 的工具不会报错，只是永远不被想起"都写在文档里。

**fork 现状**：`search_tool` / `use_tool` 桥只索引 **MCP** 工具
（`xai-grok-tools/src/implementations/search_tool/mod.rs` 首行注释即 "discover MCP tools"）；
内置工具全量声明；`tools:` 白名单是**硬删**（`xai-grok-agent/src/tool_list.rs` `Allowlist`），
删掉的工具不可恢复。

**移植**：ToolIndex 扩到内置工具 + `use_tool` 支持执行内置，再加 `tools.eager`（demote 而非删除）。

**风险**：模型不主动搜索 = 静默能力退化；子代理绕过桥拿全量；prompt 里的工具指引随声明集变化。
**判断**：先做 1.1 量出内置工具 schema 占多少 idle cost，再决定；收益不足两位数百分比就不做。

### 1.4 定时任务闸门 🟢 P2（小件，省的是真金）

**qwen 的设计**（`docs/users/features/scheduled-tasks.md`）：`CronCreate` 可带 `gate` 命令，
到点先跑；静默退出 0 就整轮跳过（不建 turn、不进对话、不调模型），有输出才把输出作为数据
附着到 prompt。失败/超 60 秒报一次后静默计数。适合"平时安静、变化才动"的轮询。

**fork 现状**：`scheduler_create` 只有 `interval` / `prompt` / `durable` / `fire_immediately`
（`xai-grok-tools/src/implementations/grok_build/scheduler/create.rs`），到期无条件跑一轮。

**移植**：加可选 `gate` 命令字段，到期先执行、无非空 stdout 则跳过并记账；权限复用 shell
命令审批规则。轮询类任务（PR/CI/部署）从"每 N 分钟一次模型调用"降到接近 0。

### 1.5 不移植：`/stats` 扩展

fork 的 `/stats` 已有 cache read、cache hit、ttft/tps p50/p90 与成本
（`xai-grok-pager/src/slash/commands/stats.rs`），qwen 1.0 的 Web Shell composer 指标对 TUI
无增量。真正的缺口仍是本地计划里的 `sessionId`/`purpose` 维度
（`token-economy-audit-2026-11/cache-observability-plan.md`），与 qwen 无关。

## 2. 插件

### 2.1 Mods：取原则，不取运行时

**形态**（`docs/users/extension/mods/`、`packages/core/src/mods/`，30+ 文件、13.7k 行含测试）：
每个插件一个 QuickJS 沙箱，`register(on)` 注册中间件；`tool.call` 可改写参数/结果，
`tool.check` 给权限裁决（只能收紧，不能把原生 deny/ask 升成 allow）；可注册斜杠命令；
无 Node/FS/网络；`tool_search`、`search_memory`、Goal 工具保持原生路径。

**fork 现状**：插件 = manifest + skills/commands/hooks/MCP（`xai-grok-agent/src/plugins/discovery.rs`、
`xai-grok-plugin-marketplace`）；hook 已有 PreToolUse/PostToolUse/UserPromptSubmit，且
`ADVERTISED_DECISIONS` 只广告 `deny`/`block`（`xai-grok-shell/src/extensions/hooks.rs`）——
"插件只能收紧权限"这条原则 fork 已经收敛，无需移植。

**结论**：不引入 JS 宿主。可借鉴并写进 hook 文档的只有两条实现经验：
① 工具发现/记忆检索保持原生，插件产出的结果不得继承原生的输出预算豁免；
② 会话一旦挂上工具中间件，就要串行化工具调用（并行 next 可能因改写参数引入副作用）。

### 2.2 Agent Plugins v1 / Qoder 插件兼容

qwen 把插件生态向 Agent Plugins v1 与 Qoder 插件开放。这是生态位竞争，不是设计可移植项；
fork 的 Claude 系 allowlist 兼容（`claude_tool_kind`）已覆盖同一诉求，不跟。

## 3. 体验

| 项 | qwen 做法 | 判断 |
|---|---|---|
| tool-use summaries | 每批工具完成后用 fast model 生成 30 字标签，fire-and-forget 藏在下一轮流式后面 | 挂起：fork 没有 fast model 路由（只有 `compact_model`），每批一次调用是净支出。等便宜模型路由（subagent-model-group）落地后再评估 |
| followup suggestions | 每轮结束用 LLM 猜下一句，占位在输入框 | 不做：默认开启=每轮多一次调用，TUI 输入习惯与 Web Shell 不同 |
| 交互卡片 / Web Shell 指标条 / Qwen Live | 参数化重算卡片、composer 下指标 | 不适用：无对应载体 |
| 待审批可见性 + 失焦响铃 | 待审批进 live panel，终端失焦响铃/桌面通知 | fork 已有 `approval_required` 通知（`xai-grok-pager/src/notifications/`），差的是 live panel 展示，属打磨项 |

## 4. 结论

| # | 项 | 现状 | 成本 | 判断 |
|---|---|---|---|---|
| 1.1 | `/context detail` + idle cost | 五类汇总，无逐项 | 小（纯观测） | **做**（P0） |
| 1.2 | microcompaction + `/compress-fast` | 只有 LLM 压缩 | 中（策略+journal） | **做**（并入既有 P1） |
| 1.4 | 定时任务闸门 | 到期无条件跑 | 小 | **做**（P2） |
| 1.3 | 内置工具 deferred | 桥只覆盖 MCP | 中 | 待 1.1 数据 |
| 2.1 | Mods 运行时 | 已有 hook 收紧原则 | 大 | 不引入，只记两条经验 |
| 3 | tool summaries / followup | 无 | — | 挂起/不做 |

## 5. 建议顺序

1. `/context detail` + idle cost：先把常驻成本变成可读数字，后面所有取舍靠它。
2. microcompaction + `/compress-fast`：ADOPTION §2.2 的 P1，本轮补上了阈值、NOOP 判定、
   撤销记账三处容易踩空的细节。
3. 定时任务闸门：独立小件，改 `scheduler_create` 一处 + 执行前判断。
4. `tools.eager`：拿到 1 的数据后再拍，避免为结构性改动赌一个没量过的收益。
