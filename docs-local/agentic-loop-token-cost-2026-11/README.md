# Agentic loop 与 token 成本优化：opencode / zcode / dsh（2026-11）

> **状态：调研入库，未开工（2026-11-28）**。范围限定为会改变模型可见 token 或模型请求次数的机制。
> 插件运行时、UI、内核契约只在有成本影响时提及。上一篇以插件/内核为主的调研保留在
> [`../plugin-harness-survey-2026-11/README.md`](../plugin-harness-survey-2026-11/README.md)。
>
> 基线：本仓 v1.0.85 账本（2026-11-22–26）；opencode `v2.4.2`（`fc31a485`）；
> zcode `v4.0.0`（本地 `a4b44451`）；dsh `v0.2.3-alpha.2`（本地 `4b399c39`）；
> psychoscript `e8380b6`（2026-11-27）。
>
> 方法：源码、设计文档、本仓账本算式。三家均未做运行时 A/B；金额按本仓
> `deepseek-flash` 价格口径估算，文中标“估算”。插件与内核细节只在解释成本路径时引用。

## 0. 结论先行

- **在 99% 缓存命中下，模型 step 数是最贵的一项**。按本仓 11-22–26 账本，一次模型调用的
  平均成本约 `$0.0049`；每请求固定省 1k tokens 只有约 `$0.00003/call`，要 160 次调用才抵得上
  少一次 step。工具目录/schema 增量属于低价值优化，原因是缓存读价格低，且 1k tokens 太小。
- **Code Mode / PTC 是结构性减少 step 数的主要机制**。opencode 用 `execute` 程序把一串
  顺序工具调用折叠成一次模型请求；dsh 的 `run_code` 同类，并用 `inspect` 把中间值留在执行环境。
  一条 10 次顺序调用的链，直接调用约 10 个 step（约 `$0.049`），折叠后约 1 个 step 加程序/结果
  （约 `$0.006–0.008`），估算省 80% 左右。
- **zcode 最值得拿的是冷窗口 microcompaction 与读去重**。zcode 默认空闲超过 60 分钟后清理
  旧工具结果，此时 provider 缓存已冷，清理没有断崖成本（本仓按网关 TTL 约 300s 校准，见 §6.1）；
  重复读同一范围且文件未变时返回 `file_unchanged`，
  不重发正文。两项都不需要 LLM 调用。
- **dsh 的 `inspect` 与持久 REPL 是 Code Mode 的必备件**。没有 `inspect`，程序返回的大对象
  会原样进入上下文，折叠 step 省下的钱会从结果侧漏回去。dsh 对 PTC 的成本表述最诚实：
  固定每请求成本与可见定义成正比，不承诺普遍下降。
- **本仓已经有的能力比想象中多**：replay diet、deferred builtins、LengthContinue、prefire/idle
  compact、MCP 20KB 截断与落盘。尚未覆盖且价值明确的是：冷窗口 diet 触发、读去重、
  结果中部裁剪、逐位置可回收 token 观测，以及 psychoscript Code Mode。
- **上一篇的 P0 四项按成本口径降级**。`is_error` 全链路、目录/指令 delta、按 key 的会话指令、
  Hook 上下文属于 wire 正确性与 UX；在 99% 命中下它们不改变主要成本。保留在
  `zcode-v4-port` 的正确性路线里，不计入成本 P0。

## 1. 成本模型：先算清每一类优化值多少钱

### 1.1 本仓的计价基线

P1 首阶段账本（2026-11-22–26，`deepseek-flash`，1,408 次调用）：

| 项 | 值 |
| --- | ---: |
| 平均 prompt | 129.9k |
| 缓存命中 | 97.94%（中位 99.01%） |
| 平均 output | 1,061 tokens |
| 输入成本 | `$0.00422/call` |
| 输出成本 | `$0.00064/call` |
| 总成本 | `$0.00486/call` |

价格：input `$0.15/M`、cache read `$0.03/M`、output `$0.60/M`。

单次调用成本：

```text
C_step = P × (h × r + (1 − h) × u) + O × o
```

`P` prompt tokens，`h` 命中率，`r` 缓存读价，`u` 输入价，`O` 输出 tokens，`o` 输出价。

### 1.2 每类优化的单项价值

| 动作 | 计算 | 估算 |
| --- | --- | ---: |
| 少一次模型 step | 129.9k prompt、97.94% 命中、1,061 output | **$0.0049** |
| 每请求固定少 1k tokens | 1k × $0.03/M | $0.00003/call；100 次 $0.003 |
| 历史里多留 10k tokens，后续 50 次请求 | 10k × 50 × $0.03/M | $0.015 |
| 一次 130k 前缀缓存断裂 | 130k × ($0.15 − $0.03)/M | $0.0156 |
| 一次 compaction summary | 170k prompt + 4k output | ≈$0.008–0.028（摘要前缀是否命中缓存），另加断崖 |
| 一条 100k-token 工具结果留 50 次调用 | 100k × 50 × $0.03/M | $0.15 |

几个换算：

- 省 1k 固定 schema 约等于 **1/160 次模型 step**。
- 一次缓存断裂约等于 **3.2 次模型 step**。
- 一条 100k-token 工具结果留 50 次调用约等于 **30 次模型 step**。

这解释了为什么“工具目录 delta”是正确的优化、成本优先级却应排在后面：它作用在最小的一项上。
同理，opencode v2.4.2 把 `generate_image`/`generate_video` 定义移入 Code Mode，真实节省约
700 tokens/request，在 99% 命中下约 `$0.00002/call`；它的价值主要在未命中首请求和接近窗口上限时。

### 1.3 优化项的优先级

1. **减少 step 数**：顺序工具链折叠、并行调用、子 agent 隔离。
2. **降低每次 step 的历史增量**：工具结果预算、`inspect`、结果裁剪、读去重。
3. **减少缓存断崖**：稳定前缀、追加式更新、冷窗口清理、压缩后一次重写。
4. **避免不必要的 compaction**：先确定性裁剪，再 LLM 摘要；后台预生成。
5. **固定前缀瘦身**：schema/目录/系统提示。命中率高时收益最小。

## 2. opencode：Code Mode 作为 step 折叠器

### 2.1 执行路径

- Code Mode 工具面只暴露一个 `execute` 入口，输入是 freeform 文本 `code`，模型写一段受约束的
  JavaScript 风格程序；非 Code Mode 工具仍可直接调用。
- 程序里通过 `tools.*` 调用工具；`search(...)` 是同步内建，目录不完整时在程序内查工具，
  **不产生额外模型 step**。
- 目录按 delta 渲染：`UPDATE_BUDGET = 1000`，只重述新增/变更/删除的 namespace 和工具；
  delta 比重发全量还大时退回全量。工具签名只转换一次，顺序稳定。
- `full_codemode` 下额外提供 `Bun.file`/`Bun.write`/`Bun.Glob` 和后台 job；模型策略隐藏
  `glob`/`grep`/`webfetch`，让程序通过 shell/fetch 完成搜索与抓取。

### 2.2 step 折叠是主要收益

直接调用 `k` 次顺序工具，需要 `k` 个模型 step；每次 step 都重发前缀与历史。Code Mode 把
这 `k` 次调用放进一个程序，只产生 1 个 step：

```text
C_direct(k) ≈ Σ_{i=1..k} C_step(P_i)
C_codemode ≈ C_step(P) + g × o + a × (h × r + (1 − h) × u)
```

`g` 是程序 tokens，`a` 是程序返回/`inspect` 的 tokens。按 130k prompt、99% 命中：

| 顺序调用数 k | 直接调用 | Code Mode | 估算节省 |
| ---: | ---: | ---: | ---: |
| 2 | $0.0098 | ≈$0.006 | ≈$0.004 |
| 5 | $0.0245 | ≈$0.006 | ≈$0.018 |
| 10 | $0.049 | ≈$0.007 | ≈$0.042 |

折点大致在 2–3 次顺序调用：程序本身有输出成本，单次调用或不依赖前一步结果的调用不值得折叠。

### 2.3 不适用的情况

- 多个独立调用已经在同一条 assistant 消息里并行发出；它们本来只占一个 step。
- 下一步要基于工具结果做判断，无法预先写进程序。
- 程序写错、工具参数错、权限拒绝造成重试；每次重试都是一个 step。
- 程序把所有中间值放进返回值；这会从结果侧把省下的 token 还回去。`inspect`/选择性返回是必需件。

### 2.4 压缩与输出上限

opencode `compaction.ts` 的默认值：

| 参数 | 值 | 作用 |
| --- | ---: | --- |
| `keep` | 15,000 tokens | 压缩后保留最近原文 |
| `buffer` | 窗口 15%，最低 16k | 触发线离满窗的余量 |
| `PRECOMPUTE_LEAD` | 60,000 tokens | 接近上限时后台生成摘要 |
| `CACHE_LIFETIME` | 5 min | 缓存过期判断 |
| `SUMMARY_OUTPUT_FLOOR` | 32,000 tokens | 摘要输出预算 |
| 输出上限续写 | 连续 3 个 length 响应（2 次续写） | 追加 Continue，不重发原请求 |

后台预生成避免在触发时阻塞，也避免一次 overflow 重试；摘要 transcript 按级别裁剪
（整体 → 截工具输出 → 去工具输出并截工具输入 → 只留用户文本 → 省略），从最旧的交换开始，
每一级全部用完才进入下一级。本仓已有 prefire（threshold 下 10 个百分点）与 idle compact，
这部分不需要重做；`keep` 15k 可作为压缩尾部保留量的校准参考。

## 3. zcode v4：确定性裁剪与缓存纪律

### 3.1 每工具结果预算

zcode 把结果预算写进工具契约：`maxInlineBytes`、`maxModelBytes`、`strategy`
（`inline` / `truncate` / `artifact`）、`preview.direction`。执行器在结果进入 provider-visible
上下文之前应用预算；完整结果写 artifact；事件带 `truncated`、`originalBytes`、`returnedBytes`、
`budgetStrategy`、`artifactPath`。

已核对的常量：

| 工具 | `maxModelBytes` |
| --- | ---: |
| Bash | 30,000 |
| Grep | 20,000 |
| Glob | 100,000 |
| WebSearch | 20,000 |
| 通用 serializer 默认 | 100,000 |

工具自身先做语义限长（Read 的文件/行上限、Grep 默认 `head_limit: 250`、Glob 默认 100 条），
`resultBudget` 是最后兜底。这个“两层上限”比单一全局 cap 更稳：工具知道什么该留，执行器负责
不可信外部输入。

### 3.2 冷窗口 microcompaction

`microcompact.ts` 的默认值：

| 参数 | 值 |
| --- | --- |
| idle 触发 | 距最近 assistant 完成 > 60 分钟 |
| 保留最近工具结果 | 5 |
| 最小节省 | 256 tokens |
| 压力触发 | `min(0.9 × autoThreshold, autoThreshold − 2,000)` |
| 可清理工具 | Read / Bash / Grep / Glob / Edit / Write / WebFetch / WebSearch |
| 清理文本 | `[Old tool result content cleared]` |
| 事件 | `pre/post token`、`tokensSaved`、`clearedToolCallIds`、`keptToolCallIds` |

关键在 idle 触发：60 分钟后 provider 缓存已经过期，清理旧结果不产生新的缓存断崖，是纯收益。
压力触发放在 full compact 阈值之下，用零模型成本换掉一次摘要调用；代价是改写前缀，因此
只在上下文已经接近上限时做。原始 tool part、transcript、artifact 都保留，resume 后重新投影。

本仓 replay diet 的当前触发是 `turn_calls >= 20` 且 `prompt_tokens >= 200k`，理由是
`turn_budget`；它可能在缓存仍热时改写历史，付一次断崖。设计稿里的冷窗口判定（TTL、模型切换、
压缩后、鉴权重试）尚未落地。这是当前最明确的一个成本改进点，详见 §6.1。

### 3.3 读去重

同一路径、同一范围（`offset`/`limit`）、`mtime`/`size` 未变时，Read 返回
`file_unchanged`，只给路径，不重发正文。重复读是长会话里常见的浪费：模型忘了已经读过、
或在不同步骤重新确认同一文件。去重是追加式历史，不触发前缀重写。

zcode 的 read-state 在 resume 时从 completed tool part 的 `readFileState` 元数据重建；
快照只存 `contentSha256`，不存正文，避免 Edit 快照把历史撑大。本仓当前没有对应机制：
`read_file` 每次返回完整内容。

### 3.4 compaction 预算与 preflight cap

```text
outputReserve = min(effectiveMaxOutputTokens, max(21_000, contextWindow × 8%))
inputWindow   = contextWindow − outputReserve
autoThreshold = max(0, inputWindow − 13_000)
```

| context / output | reserve | autoThreshold |
| --- | ---: | ---: |
| 128K / 64K | 21K | 94K |
| 200K / 128K | 21K | 166K |
| 500K / 128K | 40K | 447K |
| 1M / 128K | 80K | 907K |

主请求发送前还有 preflight cap：

```text
max_tokens = min(effectiveMaxOutputTokens, contextWindow − estimatedCurrentUsage − 1_000)
```

它把“请求超出窗口”变成可发送的更小输出预算，减少 provider 400 与重试。本仓默认
auto-compact 阈值 85%，200K 窗口约 170K，与 zcode 的 166K 接近；1M 窗口下本仓 850K、
zcode 907K，差异变大。阈值是否调整需要按实际模型窗口与任务连续性评估，不宜直接照搬。

### 3.5 输出上限续写

- 连续 `length` 最多续写 3 次；第 4 次仍 `length` 进入可见的终止错误。
- 响应里同时有完整 tool call 时，按 tool call 处理，不追加 Continue。
- Continue 是持久化的隐藏行；同轮多次续写的正文在 UI 合并为一个 assistant block。
- 续写复用同一份输入，只追加短 Continue；前缀保持稳定，输出只生成增量。

本仓已有 LengthContinue salvage：默认预算 2，Cursor 5；本轮不把它列为新杠杆。可对比的是
预算与持久化形态：zcode 把 Continue 写成隐藏历史行，冷恢复后仍保持同一段回复；本仓的
`SyntheticReason::LengthContinue` 已具备类似语义。

### 3.6 ToolSearch 的诚实结论

zcode 的 `toolSearchMode` 有 `auto` / `local` / `off`，默认 `off`。文档明确写了：
provider 未完整支持原生 ToolSearch 时，native 模式可能导致缓存失效并增加 token 用量。
Local 模式把目标 schema 展开在工具结果里，请求前缀只含常驻工具、`LocalToolSearch` 和
`DeferredToolCall`，因此前缀稳定。

本仓的 `search_tool` + `use_tool` 就是 local 形态：MCP 工具默认不进入每请求声明，模型先取
schema 再调用；直接列出是回退路径。保持这个形态即可；没有证据支持引入 native 引用协议。

## 4. dsh：PTC、`inspect` 与逐节点计价

### 4.1 `run_code` 的成本路径

- 主入口是 `run_code` transport schema；其余工具由生成的 TypeScript SDK 提供，程序里
  `await tools.name(args)`。必要时可保留单独的 `bash` schema，其余场景都由 SDK 承载。
- `inspect(...values)` 只把选中的值/图片放进外层结果；其他中间值留在执行环境。
- 默认 `persistent-in-memory` REPL：同一个 live Agent 的变量跨 `run_code` 调用保留，避免
  反复把数据塞回 prompt。
- `maxParallelSubCalls` 默认 10；`maxSubCallsPerMinute` 默认 1024；默认 120s 允许约 2048 次子调用。
- 外层结果有硬上限；中间值不能从 session replay 重建，只有 `run_code` 结果进入历史。

dsh 对 token 效果的表述：

> Fixed per-request cost proportional to the visible definitions. PTC mode trades end-tool schemas
> for generated SDK text plus one transport schema rather than promising a universal reduction.

这句话很重要：PTC 的固定成本与可见定义成正比，SDK 文本可能和工具 schema 差不多大。
真正的节省来自两处：

1. **step 折叠**：N 次顺序调用变成 1 次 `run_code`。
2. **中间值隔离**：只有 `inspect` 的值进入历史；原始工具结果留在执行环境。

如果没有 `inspect` 纪律，程序返回一个大对象，结果侧会把 step 省下的钱还回去。因此
`inspect` 是 Code Mode 的必需件。

### 4.2 工具结果 pruner

dsh 的 pruner 只在 compaction 触发条件成立后运行：

| 参数 | 默认 |
| --- | ---: |
| `thresholdChars` | 8,192 |
| `headChars` | 4,096 |
| `tailChars` | 1,024 |
| marker | `[... tool result middle pruned ...]` |

超过阈值的文本结果替换为“头部 + 中部标记 + 尾部”，不做模型调用；原始结果保留在 append-only
日志里，replacement 通过 `sourceEventSeqs` 引用它。替换前写 `compaction/prune` shadow-price
事件，用 token meter 给被替换的节点计价。重新测量后如果压力已经低于阈值，就跳过摘要调用。

这是“先确定性裁剪，再 LLM 摘要”的最干净实现：裁剪本身零模型成本，且可能直接免掉一次摘要。
本仓 replay diet 目前把旧结果整条替换为 stub；pruner 的头/中/尾保留更多可用信号，适合处理
当前轮里单个超大结果。

### 4.3 token meter

`ctx.tokenMeter.measure(session)` 返回：

- `baseline`：provider usage 锚点或全量估算；
- `surfaceDeltaTokens`：相对锚点的有符号重计价；
- `totalTokens` / `surfaceTokens`；
- `nodes[]`：每个 surface 节点的 `tokens` 与 `heuristicTokens`。

替换节点用 `heuristicTokens` 做 shadow pricing，保证 O(1) 投影折叠与自身 append 一致。
这比“只看总 prompt”更适合做裁剪决策：能知道哪个节点值得裁、裁完省多少。

本仓账本已有 `reasoning_replay_tokens` / `tool_result_tokens` 的 composition，但还没有按节点/年龄
估算可回收量。加这一层观测不需要改请求字节，却能给 diet/prune 触发提供依据。

### 4.4 system prompt 增量

`systemUpdates()` 把捕获的 section 与已提交状态比较：普通 delta 追加，rebuild 清空后续
system 节点再替换头部；compaction 后下一次请求提交一次全量 reset，作为后续比较的基线。
这影响缓存稳定性，不直接减少 token；本仓的稳定前缀已经做得较好，优先级低。

更完整的缓存机制（request/header 全量快照、in-history system/tool 更新、压缩/裁剪/
图片省略的断崖记账、逐 step cache 计量与 pressure 投影）见
[`../token-economy-port/deepseek-harness-cache-2026-11-28.md`](../token-economy-port/deepseek-harness-cache-2026-11-28.md)。

## 5. 横向对照（按成本影响排序）

| 机制 | 项目 | 减少什么 | 成本性质 | 本仓现状 | 结论 |
| --- | --- | --- | --- | --- | --- |
| 顺序工具链折叠 | opencode `execute`；dsh `run_code` | 模型 step 数 | 每次省 $0.0049 量级 | 无 | **P1 主投** |
| 中间值隔离 | dsh `inspect`；opencode 程序返回值 | 历史增量 | 10k tokens × 50 calls ≈ $0.015 | 无 | **P1 随 Code Mode** |
| 冷窗口清理旧结果 | zcode microcompact | 历史增量，无断崖 | 纯收益 | replay diet 仅热窗口 | **P0/P1** |
| 读去重 | zcode `file_unchanged` | 重复正文 | 追加式，无断崖 | 无 | **P1** |
| 结果中部裁剪 | dsh pruner | 历史增量，可能免摘要 | 零模型成本 | 整条 stub | **P1** |
| 每工具结果预算 | zcode `resultBudget` | 单结果上限 | 已有全局 cap | 40KB / MCP 20KB | 基本覆盖 |
| 缓存稳定前缀 | zcode snapshot；dsh systemUpdates | 缓存断崖 | 一次断崖 $0.0156 | 较好 | 维持 |
| 压缩预生成/尾部保留 | opencode precompute/keep | 阻塞与连续性 | 间接 | prefire/idle 已有 | 校准即可 |
| 输出上限续写 | opencode/zcode | 重试与输出 | 低 | LengthContinue 已有 | 维持 |
| 工具目录/schema delta | opencode `UPDATE_BUDGET` | 固定 1k tokens | $0.00003/call | deferred 已有 | **降级** |
| `is_error`/指令条目/Hook 上下文 | zcode | wire 正确性/UX | 成本影响小 | 部分缺失 | 正确性路线 |

## 6. 对本仓的取舍

### 6.1 P0：先把 replay diet 的断崖成本管住

**问题。** `maybe_apply_replay_diet` 的触发是 `turn_calls >= 20` 且
`prompt_tokens >= 200k`，reason 固定为 `turn_budget`。它可能在缓存仍热时重写历史，
付一次约 `$0.0156` 的断崖；如果当次只回收 64k，按缓存读价每次省 `$0.0019`，
需要约 8 次后续调用才摊平。冷窗口触发没有断崖成本，应该优先走冷窗口。

**改动。**

1. 在 `replay_diet_gate` 增加冷窗口判定：距上次模型调用 ≥ 网关 TTL（本仓按 300s 校准）、
   模型/agent profile 切换、刚完成压缩、鉴权/错误重试后。命中任一条件时允许 diet，
   reason 分别记为 `idle` / `model_switch` / `post_compact` / `auth_retry`。
2. 热窗口保留 `turn_budget` 触发，但加一个粗略的摊平检查：
   `est_reclaim_tokens × cache_read_price × expected_remaining_calls > break_cost`。
   `expected_remaining_calls` 用上一轮调用数或一个保守下限；不满足时推迟到冷窗口。
3. 事件补 `reason`、`cache_age_ms`、`break_cost_estimate`、`break_even_calls`；
   `shell.replay_diet` 日志同步。用 `~/.grok/cache/model-usage.jsonl` 与
   `cache-breaks.jsonl` 对账。

**验收。** 冷窗口 diet 的 `$/call` 低于热窗口；热窗口只在摊平检查通过时触发；
不出现“清理 → 恢复 → 再清理”；长会话输入成本在现有 -9~-27% 基础上继续下降。

**观测配套。** 账本新增 `reclaimable_reasoning_tokens` / `reclaimable_tool_result_tokens`：
在请求构建时按“超过保留窗口的 reasoning / 超过 2KB 且不在最近 N 条的工具结果”估算，
不改变请求字节，只用于触发决策和 `/stats`。这相当于 dsh token meter 的轻量版，
比只看总 prompt 更接近真实可回收量。

### 6.2 P1：psychoscript Code Mode

这是本轮最直接的结构性杠杆，也是用户已确定的方向。psychoscript 的 Rust 实现
是合适的基座：纯 Rust 字节码 VM、无 Bun/Node、MIT、宿主函数边界、`Limits`、快照/恢复 API。

**先处理三个硬约束。**

| 约束 | 现状 | 影响 |
| --- | --- | --- |
| Rust 工具链 | 本仓 `rust-toolchain.toml` 为 1.94.0；psychoscript 要求 1.99.1 | 必须先升工具链，Linux/Windows CI 双验 |
| 依赖面 | `oxc_*` 0.159、ICU4X 2.4（`compiled_data`）、`regress` 等 | 编译时间与二进制体积增加；ICU 数据内嵌 |
| 启动/首执行 | 官方称比 TS 参考快 34x 启动、4.3x 程序执行 | 仍需实测本仓首次 `execute` 延迟 |

如果工具链升级不可接受，替代方案是 vendor psychoscript 并降 MSRV；这会带来长期维护成本，
不建议作为默认路线。先做一次独立 crate 的 `cargo check`/release build，量出编译时间与
二进制增量，再决定是否并入主仓。

**第一期范围（单次执行，不做挂起/恢复）。**

- 新增 `execute` 工具，输入 `{ code: string }`；模型写 JS 子集程序。本仓工具协议暂未支持
  opencode 的 freeform 文本输入，JSON 包装会多一层转义；这是可接受的固定成本，后续可再评估
  freeform。
- 程序全局：`tools`（懒加载 namespace）、`search(query)`、`inspect(...values)`。
- `tools` 由现有工具索引/MCP 快照提供：`keys`/`resolve` 懒解析，不把全部工具函数物化。
  MCP 的 `server__tool` 映射为 `tools.server.tool`；非法标识符做稳定转义并在目录行里给出
  完整调用表达式。
- 嵌套调用走现有 `InnerDispatch`/tool registry：权限、审批、Hook、取消、事件、账本全部复用。
   `Call::key()` 的 `<runId>:<callId>` 作为稳定的子调用 ID。
- 限制：`timeout_ms = 120_000`、`max_output_bytes = 50_000`、`max_heap_bytes = 64MB`、
  `max_value_depth = 64`、`slice_ms = 100`、`max_tool_calls = 64`（宿主计数）。
- 无 fs/net/process/timer；只有宿主函数。程序不能直接读文件或发网络请求。
- 错误：`HostResult` 失败变成 JS 异常，可被程序捕获；未捕获时返回带 `Call::location()` 的
  诊断。输出截断保留成功结果并带 in-band 标记，与 opencode 一致。

**API 映射。**

| psychoscript | 本仓 |
| --- | --- |
| `Runtime::make().globals(..).limits(..)` | `execute` 工具内构造，globals 为 `tools`/`search`/`inspect` |
| `HostFunction::new_async` | 工具分发到 `InnerDispatch` |
| `Namespace::new(keys, resolve)` | 工具索引/MCP 目录懒解析 |
| `Call::key()` | 账本子调用 ID |
| `HostValue::from_json` / `to_json` | 工具参数/结果 |
| `Outcome` | `ToolOutput` 文本 + logs + warnings |
| `Runtime::program` / `resume` | 第二期再评估 |

**提示词与目录。**

- 只注入一段短说明：`execute` 用于 3 次以上顺序工具调用、批量读、聚合/过滤、需要把中间值
  留在执行环境的场景；单次调用和需要逐步判断的场景直接用普通工具。
- 目录只列常用工具；完整目录用 `search` 在程序内查。目录行给出完整调用表达式，模型不用猜。
- 明确 `inspect` 规则：只把需要的值放进结果；返回大对象会进入上下文并可能触发压缩。
- 配置开关默认关，A/B 时开；按任务类型分流，避免全量默认。

**验收。**

1. 工具密集的顺序任务，输入 tokens 降 ≥15%，`calls/turn` 降 ≥30%，任务成功率不降。
2. 缓存命中不低于基线；首轮命中不因 `execute` schema 下降。
3. 嵌套调用的权限、审批、Hook、取消、错误与直接调用一致；账本可对账子调用。
4. Windows/Linux 构建通过；首次 `execute` 延迟、二进制体积、启动时间在预算内。
5. 不改变 workflow 的 Rhai journal：`xai-workflow` 继续只认 Rhai，`execute` 是主循环工具，
   两者不共享脚本状态。

### 6.3 P1：读去重

- 在会话内维护 `ReadState`：规范化路径、最后一次范围（`offset`/`limit`）、`mtime`、`size`、
  `content_sha256`。Windows 盘符大小写不敏感。
- 同一路径、同一范围、`mtime`/`size` 未变时，返回 `file_unchanged` stub：
  `Read file <path>, lines a-b: content unchanged since last read.` 不带正文。
- 文件变化、范围不同、首次读：返回完整内容。
- 第一期放内存；冷恢复时从 chat-state 的 tool call 参数与已完成结果重建，重建不到就退回
  完整读一次。
- 不在工具结果里伪造内容；stub 明确说明“未变化”，避免模型误以为读到了旧内容。

**验收。** 用统一日志统计重复读比例与节省 tokens；同一文件连续读两次，第二次为 stub；
文件 touch 但内容不变时按 `mtime`/`size` 判定；内容变化时必须返回新正文；Windows 路径规范化
有测试。读去重是追加式历史，不引入缓存断崖。

### 6.4 P1：结果中部裁剪

- 在上下文压力达到 `autoThreshold` 之下某阈值时，对当前 surface 里超过 8KB 的文本工具结果
  做一次头/中/尾替换：head 4KB + `[... tool result middle pruned ...]` + tail 1KB。
- 零模型调用；原始结果保留在 sidecar/日志，stub 指向可恢复路径。
- 重新测量后如果压力已低于阈值，跳过 summary；否则再进入现有压缩。
- 与 replay diet 分工：diet 处理“旧结果整条归档”，pruner 处理“当前轮单个超大结果保留头尾”。
- 现有 MCP 截断是 head-only（`truncate_str` 保留前 20KB）；可改为 head+tail，尾部至少保留
  1–2KB，错误摘要和最终状态通常在尾部。完整输出已经落盘，改动不增加丢失风险。

**验收。** 压力场景下不增加 summary 调用；模型不因丢失中段而反复重读；缓存只断一次；
`/stats` 能看到裁剪前后 tokens。

### 6.5 P2：校准与参考

- **compaction 尾部保留**：opencode `keep = 15k`；本仓当前保留策略按预算裁剪，未固定 15k。
  可在长会话 A/B 中比较 10k/15k/20k 对连续性和压缩频率的影响。
- **preflight `max_tokens` cap**：zcode 的 `contextWindow − estimatedUsage − 1000` 公式；
  如果本仓仍会出现 provider 超窗 400/重试，值得接入。
- **目录/schema delta**：只在 MCP 目录会话中途频繁变化时考虑；deferred 已覆盖大部分场景。
- **`is_error`、按 key 指令、Hook 上下文**：按 `zcode-v4-port` 的正确性路线推进，不计入成本 P0。

### 6.6 明确不做

- 不为 token 成本移植 Cordis、Extension Host、GUI/TUI 插件运行时；没有成本证据。
- 不引入 native ToolSearch；本仓的 local 形态已经稳定。
- 不把“缩 schema”当成独立成本项目；1k tokens 在 99% 命中下价值太低。
- 不在没有 A/B 的情况下默认全量 Code Mode；先覆盖 MCP + 读/搜索/批处理链。

## 7. 证据缺口

- 三家均未跑运行时 A/B；Code Mode/PTC 的节省是按本仓账本与源码常量推算的估值。
- opencode 本地工作树仍在 `v2.0.32`；`v2.4.2` 通过 tag 浅取与 GitHub API 读取，未跑
  `packages/codemode` 测试。
- zcode、dsh 未实机跑；microcompaction、pruner、token meter 的结论来自源码与设计文档。
- psychoscript 未在本仓编译；工具链升级、依赖体积、首次执行延迟、Windows 构建均未实测。
- 读去重的重复率未从日志量化：`shell.tool.exec_done` 只有工具名，没有路径/参数。
- 本仓账本没有缓存写价格；断崖成本按 `$0.15 − $0.03` 估算，实际可能更高。
- 各家 tokenizer 不同；绝对金额有偏差，优先级排序不受影响。

## 8. 参考

### opencode（`anomalyco/opencode`，`v2.4.2` / `fc31a485`）

- `packages/core/src/codemode/instructions.ts`：`execute` 指令、partial catalog、`UPDATE_BUDGET = 1000`、delta。
- `packages/core/src/codemode/tool.ts`：`execute` freeform、`full_codemode`、嵌套 CallID、background。
- `packages/core/src/codemode/job.ts`：2 分钟阻塞阈值、后台 job、恢复元数据。
- `packages/core/src/session/compaction.ts`：`keep`、buffer、`PRECOMPUTE_LEAD`、`CACHE_LIFETIME`、transcript 裁剪。
- `packages/core/src/session/runner/llm.ts`：`OutputLimit` 续写、`CONTINUE_AFTER_OUTPUT_LIMIT`、连续 3 次上限。
- `packages/core/src/tool-output.ts`：`MAX_LINES = 2_000`、`MAX_BYTES = 50 KiB`、完整输出落盘。
- Release `v2.4.2`：`generate_image`/`generate_video` 移入 Code Mode，约 700 tokens/request。

### zcode（`zai-org/ZCode`，`v4.0.0` / `a4b44451`）

- `apps/zcode-cli/packages/core/src/compact/microcompact.ts`：idle 60 分钟、keep 5、256 tokens、0.9 阈值。
- `apps/zcode-cli/docs/design/v2/compact/plan.md`：`autoThreshold`、summary reserve、tool result budget。
- `apps/zcode-cli/docs/design/v2/tool/01-read.md`：`file_unchanged`、Read 范围/截断。
- `apps/zcode-cli/docs/design/v2/tool/17-read-file-state-resume.md`：read-state 重建、`contentSha256`。
- `apps/zcode-cli/docs/design/v2/tool/19-tool-search.md`：native/local/off、缓存风险、local schema 展开。
- `apps/zcode-cli/docs/design/v2/tool/00-tool-change-chain.md`：工具 schema 字节稳定、排序、`resultBudget`。
- `docs/model-request-output-tokens.md`：output reserve、auto threshold、preflight cap。
- `apps/zcode-cli/docs/design/v2/loop/prompt-snapshot.md`：prompt snapshot 的冻结/重建。
- `apps/zcode-cli/docs/design/v2/loop/meta-user-context.md`：总 token 提醒、读截断提示、压缩边界。

### dsh（`deepseek-ai/deepseek-harness`，`v0.2.3-alpha.2` / `4b399c39`）

- `packages/core/tool-executor-ptc/README.md`：`run_code`、SDK、`inspect`、预算、并发、token/cache 影响。
- `packages/ptc-runtime/ptc-runtime/README.md`：持久 REPL、`PtcImage`、`PtcInspectableValue`、取消。
- `docs/subsystems/compaction.md`：pressure/overflow、pruner、shadow price、tool pairing。
- `packages/compaction/compaction-tool-result-pruner/README.md`：8192/4096/1024、无模型调用、跳过 summary。
- `docs/subsystems/token-meter.md`：provider 锚点、surface delta、逐节点计价、shadow pricing。
- `docs/subsystems/system-prompt.md`：`systemUpdates()`、append/rebuild、compaction reset。
- `docs/subsystems/core.md`：turn/step、`maxTokens`、pre-step、append-only session。

### psychoscript（`anomalyco/psychoscript`，`e8380b6`）

- `rust/README.md`：纯 Rust 字节码 VM、34x 启动、4.3x 执行、Limits、Rust-only 特性。
- `rust/crates/psychoscript/Cargo.toml`：`psychoscript 0.1.3`、MIT、edition 2024、Rust 1.99.1、oxc/ICU4X 依赖。
- `rust/crates/psychoscript/src/lib.rs`：`Runtime`、`HostFunction`、`Namespace`、`Limits`、`Snapshot` 导出。
- `rust/crates/psychoscript/src/host_function.rs`：`HostFunction::new_async`、`Call::key()`、`Namespace`、`HostValue` JSON。
- `rust/crates/psychoscript/src/runtime.rs`：`Outcome`、`execute_async`、`program`/`resume`。

### 本仓既有文档

- `docs-local/token-economy-port/p1-observation-2026-11-26.md`：成本基线、命中率、长回合分桶。
- `docs-local/token-economy-port/replay-diet-2026-11-21.md`：replay diet 策略与代价模型。
- `docs-local/token-economy-port/harness-cost-drilldown-2026-11-20.md`：P0–P5 任务图与观测字段。
- `docs-local/token-economy-port/deferred-builtin-tools-2026-11-14.md`：deferred 工具与声明面。
- `docs-local/zcode-v4-port/README.md`：正确性路线（`is_error`、压缩回补、MCP 状态等）。
- `docs-local/workflow-pi-port/README.md`：D1 codemode 决策与 Rhai journal 边界。
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/length_salvage.rs`：本仓 LengthContinue 预算。
- `crates/codegen/xai-chat-state/src/actor/replay_diet.rs`：当前 diet gate 与纯函数。
- `crates/codegen/xai-grok-shell/src/session/compaction.rs`：prefire、summary reserve、阈值。
