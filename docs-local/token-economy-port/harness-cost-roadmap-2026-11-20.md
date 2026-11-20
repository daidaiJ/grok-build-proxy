# 下一阶段 harness 成本优化路线（2026-11-20）

> 状态：**路线已下钻，P0 观测已落地**；任务拆分见
> [`harness-cost-drilldown-2026-11-20.md`](harness-cost-drilldown-2026-11-20.md)。
>
> **2026-11-20 拍板：压缩阈值不再调整。** 后续优化不涉及 `context_window` 与
> `auto_compact_threshold_percent`，`next-steps-2026-11-14.md` 里的 400K/72%
> 窗口实验作废。
>
> 数据窗口：2026-11-19 → 11-20，7 个已完成会话（排除当轮分析会话），864 次
> 模型调用、107.7M prompt、97.7% 缓存命中、795k 输出，估算 **$3.93**。
> 价格口径：未命中 $0.15/M、缓存读 $0.03/M、输出 $0.60/M；账本没有
> `costUsdTicks`，美元均为估算，占比比绝对值可靠。
>
> 相关文档：[`README.md`](README.md)（Codex/zcode token 经济性与 code-mode 调研）、
> [`next-steps-2026-11-14.md`](next-steps-2026-11-14.md)（上一轮实账，窗口结论已被本文取代）、
> [`deferred-builtin-tools-2026-11-14.md`](deferred-builtin-tools-2026-11-14.md)、
> [`../token-economy-audit-2026-11/README.md`](../token-economy-audit-2026-11/README.md)。

## 0. 结论

1. **“在压缩点裁剪 reasoning / 工具结果”已经是现状**，不是待做项。压缩后的回放
   历史里两者都归零；剩余占比全部在两次压缩之间的周期内。
2. 周期内中途裁剪会改写前缀，按当前单价，reasoning 需要后续 **16.4 次**调用、
   工具结果需要 **12.1 次**调用才回本。平均 turn 17.3 次，只有长 turn 明确划算。
   全局收益应来自**冷缓存窗口**与**轮次减少**，不是每轮硬裁。
3. 路线：**P0 观测/零代码 → P1 冷窗口 replay diet → P2 长 turn A/B →
   P3 轮次 reminder/batch → P4 code mode 有界评估**。取消 turn（19.8% 毛成本）
   与固定前缀作为并行小项。
4. code mode 社区热度高、成本证据强，但本 workload 的 MCP 只占 4.8%、terminal
   占 62.2%。它是**轮次杠杆的第二层**，排在 reminder/batch 之后；不直接移植
   QuickJS/V8 全量实现。

## 1. 成本基线（2026-11-20）

| 指标 | 数值 |
| --- | --- |
| 完成会话 / 调用 | 7 / 864 |
| prompt tokens | 107.7M |
| cached / 命中率 | 105.2M / 97.7% |
| 输出 tokens | 795k |
| 估算成本 | **$3.93** |
| 成本构成 | 缓存读 $3.11（79%）、未命中 $0.36（9%）、输出 $0.46（12%） |

- **上下文越大越贵**：>150k 的调用占 35.5%，却占 54.1% 成本；>200k 单档占 30.7%。
- **长 turn 集中**：top 5 turn ≈45% 成本；最大 turn 122 次调用、16.2M prompt、$0.552。
- **用途集中在主回合**：main 97.5%、compaction 2.4%、recap 0.2%。
- **活动集中在 terminal**：terminal 62.2%、edit 11.4%、read 8.5%、MCP 4.8%；
  工具轮次约占 92%，成本由“轮次 × 上下文”驱动，而非单个工具类型。
- **取消 turn**：50 个事件 turn 中 11 个 `mid_turn_abort`，186 次调用、$0.778
  毛成本（19.8%）；已提交的工具结果会保留，净浪费低于毛值。
- **缓存未命中构成**：增量 63.7%、空闲过期 23.1%、冷启动 6.1%、压缩 5.8%、
  recap 1.3%。
- **上下文构成**：工具结果 41.3%、reasoning 回放 30.5%、工具参数 7.8%、
  其他注入 6.4%、工具 schema 5.7%、项目规则 4.4%、system 2.0%。

## 2. 前提纠正：压缩点已经归零

| 事实 | 证据 |
| --- | --- |
| 压缩上下文进入组装前清空 `recent_messages` | `xai-chat-state/src/compaction_utils.rs:705` `for_compaction()` |
| 压缩视图里工具结果只有 `"Tool call omitted..."` 占位，reasoning 丢弃 | `compaction_utils.rs:473-517` |
| 压缩后历史 = system + user 前缀 + AGENTS.md + 最后用户问题 + 摘要 + 状态提醒 | `compaction_utils.rs:936-1007` |
| 回归测试断言压缩后不得有 ToolResult/占位残留 | `session_compact_compacted_history_shape_tests.rs:216` |
| 主压缩路径传入 `state_context.for_compaction()` | `compaction.rs:1864`、`:1920` |

因此“reasoning 回放限窗”和“旧工具结果老化”都不能再以“压缩点执行”为卖点：
压缩点已经把两者删干净。剩下的 30.5% / 41.3% 位于**压缩周期内**。
可用的方向只有三个：更早压缩（本轮明确不调阈值）、源头减量、缓存已冷时重写。

## 3. 成本数学：中途裁剪的盈亏平衡

按当前单价（未命中 $0.15/M、缓存读 $0.03/M，5 倍差）：

| 裁剪对象 | 占上下文 | 每次后续调用节省 | 一次全价重读 | 盈亏平衡 |
| --- | ---: | ---: | ---: | ---: |
| 跨轮 reasoning | 30.5% | $0.00915/M | $0.15/M | **≈16.4 次** |
| 旧工具结果 | 41.3% | $0.0124/M | $0.15/M | **≈12.1 次** |

本窗口平均 17.3 次调用/turn，但分布极不均：top turn 62–122 次，大量短 turn
只有几次。结论：

- 中途裁剪是**长 turn 优化**，不是全局 20–30% 的省钱项。
- 缓存已冷的时刻（空闲过期、换模型、压缩后、鉴权/错误重试）裁剪是纯收益。
- 冷窗口阈值按网关 TTL 定：OpenCode Go 实测约 5 分钟（见 §7），不是 30 分钟。

## 4. 路线

| 阶段 | 杠杆 | harness 落点 | 预期 | 风险 | 工期 |
| --- | --- | --- | --- | --- | ---: |
| **P0** | 观测 + 零代码 A/B | 配置 + 账本 | 5–10% | 质量回退 | 1 周 |
| **P1** | 冷窗口 replay diet | `request_builder` + 会话触发 | 5–10% | 低 | 2–3 周 |
| **P2** | 长 turn 中点裁剪 A/B | 同上，按 turn 长度门控 | 长 turn 10–20% | 缓存断崖、Responses 配对 | 1–2 周，数据门控 |
| **P3** | 轮次：reminder → batch_tools | prompt/reminder + meta-tool | 5–15% | 模型不配合 | 1–2 周 |
| **P4** | code mode 有界原型 | MCP-only `execute`，纯解释器 | 待验证 | 模型适配、沙箱 | 2–4 周评估 |

收益有重叠，不可相加。**压缩阈值不在任何阶段内。**

### P0 — 先量后改（1 周）

1. 单模型 A/B `reasoning_summary = "none"`：先确认网关是否仍返回 reasoning item；
   仍返回则此路不通，直接关掉实验。
2. 子 agent（explore/plan）试 `use_concise = true`：`grok-build-concise` 只带
   bash/read_file/search_replace、换紧凑系统提示、关 AGENTS.md 注入
   （`xai-grok-agent/src/config.rs:1460`，`agent_ops.rs:4086`）。主 agent 不切。
3. 轮次 reminder：系统提示或每 turn reminder 加一句“相互独立的工具调用合并到
   同一轮”。这是最便宜的实验，先量 `calls/turn` 与 `$/turn`。
4. 账本补每请求的 `reasoningReplayTokens` / `toolResultTokens`（可每 N 次采样），
   否则 P1/P2 无法归因。现有 `sessionId`/`purpose` 已够，不必重做。

### P1 — 冷窗口 replay diet（2–3 周）

- 触发点：空闲超过网关 TTL（OpenCode Go ≈5 分钟）、模型/agent profile 切换、
  压缩后、鉴权或错误重试后。判定在会话层，纯函数在 `xai-chat-state`。
- 配置草案 `[compaction.replay_diet]`：`reasoning_turns = 2`、
  `tool_result_keep_turns = 3`、`stub_old_results = true`。
- 旧 terminal/read/use_tool 结果替换为一行摘要 + `tool_call_id` +
  “完整输出可从持久化取回”的提示；不做 LLM 摘要。
- `/compress-fast` 已是**手动**版本（零模型调用、保留最近 N 轮、走
  `HistoryRewrite::RetainedPrune`）。P1 做的是**自动、只在冷窗口触发**，
  不重复实现清理函数。
- 预期 5–10%；验收看“触发次数 × 每次省下的 token”，且不新增缓存未命中。

### P2 — 长 turn 中点裁剪（数据门控，不默认开）

- 仅当上一 turn 调用数超过阈值（如 >20）时，在下一 turn 起点做一次 reasoning /
  旧工具结果裁剪；短 turn 会亏（见 §3）。
- Responses API 约束：删除时把 `(reasoning, assistant tool_calls, tool_results)`
  作为整体处理；当前轮 reasoning 保留，只删更早轮次。
- 先由 P0 度量证明长 turn 占比，再开 A/B；预期长 turn 10–20%，短 turn 持平或略亏。

### P3 — 轮次：先 reminder，后 batch（1–2 周）

- 分发层已并行：`tool_calls.rs:1023` 的 `FuturesUnordered`，同一响应里的多个
  call 会同时跑。缺的是模型愿不愿意一轮发多个。
- 第一步 reminder；第二步 `batch_tools` meta-tool：参数是 `[{tool, args}]` 列表，
  复用现有并发分发，返回每调用的压缩摘要。没有脚本运行时，风险低。
- 验收：`calls/turn` 降 ≥15%，返工率（重复读/重复改）不升。

### P4 — code mode：有界评估，不直接移植

| 产品 | 落地设计 | 状态/反馈 |
| --- | --- | --- |
| **OpenCode** | `@opencode/codemode`，纯解释器 psychoscript（非任意 JS 沙箱）；工具目录/命名空间、`search`/`scanTools`、OpenAPI、hooks、`maxToolCalls`/`maxOutputBytes`/超时、可挂起恢复；MCP server 可 `codemode: false` 退回直接声明 | 设计最完整；拒绝 `execute` 会移除 code mode 并把工具改回直接声明 |
| **Codex CLI** | `features.code_mode.enabled`；配置参考称“开发中、默认关”，0.162 发布说明有 “opt-in ranked tool search in Code Mode”，0.163 有 “Code Mode cells” | 两处来源口径不一致；程序在哪里运行未公开 |
| **Claude Code** | 实验特性，`CLAUDE_CODE_EXPERIMENTAL_CODE_MODE=1`，v2.1.323+，交互终端；JS 脚本调工具，默认只显示摘要 | 第三方实测账单降 22–23%，工具密集场景累计 token 降 62–77%；主要担忧是代码隐藏、guardrail、可观测性 |
| **Cloudflare / Anthropic / OpenAI** | isolate / Python 容器 / 每程序新 V8；OpenAI PTC 官方称 GPT-5.6 质量持平、输出 token 少 24%、快 28% | 运行时在 provider 侧，第三方网关走不了 |

社区信号（2026-11-20 检索）：

- Claude Code Code Mode 发布帖 500 分/115 评论，多数在玩名字梗；真实争议是
  “默认看不到代码”和 guardrail。
- DEV 实测（2026-11-19）：同一本地模型，脚本模式 11x fewer tokens、6x fewer
  turns；作者建议只用于“多步相似操作”，需要逐步判断的任务仍用经典模式。
- HN 两派：一派认可“上下文管理”是核心收益；一派认为只是把 bash 组合包装成
  JS/Python，担心锁定与可观测性。
- Codex/OpenAI 侧的成本证据来自 provider 官方与第三方基准，模型相关性强，
  不能直接外推到 DeepSeek 第三方网关。

**适配判断**：本窗口 terminal 62.2%、MCP 4.8%。code mode 的经典靶子
（大量 MCP 工具）在这里很小；terminal 本身已能用 bash 组合，缺的是模型批量
写脚本的习惯。所以：

1. 先做 P3 reminder / `batch_tools`，覆盖“平铺批量”的大部分收益。
2. 只有当 P3 证明瓶颈是依赖链/循环，再做 MCP-only `execute` 原型：纯解释器、
   无 ambient authority、带调用数/输出/超时上限、按模型能力位开关。
3. **不引入 QuickJS/V8 全量实现**：本仓库 workflow 侧已有 D1 决策（双运行时
   破坏 journal 确定性），主循环也不应再开第二个脚本运行时。

### P5 — 取消与固定前缀（并行小项）

- 取消：`send-now` 当前会取消运行中 turn（`prompt_queue.rs:576`），结果打
  `mid_turn_abort`（`cancel.rs:916/940`）。把默认行为改成排队或二次确认，
  目标拿回 5–10% 净收益。
- 固定前缀：v1.0.78 已把零调用内建工具延迟声明（10 个工具、5,667 tokens），
  剩余主要是 AGENTS.md 4.4% 与 system 2.0%。只做精简，不再扩 deferred 面。

## 5. 已落地，不重复建设

| 能力 | 版本/提交 | 说明 |
| --- | --- | --- |
| 账本 `sessionId` + `purpose`、`/stats` 分组 | v1.0.76 / #12 | main/recap/turn_summary/title/goal 可归因 |
| 缓存断崖检测 `session.cache_break` | v1.0.76 / #16 | 只观测，不自动动作 |
| 压缩调用入账、turn_summary 跳过、title 截断 | v1.0.76 | 压缩约 5% 支出可见 |
| 零调用内建工具延迟声明 | v1.0.78 `b8a4f1d9` | 20 工具 / 5,667 tokens，字节稳定 |
| `/compress-fast` | #17 `6fd59b0a` | 手动、零模型调用清旧工具结果 |
| loop detect | v1.0.79 | 循环检测与 abort/record 开关 |
| lazy LSP | v1.0.82 | 首次使用才起 workspace 诊断 |

## 6. 不做清单

- **不调压缩阈值**：`context_window` / `auto_compact_threshold_percent` 维持现网
  配置（2026-11-20 拍板）。
- **不在压缩点新增裁剪**：已经是现状。
- **不指望 append pruning 省 token**：只插失效 marker，内容原样保留
  （`request_builder.rs:287`）。
- **不把写入时截断当主杠杆**：本窗口 699 条工具结果中位数 742 B，成本来自
  条数累积；截断只对少数 35k 尾部有效。
- **不做默认 cache keepalive**：OpenCode 的 warming 是可选能力；按当前单价，
  每 4.5 分钟一次全前缀缓存读的成本高于一次冷重读，除非会话即将继续长时间工作。
- **不移植 QuickJS/V8 code mode**：见 P4。

## 7. 缓存 TTL 口径（2026-11-20 检索）

| 服务 | 有效 TTL | 依据 |
| --- | --- | --- |
| **OpenCode Go** | 默认 ≈5 分钟；按上游 provider 而定；可用 `settings.cache.ttl` 配 `5m`/`1h`/`24h`（OpenCode v2.2.0 起），但部分模型端点拒绝 cache 字段 | OpenCode 文档“默认不发送 cache lifetime”；issue #33998 明确 “gateway's ~5 min TTL”；`x-opencode-session` 的 sticky routing 决定是否命中同一上游 |
| **Command Code GOAT** | 未公开固定值；随路由到的模型/provider 而定 | 官方只公布 cache read/write 价格与 ~95–98% 命中率；社区答复“TTL expiry, time-based, varies by model/provider” |
| DeepSeek 官方 | “几小时到几天”，best effort | DeepSeek Context Caching 文档 |
| Anthropic | 5 分钟默认，1 小时可选 | Anthropic prompt caching |
| OpenAI GPT-5.6+ | 至少 30 分钟，最长约 24 小时 | OpenAI prompt caching |

对本仓库的含义：

- 我们的 `ocgo-deepseek-v4.1-flash` 走 `opencode.ai/zen/go/v1`，空闲 30–50 分钟
  的未命中与 ~5 分钟网关 TTL 一致；不能按 DeepSeek 官方的“几小时”估。
- P1 的冷窗口触发用 **5 分钟**，不是 30 分钟；具体值做成配置，按网关实测校准。
- 不要假设“换一个模型/网关 TTL 一样”：Command Code 这类多模型路由的 TTL
  是模型级事实，只能按 provider 分别记录。

## 8. 度量与验收

- 固定任务集：从 top 会话取 5 个代表任务（websearch CI 修复、OpenSandbox 选型、
  DLL/purego 评估等），每臂 ≥3 次运行。
- 指标：`$/session`、`calls/turn`、未命中输入占比、缓存命中率、reasoning 占比、
  工具结果占比、取消 turn 花费、返工率（重复读/重复改）。
- P0：成本降 ≥5%，任务成功率不降。
- P1：冷窗口触发次数与每次节省可归因，且不新增缓存未命中。
- P2：长 turn（>20 调用）成本降 ≥10%，短 turn 不升。
- P3：`calls/turn` 降 ≥15%，或证明模型不配合、止损。
- P4：工具密集任务成本降 ≥20% 且质量不降，否则停在 `batch_tools`。

## 9. 参考

- OpenCode 文档：<https://opencode.ai/docs/models>（Cache lifetime）、
  <https://opencode.ai/docs/warming/>、<https://opencode.ai/docs/console/go/>。
- OpenCode Go TTL 讨论：issue #33998（GLM 缓存掉到 ~500，gateway ~5 min TTL）、
  issue #51109（可配置 cache TTL，v2.2.0 落地 `settings.cache.ttl`）。
- `pi-opencode-go-cache` 包说明（~5 分钟网关默认、`prompt_cache_retention` /
  `cache_control` 扩展）：<https://pi.dev/packages/pi-opencode-go-cache>。
- Command Code：<https://commandcode.ai/guides/prompt-caching>、
  <https://commandcode.ai/docs/resources/pricing-limits>、
  <https://commandcode.ai/docs/plans/goat>；社区讨论
  r/CommandCode “How long does the cache last?”。
- DeepSeek Context Caching：<https://api-docs.deepseek.com/guides/kv_cache/>。
- OpenCode code mode：`packages/codemode/README.md`（psychoscript、限制、
  suspend/resume）；Codex code mode：配置参考 `features.code_mode.enabled`、
  0.162/0.163 release notes；Claude Code Code Mode：v2.1.323+ 实验特性及
  2026-11 第三方实测；Cloudflare Code Mode、Anthropic Code execution with MCP、
  OpenAI Programmatic Tool Calling。
- 检索时间：2026-11-20 15:20–15:30（UTC+8）。

## 10. 变更记录

- 2026-11-20：创建。纠正“压缩点裁剪”前提，定 P0–P4 路线；按用户拍板移除
  压缩阈值调整；补 OpenCode Go / Command Code GOAT 缓存 TTL 口径。
