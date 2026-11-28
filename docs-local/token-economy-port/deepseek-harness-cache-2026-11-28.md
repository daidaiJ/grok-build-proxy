# DeepSeek 官方 harness 的缓存优化（dsh 0.2.3-alpha.2，2026-11-28）

> **状态：调研入库，只读源码与文档，未做运行时 A/B。**
> 基线：`D:/CODE/ai/deepseek-harness-upstream`，commit `4b399c39a6`
> （v0.2.3-alpha.2，2026-11-22）。范围限定为 prompt/KV cache 的稳定性、
> 计量与断崖治理；PTC、插件体系只在影响缓存时提及。
>
> 关联：[`agentic-loop-token-cost-2026-11/README.md`](../agentic-loop-token-cost-2026-11/README.md)
> §4、[`replay-diet-2026-11-21.md`](replay-diet-2026-11-21.md) §12。

## 0. 结论先行

dsh 的缓存优化是一套**架构约束**，不是一组阈值：

1. **请求是 session log 的纯函数。** 模型可见的每条历史都先落盘，再由逐节点纯函数投影；
   `deriveMessages()` 与 `request/header` 都可从日志重建。前缀稳定性是 append-only 的
   副产品，不靠“记住上次发了什么”。
2. **能追加就不改写。** system 更新以 `system/message` 追加；`deepseek-flash` 声明
   `systemPromptUpdate: in-history`，新 system 文本跟在对应 user/tool-result turn 之后，
   历史前缀继续命中。工具变化走 developer message 的 `tool_addition` / `tool_removal`，
   而不是重排工具 schema。
3. **所有断崖都显式记账。** 压缩、工具结果裁剪、图片省略、真实 header 变化都落成日志
   事件；压缩摘要本身是 `user/message` 的 `surfaceOp: replace`，被遮蔽的 seqs 与
   token 数一并记录；裁剪前先写 `compaction/prune` shadow-price 事件。
4. **用 provider 的用量桶计量，逐 step 观察。** 未缓存输入、缓存读、缓存写、输出四桶
   互斥；DeepSeek 的 cache 指标映射进 `cacheReadTokens` / `cacheWriteTokens`；
   带 key 的 e2e 要求首请求之后每个请求 `cacheReadTokens > 0`。
5. **不做 TTL 猜测。** 仓库里没有 cache TTL 配置，也没有“等 5 分钟再改写”的逻辑；
   provider 管缓存寿命，harness 只保证请求可重建、改写可归因。这是它与 zcode /
   本仓冷窗口方案最根本的差异。
6. **改写要批量。** 工具结果裁剪按压力批量触发；图片省略按字节/数量量子成段移除最旧
   前缀，避免每来一张新图就重写一次旧请求。

代价同样明确：压缩和图片省略仍会从首个改动点切断缓存；工具 schema 的删除/重排仍会
断崖；`in-history` 依赖 provider 能力；PTC 的固定每请求成本与可见定义成正比。

## 1. 机制总览

| 层 | 机制 | 缓存效果 | 证据 |
| --- | --- | --- | --- |
| 请求构造 | append-only log → 纯函数投影 | 默认前缀扩展 | `.agents/notes/implemented/architecture/2026-07-05-reconstructable-requests.md:19` |
| 请求 envelope | `request/header` 全量快照 | header 变化可 diff；日志事件不进 prompt | `packages/core/session/src/request-header.ts:21`；`docs/subsystems/session.md:182` |
| system prompt | surface node 追加；`in-history` 追加表示 | 保留已缓存历史 | `packages/llm/llm-deepseek/src/serialize.ts:65`、`:86-114` |
| 工具变化 | `tool_addition` / `tool_removal` developer message | 新增可追加；删除/重排仍断崖 | `packages/core/agent-loop/src/agent.ts:682-705` |
| 压缩 | `user/message` replace + shadowed seqs | 一次明确断崖 | `docs/subsystems/compaction.md:11`、`:16` |
| 工具结果裁剪 | 单节点 replace + `compaction/prune` 计价 | 按压力批量断崖 | `.agents/notes/.../reconstructable-requests.md:54` |
| 图片省略 | 占位文本 + 阶梯高水位 | 成段摊销断崖 | `packages/compaction/compaction-image-offload/README.md:12`、`:88` |
| 计量 | 互斥 usage 桶 + 逐 step 上报 | 断崖可在下一次调用看到 | `packages/llm/llm/src/types.ts:177`；`request-cache.e2e.ts:21` |
| 压力投影 | `pressureTokens` + 有符号 surface delta | 压缩效果在下一次请求前可见 | `packages/llm/token-meter/src/usage-projection.ts:78`、`:159` |

## 2. 请求 = 日志的纯函数

### 2.1 原则

> “Model-visible ⟺ durably referenced”；每个请求都能由日志 + 对象 + 代码逐字节重建。

`Session.deriveMessages()` 对每个 surface 条目只投影一次，投影结果是共享的深冻结消息；
surface 被替换（压缩的 `replace`）时才重建。调用方拿到的是新数组，但消息本身不可变。
外部重建器对同一日志前缀跑同一个纯函数，因此不存在“loop 发的请求”和“日志重放的请求”
两份真相。

`EpochHeader` 只记录**非历史状态**：调用配置、adapter 默认值标记、组装后的工具 schema。
渲染后的 system prompt 不在 header 里，它是 surface node 0 的 `system/message`；普通
prompt 变化追加 system 节点，header 保持不变。请求由 `header.config + deriveMessages()
+ header.tools` 组成，**没有顶层 `system` 字段**。

`request/header` 永远写全量快照：

| reason | 何时写 |
| --- | --- |
| `initial` | 首个 loop instance |
| `resume` | 恢复/重建后的新 instance |
| `change` | 同一 instance 内 envelope 真实变化 |
| `series` | envelope 未变，但开始显式声明的 message series，或 surface 刚被替换 |

`startSeries: true` 与 `change` 可同时存在，两个事实分开记录。普通 append 的后续 turn、
step、retry 继承最近快照。header 事件是 log-only，不是 `SurfaceEventType`，不产生模型
消息——全量快照增大的是日志，不是 prompt。

### 2.2 前缀稳定性是推论，不是目标

> “an append-only log projected by a per-node pure function yields requests that are
> append-extensions of their predecessors whenever the header is unchanged — stability is
> emergent, not managed.”

测试直接锁这个性质：`expectPrefixExtension(previous, current)` 断言当前请求的 messages
是前一请求的严格值前缀，工具数组完全相同；`request-reconstruction.spec.ts` 覆盖同一
turn 的多 step、跨 turn、显式 series、以及 header 变化。真实 provider 由带 key 的
`request-cache.e2e.ts` 验证：一次工具 turn + 一次跟进 turn，首个请求之后每个请求都必须
`cacheReadTokens > 0`；测试注释把 cache block 粒度标为 64 tokens。

对本仓的含义：**任何“每次请求都变”的字段都是缓存 bug**。时间戳、随机排序、cwd、
session id、动态工具目录、未稳定排序的 schema，只要进了 prompt 或 tools，就会让前缀
从变化点失效。dsh 的做法是把这些元数据放到模型输入之外（见 §3.6）。

## 3. 断崖治理：能追加就不改写

### 3.1 system prompt：追加到历史尾部

`systemPromptUpdate: in-history` 的路由保留初始顶层 system，新增 system 文本在序列化时
排到对应的 user/tool-result turn 之后：

- `serialize.ts` 把多条 system 变化收集进 `systemUpdates`，遇到 assistant 消息前
  `flushSystemUpdates()`；没有前置 user/tool-result turn 时直接报错。
- 未声明该能力的模型把所有 system 文本拼进顶层 system 字段；任何变化都会从 system
  起点改写前缀。

真实 API 测试给出了对照（`packages/llm/llm-deepseek/tests/runtime.e2e.ts:420-448`）：

1. 先发一次初始 system，再发一次相同请求，`cacheReadTokens > 0`，得到可复用 token 基线。
2. 在历史尾部追加 assistant + 新 system + 新 user：新 system 走 in-history，
   `cacheReadTokens >= 基线`。
3. 把新 system 放回顶层、重建请求：`cacheReadTokens` 低于 in-history 版本。

结论：同一段 system 变化，放在尾部可保留此前全部前缀；放回顶部只保留到变化点之前。
dsh 的系统提示节点 0 受保护（`assertSystemHeadRewrite`），普通变化追加后续节点；
压缩后下一轮用 `system/message` replacement 合并当前指令，这是一次显式断崖。

**本仓对照。** 本仓已有 in-history 系统提示与“稳定前缀”实践；需要检查的是所有 system
更新路径是否都追加到尾部，而不是重建整段 prompt；压缩后的 system 合并是否只发生一次。

### 3.2 工具变化：追加声明，避免重排 schema

工具 schema 在 `EpochHeader.tools` 里全量记录。`headerEquals()` **按顺序**比较 schema
（`request-header.ts:31-43`），`canonicalHeader()` 不做排序。因此工具组装顺序必须稳定；
一次无意的重排会被判成 `change`，从第一个变化的 schema 起断崖。

工具变化本身尽量走历史：

- 新增/删除工具时，loop 在当前 step 里追加一条 `developer/message`，内容是
  `tool-addition` / `tool-removal` 引用；适配器把它渲染成 system-role 的
  `tool_addition` / `tool_removal` 块，放在前一个 user/tool-result turn 之后。
- `deepseek-flash` 默认 `toolUpdate: addition-only`：删除工具或向空集合新增工具会重建
  基线；单纯的追加保持前缀。
- `in-history` 路由保留被删除工具的定义与删除消息。
- deferred 声明带 `defer_loading`，初始 schema 可以只暴露目录；后续声明仍可保持
  deferred，直到真正激活。
- 只有请求里出现 `tool_addition` / `tool_removal` 块时才发送
  `mid-conversation-tool-changes-2026-07-01` beta header。

缓存收益来自“新增是尾部追加”；删除、重排、schema 字段变化仍从首个改动 token 断崖。
所以工具面设计应优先**只增不减**，并把删除批量到一次明确的 header change。

**本仓对照。** 本仓已有 deferred builtins、MCP 懒加载（`search_tool` / `use_tool`）和
稳定的工具声明；还需要确认工具数组按稳定键排序、MCP 发现结果不会每轮改变顺序，
以及工具增删是否走显式事件而不是静默重建 schema。

### 3.3 压缩：一次替换，全程记账

压缩的三个事件都是 log-only：

- `compaction/start`：记录锁；
- `compaction/summary`：安全摘要、可选原始输出、`shadowedRange`、`shadowedSeqs`、
  `shadowedTokenCount`、provider/model、摘要调用 usage；
- `compaction/end`：释放锁。

摘要本身不是新事件类型，而是一条 `user/message`，带
`surfaceOp: { op: 'replace', startSeq, endSeq }`；下一轮模型请求看到的是替换后的 surface。
`shadowedSeqs` 是权威的被遮蔽节点集合，顺序按 surface；因为 replacement 会让可见 seq
非单调，范围用位置对而不是数值区间。

工具结果裁剪复用同一机制：对单个 tool result 做 `startSeq === endSeq` 的 replace，
保留 `callId`，并在替换前写 `compaction/prune` shadow-price 事件，用 token meter 给被
替换节点计价。重构笔记明确说这是 “cache-bust batched by the same pressure logic”。

要点：

- 压缩必然断一次缓存；dsh 不为它做 TTL 判断，而是把这次断崖的成本（摘要调用 +
  被遮蔽 token）写进日志，便于事后归因。
- 裁剪与压缩共享压力触发，避免“裁剪一次断崖、压缩又一次断崖”。
- 摘要调用自身的 usage 也记录，压缩成本可核算。

**本仓对照。** 本仓的 replay diet 是确定性的 surface 重写，冷窗口路径已经按网关 TTL
推迟到缓存冷后执行。可补的是 dsh 式的“显式计价”：在 `shell.replay_diet` /
`shell.idle_microcompact` 事件里带被替换节点数、shadow tokens、预计断崖成本与
break-even 调用数，而不是只带 `est_reclaim_tokens`。

### 3.4 图片省略：阶梯高水位

图片超出路由预算时，插件把较早的图片永久替换成占位文本（附件 id、可读路径、尺寸），
然后重试；选择跨请求、resume、replay 保留。

缓存上的关键设计是**阶梯高水位**：Files 模式按 `maxRequestFilesBytes` /
`maxImagesPerRequest`，内联模式按 base64 预算，一次移除一个“最旧前缀”量子（字节或
数量），而不是每来一张新图就重写一次旧请求。这样每次断崖摊销到一批图片上。

但断崖仍会发生：`provider cache reuse ends at the first changed image for that request`。
此外，执行世界路径变化会改写历史描述文本，刷新上传会替换 `file_id`，退避时切内联、
恢复后切回 files，也会改变历史表示。确定性请求图片字节并不等于完整前缀不变。

已知代价：每次省略都先付一次失败的 provider 尝试（按路由拒绝触发，不预判）；临时小
预算或 Files 故障会造成永久省略。

**本仓对照。** 如果本仓的图片裁剪是逐张改写，建议改成同样的量子批量；并且把“省略
发生在哪条历史消息”记录成事件，便于解释下一次 cache-read 下降。

### 3.5 retry / resume / series

- retry 复用已经提交的历史与冻结的消息，不重新跑 `agent/pre-step`，也不重新组装 prompt；
  每次 attempt 只追加自己的 assistant frame。
- `request/header` 的 `series` 明确标记“envelope 未变但开始新 message series”，例如
  surface 被替换后。这样缓存域的重置是显式事实，不靠隐式猜测。
- resume 写 `resume` 快照；如果恢复后的 envelope 与日志不同，差异可 diff。

**本仓对照。** 本仓的 request freeze 与重试路径需要保证：重试不重建 system/tools；
压缩后的第一次请求只断一次；`series` 之类的边界在日志里可识别。

### 3.6 provider 元数据留在模型输入之外

- 稳定的匿名 user id、session id 放在请求 header，不放进 prompt；
- `dsh_session_log`、`dsh_plugin_packages` 是 provider 专用顶层字段，明确在模型消息、
  system prompt、工具 schema 之外；
- 图片请求描述里的执行世界路径属于模型可见文本，因此它变化时会真实断崖——这一点被
  如实记录，而不是假装图片字节确定就万事大吉。

## 4. 缓存计量与可观测性

### 4.1 互斥用量桶

`TokenUsage` 的计数是**互斥**的：

- `inputTokens`：仅未缓存输入；
- `cacheReadTokens` / `cacheWriteTokens`：缓存读/写，单独计；
- `outputTokens`：输出；
- `totalTokens`：完整调用总量（含前三者）。

DeepSeek 适配器把 Messages 兼容层的 `cache_read_input_tokens` →
`cacheReadTokens`、`cache_creation_input_tokens` → `cacheWriteTokens`；
`totalTokens = input + output + cacheRead + cacheWrite`。e2e 注释把 provider 原生指标
称为 `prompt_cache_hit_tokens`。四桶互斥避免“把缓存读又算进未缓存输入”的双计。

### 4.2 逐 step 观测

每次 assistant settlement 在 `assistant/message` 上带 usage；token-meter 的投影按
(turn, step) 替换同一步的旧样本（`llm/retry-started` 会关闭替换槽），所以 retry 不会
把失败尝试和成功尝试重复累加。

生产观测就是**每个 step 的 cacheRead**：

- 首请求之后应持续为正；
- header change 或 compaction 之后的下一次调用应出现 cacheRead 下降；
- 降幅与日志里的 shadowed tokens / 被替换节点对得上。

带 key 的 e2e 是这条链路的验收：多 step 工具 turn + 跟进 turn，所有后续请求
`cacheReadTokens > 0`；测试用长 system prompt 确保首个请求就跨过 64-token 缓存块。

### 4.3 token meter：按节点计价与有符号投影

`TokenMeasurement` 给出：

- `baseline`：provider usage 锚点或全量估算；
- `surfaceDeltaTokens`：相对锚点的有符号重计价；
- `totalTokens` / `surfaceTokens`；
- `nodes[]`：每个 surface 节点的 `tokens`（按当前路由计价）与 `heuristicTokens`
  （固定启发式，供替换节点做 shadow price）。

替换节点用 `heuristicTokens` 计价，保证 O(1) 投影折叠与自身 append 一致。这样压缩/
裁剪不需要重新扫描历史，也能在日志里说明“省了多少”。

`ContextPressureProjection` 进一步把“下一次请求的压力”提前显示：

- `pressureTokens` = 最近一次 provider 报告的 prompt 侧用量
  （`input + cacheRead + cacheWrite`，不含输出）；
- `projectedTokens` = `pressureTokens` + 自该样本以来 surface 的有符号变化。

压缩本身没有 usage，所以只靠 `pressureTokens` 看不到它；`projectedTokens` 在压缩
落地后立刻下降，下一次请求再用真实 usage 校准。usage 样本在同一个事件加入 surface
**之前**打戳，保证它锚定的是该请求实际看到的前缀；retry 关闭替换槽，避免重复计数。

### 4.4 本仓可补的观测

本仓账本已有 cache hit 与 token 加权命中，但还缺三样：

1. **逐 step 的 cacheRead/cacheWrite**：现在多是会话累计；断崖应定位到具体 step。
2. **断崖原因**：compaction / replay diet / system rewrite / tool schema change /
   image offload；与上一次请求的 header diff 关联。
3. **有符号 surface 投影**：压缩或 diet 后，下一次请求前的预计压力应立刻变化，而不是
   等 provider 回报。

这三样都不改请求字节，属于纯观测，适合先做。

## 5. 不该照搬 / 边界

- **没有 TTL 逻辑。** dsh 不判断“缓存是否还热”，也不等待。provider 管缓存寿命；
  harness 只保证 append-only 与显式改写。对本仓的网关（实测 ~5 分钟 TTL），冷窗口
  等待仍然必要；两者假设不同，不能直接删掉本仓的 TTL 路径。
- **全量 header 快照增大日志。** 它换来单一 replay 表示与可 diff 的 header 变化；
  header 不进 prompt，所以不增加 token。若本仓日志敏感，可以保留全量快照但压缩存储。
- **压缩仍断崖。** dsh 没有把压缩变成免费；它只是让断崖一次、可计价、可归因。
- **图片省略有失败成本。** 每次省略先付一次失败尝试；预算频繁抖动会放大成本。
- **工具 schema 变化仍断崖。** 只有“新增”走尾部追加；删除/重排/字段变化从首个改动
  token 起失效。工具面应优先只增不减。
- **in-history 依赖 provider 能力。** 未声明的模型会退回顶层 system 拼接，缓存收益消失。
- **PTC 不是普适降本。** 固定每请求成本与可见定义成正比，SDK 文本可能接近工具 schema；
  真正的节省来自 step 折叠与中间值隔离。

## 6. 对本仓的落地建议

| 优先级 | 动作 | 验收 |
| --- | --- | --- |
| P0 | 请求可重建：把 call config + tool schema + message 投影落成可 diff 的 envelope；加 mock 前缀扩展测试 | 同一 turn 多 step、跨 turn 的请求 messages 是严格前缀；工具数组逐字节相同 |
| P0 | 前缀稳定性审计：cwd、时间戳、随机排序、session id、动态工具目录、MCP 发现顺序 | 连续请求除新增历史外无字节差异；工具按稳定键排序 |
| P0 | 带 key 的 cache e2e：首请求之后 `cacheRead > 0` | 多 step + 跟进 turn；失败时输出 header diff |
| P1 | 逐 step 账本：cacheRead/cacheWrite、断崖原因、shadow tokens | `/stats` 或 jsonl 能定位到具体 step 与原因 |
| P1 | 系统提示与工具新增走尾部：in-history / addition-only / deferred | system 变化后历史前缀仍命中；工具新增不重排 schema |
| P1 | 压缩/裁剪显式计价：`reason`、被替换节点、shadow tokens、break-even | 断崖成本可在事件里复核 |
| P1 | 有符号 pressure 投影：压缩/diet 后下一次请求前压力立即变化 | 显示值与下一次 provider usage 校准 |
| P2 | 图片/大结果按量子批量改写 | 一次断崖摊销一批，而不是逐条 |
| P2 | series 边界显式化：压缩后、模型切换后重置缓存域 | 日志可解释 cacheRead 下降 |

**不建议**把 dsh 的“无 TTL”直接搬过来：本仓网关的缓存寿命是实测的 ~5 分钟，冷窗口
diet 的 330s 等待仍然成立。可以搬的是它的纪律：改写必须显式落日志、必须计价、必须
尽量批量，并且下一次调用的 cacheRead 要对得上。

## 7. 与 zcode / 本仓冷窗口的对照

| 方案 | 触发 | 是否等缓存冷 | 成本归因 |
| --- | --- | --- | --- |
| zcode microcompaction | 空闲约 60 分钟 | 是，按 provider 缓存过期假设 | 旧工具结果清理，零模型调用 |
| 本仓冷窗口 diet | 普通空闲 >330s | 是，按网关 ~5 分钟 TTL | `shell.idle_microcompact.fired/skipped` + replay diet 事件 |
| dsh | 无 TTL；压缩/裁剪/图片省略按压力或路由拒绝触发 | 不判断；改写仍断崖 | `compaction/*`、`image/offload`、`compaction/prune`、逐 step usage |

三者的共同点是“不要在缓存还热时随便改写历史”。差异在**怎么知道热不热**：zcode 与本仓
用时间假设，dsh 用“只追加 + 改写必记账”，把判断留给 provider 的 cacheRead 反馈。
对本仓，最稳的组合是：保留冷窗口 TTL 门槛，同时把每次改写的 shadow 成本与下一次
cacheRead 对账；长期再用实测 TTL 校准 330s。

## 8. 证据索引

| 主题 | 路径 |
| --- | --- |
| 可重建请求 / 前缀稳定推论 | `.agents/notes/implemented/architecture/2026-07-05-reconstructable-requests.md:19`、`:25`、`:31`、`:54` |
| header 全量快照与 reason | `docs/subsystems/session.md:180-209`；`packages/core/session/src/request-header.ts:21-62` |
| 请求组装与 series | `packages/core/agent-loop/src/agent.ts:656-730` |
| 前缀扩展测试 | `packages/core/agent-loop/tests/request-reconstruction.spec.ts:70-104` |
| 真实 provider cache e2e | `packages/core/agent-loop/tests/request-cache.e2e.ts:21-29`、`:80-105` |
| system in-history 序列化 | `packages/llm/llm-deepseek/src/serialize.ts:65`、`:86-130`；`README.md:88`、`:189` |
| in-history vs 顶层 system 对照 | `packages/llm/llm-deepseek/tests/runtime.e2e.ts:420-448` |
| 工具新增/删除事件 | `packages/core/agent-loop/src/agent.ts:682-705`；`packages/llm/llm/README.md:189` |
| 压缩事件与 shadowed seqs | `docs/subsystems/compaction.md:11-24`、`:232-248` |
| 工具结果裁剪 / shadow price | `.agents/notes/.../reconstructable-requests.md:54`；`docs/subsystems/compaction.md:239-241` |
| 图片省略与阶梯高水位 | `packages/compaction/compaction-image-offload/README.md:12`、`:88-95`；`packages/llm/llm-deepseek/README.md:109` |
| 互斥 usage 桶与映射 | `packages/llm/llm/src/types.ts:177-195`；`packages/llm/llm-deepseek/src/translate.ts:37`、`:157` |
| pressure / projectedTokens | `packages/llm/token-meter/src/usage-projection.ts:78`、`:159-210`；`docs/subsystems/token-meter.md` |
| provider 元数据在模型输入之外 | `packages/llm/plugin-package-inventory-deepseek/README.md:51`；`packages/llm/llm-deepseek/README.md`（provider-specific request fields） |

## 9. 开放问题

1. **官方 DeepSeek 缓存的真实 TTL 与容量策略未在本仓文档中给出。** 本仓网关的 ~5 分钟
   不能外推到 `api.deepseek.com`；需要按路由实测，再决定冷窗口阈值。
2. **全量 header 快照的日志增长**在超长会话里需要治理；当前设计选择“一份表示优先于
   字节节省”，本仓若移植应同时评估日志轮转/归档。
3. **shadow price 的精度**依赖 heuristic estimator；替换节点越多，投影误差越需要
   用 provider usage 校准。
4. **64-token 缓存块**意味着小于一块的尾部变化可能部分命中；观测应看 token 数，不要
   只看“是否完全命中”。
