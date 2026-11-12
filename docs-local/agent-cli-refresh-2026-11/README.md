# zcode v3.18.0 + Kimi Code 2.5.x：成本 / 稳定性 / 体验增量与 fork 吸收建议

> **状态：调研完成（2026-11-12），建议待拍板；零代码改动。**
> 数据源：zcode `b95ebaff`（v3.18.0，2026-11-11）对比 `a9fe401e`（v3.17.2）；
> Kimi Code `main 1ec24a5f`（2026-11-12）+ `apps/kimi-code/CHANGELOG.md` 2.4.0–2.5.3。
> fork 基线：`main ad4a30f2`（v1.0.73 之后）。关联：
> [`../zcode-317-port/README.md`](../zcode-317-port/README.md)、
> [`../harness-model-fit/README.md`](../harness-model-fit/README.md)、
> [`../agent-cli-tracking/`](../agent-cli-tracking/README.md)。

## 范围

只收**成本 / 稳定性 / 体验**三轴，按“对 fork 可落地 + 有明确收益机制”排序；
不重复 `zcode-317-port` 已收的协议兼容、工程治理与插件项，也不做全量 changelog 复述。

## 结论表

| # | 设计 | 来源 | 解决什么 | fork 现状 | 判定 |
|---|---|---|---|---|---|
| 1 | **Prompt Snapshot** | zcode 3.18 | 冷恢复/换版本后 system 字节变化 → 整段前缀缓存失效 | 顶层 resume 保留旧 head；仅子 agent spawn / 客户端 override / 换模型覆写 | 🟡 P3（缓存收益已被覆盖） |
| 2 | **一次性修复持久化**（signedReasoningReset / reduced images） | zcode 3.18 + Kimi 2.4.0 | 请求级修复每轮重付一次 400 与重试/转写 | D9 只改本次请求副本 | ⭐ P1 |
| 3 | **后台 subagent 停滞提醒** | zcode 3.18 | 后台任务无声卡死，父会话空等或反复轮询 | 无 | 🟢 P1（体验） |
| 4 | **重试判定结构化**（显式 retryability / 不猜配额 / 内容过滤不重试） | Kimi 2.5.0/2.5.1 | 文本推断导致误重试或过早放弃 | 部分 body 启发式 | 🟢 P2 |
| 5 | **上下文预算 / 组成** | zcode 3.18 | 模型可见成本约束 + 分项观测 | 只有百分比 | 🟡 P2（先 A/B） |
| 6 | **运行中 UX 批量**（/model、/effort、steer、/compact） | Kimi 2.5.0/2.5.3 | 长回合中改配置/插话被迫等流结束 | steer 只在安全点 drain | 🟢 P2 |

## 1. Prompt Snapshot（zcode 3.18）

**机制**（`apps/zcode-cli/docs/design/v2/loop/prompt-snapshot.md`、
`packages/core/src/runtime/helpers/prompt-snapshot.ts`）：

- 每个会话持久化一行 `runtime/prompt_snapshot`，内容是**本轮渲染出的 system blocks**
  （role/content/cacheControl）与模型 id；请求时若模型匹配，就用快照替换当前渲染结果。
- 只快照 system，**不快照 tools**：MCP 可能在首请求后连接并改变工具集，记录的
  tools 无法保持固定；接受 tools 段之后的缓存失效。
- 成功压缩（手动/自动/reactive）后清空快照并写 tombstone，下次主循环重新记录；
  microcompact / 跳过的压缩保留快照。fork 复制父会话快照给子会话。
- 读失败/非法行按“无快照”处理，不阻塞恢复；写失败只告警。

**fork 现状（2026-11-12 核对修正）**：顶层用户会话冷载**保留 journal 里的旧 head**——
`spawn.rs:1419` 调 `install_system_prompt`，非 subagent 分支不覆写；只有 subagent
spawn、客户端 `systemPromptOverride`（`mvp_agent/mod.rs:1050` → `ReplaceSystemPrompt`）
和换模型（`model_switch.rs:484`）会换字节。也就是说跨进程/跨版本恢复**不会**重写
system 前缀，Prompt Snapshot 的主要缓存收益在 fork 里已经由“保留旧 head”实现；
代价是恢复后的提示词/日期可能陈旧，这属于新鲜度取舍，不是缓存缺口。
`system_prompt_update = in-history`（`request_builder.rs:145`）补的是**会话内**变更。
真正剩下的缓存口子不在 system，而在 tools：`send_feedback` 描述嵌入本会话草稿路径
（`send_feedback.rs:137`），使每个会话的 tools 前缀唯一（见 §7）。

**建议**：降为 P3。只有确实需要“恢复后强制刷新提示词、同时保住缓存”时，才做
会话级 system snapshot；否则维持现状，把精力放到 tools 前缀的会话无关化。

**验收**：同一会话在二进制升级后恢复，首请求的 limit-probe prefix hash 与
`cached_tokens` 不出现断崖；压缩后首请求允许重建一次。

## 2. 一次性修复持久化（zcode + Kimi）

**同一类 bug 的两个现场**：

- zcode：Anthropic thinking 块被 400 拒后，adapter 只在**本次请求副本**去掉 reasoning
  重发；下一轮历史未变，再 400、再修——每轮多一次失败请求，模型也再看不到旧 thinking。
  修法是成功后在 assistant 消息上记 `signedReasoningReset`，之后投影时去掉标记前的
  全部已签名 reasoning，与修复那次发送的前缀逐字节一致
  （`docs/issues/reasoning-signature-repair/01-request-local-repair-repeats.md`）。
- Kimi 2.4.0（#4413）：provider 拒图后，后续轮次与压缩继续使用缩小后的图片，
  直到换模型或历史被重写；不再每轮重发原图。

**fork 现场**：视觉桥 D9 把 `image_bridge` 挂在请求上，重试时
`strip_images_with_transcriptions` 只改本次请求
（`xai-grok-sampler/src/actor/request_task.rs:414-450`）；chat-state 历史保留原图。
“能力位 true 但上游拒图”的场景下，**每轮都会重新转写一次**，代价 = 识图调用 + 延迟，
且每次首请求都先吃一个 400。

**建议**：在服务端确认拒图后，把转写 envelope 或“已缩减”标记写回会话状态
（按消息/工具结果 id），后续构建直接复用；换模型或历史重写时清除。把这条提升为
通用规则：**任何请求级修复成功后都必须落一个会话级标记**，否则每轮重付。

**验收**：同一图片第二次进入请求不再触发转写（日志/计数为零），首请求不再 400。
**1–2 人天**。

## 3. 后台 subagent 停滞提醒（zcode 3.18）

`packages/core/src/subagent/background-stall.ts` + `subagent-stall-reminder.ts`：

- 只把“新的模型请求/回复、工具开始/结束”算进展；Bash 每秒心跳与流式 token 不算
  （卡死的命令也会心跳）。
- 首次无进展 15 分钟提醒，之后间隔翻倍，上限 4 小时；有新进展计数清零。
- 提醒排队时若父会话正忙，**送达前重新校验**：Agent 已结束、有了新进展、进入审批、
  或被执行 id 变化（SendMessage 恢复）就丢弃，避免误导父代理甚至停掉在等人的 Agent。
- 措辞刻意不用 `<task-notification>`，避免 UI 把非终态读成“已完成”；停还是等由主代理决定。
- 定时器分段等待，规避 `setTimeout` 2^31-1 上限导致的毫秒忙循环。

**fork 缺口**：无停顿检测；后台任务靠 `wait_tasks`/`get_command_or_subagent_output`
轮询，长任务要么空等，要么模型反复查。

**建议**：复用后台任务 registry 记录 `lastProgressAt`，阈值触发后以 append-only
system reminder 注入父会话；送达前按同一套条件复验。**2–3 人天**。
**验收**：卡死的后台 subagent 只产生一条提醒；正常跑长工具不误报。

## 4. 重试判定结构化（Kimi 2.5.x）

- 2.5.0（#4477）：原样展示后端错误；**停止从文案推断配额**；解码后的模型错误必须带
  显式 retryability 才进入重试策略。
- 2.5.1（#4687）：动态工具 schema 产生空 system message，导致会话内反复 400——按
  协议边界过滤空消息。
- 更早的 2.4/2.5 批次还有“内容过滤命中不重试，立即提示”。

**fork 现状**：429/5xx 走结构化分类，但图片 413/拒图仍靠 `is_likely_body_rejected`
与签名匹配（`request_task.rs:380`、`strip_reason_for_image_error`）。这条对 D9 是必要的
方言适配，但要防止把“文案像限流”当成可重试。

**建议**：列一张 error → retry 决策表（结构化信号优先，文案启发式只用于已知方言），
每个方言一条夹具；新增重试分支必须写明 fallback。**约 1 人天**。

## 5. 上下文预算 / 组成（zcode 3.18，先 A/B）

- **总预算**：`TOTAL_TOKENS_BUDGET = 15_000_000` 固定值，system 末尾一行
  `<total_tokens>15000000 tokens left</total_tokens>`，每轮用附件更新倒数；
  只有主会话里**用户本人**的输入重新起算（轮首真实 prompt、或纯文本后取出的用户
  steer），工具批次照常提醒但不重置，goal 续跑/子代理轮首不提醒
  （`core/src/context/total-tokens.ts`、`runtime/helpers/total-tokens-budget.ts`）。
- **组成分解**：11 个细类别（system/meta/skills/tool_prompt/内置 schema/MCP schema/
  app schema/deferred schema/reminders/compact summary/messages），只给名字与大小，
  变更带修订号、支持草稿预演（`docs/design/v2/model/context-composition.md`）。
- **反证**：Kimi 曾在自动压缩前提醒上下文预算（#3423），后来又移除（#3526），
  说明“模型可见预算”的收益未被证明。

**fork 现状**：只有 `context_usage_pct`（`pager/src/app/acp_handler/mod.rs:519`）
与状态行 cache 段；没有模型可见预算，也没有分项。

**建议**：先 A/B 一条 15M 总预算行（长会话、同一任务），确认是否减少无效探索；
分项组成只在有 UI 消费方时再做。**1–2 人天（预算）/ 3–5 人天（组成）**。

## 6. 运行中 UX 批量（Kimi 2.5.x）

- 2.5.0（#4621）：`/model`、`/effort` 在 agent 工作时可用；**已开始的回合保留旧设置**，
  下一回合生效；“压缩并继续”排在当前回合之后。
- 2.5.3（#4762）：steering **中断当前模型请求**并在同一回合继续；新增单请求 steer API。
- 2.5.0：忙碌时 `/compact` 入队而不是报错；若期间已自动压缩则跳过。
- 2.5.0（#4627）：被 steer 的消息先淡显，读入后再转正常，避免“已生效”的误解。

**fork 现状**：steer 在工具批次后/循环顶/回合尾 drain
（`xai-grok-shell/src/session/acp_session_impl/interjection.rs:333`），不打断进行中的流；
长推理时插话要等模型生成完。`/model` 运行中是否可用未核实。

**建议**：steer 改为“取消当前流 + 保留已生成内容 + 同回合继续”，并允许 `/model`、
`/effort` 入队到下一回合；设置变更沿用“本回合不变”的规则，避免半途换模型导致前缀与
工具集同时变化。**2–3 人天**。

## 7. 缓存命中现状复核（2026-11-12，relay `deepseek-flash` = V4.1 Flash）

**实测基线**（`~/.grok/cache/model-usage.jsonl`，2026-11-02 → 11-12，1486 轮）：

- 稳态命中率：均值 96.5%、中位 98.95%、p10 94.2%；prompt token 总量 206.0M，
  其中 202.3M 命中（98.2%）。
- <50% 命中的 25 轮全部落在会话起点、隔夜恢复或压缩后；会话内连续轮次基本
  97–99.9%。
- 单轮最大 prompt 345k，**10 天没有一轮越过 500k**，即 `prune_mode` 覆盖的
  50% 上下文裁剪门从未触发。

**relay 探针**（本机配置，自签证书放行，请求 37–2461 prompt tokens）：

- 尾部/中段 `system` 均返回 200，且最新 system 指令生效（"secret word BANANA"）；
- 前缀缓存可见：同一 1245-token 请求第二次 `cached_tokens=1152`（92.8%）；
- 在 system 首字节前加 10 字节（模拟日期/提示词变更）→ `cached_tokens=0`。

**新发现（真正剩下的口子）**：`send_feedback` 的工具描述嵌入本会话草稿路径
（`crates/codegen/xai-grok-tools/src/implementations/grok_build/send_feedback.rs:137`），
使每个会话的 tools 段唯一。对比两个会话的 `tool_definitions.json`，23 个工具里只有
`send_feedback`（会话路径）和 `spawn_subagent`（模型清单随配置）不同；新会话首轮
命中停在 **10,240 tokens**（system 11KB + tools 54KB ≈ 16–18k tokens），后续轮次才补齐。
grok2 每次新建会话都要重付这段 prefill（首轮 TTFT 4–9s 的大头之一）。

**建议（按收益）**：

1. **P0**：把会话路径从 `send_feedback` 描述移到工具输出或静态占位（描述只说明
   “草稿保存在本会话目录”）。这是纯描述改动，风险低，能让新会话首轮命中
   system+tools 的完整前缀。
2. **P1**：对 cached-prefix 模型显式开两个能力位（relay 探针已过）：
   ```toml
   [model."relay/deepseek-flash"]
   prune_mode = "append"
   system_prompt_update = "in-history"
   ```
   触发频率低（见上），但越过 500k 或会话内提示词变更时避免整段断崖。
3. **P2**：把 50% 裁剪门（`request_builder.rs:217` 硬编码）上调或做成可配。
   cache_read $0.003/M 比 input $0.15/M 便宜 50 倍，500k 处裁剪省不下钱，
   却会打断前缀并丢工具结果。
4. **P3**：会话创建时预热 system+tools 前缀（空跑一次），把首轮 prefill 藏到
   用户输入期间；成本约 $0.003/会话。

## 8. Token 经济：阻碍与安全改造（2026-11-12 复核）

**Token 去向**（relay `deepseek-flash`，1502 轮）：

- 平均每轮 prompt 139.6k，其中 137.2k 命中（98.2%）；新增未命中约 2.5k/轮。
- 输出 772 tokens/轮，其中 reasoning 466（60%）。
- 成本拆分：cached $0.62 / uncached $0.55 / output $0.70，合计约 $1.87 / 10 天。
- 会话基座约 40k tokens：system 2.8k + tools 13.4k + AGENTS/rules 17.5k +
  skills 6.6k。首轮全额，之后按 cached 计价，所以“瘦基座”对账单影响很小，
  主要影响首轮 prefill 与上下文占用。

**阻碍（按影响排序）**：

1. **前缀字节稳定性**：`send_feedback` 的会话路径把新会话 tools 前缀变成唯一
   （首轮命中停在 10,240 tokens）；`spawn_subagent` 的模型清单随配置集合变化；
   换模型、改 skills/AGENTS 都会整段失效。断崖成本与上下文长度成正比，是唯一
   带乘数效应的杠杆。
2. **新 token 侧**：bash 输出上限 200k 字符（约 50k tokens/次，`agent/config.rs`
   的 `default_output_byte_limit`），全量进入历史；`[tool_output_compression]`
   默认关，且一期是 head/tail 有损压缩（唯一型 JSON、搜索结果尾部会丢）。
3. **输出侧**：config 设了 `default_reasoning_effort = "high"`，reasoning 占输出
   60%；subagent 未配 `[subagents.efforts]`。主线程降 effort 有质量风险，
   subagent 是安全面。
4. **裁剪/压缩定位**：prune 门 50% 硬编码、默认 rewrite；1M 模型下 500k 才触发，
   10 天最大 345k 未触发。压缩主要解决上下文与延迟，不是当前账单主项。
5. **观测缺口**：`model-usage.jsonl` 没有 session id/purpose，无法把辅助调用、
   压缩收益与断崖归因到会话；limit-probe 未开。
6. **辅助调用**：recap 默认开（10 次）、two-pass compaction（2 次）、title refresh
   关；金额可忽略，先不要动。
7. **压缩已做缓存对齐**：`compaction_verbatim_input` 默认 true，保留 tool I/O 以
   复用前缀（`compaction.rs:1010`）。为“省输入”剥 reasoning/tools 会让整段
   summarizer 输入从 cached 变 uncached，成本反而上升。

**安全改造（不伤稳定/体验）**：

- **P0（配置）**：`[toolset.bash] output_byte_limit = 40000`——中段截断与全量日志
  路径已有（`bash/mod.rs:364`），信息不丢；`prune_mode = "append"` +
  `system_prompt_update = "in-history"`（relay probe 已过）。
- **P0（代码）**：`send_feedback` 描述去掉会话路径，路径放进工具输出。
- **P1（代码）**：无损压缩层（重复行折叠、JSON 精确去重、search 全量保留、运行时
  往返自校验、CCR 可取回），一期有损层继续默认关；跨轮去重保持 keep-earliest +
  前缀单调。这是唯一能同时降 token、保语义、不破缓存的方向。
- **P2**：prune 门做成可配（默认不变）；`model-usage` 增 sessionId/purpose；
  按需配 `[subagents.efforts] explore = "low"`。
- **不做**：缩工具面/deferred tool exposure（缓存下收益极小、质量风险大）；
  全局降 reasoning effort；激进裁剪；为压缩重写历史。

**验收**：同任务 A/B 5 个会话，比较 uncached prompt tokens、TTFT、compaction 次数、
工具重跑/失败率；压缩层要求 prefix 字节不变且 `expand_output` 可取回原文。

## 不建议 / 存疑

- **context-composition 协议面**：fork 没有对应的插件/App 消费方，先不做线格式。
- **总预算行**：Kimi 加过又撤，收益未证；只作为 A/B 候选。
- **会话 30 分钟卸载、启动内存、全局搜索默认关**：服务端/桌面运维项，终端 fork 不适用。
- **off-peak 票据、session-lifecycle V4、dynamic-workflow**：维持既有负面清单。

## 未决

- [x] relay 冷恢复 head 是否断缓存：顶层 resume 保留旧 head（`spawn.rs:1419`），
  不重写；真正的会话级差异在 `send_feedback` 工具描述（见 §7）。
- [ ] `send_feedback` 描述去掉会话路径后，新会话首轮 `cached_tokens` 是否从
  ~10k 升到 ~16k（改完开新会话实测）。
- [ ] D9 转写在一次会话里平均触发几次（决定修复优先级）。
- [ ] 本地后台 subagent 的停滞阈值（15 分钟对交互式偏长，可先 5 分钟）。
- [ ] 总预算行 A/B 的判定口径（完成率 / token / 轮次）。

## 刷新方法

```bash
# zcode：按 tag 取两棵树做路径 diff，别信 commit message（快照提交）
curl -s "https://api.github.com/repos/zai-org/ZCode/git/trees/<tag>?recursive=1"
# 新设计文档集中在 apps/zcode-cli/docs/design/v2/ 与 docs/design/
# Kimi：CHANGELOG 是版本级事实源，PR 号可直接回查
curl -s "https://cdn.jsdelivr.net/gh/MoonshotAI/kimi-code@main/apps/kimi-code/CHANGELOG.md"
```

注意：zcode 是源码快照仓库，commit message 只有 “sync source vX.Y.Z”；Kimi 的
CHANGELOG 由 changesets 生成，条目带 PR 号，适合做锚点。
