# dsh 上下文工程移植 — 设计原型（缓存友好的系统提示/工具/reasoning 通道）

> **状态：设计原型，未拍板未开工。** 隶属 [`README.md`](README.md)（dsh 适配移植
> 预研）P1。dsh 基线 `v0.2.1-rc.2`；fork 基线 `e7edf08`。可信度分级与矩阵见
> README；本文只做设计。行级锚点标注"待补"者为未精读，落地前先精读补齐。

## 0. 问题陈述

DeepSeek 的 prompt cache 是**前缀精确匹配**，且缓存键覆盖请求的多个组成部分：

- 本仓实测（`prompt-cache-notes.md`，火山 Ark）：tools 数组一变，**整段前缀（含
  ~13k system prompt）作废**；工具集稳定的会话逐轮命中（deepseek 98.7%）。
- dsh 文档（llm-deepseek README "KV Cache effect"）：model-route、prompt、schema、
  history、image-budget 任一变更，从**首个变更 token** 起不可复用。

fork 现状 = 每轮整请求重建、系统提示固定在 messages[0]。系统提示的任何变更
（模型切换、工具集变化、rules 注入、plan 模式切换）都把整段前缀打掉——在
DeepSeek"缓存命中费用占比高"的成本结构下（1017 官方口径），这是 Agent 长会话
费用的主要泄漏点之一。

dsh 为同一模型做的解法是"**历史不动、变更追加**"：系统提示快照与工具增删都以
追加块进历史，保住前缀。本文把这三条通道移植成本 fork 的请求装配层设计。

## 1. dsh 侧机制（A 级，源码直读）

### 1.1 `systemPromptUpdate: 'in-history'`

- `src/models.ts` 默认目录中 `deepseek-flash` 条目声明
  `systemPromptUpdate: 'in-history'`（`deepseek-v4-pro` 未声明，需显式声明且
  非 `in-history` 值在加载时报错）。
- API 语义（dsh 文档转述，B 级）：端点把 messages 中**任意位置的最新一条
  system 消息**读作完整生效系统提示。
- 于是 dsh 的做法：系统提示变更时，把新快照**追加在缓存历史之后**（跟在对应
  的 user/tool-result 轮次后），而不是改写开头的 system——前缀直到历史末尾
  保持字节不变，缓存复用从变更点之后才失效。
- 约束（README "Known Limitations"）：in-history 更新之后必须存在 retained 的
  user/tool-result 轮次；若更新后全是被省略的空 user 输入且前一个 wire 轮次是
  assistant，序列化失败（`UNSUPPORTED_CONTENT`）。

### 1.2 `toolUpdate: 'addition-only'`

- 默认 `deepseek-flash` 条目声明 `toolUpdate: 'addition-only'`。
- 会话中途增删工具：投喂 system 角色的 `tool_addition` / `tool_removal` 块
  （引用已声明工具名，延迟声明带 `defer_loading`），并携带
  `mid-conversation-tool-changes-2026-07-01` beta header。
- 本质是拿"**只增不删不改**"的约束换前缀可复用：工具 schema 一旦变化仍从首个
  变更 token 失效（README 明说），所以只走追加式增删。

### 1.3 reasoning 回传

assistant 历史（**包括带工具调用的轮次**）的 thinking/reasoning 块逐字回传进
后续请求，保持跨轮推理连续性；缓存会计把 reasoning 回传计入（每次 reasoned
轮次都在追加点破坏旧前缀——这是回传的固有代价，dsh 照收）。

### 1.4 配套的失效面管理

1M 上下文 / 256k 输出默认预算（`defaults.ts`）；cache-hit 指标映射进 usage；
请求图片做确定性版本（Files API 上传复用），避免图片表示变化打掉前缀。

## 2. fork 现状锚点

- **请求装配**：`xai-chat-state/src/actor/request_builder.rs`（in-history 的
  放置点在请求装配层）；系统提示装配：`xai-grok-agent/src/prompt/context.rs`、
  `builder.rs`（行级锚点待补）。
- **usage 解析**：`xai-grok-sampling-types::types::Usage`——`cached_tokens` 与
  DeepSeek 风格 `prompt_cache_hit_tokens` 取 max（`prompt-cache-notes.md` 已核实）。
- **系统提示/工具集的触发面盘点**（每处都是今天的前缀破坏点）：
  模型切换（零回合重建簿记，lsp 专题复用过该机制）、工具面变化
  （deferred-tool-exposure P2 T2 未开工）、rules / `systemPromptOverride`
  （session `_meta`）、plan 模式切换、compaction 重写历史
  （`xai-grok-compaction`，含 idle_auto_compact）、fork/rewind
  （RewindMarker SwitchEdge）。
- **"已有"判定对照**（fork-feature-inventory 纪律）：in-history / addition-only /
  reasoning 回传均无（`reasoning_content` 在 sampler/sampling-types grep 0 命中）；
  验收设施已有（limit-probe：sampler HTTP 层被动落盘 prefix hash +
  cached_tokens 到 `~/.grok/limit-probe/records.ndjson`）。

## 3. 设计

### 3.1 能力位（per-model，默认关）

```toml
[model.<id>]
system_prompt_update = "off" | "in-history"   # 默认 off
tool_update           = "off" | "addition-only"  # 默认 off
```

命名对齐 dsh 便于跨文档对照。落点沿用 `use_proxy`（二十三期）的先例：
`SamplingConfig`/`ModelEntryConfig` 加字段 + config.rs 装配 + 随会话持久化
（`xai-grok-sampling-types`）。是否暴露进 ACP `configOptions` 与 /auth 面板
回填：待拍板（§6）。

### 3.2 端点语义 probe（T1 前置门）

in-history 语义目前只有 dsh 对 **Anthropic 兼容端点**（`api.deepseek.com/anthropic`
`/v1/messages`）的背书；fork 的 deepseek 条目实际配置走 chat_completions /
responses（用户 config 即 responses）。**probe 通过才允许该 backend×model 组合
开启能力位**。

零成本决定性实验（复用 `prompt-cache-notes.md` 的两连发打法）：

1. 同一段 messages，system@0 声明"回答必须带标记 A"→ 发两次确认命中与行为；
2. 第三次请求：messages 中部（最后一条 tool-result 之后）追加一条**内容改为
   标记 B** 的 system → 看模型服从 A 还是 B。服从 B = in-history 语义成立；
   服从 A/报错 = 该端点只认首条 system，能力位对该组合保持 off。

对三种 `api_backend`（chat_completions / responses / messages）分别跑一轮，
probe 结果回填本节。

**probe 结果（2026-10-24 已执行，全部通过）**：两连发实验（system@0 规则 A
连发两次确认 → 中部追加规则 B system 看服从），payload 见 `.probe/`（工作区
未入库）。标记 B（新快照）在三通道全部生效、请求全部接受：

| 通道 | 端点 | 接受中部 system | 最新快照生效 | 语义备注 |
|---|---|---|---|---|
| chat_completions | DeepSeek 官方 `/v1/chat/completions` | ✅ | ✅ B 胜 | 最干净 |
| chat_completions | **Command Code 网关**（`api.commandcode.ai/provider/v1`，`deepseek/deepseek-v4.1-flash`，用户主用通道） | ✅ | ✅ B 胜 | 生产通道验证 |
| responses | DeepSeek 官方 `/v1/responses` | ✅ | ✅ B 胜 | 端点把中部 system **降级为 user 消息**（模型 reasoning 明说「用户消息里的规则」），功能性达标但为用户级指令语义 |
| messages | DeepSeek 官方 `/anthropic/v1/messages` | ✅（非标 Anthropic 也接受） | ✅ B 胜 | 同为用户级；且 fork 转换层把 System 项折叠进顶层 `system` 参数（`conversation/messages.rs:251`），中部快照到不了原位——**该 backend 的能力位暂不放行**（probe 语义不可直接迁移） |

**放行矩阵**：chat_completions / responses 开能力位；messages backend 门禁
拒绝（折叠转换层 + 语义降级双原因，如需支持先改 messages 转换层保留原位再
重 probe）。采样请求极小（53–92 prompt tokens），零成本门槛达成。

### 3.3 放置与重放确定性

- **放置规则**（沿用 dsh 约束）：新快照追加在最后一条 user/tool-result 轮次
  之后；更新点后不存在 retained 轮次时退回"改写首条"旧行为（fork 天然允许，
  无 dsh 的 UNSUPPORTED_CONTENT 硬失败问题——这是 fork 反而更简单的点）。
- **确定性来源**（拍板点，两案）：
  - **a) 派生式（倾向）**：journal schema 不动；request_builder 按 journal
    状态推导"本回合系统快照相对上次 wire 请求是否变化"（触发事件：模型切换/
    工具集/rules/plan 变化都是既有事件）。改动面最小，重放天然确定。
  - **b) journal 直记 `system_update` 事件**：对齐 canonical-context-edit 先例
    （workflow-pi-port P1，journal 直记 replace_visible/hide_visible）。语义
    显式、跨协议（ACP resume）更稳，但 schema 面与 replay 路径改动更大。
- **rewind/branch 交互**：SwitchEdge 切分支后按派生规则重算，追加块属于
  wire 层派生物、不进 journal——两案在此一致。
- **compaction 交互**：压缩本身重写历史，压缩后首请求即新前缀；in-history
  基点在压缩后重置（"上次快照"状态清空，首轮回到普通 messages[0] 形态）。

### 3.4 工具 addition-only（P2，随 deferred-tool-exposure T2）

动态增删工具时，对 `tool_update = "addition-only"` 的模型：
增删以 system 角色 `tool_addition`/`tool_removal` 块进历史（wire 层派生，同
§3.3 方案），采样器对含这类块的请求**条件注入**
`mid-conversation-tool-changes-2026-07-01` beta header（extra_headers 是静态
配置，条件注入需在请求装配处做——落点同 §3.3）。

与 P2 T2 的天然同构：`deferred → direct` 升格就是"只增"；`hidden → direct`
同理。删除类变更（`direct → hidden`/server 掉线）在 addition-only 下退回
"改写 tools 数组"旧行为（前缀作废可接受，删除是低频路径）。beta header 是
DeepSeek 私有契约，按 §5 漂移风险处理。

### 3.5 reasoning 回传（P3，独立 probe）

- fork 无回传路径；dsh 只在 Messages 端点做（thinking 块）。chat_completions
  端 DeepSeek 是否接受 assistant 消息携带 `reasoning_content` 输入**未证实**
  （历史版本明确 output-only，V4.1 行为待 probe，§3.2 同批跑）。
- probe 通过才做；落点 `xai-chat-state` 序列化层（assistant 历史附
  reasoning 块）+ `xai-grok-sampler` 出站开关（能力位
  `reasoning_passback = true`）。
- 与 compaction/rewind 对齐：回传块只影响 wire 层，不进摘要、不进 journal。

### 3.6 验收（复用 limit-probe，零新增设施）

同会话 A/B（能力位 on/off），场景选**触发密集型**（会话中段做 2–3 次模型
切换/工具集变化/plan 切换）：

| 指标 | 取数 | 预期（C 级） |
|---|---|---|
| 触发点后首轮 cache 命中率 | limit-probe `cached_tokens/prompt_tokens` | off ≈ 0（前缀全废）→ on ≈ 会话稳态水平（对照 prompt-cache-notes 的 98.7%） |
| 触发点后首轮 TTFT | status-line perf 段口径 | on 显著下降（缓存命中免重算前缀） |
| 成本 | `/stats` 账本 | 长会话 cache 读占比上升 |

## 4. 分期

| 期 | 内容 | 分支拟名 | 规模（C 级粗估） |
|---|---|---|---|
| P1 | 能力位 + in-history 放置 + 三 backend probe + limit-probe A/B | `feat/local-dsh-in-history` | 净开发 3–5 人天 |
| P2 | 工具 addition-only 块 + 条件 beta header | 随 `feat/local-deferred-tool-exposure` T2 | +1–2 人天 |
| P3 | reasoning 回传 | `feat/local-dsh-reasoning-passback` | ≤1 人天 |

## 5. 风险与未验证

- **端点语义**（最大风险）：in-history / beta header / reasoning 回传三条的
  DeepSeek 侧行为均转述自 dsh 文档，fork 实际 backend 组合必须 probe 后放行
  （§3.2）；probe 失败则 P1 降级为"仅 messages backend 可用"或放弃。
- **上游同步**：`xai-chat-state`/`xai-grok-sampler` 属同步面，LOCAL 补丁按
  PATCHES.md 登记便于重放；能力位字段命名尽量贴上游风格减少冲突面。
- **契约漂移**：`mid-conversation-tool-changes-2026-07-01` 是 DeepSeek 私有
  beta 契约，可能随版本变化；header 名进 config（跟随能力位）而非硬编码。
- **收益边界**：增益集中在"触发点后的第一轮"；纯稳态会话（deepseek 已 98.7%）
  无增益。立项叙事按"触发密集场景的费用/延迟"表述，勿夸大为全局提升。

## 6. 待拍板决策

| # | 决策 | 建议 |
|---|---|---|
| D-a | §3.3 确定性方案：派生式 vs journal 直记 | 派生式（改动面小，先跑通） |
| D-b | 能力位是否进 ACP configOptions / /auth 面板回填 | 先纯 config；有真实需求再进面板 |
| D-c | P3 reasoning 回传做不做 | 待 §3.2 同批 probe 结果 |
| D-d | 施工顺位 | 进 port-roadmap §6 与现有候选一起拍板 |

## 来源

见 [`README.md`](README.md) 来源节；本文增量引用：
`docs-local/workflow-pi-port/canonical-context-edit.md`（journal 直记先例）、
`docs-local/subagent-model-group-todo.md`（能力位装配先例）、
`docs-local/model-picker-catalog-details-todo.md`（零回合重建簿记先例）。
