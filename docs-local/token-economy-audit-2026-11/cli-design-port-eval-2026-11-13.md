# 三家 CLI 的缓存/经济设计对照与移植收益（2026-11-13）

> 状态：调研完成，未改代码。对照对象：kimi-code（2.5.x）、zcode（v3.17.2 + 3.18
> 增量）、codex（本地 `D:\CODE\ai\codex-src\codex-main`）。基准：本 fork
> v1.0.74 + 本机 10 天实测（1910 次主回合、277.3M prompt、98.3% 命中、
> 4.65M 未命中、1.36M 输出、约 $2.33/10 天）。kimi/zcode 本地克隆已不在，
> 本轮用既有笔记 + GitHub raw 原文；codex 读本地源码。

## 0. 结论

- 三家在"缓存友好 + 省 token"上真正还没被 fork 吸收的，只剩**观测口径**和
  **网关路由键**两类；体积类设计（延迟工具声明、microcompact、Prompt
  Snapshot、压缩后文件回补）在本机 1M 窗口 + 98.3% 命中下收益接近 0 或为负。
- 值得做：zcode 的命中率口径（main-turn、token 加权、排除 sidecar）→ 落成
  `model-usage` 的 `sessionId`/`purpose`；Kimi 的缓存断裂检测 + 闲置过期提示；
  Codex 的 `prompt_cache_key`（仅 Responses 后端有面）。
- 不值得做：延迟工具声明（省 4.5k/turn，约 $0.03/10 天，发现质量风险大）、
  microcompact（原地删=自造断崖）、压缩后文件回补（本机 10 天 0 次压缩）、
  Prompt Snapshot（resume 已保留 head）、previous_response_id（不省账单）。

## 1. 对照矩阵

| 机制 | 来源 | fork 现状 | 本机实测下的预期 | 判定 |
|---|---|---|---|---|
| 命中率口径（main-turn、token 加权、排除 sidecar） | zcode `docs/context-cache-hit-rate.md` + `core/src/compact/policy.ts`（token 源可切 provider_usage） | 账本只有 model/用量，无 session/purpose；sidecar 不进账本 | 不省钱；把 1.7% 未命中与断崖变成可归因数据 | **P1 移植（并入账本字段）** |
| 缓存断裂检测 + 闲置过期提示 | kimi `tui/controllers/cache-hint-controller.ts`、`tui/utils/cache-hint.ts` | 无逐轮检测；limit-probe 只在 `GROK_LIMIT_PROBE=1` 时被动落盘，无消费方 | 每次会话中段断崖约 $0.02（150k 上下文）+ 一次 TTFT 抖动；空闲恢复首轮按上下文线性放大 | **P1 检测器 / P2 提示** |
| `prompt_cache_key` 会话稳定键 | codex `core/src/client.rs:405-449`、`codex-api/src/common.rs:297` | 字段全链支持，但 `request_builder.rs` 恒 `None`；只有 Responses 映射上线 | DS relay 走隐式前缀缓存，0 收益；Responses 网关可能改善路由粘性 | **P2（先 probe）** |
| 请求失效清单（model/tools/tool_choice/parallel/reasoning/key/text） | codex `client.rs:405-449` | 无显式清单；in-history/重置点分散在代码 | 纯参照，防止以后新增请求字段时静默断缓存 | **参照，不写代码** |
| 工具结果 spill（超限落盘 + 指针 + 头尾预览） | kimi `toolResultTruncationService.ts`（50k chars，头 4k/尾 1k，单行 2k） | 已覆盖：bash 40k + 全量文件路径；MCP 20k + `mcp/` dump + 取数指引 | 0 | 已覆盖 |
| 压缩"交接文档"prompt | kimi `fullCompaction/compaction-instruction.md` | 已移植（`xai-grok-compaction` 的 structured prompt，测试名 `summary_prompt_requires_handoff_document_properties`） | 0 | 已覆盖 |
| microcompact（按工具名白名单清旧结果，0.9 ratio、保留 5 条、最小省 256） | zcode `core/src/compact/microcompact.ts` | 50% 门 + `prune_mode=append`（保原字节，只追加失效标记） | 本机从不过 512k，0 触发；其原地删会让前缀断崖 | **不移植（fork 方案更优）** |
| 压缩后文件回补（≤5 文件、单 5k、总 50k tokens） | zcode `core/src/runtime/helpers/compact-post-reminders.ts:27-56` | 无 | 10 天 0 次压缩；1M 窗口下无面 | **暂不移植** |
| Prompt Snapshot（按会话持久化渲染后的 system 块） | zcode 3.18（详见 `agent-cli-refresh-2026-11/README.md`） | 顶层 resume 保留旧 head；仅子 agent spawn / 客户端 override / 换模型覆写 | 本机首轮冷启动 597k/10 天（$0.09），快照不改变它 | **P3 / 不立项** |
| 延迟工具声明（ToolSearch / namespace 目录） | zcode `core/src/tool/search/exposure.ts`；codex `world_state/tools_budget.rs`、`top_level_tools.rs` | 未开工（workflow-pi-port P2 T2） | tools 12.5k → 约 8k，省 4.5k×1910 ≈ 8.6M cached ≈ $0.026/10 天 | **不移植（收益 < 风险）** |
| `previous_response_id` / 服务端压缩 | codex `core/src/client.rs:18`、`compact_remote_v2.rs` | 全量历史无状态上报 | 省请求字节，不省账单；DS relay 不支持 | **不移植** |
| world-state 分区 + 快照 diff（动态段只追加） | codex `core/src/context/world_state/mod.rs:69-140` | 已有 in-history + 日期 rollover，覆盖高频变更 | 剩余动态段（skills/MCP 列表）本身是会话级静态 | **参照，不移植** |
| rollout token 预算广播（"你还剩 N tokens"） | codex `core/src/context/rollout_budget.rs` | 无 | 行为层自限，可能提前收工 | **不做** |
| reasoning 方言自学习 | kimi `kosong/src/providers/reasoning-key.ts` | 按 backend 固定 `reasoning_content` | 兼容性问题，与缓存/经济无关 | **另立（不属本专题）** |

## 2. 值得移植的三项

### 2.1 zcode 命中率口径 → 账本 `sessionId` + `purpose`（P1）

zcode 的命中率是「main-turn、token 加权、排除 title 等 sidecar」——
`docs/context-cache-hit-rate.md`，压缩决策的 token 源还可切 provider 回报值
（`core/src/compact/policy.ts`）。fork 的 `model-usage.jsonl` 只在主回合循环
追加（`turn.rs:3489`），sidecar（turn_summary/recap/compaction）不进账本，也没有
session id，所以：

- 无法把 4.65M 未命中拆成「首轮 / 新增内容 / 断崖」；
- 无法量化 turn_summary 这类每回合一次的额外调用；
- `/stats` 的成本列只能给全局数。

落法：`ModelCallSample` 增 `session_id`、`purpose`（main/recap/turn_summary/
compaction/title），pager `/stats` 增加「按会话/用途」视图。收益是让后面所有
优化可验收；不改请求字节，零风险。

### 2.2 Kimi 缓存断裂检测（P1）+ 闲置过期提示（P2）

Kimi 的纯规则（`tui/utils/cache-hint.ts`）是「空闲时间 > 该模型 cache_duration
且上下文 ≥ min_tokens_to_hint → 提示」；控制器还做两件事：resume 后弹窗、
提交前拦截。另外它有逐轮断崖检测：某步 cache read 比上一步跌 <95% 即报
`cache_break_detected`（笔记 §⑤.2，本地克隆已不在，未复核实现细节）。

fork 已有原料：每轮 provider 回报的 `cached_prompt_tokens`、limit-probe 的
`tools_hash/msgs_hash`（默认关）、`/stats` 成本列。缺的是：

1. 逐轮判定：本轮 `cached/prompt` 明显低于上轮 → 记一条 unified 事件
   （带 tools/msgs hash、模型、上下文大小），`/stats` 可查；
2. 空闲提示：resume/提交前按模型 TTL 表判断，提示「缓存已过期，本轮约 N
   tokens 按未命中计价」——昂贵模型上单次 200–300k 上下文差额可达 $0.5–1，
   对 DS 只有约 $0.03。

这两件都不改请求内容。检测器建议做（能抓住类似 send_feedback 这种回归）；
提示按「只在单价高于阈值时弹」实现，避免 DS 场景打扰。

### 2.3 Codex `prompt_cache_key`（P2，仅 Responses 面）

Codex 每个会话带稳定 key，并把「model / tools / tool_choice /
parallel_tool_calls / reasoning / store / prompt_cache_key / text」变化列为
continuation 失效原因（`core/src/client.rs:405-449`）。fork 的字段与序列化
已经齐全（`conversation.rs:670`、`types.rs:1169` 的 `forwards_prompt_cache_key`），
只是 `build_conversation_request` 恒填 `None`。

收益条件：只有 Responses 映射上线（`gpt-6-luna`、ocgo 子 agent 的
`api_backend = responses`），且网关按 key 做会话粘性。DS relay 的
chat_completions 隐式前缀缓存与 key 无关。做法：shell 在会话创建时生成一次
`prompt_cache_key = session_id`，随请求透传；换模型/重建 agent 时保持不变。
probe：同前缀连发两次，比较带 key / 不带 key 的 `cached_tokens` 与网关
响应头（若网关返回路由信息）。

## 3. 收益估算（按本机 10 天实测摊）

| 项 | 直接 token 收益 | 间接收益 | 风险 |
|---|---|---|---|
| 账本 purpose/session | 0 | 断崖归因、sidecar 量化、优化可验收 | 无 |
| 断崖检测 | 0 | 一次中段断崖 ≈ $0.02 + TTFT；可防回归扩散 | 无 |
| 闲置提示 | 0 | 昂贵模型单次 $0.5–1；DS 约 $0.03 | 打扰（默认关/阈值门控） |
| prompt_cache_key | 0（DS）/ 未知（Responses 网关） | 路由粘性，避免同会话跨后端 | 极低 |
| 延迟工具声明 | 约 8.6M cached/10 天 ≈ $0.03 | 基座 -4.5k/轮 | 发现质量、schema 维护 |
| microcompact | 本机 0 | — | 自造断崖（负） |
| 压缩后文件回补 | 本机 0 | 小窗口模型减少 1–3 轮重读 | 预加载可能白付 |
| Prompt Snapshot | 0 | — | — |
| previous_response_id | 0（账单） | 请求字节/延迟 | 大改造、协议绑定 |

一句话：三家都没有比 fork 现有做法更省 token、且不伤稳定性的机制；它们值钱的
是**让问题可见**。这也与前一篇复核一致：稳态 98.3% 的残余是新增内容，不是
命中策略。

## 4. 建议落地顺序

1. **P1**：账本加 `sessionId` + `purpose`；`/stats` 按会话/用途聚合。
   验收：同一会话的 main/sidecar 调用可分列；断崖轮能在账本里定位。
2. **P1**：断崖检测事件（复用每轮 usage + limit-probe 的 hash），
   写 unified.jsonl 并由 `/stats` 汇总。
3. **P2**：闲置过期提示（模型 TTL 表 + 单价阈值门控）；DS 默认静默。
4. **P2**：`prompt_cache_key`（会话 id，Responses 面）先 probe 再开。
5. 其余全部挂起，等真实场景触发（小窗口模型、网关多后端、压缩频率上升）。

## 5. 锚点

- zcode：`docs/context-cache-hit-rate.md`、`core/src/compact/policy.ts`、
  `core/src/compact/microcompact.ts`、`core/src/runtime/helpers/compact-post-reminders.ts:27-56`、
  `core/src/tool/search/exposure.ts`；3.18 Prompt Snapshot 见
  [`../agent-cli-refresh-2026-11/README.md`](../agent-cli-refresh-2026-11/README.md)。
- kimi：`apps/kimi-code/src/tui/controllers/cache-hint-controller.ts`、
  `apps/kimi-code/src/tui/utils/cache-hint.ts`（`evaluateCacheHint`：idle >
  cache_duration 且 totalTokens ≥ min_tokens_to_hint）、
  `packages/agent-core-v2/src/agent/toolResultTruncation/toolResultTruncationService.ts`
  （50k chars 上限、spill、头 4k/尾 1k、单行 2k）、
  `packages/agent-core-v2/src/agent/fullCompaction/compaction-instruction.md`、
  `fullCompaction/strategy.ts`（85%、保留 4 条/≤20%、3 次递进）。
- codex（本地 `codex-src/codex-main`）：`core/src/client.rs:405-449`（失效清单）、
  `codex-api/src/common.rs:297`（`prompt_cache_key` 字段）、
  `core/src/context/world_state/mod.rs:69-140`（Placement / split_prefix_updates）、
  `core/src/context/world_state/tools_budget.rs`（namespace 描述预算）、
  `utils/output-truncation/src/lib.rs:23-41`（截断告警头 + token/byte 策略）、
  `core/src/compact_token_budget.rs`（不调模型的 token-budget 压缩）、
  `core/src/compact_remote_v2.rs`（服务端压缩）、
  `history/src/reconciled_retained_context.rs`（压缩后保留上下文对账）。
- fork 现状：`xai-chat-state/src/actor/request_builder.rs`（`prompt_cache_key:
  None`）、`xai-grok-sampling-types/src/types.rs:1169`（`forwards_prompt_cache_key`）、
  `xai-grok-shell/src/session/acp_session_impl/turn.rs:3489`（账本仅主回合）、
  `xai-grok-tools/src/util/mcp_truncate.rs`（MCP 落盘 + 指针）、
  `xai-grok-compaction/src/code_compaction/prompt.rs`（handoff 摘要已移植）。
