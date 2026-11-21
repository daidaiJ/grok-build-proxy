# 轮内 replay diet 与长程接力压缩（2026-11-21）

## 1. 数据依据

发版后 18.4 小时窗口（2026-11-20 21:53 → 2026-11-21 16:08，排除分析会话）：
6 个会话、841 次模型调用、1.742 亿 prompt tokens、缓存命中 99.1%、成本 $5.83。

| 会话 | 调用 | Prompt | Reasoning replay | Tool results | 成本 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `01a1fc97` mmdx 主会话 | 533 | 110.1M | 20.3% | 38.0% | $3.75 |
| `01a1f89e` mmdx SVG 尾巴 | 226 | 44.0M | 20.0% | 34.8% | $1.44 |
| `01a1f8fb` tcs websearch 尾巴 | 45 | 18.7M | 52.4% | 35.8% | $0.58 |

主会话 11 个 turn 的长度为 `[42, 20, 145, 2, 5, 110, 64, 93, 21, 1, 23]`；
前三个 turn 占 68% 成本；prompt ≥250k 的调用占 38%、成本占 52%。
526 个 loop / 793 次工具调用 = 1.51 tools/loop，342 个 loop（65%）只发一个工具。

长会话 `01a1f8fb` 的历史构成：

| 类型 | 字节 | 说明 |
| --- | ---: | --- |
| Reasoning | 926,752 | 331 条，占历史 52% |
| Tool results | 551,672 | 438 条；95 条 >2KB，合计 398KB（72%） |
| Assistant | 299,361 | 含工具调用参数 |
| User | 17,998 | 33 条 |

结论：旧 reasoning 和旧大工具结果是两个可直接裁剪的 replay 大头；
继续下调压缩水位线不是当前瓶颈，且会牺牲任务连续性。

## 2. 社区调研

| Harness | 关键策略 | 可借鉴点 | 已知问题 |
| --- | --- | --- | --- |
| Claude Code | 三层：旧工具结果占位 → 会话记忆结构化压缩 → LLM 摘要；保留最近约 2 个用户轮；九段式摘要 | 大部分自动压缩不调用模型；摘要前先做零成本裁剪 | 压缩后靠重读文件重建上下文，会丢“为什么” |
| Codex CLI | 单层摘要；90% clamp；工具输出默认上限 12k tokens；用户消息逐字保留；handoff 摘要保留进度/决策/约束/TODO/关键数据；最近约 20k tokens | 接力式 handoff 模板；明确工具输出上限；用户消息不摘要 | 删除工具输出后模型反复重读（issue #16812）；无可选保留（#16839） |
| OpenCode | 保留最近约 15k tokens；结构化摘要（目标、下一步、决策、文件）；阈值前预生成摘要再替换 | 一次替换、后台预生成；先 prune 再 summary | 社区请求“summary 前先 prune 旧工具输出”曾被关闭 |
| Gemini CLI | `/compress` 摘要 + 分层 GEMINI.md | 持久化项目约束独立于对话压缩 | 摘要本身仍是单层 |

共同原则：

1. 先做确定性的零模型裁剪，再做 LLM 摘要。
2. 保留最近窗口；旧内容按大小/年龄淘汰。
3. 摘要保留“下一步”和硬约束；用户原始消息尽量逐字保留。
4. 被裁内容要有恢复路径，否则产生重读循环。
5. 一次重写 + 稳定字节，避免每轮滑动窗口连续打断前缀缓存。

## 3. Batch 1：轮内 selective replay diet

实现文件：

- `crates/codegen/xai-chat-state/src/actor/replay_diet.rs`：纯函数、策略、单测。
- `crates/codegen/xai-chat-state/src/actor/mutations.rs`：`maybe_apply_replay_diet` 与持久化重写。
- `crates/codegen/xai-chat-state/src/commands.rs` / `handle.rs`：`ApplyReplayDiet` 命令。
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/turn.rs`：每次请求构建前触发。
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/run_loop.rs`：`shell.replay_diet` 日志。

策略默认值：

| 参数 | 默认 | 环境变量 |
| --- | ---: | --- |
| 开关 | 开 | `GROK_REPLAY_DIET=0` 关闭 |
| 当前 turn 最少调用 | 20 | `GROK_REPLAY_DIET_MIN_CALLS` |
| 最小 prompt tokens | 200,000 | `GROK_REPLAY_DIET_MIN_PROMPT_TOKENS` |
| 最小可回收 tokens | 64,000 | `GROK_REPLAY_DIET_MIN_RECLAIM_TOKENS` |
| 保留最近工具结果 | 10 | — |
| 保留最近 prompt 轮 | 2（当前 + 上一轮） | — |
| 小结果保留阈值 | 1KB | — |
| 可 stub 最小大小 | 2KB | — |

行为：

- reasoning：删除早于“最近 2 个 prompt 轮”起点的 `Reasoning` item。
- 工具结果：保留最新 10 条、所有 ≤1KB 结果、带图片结果；更旧且 ≥2KB 的结果替换为
  `[Archived earlier tool result: call ... (tool_name), original N bytes. First line: ...]`。
- 每个 prompt turn 最多执行一次；刚发生压缩的 turn 跳过；Messages 后端跳过。
- 重写通过 `replace_history` 持久化，并同步 `total_tokens` 估算。
- 事件：`ChatStateEvent::ReplayDietApplied` → `shell.replay_diet` 统一日志，字段
  `reason / removed_reasoning / stubbed_tool_results / bytes_freed / est_tokens_freed /
  turn_calls / prompt_tokens_before / prompt_tokens_after`。

代价模型：一次 diet 会在最早被改写的 item 处打断前缀缓存，之后每个调用都少传被裁字节。
当可回收 ≥64k 且 turn 还长时，缓存未命中成本可在约 10 次调用内摊平。

验收：

- 单次触发净收益为正；
- 长 turn 成本降 10–20%；
- 无“清理 → 恢复 → 再清理”循环；
- 任务成功率不下降；
- `/stats` composition 与 `cache-breaks.jsonl` 可解释。

## 4. Batch 2：工具结果 sidecar 与恢复工具

- 推送工具结果时把原文归档到 `<session_dir>/tool-results/<call_id>.txt`。
- 新增 `read_tool_result(tool_call_id)`，stub 提示指向它。
- 归档失败时不 stub，保留原文；避免制造不可恢复的上下文。
- 验收：被裁剪结果可恢复；重读循环不增加；sidecar 不进入 prompt。

## 5. Batch 3：长程多轮压缩的接力式 brief

### 5.1 多轮压缩为什么会跑偏

1. **summary-of-summary**：每次压缩把上一次摘要再摘要，细节和因果关系逐轮衰减。
2. **目标漂移**：摘要模型会把最近的局部工作当成主目标，原始用户意图被稀释。
3. **约束丢失**：AGENTS.md、用户偏好、验收标准不在最近窗口时容易被省略。
4. **重复劳动**：已完成/已否决的路径被遗忘，下一段重新探索。
5. **压缩空转**：连续压缩但没有新用户输入或新进展，上下文仍持续膨胀。
6. **不可恢复**：工具输出被删后没有检索路径，模型反复重读。

### 5.2 Codex 的接力式 handoff

Codex CLI 的压缩不是“把整段对话缩成一段”，而是产出一份给下一段的交接：

- 当前进度与已完成工作；
- 关键决策与理由；
- 约束与用户偏好；
- TODO / 下一步；
- 关键数据（路径、命令、ID、错误信息）；
- 用户消息逐字保留；
- 最近约 20k tokens 原文保留。

它的问题也明确：删除工具输出后没有恢复路径，模型会重读；摘要不可逆。
我们采用它的模板，但把“原始目标/约束/决策日志”放在摘要之外，避免二次摘要。

### 5.3 已落地 v1：RelayAnchor

第一版先保护最容易漂移的两类信息，不引入新的摘要调用：

- `original_goal`：首次压缩时从真实用户消息中取第一条任务型请求，之后永不改写。
- `user_messages`：真实用户消息逐字追加、去重、限 24 条 / 24k 字符；
  首条目标固定在首位，超限时保留最新消息。
- `generation` / `compaction_count`：每次压缩递增，用于观测连续压缩。
- 载体：`ConversationItem::user_meta`，`SyntheticReason::CompactionMeta`，
  内容为 `<relay_anchor>{json}</relay_anchor>`。它随压缩历史持久化，
  后续压缩解析旧 anchor 后只追加新消息。
- 注入位置：压缩历史中 system + `<user_info>` 之后；若已有 anchor 则原位替换，
  保证重复压缩不产生多份。
- 观测：`shell.relay_compact` 记录 generation、用户消息数、新增用户消息数、
  anchor 字符数、summary_count；连续压缩且没有新用户消息时打 warning。
- 开关：默认开启；`[compaction] relay_anchor = false` 恢复旧的
  summary-only 压缩历史（不生成、不解析、不注入 anchor，也不写
  `shell.relay_compact`）。该值在会话创建/恢复时读取，已运行会话需重开或
  resume 后生效。

实现文件：

- `crates/codegen/xai-chat-state/src/compaction_relay.rs`
- `crates/codegen/xai-grok-shell/src/session/compaction.rs`

v1 解决“目标/用户意图被二次摘要磨掉”。决策、已完成项、下一步仍由现有
九段式 handoff 摘要承载；下一版再把这些字段结构化。

### 5.4 完整 RelayBrief（后续）

每次压缩维护一份结构化 `RelayBrief`，落盘 `<session_dir>/relay_brief.json`，
并在新历史头部注入一份紧凑文本（约 2–4k tokens）：

```json
{
  "generation": 3,
  "original_goal": "用户原始目标，逐字或近逐字，永不摘要",
  "user_messages": ["最近 N 条真实用户消息，逐字"],
  "constraints": ["AGENTS.md / 用户规则 / 验收标准，取并集"],
  "decisions": [{"what": "...", "why": "...", "status": "accepted|rejected"}],
  "completed": ["可验证的完成项"],
  "in_progress": ["当前正在做的事"],
  "next_steps": ["下一段必须继续的步骤"],
  "blockers": ["未决问题"],
  "critical_data": ["文件路径、命令、ID、关键输出摘要"],
  "acceptance": ["如何验证完成"],
  "compacted_at": "2026-11-21T..."
}
```

压缩流程：

1. 读取上一份 `RelayBrief`（首次为空）。
2. 只把“上次检查点之后的新片段 + 上一份 brief + 最近窗口”交给摘要模型。
3. 摘要模型输出结构化字段；`original_goal`、`constraints` 只允许追加/取并集，不允许改写或删除。
4. 字段校验：缺失字段回退到上一份 brief；`next_steps` 为空时保留上一份。
5. 追加决策日志；同一决策重复出现时合并，不重复计数。
6. 新历史 = `[RelayBrief 注入, 最近约 20k tokens 原文]`；用户消息按条保留。
7. 写 `session.relay_compact` 事件：generation、brief bytes、保留用户消息数、字段缺失数、
   压缩前后 tokens、距上次压缩的调用数/工具数。

漂移闸门：

- `generation` 连续增加但 `completed`/`next_steps` 无变化 → 标记 `stalled`，提示用户或
  停止自动压缩，改为显式检查点。
- `original_goal` 与首份 brief 不一致 → 拒绝写入，保留旧 brief。
- `constraints` 集合变小 → 拒绝删除，只取并集。
- 连续两次压缩之间没有新的真实用户消息 → 降低自动压缩频率，避免空转。
- 压缩后首轮请求检查 brief 是否在请求 items 中；缺失则阻止请求并重建。

验收：

- 连续 5 次以上压缩的长任务，原始目标/约束保留率 100%；
- `next_steps` 不空，且下一段实际执行步骤与 brief 一致；
- 已完成项不重复执行；
- 每次压缩后首轮成本与压缩前同量级，不出现持续膨胀；
- `session.relay_compact` 可复盘每次压缩的字段变化。

### 5.5 与现有压缩引擎的接入点

待读代码确认（Batch 3 开工前）：

- `crates/common/xai-grok-compaction`：摘要 prompt、保留策略、token 计数。
- `crates/codegen/xai-chat-state/src/compaction_utils.rs`：`ModelRequestHistory` 与新历史组装。
- shell 的 auto-compact / inline compact / `/compact` 路径：触发与摘要调用。
- 现有 `CompactionDetail` / compaction checkpoint 是否可复用为 brief 存储。

实现顺序：先落 `RelayBrief` 纯类型、序列化、校验与单测；再接摘要调用；
最后接自动压缩与 `/compact`，并用 `session.relay_compact` 观测。

## 6. 后续批次

- Batch 4：调用合并提醒（每轮末尾一条，默认关闭），目标 calls/turn −10%。
- Batch 5：旧 reasoning 剥离开关（chat_completions 下移除旧 `reasoning_content`，保留当前轮）。
- Batch 6：子代理/fan-out 归因与预算。
- 暂缓：压缩水位线调整、P4/P5、`batch_tools`。

## 7. 风险与回滚

- `GROK_REPLAY_DIET=0` 立即关闭 diet；历史已被裁剪的会话不会自动恢复原文，需 sidecar
  或重新执行工具。
- `[compaction] relay_anchor = false` 恢复旧压缩历史；新会话/恢复会话的下一次压缩起
  不再注入 anchor，已存在的 anchor 会随本次压缩消失。已运行会话保持启动时的策略。
- diet 只删除 reasoning 和替换旧工具结果，不改 system、user、assistant 工具调用配对。
- 压缩 brief 写入失败时保留旧 brief 并继续使用旧历史，不阻塞会话。
- 每次重写都记录 `prompt_tokens_before/after` 与 `bytes_freed`，按会话核对净收益。

## 8. 压缩策略 bench：glm-5.3-flash（2026-11-21）

脚本用真实九段式压缩 prompt 模拟 5 代连续压缩：同一份约 140k 字符（约 35k
char/4 估算 tokens）的初始会话，legacy = 仅摘要替换，relay = 摘要 + anchor；
temperature 0、effort low。每轮落盘原始会话、压缩原文、清洗后摘要、压缩后
上下文、探针回答与评分、真实 usage。

| 指标 | legacy | relay |
|---|---:|---:|
| post/pre r1 | 7.12% | 6.88% |
| post/pre r5 | 8.91% | 9.18% |
| summary tokens r1→r5 | 1,044 → 1,844 | 821 → 1,653 |
| anchor tokens r1→r5 | 0 | 137 → 289（占 post 5.7% → 8.5%） |
| 压缩后上下文 r5 | 3,293 | 3,394（+3.1%） |
| 压缩 completion tokens（5 轮合计） | 12,537 | 11,041（−11.9%） |
| 压缩 list cost（5 轮合计） | $0.0558 | $0.0475（−14.9%） |
| 事实覆盖率（5 轮均值） | 1.0 | 1.0 |
| 逐字用户消息保留率 | 1.0 | 1.0 |
| 探针关键词准确率（5 轮均值） | 0.907 | 0.934 |
| 交叉模型 judge（5 轮均值） | 0.980 | 0.909 |

结论：

1. **压缩收益两边都是 ~91–93%**，没有量级差别。relay 的摘要短 10–21%，压缩
   输出 token 少 11.9%、list cost 少 14.9%；anchor 毛开销占压缩后上下文
   5.7–8.5%，但被更短的摘要抵消，r5 净上下文只多 3.1%。relay 不是净增 token，
   而是把“用户消息原文”从摘要里挪到 anchor。
2. **5 轮内没有显著准确率差别**。覆盖率与逐字用户消息保留率两边都是 1.0；
   关键词准确率 relay +2.7 点、judge −7.1 点，方向和幅度都在 5 轮噪声内。
   legacy 的九段式 prompt（尤其第 6 节 All User Messages）在该模型上已经能
   完整保留用户消息，anchor 在本场景是冗余保证。
3. **anchor 的价值是保证性，不是这 5 轮的平均分**：摘要被截断、模型变弱、
   压缩轮次继续增加、或用户消息很多时，legacy 的第 6 节可能压缩/遗漏；
   anchor 把 original_goal 和用户消息放在摘要之外，代价是 3% 左右的净上下文。
4. **默认策略**：保持 relay 默认开启，作为低成本保险；如果只按 glm 5 轮成本
   优先，legacy 并不更差。要证明 relay 的准确率收益，需要 8–12 轮、更弱的
   摘要模型或更紧的摘要预算，本轮不继续烧 token。

限制：单一合成任务、每轮一个样本；char/4 估算对中文偏低（实际 prompt
tokens 约为估算的 1.8 倍）；anchor 为 Python 等价实现，真实 Rust 路径由 CI
单测覆盖；judge 用 auto，可能路由到同一后端。原始会话与逐轮现场在
`C:\Users\panda\AppData\Local\Temp\relay-compact-bench\`（临时目录）。

### 8.1 auto 侧（TCS Debian 的 LightVela 聚合网关，实际路由 glm-5.3-flash）

`auto` 不是本地别名，而是 TCS Debian `/etc/agentgateway/config.yaml` 里的
LightVela 聚合入口：`providerOverride: lightvela`、
`baseUrl: https://token.lightvela.com/v1`、`model: auto`。配置注释明确写了
上游按请求自动选模型，“实测落到 z-ai/glm-5.3-flash”。本次 10 次压缩响应
全部回显 `model=glm-5.3-flash`，所以它是同一后端的第二组样本，不是独立的
LightVela 模型。

| 指标 | legacy | relay |
|---|---:|---:|
| post/pre r5 | 8.76% | 9.64% |
| summary tokens r1→r5 | 909 → 1,785 | 907 → 1,859 |
| anchor tokens r5 | 0 | 289（占 post 8.0%） |
| 压缩后上下文 r5 | 3,234 | 3,600（+11.3%） |
| 压缩 completion tokens（5 轮合计） | 12,300 | 12,939（+5.2%） |
| 事实覆盖率 / 逐字用户消息 | 1.0 / 1.0 | 1.0 / 1.0 |
| 关键词准确率均值 | 0.897 | 0.946 |
| judge 均值 | 0.906 | 0.917 |

### 8.2 合并结论

- 两次运行都没有可分辨的准确率差别；`auto` 与 glm 同后端，不构成独立验证。
- 成本效应不一致：glm 上 relay 压缩输出 −11.9%、净 post +3.1%；auto 上
  relay 压缩输出 +5.2%、净 post +11.3%。5 轮样本不足以证明 relay 的平均收益。
- 能确认的是机制正确：anchor 只注入一次、不混入摘要、逐字用户消息保留率
  1.0，r5 毛开销 289 tokens。价值在“保证性”，不在当前平均分。
- 默认值选择：按成本优先可以把 `relay_anchor` 默认改为 false；按长程保险
  保持 true。当前代码保持 true，等更长轮次或更弱摘要模型再验证。
