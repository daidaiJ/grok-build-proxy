# zcode × Codex：token 经济性与 code-mode 设计移植调研

> 状态：**调研完成，选型待拍板**；本文不含代码改动。
>
> 2026-11-14 实账与下一步：见 [`next-steps-2026-11-14.md`](next-steps-2026-11-14.md)
> （17 小时用量：缓存重读占 78%；拟用 400K 窗口 × 72% = 288k 触发把活动上下文放进
> 200–300k 甜区，全天成本约 +9%，压缩 23→17–19 次）。
>
> 范围：**token 经济性**——每次请求重发的上下文、前缀缓存、工具声明面、压缩、子 agent
> 放大与 token 预算。美元计费/账单对账只在旁支提及，不作为本专题的主线。
>
> 调研基线：
>
> | 对象 | 版本 / 提交 | 本地路径 |
> | --- | --- | --- |
> | fork | `main` `4dc0a075`（v1.0.73 之后） | `D:/CODE/ai/grok-build-proxy` |
> | Codex CLI | `0d58012`（2026-10-14，含完整 code-mode 运行时） | `D:/CODE/ai/codex` |
> | zcode | v3.17.2，`a9fe401e`（2026-11-03） | `D:/CODE/ai/zcode` |
>
> 方法：上游逐文件读源码（A 级证据）；fork 侧逐项 grep/读码复核；人天为 C 级估算。
>
> 相关专题：[`codex-port`](../codex-port/README.md)（Codex 特性总表）、
> [`workflow-pi-port`](../workflow-pi-port/README.md)（code-mode D1 / 工具面收敛）、
> [`cache-strategy-port`](../cache-strategy-port/README.md)（前缀缓存与 append-only 裁剪）、
> [`agent-cli-tracking/ADOPTION.md`](../agent-cli-tracking/ADOPTION.md) §2.5（引入定级）。
>
> 旧结论 [`../codex-port/README.md`](../codex-port/README.md) 把 code-mode 记作 “QuickJS”；当前 Codex
> 已换成 **独立 host 进程 + V8（`v8_enable_sandbox`）**，本文按新实现重述，旧判定中
> “不把第二运行时塞进 workflow journal”的结论仍然成立。

## 0. 结论先行

**token 经济性的主矛盾**：每次模型调用都会重发整个请求前缀，缓存读通常只按输入价的
约 1/10 计费；因此 token 花费 ≈ **前缀大小 × 调用次数 × 缓存折扣**。fork 已有相当强的
基线：按次 `model-usage.jsonl`、按会话 `usage.json`、`/stats` 四时间窗 × 模型（含 cache
hit）、前缀 `cache_control` 断点、side-call 复用父 `prompt_cache_key`、append-only 裁剪、
in-history 系统提示、goal token 硬预算、workflow agent-call 预算、子 agent 用量回折、
MCP 懒加载。
真正值得补的是四件：

1. **跨会话树的共享 token 预算**（Codex `rollout_budget`）：main + subagent + workflow
   共用一份加权 token 额度，阈值注入提醒，可选硬停。fork 目前只有 goal 的硬预算和
   workflow 的 **agent 调用数**预算（workflow 的 `budget()` 只接 agent-call quota），
   token 预算没有接到整棵树；workflow 因此可以成为 token 无底洞。
2. **按调用归因的用量账本**（zcode `model_usage`）：给 `model-usage.jsonl` 补
   `session_id / turn_id / query_source / attempt / status`，让 `/stats` 能按
   main / subagent / workflow / compact / title 拆 token。现在只有模型维度和会话总量，
   无法判断“token 花在主对话还是子 agent”。
3. **工具声明面的字节预算**（Codex `tool_input_schema_max_bytes`）：fork 已有 MCP 懒加载
   （`search_tool` + `use_tool`），但搜索结果原样返回完整 `input_schema`；一个超大 MCP
   schema 就能把发现面撑大，而且每轮都要重发。加 cap 是低成本、可独立验收的优化。
4. **缓存友好作为所有新功能的硬约束**：任何预算/压缩/code-mode 设计，先看它是否保持
   前缀稳定、是否原地改写历史、是否每轮重排工具面。否则省下的 token 会被 cache miss
   吃回去。

**code-mode**：完整移植是一个**独立 host 进程 + V8 运行时**的重件，不建议现在做。可以先
把它的“外形”移植过来：单一 `exec` 工具的模型契约、`wait`/yield 的异步 cell 语义、输出
token 上限、延迟工具目录与 BM25 发现、嵌套调用复用既有审批/取消/权限链、按模型能力位
开关与 fail-closed 降级。zcode 的 `dynamic-workflow` 是另一种“代码编排”：fork 已有 Rhai
版，值得借的是 **hole / amend / 运行中并发调整 / 用量沿袭**，不是 TS 编译器。

**建议顺序**：P0 共享预算 + 用量归因（独立、可先做）→ P1 工具声明面 cap 与内建工具延迟
声明（已有 MCP 基础）→ P2 再评估 code-mode host（只做主 agent 工具，不碰 workflow
journal）。所有阶段都以“缓存命中率不下降”为验收门。

---

## 1. Codex 的 code-mode

### 1.1 模型可见面：一个 `exec`，而不是 N 个工具 schema

Codex 的 `ToolMode` 有三档（`codex-rs/core/src/tools/mod.rs:63-84`）：

| 模式 | 模型看到什么 | 用途 |
| --- | --- | --- |
| `Direct` | 普通工具 JSON schema 全量声明 | 传统模式 |
| `CodeMode` | 直接工具 + `exec`/`wait`，嵌套工具也可经 `exec` 调用 | 过渡/兼容模式 |
| `CodeModeOnly` | **只看到 `exec` 与 `wait`**；嵌套工具从直接声明面隐藏 | token 经济性优先，Codex 给 GPT-6 系列默认此档 |

模式来自模型目录的 `tool_mode` 字段（`codex-rs/models-manager/models.json` 中
`gpt-6-astra`、`gpt-6-luna`、`gpt-5.6-*` 等均为 `code_mode_only`），不是纯用户开关。
host 不可用时，`CodeMode` 可回退 `Direct`；`CodeModeOnly` 保持 fail-closed
（`codex-rs/core/src/tools/code_mode/mod.rs:100-118`）。

`exec` 的输入是**原始 JavaScript 源码**，首行可选 pragma：

```js
// @exec: {"yield_time_ms": 10000, "max_output_tokens": 1000}
const files = await tools.glob({ pattern: "**/*.rs" });
```

工具描述见 `codex-rs/code-mode-protocol/src/description.rs:15-50`：
- 在**全新的 V8 isolate** 里作为 async module 执行；没有 Node、文件系统、网络、`console`。
- 所有嵌套工具在全局 `tools` 对象上，例如 `await tools.exec_command(...)`，
  MCP 名归一化成 `tools.mcp__server__tool`。
- 辅助函数：`text()` / `image()` / `audio()` / `generatedImage()` / `store()` / `load()` /
  `notify()` / `setTimeout()` / `clearTimeout()` / `yield_control()`。
- `ALL_TOOLS` 是 `{name, description}` 目录；延迟工具可通过
  `tools.tool_search({query, limit})` 做 BM25 发现，返回可调用的名字与 TS 声明。
- 输出默认 10,000 token 上限（`DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL`，
  `codex-rs/code-mode-protocol/src/runtime.rs:17`）；`yield_time_ms` 默认 10s（协议层），
  core 配置默认 30s（`DEFAULT_CODE_MODE_EXEC_YIELD_TIME_MS`）。
- `wait` 只用于 `exec` 返回 `Script running with cell ID ...` 之后；可续等、可 `terminate`。

`CodeModeOnly` 下，`exec` 的描述里会渲染每个嵌套工具的 **TypeScript 声明**：
`declare const tools: { read_file(args: {...}): Promise<...>; ... };`。JSON schema 到 TS 的
转换受 `tool_input_schema_max_bytes` 限制（`codex-rs/code-mode-protocol/src/description.rs:405-480`），
超预算时用截断后的声明。这是“工具面压缩”的核心：模型看到的是可调用的类型签名，而不是
每个工具一份独立 JSON schema。

### 1.2 运行时：独立 host 进程 + V8，主进程只做调度

结构（`codex-rs/code-mode*` 四个 crate）：

- `codex-code-mode-host`：独立二进制，stdio 或 gRPC 传输；对端是主进程的 code-mode client。
- `codex-code-mode-runtime`：V8 执行层。每个 cell 新建 `v8::Isolate`，带
  `v8_enable_sandbox`；`initialize_v8` 里固定了若干 V8 flags（`--no-maglev` 等），
  可按需 `--jitless`（`codex-rs/code-mode-runtime/src/v8_init.rs`）。
- `codex-code-mode-protocol`：host/client 协议、工具定义、执行/等待/输出类型。
- `codex-code-mode`：client 侧 session provider（进程内或远程）。

关键设计：

1. **隔离在进程边界**：主进程不直接跑 V8；host 可单独发布、单独失败、单独限资源。
2. **每次 cell 新 isolate**：脚本生命周期结束即销毁，未 await 的 promise 被丢弃；
   跨 cell 的 `store/load` 由 host 侧序列化成 JSON（`stored_values`），只接受普通可序列化对象
   （`runtime/callbacks.rs:235-276`）。
3. **嵌套工具调用回到主进程**：host 通过 delegate 把 `invoke_tool` 发回父进程，父进程复用
   普通工具调度器（审批、hook、超时、取消、结果媒体、turn-stop 全走同一套），
   见 `codex-rs/core/src/tools/code_mode/delegate.rs`。code-mode 自己只负责 JS 执行和输出编排。
4. **资源与取消**：session 可选 `max_yield_time_ms` / `max_heap_size_bytes`；cell 可被
   `preempt`/`terminate`；host 有请求上限与关闭超时。
5. **遥测**：cell 级 analytics（`CellStarted` / `ChildStarted` / `CellClosed` /
   `SamplingResponseCompleted` / `Completed`），并记录 host 耗时与 cell 开销
   （`CodeModeToolOutput::set_handler_duration_ms`）。

### 1.3 token 经济性：省的是“中间结果回到模型”和“工具声明面”

Codex 的 code-mode 不是简单的“批量调用工具”。fork 的 sampler 已经支持一个 assistant
消息里并行多个 tool call（`tool_calls.rs:1023` 的 `FuturesUnordered`），所以“把 N 个独立调用
塞进一轮”本身收益有限。code-mode 真正改变的是：

- **中间值留在 JS 里**：过滤、聚合、循环、条件分支在 cell 内完成，只有最终 `text()` 结果
  回到模型；传统 tool-calling 每个中间结果都要成为一条 tool result，下一轮再重发上下文。
- **工具声明面收敛**：`CodeModeOnly` 只声明 `exec`/`wait`，嵌套工具以类型声明/延迟目录出现；
  对 MCP 多、工具多、前缀大的会话，直接减少每次请求的固定前缀。
- **输出可控**：`max_output_tokens` 截断 `exec` 结果，`wait` 只返回增量；避免一次工具输出
  把上下文顶满。
- **失败/取消语义统一**：嵌套调用仍走原有审批与权限，不会因为“代码里调用”绕过安全门。

代价也要算进去：模型要写 JS（弱模型错误率高）、代码 token、host 启动/调度开销、
V8 二进制约数十 MB、跨平台构建成本。Codex 用 **模型能力位** 控制默认档，是对代价的承认。
从 token 角度看，code-mode 的净收益取决于“被省掉的中间 tool result/重发前缀”是否大于
“脚本本身 + 错误重试”的 token；这必须用同一任务的 A/B 实测，不能只看声明面变小。

### 1.4 可移植性判断

| 设计 | 价值 | 风险 | 判定 |
| --- | --- | --- | --- |
| 单一 `exec` + `wait` 的模型契约 | 高：中间结果不回模型，轮次和前缀双省 | 需要 JS 运行时；弱模型代码质量 | 🟡 完整版 P2；契约可先设计 |
| `CodeModeOnly` 隐藏嵌套工具 | 高：声明面收敛 | 工具发现失败会退化 | 🟢 先借“延迟声明 + 搜索”，不急着上运行时 |
| TS 声明渲染 + schema 字节预算 | 中高：大 schema 可控 | 声明被截断时模型可能调错 | 🟢 P1，可独立落地 |
| V8 host 进程隔离 | 高：不污染主进程、可 fail-closed | 二进制/构建/Windows | 🟡 P2，独立 host 二进制 |
| 嵌套调用复用审批/hook/取消 | 必须：否则安全面裂开 | 调用链和 call id 设计复杂 | 🟢 做 code-mode 时必须照此 |
| `store/load` + `yield_control` | 中：跨 cell 状态、长任务 | 语义边界多 | 🟡 随完整版 |
| cell 级遥测/开销 | 中：区分模型 token 与运行时开销 | 低 | 🟢 随完整版 |

**与 D1 的关系**：workflow-pi-port 的 D1 反对的是“在 Rhai workflow 的 journal 里再塞一个
非确定性运行时”，因为 journal 重放依赖 host call 的 request hash。Codex 的 code-mode 可以
放在 **workflow 之外**：主 agent 或普通 subagent 的一个工具，嵌套调用记录在会话历史，
不进入 `xai-workflow` 的 journal。这样既保留 D1，又能获得 code-mode 的收益。若要把
code-mode 放进 workflow 子 agent，则必须先解决“cell 执行如何进入 journal/重放”的问题，
不建议在 MVP 做。

---

## 2. Codex 的 token 经济性设计

### 2.1 前缀缓存：session 级 key + 稳定前缀

Codex 的 token 经济性首先体现在“让子 agent 复用父前缀”：

- `prompt_cache_key` 默认取 session id；非 root/internal 会话取
  `{source}:{parent_thread_id}`，让子会话与父会话共享缓存亲和
  （`codex-rs/core/src/client.rs:646-666`）。forked session 也有 `fork_cache_key`。
- `TokenUsage` 把 `cached_input_tokens` 与 `cache_write_input_tokens` 分开记账，
  reasoning token 单独一列（`codex-rs/protocol/src/protocol.rs:2366-2380`）；
  Responses SSE 解析同样读取 `input_tokens_details.cached_tokens` 与
  `cache_write_tokens`（`codex-rs/codex-api/src/sse/responses.rs:131-138`）。
- Codex 走 OpenAI 的隐式前缀缓存，不发 Anthropic 的 `cache_control`；可移植的是
  **“session 级稳定 key + 稳定前缀”**，不是某个 wire 字段。
- 请求形态变化被视为缓存事件：client 在 diff 请求时把 `prompt_cache_key_changed`、
  `include_changed` 等列为独立原因（`codex-rs/core/src/client.rs:445-452`）。
- 工具描述是静态的；code-mode 的 `exec` 描述在同一会话内稳定；延迟工具走请求级目录，
  不替换历史文本。切换 `Direct`/`CodeModeOnly` 会改工具面，属于已知的缓存重置点。

> 旁支：Codex 还有服务端 turn cost worker（按 response id 查 `total_usd`）。它解决的是
> 美元账单对账，与 token 经济性的结构分析是两条线；本文不展开。

### 2.2 `features.token_budget`：上下文窗口预算 + 提醒 + 兜底

这是**上下文窗口**的预算，不是跨会话树的总开销：

- 模型目录可带 `token_budget` 默认值：`reminder_threshold_tokens`、提醒模板、
  guidance、`auto_compact_fallback_prompt`、fallback buffer
  （`codex-rs/protocol/src/openai_models.rs:746-752`）。
- 接近阈值时注入一条 `<token_budget>` 提醒（模板默认
  “Your context window is nearly exhausted …”，`codex-rs/prompts/src/model_messages.rs:55`）。
- 剩 0 且允许 auto-compact fallback 时，注入一次性 fallback prompt，要求模型先写 notes、
  再 `new_context`（Codex 的 notes/history 上下文重置体系）。
- 提醒是**软约束**；真正的硬停来自 `rollout_budget` 或模型窗口耗尽。

**可借的点**：提醒阈值可以由模型目录/供应商默认值提供，用户 config 只做覆盖；提醒文案
要短、带剩余量、变更才追加。**不借的点**：notes/history/new_context 是 Codex 自有的上下文
重置体系，fork 已有 compaction + session 历史，全盘移植会与既有压缩路径冲突。

### 2.3 `features.rollout_budget`：跨会话树的共享 token 预算

这是最直接的“token 经济学”设计（`codex-rs/core/src/rollout_budget.rs`）：

```toml
[features.rollout_budget]
limit_tokens = 2_000_000
reminder_at_remaining_tokens = [500_000, 100_000, 10_000]
sampling_token_weight = 1.0
prefill_token_weight = 1.0
```

- 一个 root-thread 的整棵 session tree 共享一份 `weighted_tokens_used`；
  main、subagent、后续 thread 都往里记。
- 权重：优先用 provider 回报的 `usage.codex_rollout_budget_units`；没有则
  `output_tokens × sampling_weight + non_cached_input_tokens × prefill_weight`。
  这让“输出贵、缓存读便宜”的计费差异能进入预算。
- 阈值按 `remaining = limit - used` 计算，每个 thread 单独记录已投递的最高 threshold index，
  所以每个子线程都能看到自己跨过的阈值，不会因为主线程先收到而漏掉。
- 提醒投递后才 `mark_reminder_delivered`（先写历史，再确认；取消则下次重试）。
- 超过 limit 时 `record_usage` 返回 true，上层映射为 `SessionBudgetExceeded` 硬停。
- 提醒内容只有一行：`You have {remaining} weighted tokens left in the shared session token budget.`
  （`codex-rs/core/src/context/rollout_budget.rs:5-23`）。

**这是 fork 最值得直接借鉴的 token 经济性设计**：一份账本、按树共享、加权、阈值提醒、
可选硬停、按 thread 去重投递。fork 已有 `UsageLedger` 和子 agent 用量回折，接这套语义
不需要新造账本。

### 2.4 Codex token 经济性的可移植性

| 设计 | fork 现状 | 判定 |
| --- | --- | --- |
| session 级 `prompt_cache_key` + 子会话继承父 key | side-call 复用父 key；主回合 `request_builder.rs:102` 仍为 `None`；子 agent 前缀共享待核实 | 🟢 先核对，已有缺口再补 |
| `cached_input` / `cache_write` 分开记账 | 已有 `cached_read_tokens` / `cache_creation_tokens` | ✅ 已有 |
| 上下文预算提醒 | 有 auto-compact 阈值，无“剩余量”提醒注入 | 🟢 P0/P1，轻量 |
| 跨树共享 token 预算 | goal 有硬预算；workflow 只有 agent-call quota；无树级 token 预算 | 🟢 **P0，最高优先** |
| 模型目录提供预算默认值 | 模型目录已有上下文/价格/能力位，无预算默认 | 🟢 随共享预算一起 |
| notes/history 上下文重置 | 有 compaction，无 notes/new_context | 🟡 不整体移植 |

---

## 3. zcode 的 token 经济性设计

### 3.1 `model_usage`：按请求 / 按 turn / 按工具的三层事实表

zcode 把用量做成 SQLite 事实表，而不是只写一行总量（`UsageStorePort`，
`apps/zcode-cli/packages/contracts/src/interfaces/session-store.port.ts:1099`）：

- `model_usage`：一次逻辑请求一行（重试按 `attempt_index` 分行），字段含
  `session_id / turn_id / trace_id / span_id / assistant_message_id / parent_user_message_id /
  query_source / provider_id / model_id / reasoning_level / agent / mode / task_type /
  status / started_at / first_token_at / completed_at / duration_ms / ttft_ms /
  finish_reason / tool_call_count / input / output / reasoning / cache_creation / cache_read /
  provider_total / computed_total / retry_count / retryable / cancelled_by_user /
  context_exceeded / error_type / error_code / error_message / raw_usage_json /
  provider_metadata_json`。
- `turn_usage`：按 turn 聚合（请求数、重试数、工具调用/错误数、TTFT、token、状态）。
- `tool_usage`：按 tool call 记录（是否只读/破坏性、审批状态、耗时、错误）。
- 保留 30 天（`usage.ts:9-10`），写入失败只 warn，不影响推理主流程。

`querySource` 是关键维度：`main_turn / compact / session_title /
goal_completion_verification / subagent / workflow_child`。这让同一模型的 token 花费能按
“谁花的”拆开：主对话、压缩、标题生成、子 agent、workflow 子 agent。fork 当前
`ModelCallSample` 只有 `model_id + token + ttft/tps`，没有 session/turn/来源。

`usage-stats-builder.ts` 再把它做成应用级快照：总量、按日热力图、连续天数、模型排行、
cache hit rate（分母是 total input，不重复加 cache read/write）、模型错误率、工具错误率。
zcode 不算美元，只算 token；这正好是本次专题需要的“用量结构”视角。移植时不要引入
美元计价，先把 token 归因做对。

另一个 token 口径坑：服务端执行的内置工具会把一个 assistant 回合拆成多轮，provider
usage 是这些轮的合计；zcode 的 `provider-tool-rounds` 不给单轮挂 tokens，分组键取
**持久化 assistant 消息 id**（不是相邻响应会复用的 tool call id），避免压缩后回放或
统计时重复累加（`core/src/runtime/provider-tool-rounds/CONTRACT.md:1-37`、
`rounds.ts:15-79`）。fork 接 hosted search 或 code-mode 嵌套调用时，同样要按
response/turn 归因，别按“轮”重复记账。

### 3.2 压缩的 token 账：provider_usage 锚点 + microcompact

zcode 的 auto-compact 有两种 token 源（`core/src/compact/policy.ts:28`）：

- `estimate`：本地估算消息 token。
- `provider_usage`：从**最近一条已提交 assistant** 的 usage 取 baseline，再加
  baseline 之后的增量估算（`core/src/runtime/methods/compact.ts:317-345`）。
  锚点跟着消息走，不依赖绝对 message cursor；历史替换后仍能找到正确位置。
- 有效上下文窗口先扣掉输出预留（`PREFLIGHT_AUTOCOMPACT_OUTPUT_RESERVE_TOKENS = 21_000`），
  避免把整个窗口都当输入；阈值 = 有效窗口 − buffer（13k）。
- 连续失败 3 次熔断，避免压缩死循环。

`microcompact` 是更轻的一层：按工具名白名单（Read/Bash/Grep/Glob/WebFetch/WebSearch/
Edit/Write/ApplyPatch）清理旧 tool result，保留最近 5 条；触发阈值取
`min(0.9 × auto-compact 阈值, auto-compact 阈值 − 2_000)`，最小节省 256 token；
被清内容替换成 `[Old tool result content cleared]`
（`core/src/compact/microcompact.ts:1-180`）。

**压缩后文件回补**：zcode 在 compact 后按 `readFileState` 从新到旧重读磁盘，最多 5 个文件、
单文件 5k、总 50k token，跳过其他写入者与排除路径
（`core/src/runtime/helpers/compact-post-reminders.ts:27-30,44-56`）。这是用一次受控的
token 支出，换掉模型压缩后
盲目重读文件的多次工具调用；在“compact 已经重置缓存”的前提下，这笔账通常划算。

**对 fork 的警示**：microcompact 是**原地改写历史**，会直接打断前缀缓存。fork 的缓存策略
已经选择了 append-only 裁剪（`prune_mode = "append"`），如果把 zcode 的 microcompact
原样搬进来，会和缓存优化目标冲突。可借的是“按工具名白名单 + 保留最近 N 条 + 最小节省”
的判定策略；改写形态应改成追加式省略/摘要，或只在 compact 边界做。

### 3.3 并发 governor 与重试预算：token 放大也来自“打爆请求”

zcode `dynamic-workflow/concurrency.md` 把并发拆成两个界：

- **run 自己的界** `caps.maxConcurrency`：每 run 同时在飞的 ask 数，提交时定，运行中可由用户/
  模型改（只改这一个数时原地生效，不重跑脚本）。
- **共享 cap**：每 provider key、每进程的模型请求并发，由 governor 从 provider 反馈学习，
  在 1 与 key 的增长上限之间浮动；默认并发 D = `max(4, min(16, cores - 2))`，
  governor 自动增长到 2D，运行中的 run 可以把它推得更高。

两个执行点：scheduler 在 ask 发起时限制；seat gate 在已派发 subagent 的下一个 turn step
前扣住，保证“调小”不用取消在飞任务。429/失败策略由 governor + retry budget 表决定；
retry budget 分 `single-attempt` / 有界 / `unbounded`，unbounded 只放宽“放弃条件”，
退避曲线和可重试分类不变（`adapters/src/model/retry-budget.ts`）。

**对 fork 的意义**：fork 已有共享 subagent sampling semaphore（进程树级）和 workflow
agent slot；缺的是“按 provider key 学习共享 cap + 运行中调整 + 429 反馈”。429 的重试
会把同一前缀再发一次，直接放大 token 花费；并发治理因此也是 token 经济性的一部分，
同时对 BYOK 多网关的稳定性有价值。

### 3.4 “Usage, not budget” 与沿袭计量

zcode 的 workflow 明确**不提供 budget 构造**：没有 token budget、没有 node cap，
用量只作为观测面；开销控制属于 harness 和人，不属于脚本
（`docs/dynamic-workflow/authoring.md:1364-1386`）。

它的计量有两个细节值得借：

- **`spentTokens` 是 run 的 whole lineage**：amend 产生的新 run 从 predecessor 的
  `spentTokens` 起算；cache hit 不额外加 token（token 已计入继承值）；多个 amendment
  天然累加，不用遍历链。
- **`usage-updated` 与 journal 写入同步**：每 turn 后先写 run 行再发事件；
  resume 恢复 `spentTokens`，并在 `run-started` 后补发一次非零总量。

fork 的 workflow 有 `AgentResult.tokens_used`，但 run 级没有“沿袭总用量”语义；
如果以后做 amend/resume 链，这组规则可以直接借。

---

## 4. 缓存友好设计：token 经济性的硬约束

### 4.1 先算账：前缀 × 调用次数 × 折扣

每次模型调用都会重发整个请求前缀；缓存读通常只按输入价的约 1/10 计费。于是：

- **一次性缩小前缀**（工具声明、系统提示、AGENTS.md）会在每一次调用上复利。
- **前缀稳定性**比“单次略小”更重要：前缀前部改一个字节，后面的缓存全废。
- **append-only 增长便宜**：在尾部追加新消息只增加新 token；改写旧消息会让旧前缀失效。
- **子 agent 会乘算**：每个子 agent 都有自己的前缀；能共享父前缀的 `fork_context`
  比“每个子 agent 一套 system”便宜得多。
- 因此 token 经济性要同时看：总 input（cached + uncached）、uncached 占比、output、
  调用次数、cache hit；只看“总 token”会掩盖缓存断崖。

### 4.2 Codex 的缓存友好点

- `prompt_cache_key` 按 session 稳定；子会话用 `{source}:{parent_thread_id}` 继承父缓存亲和
  （`codex-rs/core/src/client.rs:646-666`）。
- 工具描述静态；code-mode 的 `exec` 描述在同一会话内稳定；延迟工具走请求级目录，
  不替换历史文本。
- `cached_input_tokens` / `cache_write_input_tokens` 分开记账；请求形态变化
  （`prompt_cache_key_changed` 等）被显式列为缓存事件。
- 输出 token 上限（`max_output_tokens`）避免单次工具输出把上下文顶满。
- 它依赖 OpenAI 隐式缓存，不发 Anthropic `cache_control`；对 fork 的 Anthropic 路径，
  zcode 的断点设计更直接。

### 4.3 zcode 的缓存友好点

- **滚动断点**：每次请求先清掉非 system 消息上的旧 `cache_control`，只给“最新一条非
  system 消息”打 `ephemeral` 断点；标记跟随具体 tool result 块，避免 SDK 合并连续 tool
  message 时丢失或错位（`core/src/runtime/helpers/provider-request-messages.ts:502-517`；
  `adapters/src/model/transform.ts:536-547`）。
- **compact 不写缓存**：compact 请求 `applyCacheControl: true` 但 `skipCacheWrite: true`，
  把断点前移到 compact prompt 前的真实上下文，避免把压缩摘要本身写成缓存
  （`runtime/methods/compact-active.ts:333-338`）。
- **按协议固定请求形态**：Anthropic Messages 把 system-reminder 折进 tool result，
  OpenAI 兼容协议保持独立 user 消息；同一会话中途不换形态，前缀才稳定。
- **provider_usage 锚点**：用 provider 回报的 cache read/write 判断压缩水位，
  比本地估算更接近真实上下文（`runtime/methods/compact.ts:317-345`）。
- **cache hit 口径**：分母是 total input，不把 cache read/write 重复加到分母
  （`usage-stats-builder.ts`）。
- **反面教材**：microcompact 原地改写旧 tool result，会打断前缀缓存；zcode 用它换局部
  上下文压力，fork 不应原样照搬。

### 4.4 fork 已有的缓存设计

fork 的 Messages 路径按 API 的四槽预算分配三处断点：

- system 最后一个 block；
- 当前消息 tip（跳过 thinking，因为 API 不接受在该块上打断点）；
- 上一请求结束点（向前找 assistant 前的 user，避免 trailing run 错位）；
- 第四槽留给网关的自动缓存（第五个会被拒）。
  实现：`crates/codegen/xai-grok-sampling-types/src/conversation/messages.rs:120-160`。
- side call 复用父会话 `prompt_cache_key`，并保留 reasoning effort，保证与主回合共享前缀
  （`xai-grok-shell/src/session/acp_session_impl/side_call.rs:100-130`）。主回合的
  `request_builder.rs:102` 仍是 `None`：Responses 后端是否需要 per-session key 待核实；
  chat_completions 走隐式前缀，不依赖这个字段。
- `cache_break_calls` 检测（同模型上一次 cache read 的 5% 下跌，且前值 ≥512 token），
  状态行/通知可见（`xai-chat-state/src/usage.rs:45-75`）。
- append-only 裁剪（`prune_mode = "append"`）与 in-history 系统提示已落地
  （cache-strategy-port P1/P2）。
- `/stats` 已展示 cache hit（`pager/src/views/stats_modal.rs:447-568`）。

### 4.5 给后续实现的缓存规则

1. **固定前缀顺序**：system → tools → history；每轮动态内容放在 history 之后，别插到前面。
2. **工具面稳定**：工具排序确定、描述静态；延迟发现只追加“新增/移除”公告，
   不重排、不重生成已有 schema。
3. **历史只追加**：旧 tool result 不原地改写；需要压缩时接受一次缓存重置，在边界做，
   不要每轮小改。
4. **动态上下文用增量**：预算提醒、工具公告、进度事件只在变化时追加一条；写历史成功后再
   标记已投递（Codex 的 `mark_reminder_delivered` 顺序）。
5. **子 agent 共享前缀**：优先 `fork_context`/共享 system+tools；每个子 agent 独立动态
   system 段会破坏父前缀复用。
6. **code-mode**：`exec` 描述稳定；嵌套调用作为新的 tool result 追加；不要每个 cell 重生成
   工具声明；输出走 cap。
7. **度量**：每个调用记录 cached_read / cache_creation / uncached input / calls / cache hit，
   并按 `query_source` 归因；验收看 uncached 占比与 cache hit，不只看总 token。

---

## 5. zcode 的 code-mode 近亲：dynamic-workflow / node_repl

zcode 没有 Codex 那种“单 cell JS 调工具”的通用 code-mode；它的对应物是：

### 5.1 dynamic-workflow：模型写 TypeScript 工作流

- 单一 `Workflow` 工具，模型写 TS 脚本，facade 提供 `agent/ask/files/git/world.run/
  report/artifact/phase/hole/log`。
- 提交前做**虚拟 host typecheck + 静态分析**（site table、taint fixpoint、时序走查、
  site/actor/control-flow/hand-off 四张图），诊断码 9001–9012。
- 执行在独立 Node 子进程 + `vm` cell + NDJSON host bridge；脚本不能 `Date.now`/
  `new Date()`/`Math.random`/动态 `import()`，所有世界访问走 host call。
- SQLite journal 支持 resume；`hole` 让 run 停在留白，模型补函数体后继续；
  `AmendWorkflow` 可改脚本并继承 predecessor 的 journal cache 与 `spentTokens`。
- 运行中有 run cap，共享 provider governor 在 driver 下层。

**可借**：单一工具 + 脚本 facade 的产品形态；hole/amend 的交互；run 级 usage 沿袭；
运行中并发调整。**不借**：TS 编译器/静态分析（Rhai 侧收益不明、工程量高）、
Node/vm 运行时（与 D1 同题）。

### 5.2 node_repl：不是通用 code-mode

`node-repl-host` 是 Browser Use / Computer Use 的共享 JS 宿主：每次 `js` 调用新 kernel，
只允许 `node:*` 内建和技能根下 `file://` 导入；调用账本由主线程 broker 写，防 Worker 冒充。
它绑定桌面能力，终端 TUI 没有对应宿主；除“每次新 kernel + 只读导入白名单 + 调用账本”
的安全思想外，不构成 fork 的 code-mode 参考。

---

## 6. fork 现状对照（防重复建设）

| 面 | fork 已有 | 锚点 | 缺口 |
| --- | --- | --- | --- |
| 按次用量 | `model-usage.jsonl`（ts/model/token/cache/ttft/tps/duration） | `xai-grok-tools/src/model_usage_ledger.rs:92-110` | 无 session/turn/query_source/attempt/status |
| 按会话用量 | `usage.json`（含 turns[]） | `xai-chat-state/src/usage.rs`；memory 实测账本 | 无按来源拆分 |
| 前缀缓存断点 | Messages 路径四槽（system 末块 / tip / 上一请求结束点 / 网关留槽） | `xai-grok-sampling-types/src/conversation/messages.rs:120-160` | 子 agent 是否继承父前缀待核实 |
| 缓存键 | side call 复用父会话 `prompt_cache_key`；Responses 才上 wire | `acp_session_impl/side_call.rs:100-130`；`sampling-types/src/conversation.rs:670` | 子 agent 的 key 继承待核实 |
| 缓存诊断 | `cache_break_calls`（同模型上次 read 的 5% 下跌，前值 ≥512） | `xai-chat-state/src/usage.rs:45-75` | 无按来源/子 agent 分桶 |
| 统计 | `/stats` 5h/day/week/month × model，cache hit，ttft/tps | `pager/src/views/stats_modal.rs:447-568` | 无来源筛选/热力图（低优先） |
| 上下文预算 | auto-compact 阈值、idle auto-compact、prune append | `xai-grok-compaction`；AGENTS.md 四十三期 | 无“剩余量”提醒注入 |
| 树级 token 预算 | goal 硬预算；workflow agent-call quota；无树级 token 预算 | `acp_session_impl/goal.rs:1185`；`workflow/host_service.rs:360-367` | **缺**（Codex rollout budget 对口） |
| 子 agent 用量 | 回折到父账本（`subagent_usage_*`） | `acp_session_tests/subagent_usage_fold_tests.rs` | 无按子 agent 的独立展示 |
| 工具面 | MCP 默认懒加载：`tool_definitions_builtins_only` + `search_tool`/`use_tool`（本地模式恒开） | `sampler_turn.rs:522`；`search_tool/mod.rs`；`PATCHES.md:310` | 内建工具未延迟声明；`search_tool` schema 无 cap |
| code-mode | 无 | — | **缺**（P2 评估） |
| 并发 | 子 agent sampling semaphore；workflow agent slots | `agent/config.rs:1092`；`workflow/host_service.rs:115` | 无按 provider key 自适应共享 cap |
| workflow | Rhai、journal 重放、resume/pause/stop、agent-call budget | `xai-workflow` | 无 hole/amend/run 级 usage 沿袭 |

> 说明：`fork-feature-inventory.md` 把 workflow 记作“token/agent 双预算”，但当前
> `host_service.rs:360-367` 的 `BudgetState.total` 实际取 `agent_budget`，token 侧字段
> 仍为 `None`。以源码为准，文档后续再校正。

---

## 7. 移植建议（按优先级）

### P0：共享 token 预算（Codex `rollout_budget` 语义）

**目标**：main + subagent + workflow 共用一份加权 token 额度；阈值提醒；可选硬停。

设计要点：

1. 账本放在 root session tree 的 `UsageLedger` 旁边，新增 `RunBudget`（或复用
   `UsageLedger` 的 totals），用 `Arc<Mutex<...>>` 共享给子 agent / workflow host。
2. 配置：
   ```toml
   [budget]
   limit_tokens = 0            # 0 = 不限
   reminder_at_remaining_tokens = [500_000, 100_000, 10_000]
   sampling_token_weight = 1.0
   prefill_token_weight = 1.0
   cache_read_weight = 0.1     # 可选，fork 的缓存读占比高
   enforce = "soft"            # soft | hard
   ```
   模型目录可提供默认值（Codex 的 model-owned defaults 模式）；用户 config 覆盖。
3. 计量：每次模型调用完成后，用 `TokenUsage` 折算 weighted units；优先用 provider
   回报的 budget units（如果将来有），否则用 output + non-cached input + cache read × weight。
4. 提醒：在下一轮请求构建时，若 `remaining <= threshold` 且该 thread 未投递过该阈值，
   注入一条短 `<session_budget>` system-reminder；写历史成功后再标记投递。
5. 硬停：`enforce = hard` 时，在下一次 sampling gate 前返回
   `SessionBudgetExceeded`，保留当前会话历史，给出“提高上限/清空预算”的提示。
6. 展示：状态行/`/stats` 增加树级剩余量；`/budget` 可查看与调整（可先做只读）。

**验收**：主会话 + 两个 subagent + 一个 workflow 的合成账本等于各调用之和；
阈值只投递一次/每 thread；cache read 按权重折算；hard 模式下超限停在下一次请求前；
`/stats` 不重复计算子 agent 的 token。估算 4–6 人天（含测试）。

### P0：建立 token/缓存基线（先量后改）

**目标**：在动任何结构之前，先有一张“前缀构成 + cache hit + 按来源 token”的基线表。

1. 对一个典型长会话，记录：system / tools / AGENTS.md / 历史 / 最新消息各占多少 token；
   每次调用的 cached_read、cache_creation、uncached input、output、cache hit。
2. 记录工具声明面的 token（内置 + MCP 搜索返回的 schema），以及子 agent / workflow
   的调用次数与 token 占比。
3. 用 `[api_dump]` 或现有 unified 日志核对“请求前缀是否逐字节稳定”；找出每轮变化的
   动态段（时间、cwd、公告、工具排序）。
4. 把这张表作为后续 P1/P2 的验收基线；没有基线就不做 code-mode 投入。

**验收**：能回答“前缀大头是谁、cache hit 多少、子 agent 占多少”；同一会话连续调用的
前缀 diff 可解释。估算 1–2 人天。

### P0：用量归因（zcode `model_usage` 的轻量版）

**目标**：让 `model-usage.jsonl` 能回答“这些 token 是主对话、子 agent、workflow 还是压缩花的”。

1. `ModelCallSample` 加可选字段（serde default，旧行兼容）：
   `session_id`, `turn_id`, `query_source`（`main_turn|subagent|workflow_child|compact|title|memory`），
   `attempt`, `status`, `error_type`。
2. shell 写入点补上下文；`query_source` 可从现有 `task_type` / 调用路径推导。
3. `/stats` 增加“按来源”行或筛选；保持现有按模型/时间窗不变。
4. 可选：同步 `usage.json` 的 turns 记录，让会话级也能按来源拆。

**验收**：同一会话中主对话/子 agent/压缩三类样本可区分；旧 JSONL 不报错；
`/stats` 合计与现有账本一致。估算 2–3 人天。

### P1：`search_tool` schema 字节预算（Codex `tool_input_schema_max_bytes`）

**目标**：MCP 工具发现面不被单个超大 schema 撑爆。

1. 配置 `[tools] search_schema_max_bytes`（默认例如 4096）。
2. `search_tool` 返回结果里对 `input_schema` 做预算化渲染：超限时返回紧凑 TypeScript
   签名/截断标记，保留 `truncated: true`；`use_tool` 仍接受完整参数。
3. 与现有 `truncate_description`（2048 chars）并列，别改 description 语义。
4. 用真实 MCP 工具（大 schema）做 prompt token 对比。

**验收**：单工具 schema 超限时搜索输出 token 明显下降；模型仍能正确调用
`use_tool`；现有 MCP 搜索用例全绿。估算 1–2 人天。

### P1：内建工具延迟声明（已有 P2 T2 的延续）

fork 的 MCP 已懒加载；内建工具仍是全量声明。Codex 的 `CodeModeOnly` 是“所有嵌套工具
都不直接声明”，fork 可先做 **按需声明**（名字公告 + `search_tools`/`use_tool` 式展开），
不必上 code-mode。现有 [`../workflow-pi-port/deferred-tool-exposure.md`](../workflow-pi-port/deferred-tool-exposure.md) 已定三档与能力门，
直接按该设计推进；不要重复设计。

**验收**：内建工具声明面 token 下降；弱模型回落 direct 全量；resume 后声明态恢复；
工具调用成功率不降。估算 5–8 人天（按原专题切片）。

### P2：code-mode host（仅主 agent，不碰 workflow journal）

只有在 P1 的工具面收敛仍不够、且 A/B 证明“中间结果回到模型”是主要 token 花费时才做。
建议按 Codex 的进程隔离形态：

1. **协议先行**：`exec`（raw JS + pragma）与 `wait`（cell id / yield / terminate）；
   输出 item（text/image/audio）；`store/load`；`ALL_TOOLS`；错误分类。
2. **独立 host 二进制** `grok-code-mode-host`：stdio JSON-RPC（先不做 gRPC），
   V8 或同等 isolate 运行时（例如 deno_core 封装）隔离；每次 cell 新 isolate；
   无 fs/net；堆/时间上限；可 kill。
3. **父进程调度**：嵌套 `invoke_tool` 回到现有 `ToolBridge`，复用审批、hook、权限、
   超时、取消、结果媒体；call id 用父进程生成，历史里记录 exec + 嵌套调用事件。
4. **能力门**：模型目录 `tool_mode`（direct/code_mode/code_mode_only）；默认 direct；
   host 不可用时 `CodeModeOnly` fail-closed，`CodeMode` 回退 direct。
5. **历史/重放**：exec cell 不进入 workflow journal；主 agent 的会话历史记录 exec 输入、
   嵌套调用与最终输出，保证 rewind/resume 可见。若未来要进 workflow，先单独设计 journal 语义。
6. **token**：默认 `max_output_tokens = 10_000`、`yield_time_ms = 10_000`；
   记录 host 耗时、cell 开销、嵌套调用耗时；`/stats` 区分模型 token 与运行时开销。

**验收**：同一任务在 direct 与 code-mode 下的 token/轮次/TTFT 对比；嵌套调用审批不绕过；
取消能杀掉 host；Windows/Linux 双平台构建；崩溃不影响主进程。估算 15–25 人天（不含 V8
构建/打包的长期维护）。

### P2：provider 自适应并发 governor（zcode 简化版）

fork 已有共享信号量；在其上增加按 `(provider, key)` 的 in-flight cap：
成功缓慢增长、429/5xx 收缩、主 agent 作为 observer 不排队、子 agent/workflow 排队；
运行中可调 run cap。先做固定上下限 + AIMD，不做完整学习器。
估算 5–8 人天；与 P0 预算独立，可并行。

### 不建议现在做

| 设计 | 原因 |
| --- | --- |
| zcode microcompact 原样移植 | 原地改写旧 tool result，直接打断前缀缓存；fork 的 append-only 裁剪已是对策 |
| zcode dynamic-workflow TS 编译器/静态分析 | Rhai 侧收益不明，TS AST 分析是重件；只借 hole/amend/usage 沿袭 |
| Codex notes/history/new_context 整体 | 与 fork compaction 体系冲突；只借阈值提醒 |
| 美元账单对账 | 与 token 经济性无关；BYOK 网关也没有同形态端点 |
| `node_repl` 宿主 | 桌面 Browser Use/CUA 绑定，终端无宿主 |

---

## 8. 风险与验收口径

1. **前缀缓存**：任何“改历史/改工具声明”的 token 优化都可能打断 cache。Codex 的做法是
   描述静态化、公告只发增量、延迟工具不替换历史文本；fork 的 P2 缓存策略是 append-only。
   code-mode 的工具声明是请求级 `tools`，不进历史，但**每轮声明必须稳定**；不要把
   “按需发现”做成每轮重排。
2. **重复计量**：子 agent 用量已回折父账本；code-mode 嵌套工具调用是本地执行，不产生
   额外模型请求；统计时按 response/turn 归因，避免把服务端多轮 usage 当单轮累加
   （zcode `provider-tool-rounds` 的教训）。
3. **模型能力**：code-mode 默认 direct，按模型目录能力位开启；先做 A/B，不要在弱模型上
   默认 code_mode_only。工具搜索失败要有 direct 回落与可见日志。
4. **安全**：V8 sandbox 不是绝对安全；host 进程要无网络/无文件系统、可 kill、资源上限；
   嵌套调用必须走审批/hook，不能因为“在代码里”绕过。
5. **构建/发布**：V8 会显著增加二进制和构建时间；独立 host 二进制能把影响限制在可选组件，
   但要处理 Windows 交叉编译、签名、随包分发。
6. **验收数据**：至少记录 direct vs code-mode 的
   token/轮次/TTFT/cache hit/工具成功率；预算功能记录树级合计、阈值投递、硬停行为；
   归因功能记录旧 JSONL 兼容与合计一致性。**cache hit 下降视为回归**，不管总 token 是否变小。

---

## 9. 待拍板问题

1. **预算语义**：默认 soft（提醒）还是 hard（停）？建议默认 soft，`enforce = "hard"` 显式开；
   goal 已有 hard 先例，树级预算可以先只读展示。
2. **预算范围**：只 main + subagent，还是把 workflow 子 agent 也纳入？建议整棵树，
   否则 workflow 是 token 黑洞。
3. **code-mode 投入**：先做 P1 工具面收敛，还是直接立项 host MVP？建议先用 P1 的实测数据
   回答“中间结果/声明面各占多少”，再决定是否烧 15–25 人天。
4. **缓存门**：是否把“同一会话 cache hit 不下降”作为所有性能/token 改动的合并门？
   建议是；否则很容易用 cache miss 换来一个更小的声明面。

---

## 附录：关键锚点

### Codex

| 主题 | 路径 |
| --- | --- |
| ToolMode / 能力门 | `codex-rs/core/src/tools/mod.rs:63-84`；`codex-rs/models-manager/models.json` |
| CodeModeOnly 隐藏嵌套工具 | `codex-rs/core/src/tools/spec_plan.rs:854-900,1069-1100` |
| `exec` 描述 / TS 声明 / schema 预算 | `codex-rs/code-mode-protocol/src/description.rs:15-50,279-480` |
| `exec`/`wait` 协议与输出上限 | `codex-rs/code-mode-protocol/src/runtime.rs:17-40` |
| host 进程 / V8 / 资源上限 | `codex-rs/code-mode-host/src/lib.rs`；`codex-rs/code-mode-runtime/src/v8_init.rs`；`codex-rs/code-mode-protocol/src/session.rs:20-40` |
| 嵌套调用复用工具运行时 | `codex-rs/core/src/tools/code_mode/delegate.rs` |
| 前缀缓存键 | `codex-rs/core/src/client.rs:646-666`；请求形态 diff `client.rs:445-452` |
| cache token 记账 | `codex-rs/protocol/src/protocol.rs:2366-2380`；`codex-rs/codex-api/src/sse/responses.rs:131-138` |
| token budget | `codex-rs/core/src/session/token_budget.rs:81-210`；`codex-rs/prompts/src/model_messages.rs:55` |
| rollout budget | `codex-rs/core/src/rollout_budget.rs:20-113`；`codex-rs/core/src/session/rollout_budget.rs` |
| 美元账单（旁支） | `codex-rs/app-server/src/turn_cost_worker.rs:27-40,330-530` |

### zcode

| 主题 | 路径 |
| --- | --- |
| usage 三层事实表 | `apps/zcode-cli/packages/core/src/runtime/methods/usage-observability.ts:65-240` |
| usage schema / 保留期 | `apps/zcode-cli/packages/adapters/src/storage/session-store/repositories/usage.ts:9-180` |
| 应用用量快照 | `apps/zcode-cli/packages/bootstrap/src/zcode-protocol/usage-stats-builder.ts` |
| 服务端多轮 usage 归属 | `core/src/runtime/provider-tool-rounds/CONTRACT.md:1-37`；`rounds.ts:15-79` |
| auto-compact token 源 | `apps/zcode-cli/packages/core/src/compact/policy.ts:28-140`；`runtime/methods/compact.ts:317-345` |
| microcompact | `apps/zcode-cli/packages/core/src/compact/microcompact.ts:1-180` |
| 滚动 cache_control 断点 | `core/src/runtime/helpers/provider-request-messages.ts:502-517`；`adapters/src/model/transform.ts:536-547` |
| compact 跳过缓存写 | `core/src/runtime/methods/compact-active.ts:333-338` |
| 按协议折叠 reminder | `core/src/runtime/helpers/provider-request-messages.ts:55-90` |
| 并发 governor / retry | `docs/dynamic-workflow/concurrency.md`；`adapters/src/model/retry-budget.ts` |
| usage-not-budget / lineage | `docs/dynamic-workflow/authoring.md:1364-1386`；`dynamic-workflow/docs/execution-engine.md:168-170,713-715` |
| dynamic-workflow 执行面 | `apps/zcode-cli/packages/dynamic-workflow/README.md`；`docs/execution-engine.md` |

### fork

| 主题 | 路径 |
| --- | --- |
| 用量账本 | `crates/codegen/xai-chat-state/src/usage.rs:20-140` |
| 按次采样 | `crates/codegen/xai-grok-tools/src/model_usage_ledger.rs:92-170` |
| 缓存断点 | `crates/codegen/xai-grok-sampling-types/src/conversation/messages.rs:120-160` |
| side call 复用父缓存键 | `crates/codegen/xai-grok-shell/src/session/acp_session_impl/side_call.rs:100-130` |
| cache break 诊断 | `crates/codegen/xai-chat-state/src/usage.rs:45-75` |
| 美元成本（旁支） | `crates/codegen/xai-grok-model-catalog/src/cost.rs` |
| `/stats` 渲染 | `crates/codegen/xai-grok-pager/src/views/stats_modal.rs:447-568` |
| goal 硬预算 | `crates/codegen/xai-grok-shell/src/session/acp_session_impl/goal.rs:1185-1205` |
| workflow agent 预算 | `crates/codegen/xai-grok-shell/src/session/workflow/host_service.rs:360-367` |
| 子 agent sampling 量 | `crates/codegen/xai-grok-shell/src/agent/config.rs:1092-1095`；`agent/mvp_agent/mod.rs:779-781` |
| MCP 懒加载 | `crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs:522-530`；`xai-grok-tools/src/implementations/search_tool/mod.rs:240-330` |
| MCP schema 输出 | `crates/codegen/xai-grok-tools/src/implementations/search_tool/mod.rs:280-300` |
| 既有 code-mode 判定 | [`../codex-port/README.md`](../codex-port/README.md)；[`../workflow-pi-port/deferred-tool-exposure.md`](../workflow-pi-port/deferred-tool-exposure.md) |
