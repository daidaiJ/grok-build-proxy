# Harness 成本优化下钻（2026-11-20）

> 父文档：[`harness-cost-roadmap-2026-11-20.md`](harness-cost-roadmap-2026-11-20.md)。
> 父文档定路线与成本数学；本文把 P0–P5 拆成可执行任务，并记录首批落地。
>
> 硬约束：压缩阈值不动；每次推送一批改动；本机不编译，由 main 的 `build.yml`
> 验收；同一会话 cache hit 下降按回归处理。

## 0. 本批落地（P0-A / P0-B）

| 项 | 内容 | 锚点 |
| --- | --- | --- |
| P0-A | 账本每条主回合采样新增 `reasoningReplayTokens` / `toolResultTokens`，按请求实际下发的 `ConversationRequest.items` 做 bytes/4 估算 | `xai-grok-tools/src/model_usage_ledger.rs:122`；`xai-chat-state/src/actor/state.rs:107`；`xai-grok-shell/src/session/acp_session_impl/turn.rs:3614` |
| P0-B | `/stats` 卡片新增 composition 行：`reasoning replay` / `tool results`；旧行字段为 0 时整行隐藏，卡片布局保持原样 | `xai-grok-pager/src/views/stats_modal.rs:584`；`xai-grok-tools/src/model_usage_ledger.rs:375` |
| P0-C | 规划文档与任务状态（本文） | — |

口径说明：

- 计数对象是**请求 items**，包括跨轮 reasoning 与全部 tool result；工具参数、system、
  AGENTS.md、用户文本不在本文两个字段内。
- 估算沿用 `/context` 的 bytes/4 口径，绝对值会低于 provider 的 tokenizer；用途是
  **同口径对比与归因**，不用于对账。
- 只加观测，不改请求字节、不改历史、不改压缩路径。旧 JSONL 行反序列化为 0。
- sidecar（recap / title / turn_summary / goal / compaction）这两个字段暂记 0；
  本批归因对象是主回合。要拆 sidecar 前缀时，再把 `AuxCall.items` 接进采样点。
- `/stats` 的 composition 行按分组聚合（模型 / 用途 / 会话），与现有 `g` 维度一致。

验收：

- CI `build` 双平台通过（本机不编译）。
- 新会话的 `~/.grok/cache/model-usage.jsonl` 出现两个 camelCase 字段。
- `/stats` 在有采样时显示 composition 行；旧账本窗口不显示该行。
- `reasoningReplayTokens` / `toolResultTokens` 的合计与 `/stats` 行一致。

## 1. 任务地图

| ID | 阶段 | 任务 | 代码落点 | 依赖 | 验收 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| P0-A | P0 | 账本补 reasoning replay / tool result 估算 | `model_usage_ledger.rs`、`state.rs`、`turn.rs` | — | 字段落盘、旧行兼容 | **本批** |
| P0-B | P0 | `/stats` composition 行 | `stats_modal.rs`、`model_usage_ledger.rs` | P0-A | 有值才显示、复制文本一致 | **本批** |
| P0-C | P0 | 单模型 `reasoning_summary = "none"` A/B | 配置 + 网关证据 | — | 网关仍回 reasoning item 则止损 | 待跑 |
| P0-D | P0 | 子 agent `use_concise = true` A/B | `grok-build-concise` 配置 | — | 成功率不降、成本降 | 待跑 |
| P0-E | P0 | 工具轮次 reminder（合并独立调用） | prompt/reminder 注入点 | 缓存追加规则 | `calls/turn` 与 `$/turn` 可量 | 待开工 |
| P1-A | P1 | 冷窗口判定（TTL、模型切换、压缩后、鉴权重试） | 会话层 | P0-A 数据 | 触发次数与节省可归因 | 待开工 |
| P1-B | P1 | replay diet 纯函数：旧 reasoning / 旧 tool result 替换 | `xai-chat-state` | P1-A | 单测覆盖配对与幂等 | 待开工 |
| P1-C | P1 | 持久化与闩锁：一次改写，后续请求前缀稳定 | `mutations.rs` / `HistoryRewrite` | P1-B | 触发后不再新增断崖 | 待开工 |
| P1-D | P1 | 事件与验收：`session.replay_diet.*` | 会话事件 / 账本 | P1-C | 触发次数 × 节省可回归 | 待开工 |
| P2-A | P2 | 长 turn 门控（上一 turn >20 调用才裁） | 会话层 | P0-A | 长 turn 降 ≥10%，短 turn 不升 | 数据门控 |
| P2-B | P2 | 中点裁剪 A/B，Responses 三元组整体处理 | `xai-chat-state` | P2-A | 缓存断崖可解释、配对不破 | 待开工 |
| P3-A | P3 | 轮次 reminder 实验 | prompt/reminder | P0-E | `calls/turn` 降 ≥15% | 待开工 |
| P3-B | P3 | `batch_tools` meta-tool（复用并发分发） | `xai-grok-tools` | P3-A 数据 | 返工率不升 | 待开工 |
| P4-A | P4 | code mode 有界评估（MCP-only） | 独立 host | P3 数据 | 工具密集任务降 ≥20% | 数据门控 |
| P5-A | P5 | `send-now` 默认排队或二次确认 | `cancel.rs` / 队列 | — | 取消 turn 毛成本降 5–10% | 待拍板 |

收益阶段之间重叠，不能相加。P1 是下一批的主施工面。

## 2. P0 下钻：先量后改

### 2.1 已落地的观测（本批）

- 主回合样本在响应返回后写账本，`context_composition(&request.items)` 只遍历两个
  变体：`ConversationItem::Reasoning` 与 `ConversationItem::ToolResult`。
- 聚合层新增两个 `Σ` 字段，供 `/stats` 与后续脚本按模型 / 用途 / 会话下钻。
- 代价：每次主回合多一次 O(items) 估算，无克隆、无 IO 增量；账本每行多约 60 B。

### 2.2 两项零代码 A/B（P0-C / P0-D）

P0-C：先确认网关行为，再决定是否跑。

1. 选一个固定长任务，单模型跑 3 次基线。
2. 把该模型配置改为 `reasoning_summary = "none"`（或等价字段）再跑 3 次。
3. 解析账本：`reasoning_tokens` 与请求里的 reasoning item 是否同时消失。
   网关仍回 reasoning item 时停止实验，不进入第二轮。

P0-D：只动子 agent，主 agent 保持现状。

- `grok-build-concise` 只带 `bash` / `read_file` / `search_replace`，紧凑系统提示，
  关 AGENTS.md 注入（`crates/codegen/xai-grok-agent/src/config.rs:202/290/1462`）。
- A/B 记录：子 agent 调用数、成功率、返工率、总 token；主 agent 不切换。
- 失败即回滚配置，不改代码。

### 2.3 P0-E：轮次 reminder

目标：验证模型是否愿意把相互独立的工具调用塞进同一轮。

- 注入点优先选**每 turn reminder**，不改进 system 前缀；动态内容放在 history 尾部
  追加，保持缓存友好。
- 文案只一句，例如“相互独立的工具调用合并到同一轮；有依赖的调用保持顺序”。
- 配置门控默认关，A/B 时开；验收看 `calls/turn` 与 `$/turn`，同时看返工率
  （重复读 / 重复改）。
- 若模型不配合，止损，不进入 P3-B。

## 3. P1 下钻：冷窗口 replay diet（下一批）

### 3.1 触发状态机

判定在会话层，纯函数在 `xai-chat-state`。触发条件（任一）：

1. 距上次模型调用超过网关 TTL（默认 300 s，配置可调；OpenCode Go 实测约 5 分钟）。
2. 模型 / agent profile 切换。
3. 压缩完成后。
4. 鉴权或错误重试后（缓存已冷）。

门控：只在主会话、`enabled = true`、至少有一个可清理的旧结果时触发；子 agent 不触发。

### 3.2 配置草案

```toml
[compaction.replay_diet]
enabled = false              # 默认关，A/B 显式开
idle_trigger_secs = 300      # 按网关实测校准
reasoning_keep_turns = 2
tool_result_keep_turns = 3
stub_old_results = true
```

字段与 `CompactionConfig` 同层解析，不进入 `SamplingConfig`，避免每次请求重算。

### 3.3 纯函数

在 `xai-chat-state` 新增：

```rust
pub struct ReplayDietOutcome {
    pub dropped_reasoning: usize,
    pub stubbed_tool_results: usize,
    pub bytes_freed: u64,
}

pub fn apply_replay_diet(
    conversation: &mut Vec<ConversationItem>,
    reasoning_keep_turns: usize,
    tool_result_keep_turns: usize,
) -> ReplayDietOutcome;
```

规则：

- 旧 terminal / read / `use_tool` 结果替换为一行 stub：保留 `tool_call_id`、原始工具名、
  首行摘要和“完整输出可从持久化取回”的提示；不做 LLM 摘要。
- reasoning 只保留最近 `reasoning_keep_turns` 个真实用户轮；合成 user item 不计数。
- Responses 配对：删除时把 `(reasoning, assistant tool_calls, tool_results)` 当整体检查；
  当前轮 reasoning 永不删。
- 幂等：已 stub 的结果不重复计数；第二次调用返回 0。
- 保留 `/compress-fast` 的语义：`prune_mode = "append"` 下同时写入失效 marker，
  保证线上字节稳定。

### 3.4 持久化与闩锁

- 走 `HistoryRewrite::RetainedPrune` 持久化，`rewrite_history` 负责磁盘替换；
  resume 后仍是 diet 后的历史。
- 一次触发只付一次前缀断崖；触发后历史稳定，后续请求沿用新前缀。
- 若改为请求层投影，必须把“清理边界”冻结在触发时刻，否则每轮窗口滑动会反复
  改写前缀；本批不选这条路线。
- 失败回滚：写入前保留现有 persistence 语义（替换失败静默、下次重试），不做
  破坏性删除；stub 文本包含可恢复提示。

### 3.5 事件与验收

事件：

- `session.replay_diet.applied`：`dropped_reasoning`、`stubbed_tool_results`、
  `bytes_freed`、`reason`（idle / model_switch / post_compact / auth_retry）、`idle_ms`。
- `session.replay_diet.skipped`：`reason`（below_threshold / nothing_to_clear / disabled）。
- `session.replay_diet.failed`：`error`。

验收：

- 冷窗口触发次数、每次节省 token 可归因。
- 触发请求本身允许一次缓存断崖；触发后的下一次请求 cache hit 恢复。
- 同一会话不出现“清理 → 恢复 → 再清理”的前缀抖动。
- 长空闲会话成本降 5–10%，任务成功率不降。

## 4. P2 下钻：长 turn 中点裁剪

- 仅当上一 turn 调用数超过阈值（默认 20）时，在下一 turn 起点裁一次。
- 短 turn 不动；P1 已覆盖冷窗口，P2 只处理“热窗口 + 长 turn”。
- Responses 侧按 `(reasoning, assistant tool_calls, tool_results)` 整体删除；当前轮
  reasoning 保留。
- 先由 P0-A 数据证明长 turn 占比，再开 A/B；没有数据不默认开。

## 5. P3 下钻：轮次杠杆

分发层已经并行（`tool_calls.rs:1023` 的 `FuturesUnordered`）；缺的是模型在一轮里
发多个独立调用的意愿。

1. P3-A：先跑 P0-E 的 reminder，量 `calls/turn` 与返工率。
2. P3-B：只有 reminder 证明瓶颈在“平铺批量”时，才做 `batch_tools` meta-tool：
   参数是 `[{tool, args}]`，复用现有并发分发，返回每调用的压缩摘要。
3. 没有脚本运行时；`batch_tools` 不引入第二运行时，风险低于 code mode。
4. 验收：`calls/turn` 降 ≥15%，重复读 / 重复改不升。

## 6. P5 下钻：取消与固定前缀

- `send-now` 当前会取消运行中 turn（`cancel.rs:132`），产生 `mid_turn_abort`
  （`cancel.rs:916/940`）。把默认行为改为排队或二次确认，目标拿回 5–10% 净收益。
- 配置门控默认保持现状，A/B 后拍板；交互路径需要补一条“取消确认”测试。
- 固定前缀只做精简：AGENTS.md 4.4% 与 system 2.0%；不扩大 deferred 面。

## 7. 批次与门禁

| 批次 | 内容 | 验收 |
| --- | --- | --- |
| Batch 1（本次推送） | P0-A、P0-B、本文 | CI build 双绿；字段落盘；`/stats` 行显示 |
| Batch 2 | P1-A/B/C/D，默认关 | 单测 + 冷窗口 A/B；触发后无新增断崖 |
| Batch 3 | P0-E + P3-A，配置门控 | `calls/turn`、返工率、$/turn |
| Batch 4 | P3-B（数据门控） | 批量工具成功率、返工率 |
| Batch 5 | P5-A（拍板后） | 取消 turn 毛成本、净收益 |

每次推送只带一个批次；CI 红在同一批修完再推。P4 在 P3 数据出来前不立项。

## 8. 不做

- 不调 `context_window` / `auto_compact_threshold_percent`。
- 不在压缩点新增裁剪；压缩视图已清空 reasoning 与工具结果。
- 不指望 append pruning 省 token；它只插失效 marker。
- 不做默认 cache keepalive。
- 不移植 QuickJS/V8 code mode；P4 只评估 MCP-only 的纯解释器原型。
- 不把 `/stats` 的 composition 估算当 provider 账单；它只用于同口径下钻。

## 9. 待拍板

1. P1 的持久化路线：建议 `HistoryRewrite::RetainedPrune` + 一次断崖，换取后续前缀稳定。
2. P0-E reminder 的默认值：建议默认关，A/B 时开。
3. P2 的长 turn 阈值：建议先取 20，按 P0-A 的分布校准。
4. P5 的默认交互：排队与二次确认二选一，A/B 后定。

## 10. 变更记录

- 2026-11-20：创建。下钻 P0–P5；落地 P0-A/P0-B 观测；定 P1 触发、纯函数、
  持久化与验收口径。
