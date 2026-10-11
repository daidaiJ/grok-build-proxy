# 引入评估：五个 agent CLI 的设计如何落到 grok-build

> 范围定调（2026-09-19）：**不引入内置模型目录/厂商 preset**（维护不起）。只评估三件事：
> ① **协议接口兼容性**（OpenAI 兼容层的方言鲁棒性）；② **token 经济学**（前缀缓存/压缩/上下文成本）；③ **TUI 易用性与便利性**。
> 外部路径均为各仓库相对路径，机制细节见 [notes/](notes/) 分仓库笔记；现状证据来自 2026-09-19 对本仓库的摸底。

## 0. 现状基线（评估前提）

本仓库已完善：三协议流式（ChatCompletions/Responses/Messages，`xai-grok-sampler/src/stream/`）、指数退避+Retry-After 重试（`xai-grok-sampler/src/retry.rs`）、断路器（`crates/common/xai-circuit-breaker`）、循环检测（`doom_loop.rs`/`stream_classify.rs`）、压缩（`crates/common/xai-grok-compaction`，85% 阈值，`xai-compaction-transcript` 段落落盘）、会话持久化/搜索/记忆（`xai-chat-state` JSONL、`xai-session-search` FTS5、`xai-grok-memory` embedding+dream）、ratatui 流式 markdown（`xai-grok-markdown/streaming.rs` checkpoint 增量）。

薄弱点：① reasoning/方言处理偏 xAI 单一后端；② 前缀缓存**只有统计没有请求侧设计**（`sampling-types/types.rs:538` cached_tokens 有，注入/稳定性无）；③ 工具输出压缩仅一期简化版；④ TUI 便利件（队列状态、折叠展开、缓存提示）缺。**以下所有建议都对准这四点，不碰架构。**

---

## 1. 协议接口兼容性（适配性）

### 1.1 reasoning 方言自学习 ⭐ P0 ✅ 已落地（本分支）
- **来源**：kimi-code `packages/kosong/src/providers/reasoning-key.ts`（notes/kimi-code.md §C）——三方言 `reasoning_content`/`reasoning_details`/`reasoning`，入站按优先级扫描，出站**逐端点学习回放**（端点说什么方言就回什么方言），可显式 pin。
- **为什么**：不硬编码厂商映射，天然兼容任意 OpenAI 兼容网关/vLLM 版本漂移——正是"不做 preset 也要兼容性好"的正解。
- **落点**：`xai-grok-sampling-types`（reasoning 字段已有）加方言枚举 + sampler 层按 (endpoint,model) 记忆观察结果。
- **实现**（2026-09-19）：`sampling-types/src/reasoning_dialect.rs`——`ReasoningDialect` 三方言枚举（`wire_key`/`from_wire_key` 可扩展）+ `ReasoningDialectMemory`（按 model 的进程内记忆，宿主为 per-endpoint 的 `SamplingClient`，即 (endpoint, model) 作用域）；`ChatChunkDelta` 增 `reasoning` 入站解析（当前 vLLM 改名后的真实丢失点；`reasoning_details` 数组形态按 kimi 语义跳过）；L2 stream 观察方言，client 在 `conversation_stream`/`conversation` 出站回放（`reasoning_content` → 学到的方言字段）。kimi 的 detect-never-clears / last-write-wins 语义已测试移植。显式 pin 留给 1.4 能力位。

### 1.2 reasoning 回写防 400 + `<think>` 泄漏清洗 ⭐ P0 ✅ 已落地（本分支，清洗为实验开关）
- **来源**：qwen-code `packages/core/src/openaiContentGenerator/converter.ts:817-828`（assistant 消息回写 reasoning 防严格端点 400）、`taggedThinkingParser.ts` 兜底 `<think>` 标签泄漏（有生产事故回放测试）；opencode `provider/transform.ts:303-315` DeepSeek 特判——assistant 必须带 reasoning part，**为空也要补**，interleaved 模型空 reasoning 回传（:329-346）。
- **落点**：`xai-grok-sampler/src/stream/` collect 层 + chat-state 历史回放路径。
- **实现**（2026-09-19）：回写防 400 的核心（Reasoning 折叠进 `reasoning_content`、空文本为 `""` 非 null）上游已具备，本分支补齐方言正确性（见 1.1）。`<think>`/`<thinking>` 泄漏清洗：`sampler/src/thinking_scrub.rs` 流式状态机，**qwen 语义 1:1 移植**（大小写不敏感、binary toggle、跨配对、partial tag 跨 chunk 缓冲、final flush），21 个用例自 qwen 测试套件移植；L2 层清洗使 UI token 与持久化历史同时干净。**【实验特性】`experimental.thinking_tag_scrub`，默认关**——toggle-anywhere 会把正文中字面 `<think>` 吞进推理通道（qwen 生产可接受的取舍，但属可见输出变更）。opencode 的"assistant 必须带空 reasoning part"特判依赖模型能力位，**顺延至 1.4 一并做**（无差别补空字段对严格端点本身就有 400 风险）。

### 1.3 协议边界消息合并 ⭐ P0 ✅ 已落地（本分支）
- **来源**：kimi-code `packages/kosong/src/providers/merge-user-messages.ts`（notes/kimi-code.md B2）——压缩后连续 user 消息在严格 provider 会 400；在协议边界合并连续 user 消息，并保证并行 tool-result 同 turn。
- **为什么**：grok 的 `replace_history` 压缩/回滚路径随时可能产出非法消息序列，这是埋在压缩功能里的雷。
- **落点**：`xai-chat-state` replace_history 出口 + sampler 请求构造入口，双保险。
- **实现**（2026-09-19）：按 kimi 原设计**收敛到协议转换边界单点**（其教训："Keeping the algorithm in one place stops a provider from silently omitting it"；且 chat-state 存储层合并会破坏 `synthetic_reason`/`prompt_index` 等回放依赖的结构标记——原计划的"双保险"改为"边界归一"）：Messages 路径 `merge_consecutive_user_turns`（不对称规则 1:1：tool-result-only 吸收后续、text 不吸收 leading tool-result），ChatCompletions 路径 `merge_consecutive_user_messages`（tool 为独立 role，无需不对称）；两协议合并确定性 ⇒ 前缀字节稳定（有测试锁定）。opencode 的"tool 后必须插 assistant"特判不移植（OpenAI 规范允许 tool→user，属 DeepSeek 专属 quirk）。

### 1.4 thinking 开关按协议编码 + 能力位挂模型条目（用户自填） 🟡 P1
- **来源**：kimi-code `kosong/src/catalog.ts:419-429` thinking off 编码按协议区分；qwen-code ModelSpec 的 `capabilities.reasoning{thinking,toggleOnly,disableField}`（`presets/alibaba-coding-plan.ts:22-35`）。
- **做法**：不做内置目录——用户在配置里写模型条目时允许携带 `protocol/reasoning/context_window` 覆盖字段（参考 kimi `refreshProviderModels.ts` 的自定义 provider 凭据+清单刷新思路），缺省值按协议给安全默认。

### 1.5 流式残缺 tool-call chunk 容错对照 🟡 P1
- **来源**：qwen-code `streamingToolCallParser.ts`（分块 tool-call JSON 残缺 chunk）。
- **做法**：与 `xai-grok-sampler/src/stream/collect.rs` 对照做一轮 gap 分析，缺则补（不重写）。

---

## 2. token 经济学

### 2.1 前缀缓存四件套 ⭐ P0（本报告最高优先级）

**✅ 全部落地（本分支，2026-09-19）**：

1. **字节级前缀一致不变量** ✅ — MiMo-Code `session/llm-request-prefix.ts`（notes/mimo-code.md ②2）——构造收敛为纯函数 + 单测断言。**实现**：三条协议转换（ChatCompletions/Responses/Messages）本就纯函数化，补 `conversation/prefix_invariant_tests.rs`：两次构造 byte-equal（三协议）、跨 turn 消息列表 element-wise 前缀稳定（Messages 剥离滚动的 cache_control 标记）、**mimo fork 前缀不变量移植**（fork 请求保留父前缀至最后一条 user turn，尾部被合并吸收——与合并语义的一致行为有测试锁定）。上游 responses 套件本有 `assert_prefix_stable`，现三协议对齐。
2. **显式断点注入（按协议门控）** ✅ **上游已带**——`Synced from monorepo` 已含 `build_messages_request` 的 `apply_cache_breakpoints`（system 末块 + 消息 tip + 上一 user，注释明确第 4 槽位留给网关自动缓存、5 个即拒）。opencode 设计中的"最后一个 tool 定义"断点**有意不加**：会占掉上游预留的第 4 槽位（`ToolParam` 无 cache_control 字段，保持现状）。auto/手工策略开关留待有网关需求时再做。
3. **断裂检测** ✅ — kimi-code `cache-hint-controller.ts` 的 <95% 环比判定已移植：`xai-chat-state/src/usage.rs` `is_cache_break`（prev≥512 token 才参与判定，阈值/比率常量化）+ `UsageLedger.last_cache_read_by_model`（会话态，不序列化）+ `UsageTotals.cache_break_calls`，经 `PromptUsageModel.cache_break_calls` 上 ACP wire（默认关的 LOCAL 字段，仿 cache_miss_calls）。**闲置过期提醒对话框（4.5）未做**——需 resume/提交入口的 UI 时机，随 P1 做。
4. **cacheReadRatio 上 status line** — 检测数据已就位（cache_break_calls/cache_miss_calls 在账本与 wire 上）；status line 具体展示随 P1 UI 轮做。

### 2.2 压缩升级
- **交接文档式摘要 prompt** ⭐ P0 ✅ 已落地（本分支）：kimi-code `agent/fullCompaction/compaction-instruction.md`（notes/kimi-code.md B5③）——保真最近意图/已运行命令与结果/已决未决/前向计划，跟随会话语言。**实现**（2026-09-19）：`full_replace_summary_prompt.txt` 定向增强——语言跟随 + 命令/结果保真（§4/§8）+ 已决/未决切分（§5）+ handoff-document 定调，全部为纯提示词改动；qwen/kimi 风格差异以最小增量合入，9 段结构与小节名不变（下游测试依赖小节名）。
- **microcompaction** 🟡 P1：qwen-code `services/microcompaction/microcompact.ts`（notes/qwen-code.md B2）——只清老工具结果内容为占位符、不动对话骨架；opencode 同思路（2000 字符+skill 豁免）。落点 `xai-grok-compaction` history 模块，作为 85% 全量压缩之前的轻量档。
- **溢出递进压缩 ≤3 次** 🟡 P2：kimi-code `agent/fullCompaction/strategy.ts:19-27`。

### 2.3 工具输出 spill-to-disk + 指针 ⭐ P0
- **来源**：kimi-code `agent/toolResultTruncation/toolResultTruncationService.ts:46-77`——超限写 spill 文件，消息只留指针+尾部；opencode `tool/truncate.ts:19` 返回 `{truncated, outputPath}` 并在提示里教模型用子代理/Grep 取全文（上下文压力转化为 task 用法）。
- **为什么**：与本仓库 `docs-local/tool-output-compression-plan.md` 已规划的 pull 式 `expand_output(hash)` **完全同构**——外部两个项目独立验证了这条路线，直接按原计划推进即可，一期简化版已有。
- **落点**：`xai-grok-tools` ToolBridge 输出出口 + `xai-compaction-transcript` 落盘。

### 2.4 子代理共享父前缀 🟡 P1
- **来源**：qwen-code `agents/forkedAgent.ts`——带 cacheSafeParams 的 fork 走单轮共享父 prompt（保缓存），否则独立多轮。**落点**：`xai-grok-subagent-resolution` spawn 时标记"共享前缀/独立会话"两种模式。

---

## 3. 稳定性小 trick（协议相关）

| # | trick | 来源（仓库相对路径） | 落点 | 级别 |
|---|---|---|---|---|
| 3.1 | **提交边界重试**：缓冲流事件，出现首个可见 delta 才绑定物理请求；之前失败无痕重试，不重复输出 | minimax `agent-core/src/pi-turn-runner/llm-retry.ts:458-474`（notes/minimax-code.md B1） | sampler stream collect 层加"已 emit"标志 | ⭐ P0 |
| 3.2 | **429 限流 vs 配额型分类**：限流→无限退避；配额型（如 DashScope `Throttling.AllocationQuota`）→有界重试直接失败 | qwen `utils/retry.ts`+`retryPolicy.ts`；kimi `kosong/src/providers/kimi-errors.ts` | `xai-grok-sampler/src/retry.rs:232` 补配额子类 | 🟡 P1 |
| 3.3 | **错误 kind 归一**：16 种 kind（rate_limited/tpm/usage_limit/credits/content_filter/overloaded/empty_response…）+ 对外脱敏 | minimax `shared/src/llm-error-classifier.ts` | 挂到 failed_model_calls 账本 | 🟡 P1 |
| 3.4 | 无限重试开关（长任务挂机场景） | kimi `KIMI_CODE_INFINITE_RETRY` | retry.rs 配置项 | 🟢 P2 |
| 3.5 | stale-compaction-repair：修复历史里失效的压缩标记 | minimax `local-runtime-v2/.../stale-compaction-repair` | chat-state 回放校验 | 🟢 P2 |

---

## 4. TUI 易用性 / 便利性审视

| # | 设计 | 来源 | grok 现状对照 | 级别 |
|---|---|---|---|---|
| 4.1 | **运行中转向 steer()**：消息到达时若 agent 活跃则注入当前 turn 而非排队 | kimi `agent/loop/loop.ts:38,209` `steer/steerIfActive` | `xai-interjection-core` 疑似同域，先对照再定差距 | 🟡 P1 |
| 4.2 | **队列三态面板 + 插队**：queued/paused/failed 可重试/编辑/丢弃；中断时选"插队发送 or 清空队列" | minimax `tui/features/queue/panel.ts` + `paused-send-panel.ts` | `xai-prompt-queue` 有合并规则，缺 UI 态与插队 | 🟡 P1 |
| 4.3 | **工具调用默认折叠 + Ctrl+O 展开**，按工具分渲染器 | kimi `apps/.../messages/tool-call.ts:583,787` | pager 工具卡片对照补 | 🟡 P1 |
| 4.4 | **流式 fence 闭合防闪烁**：流中修剪残缺代码围栏；按渲染行高度整块 commit | kimi `pi-tui/src/components/markdown.ts:252`；qwen `use-llm-stream.ts:1924-1936` splitFencedMarkdown | `xai-grok-markdown/streaming.rs` 已有 checkpoint，对照补 fence 处理 | 🟡 P1 |
| 4.5 | **缓存过期提醒对话框**（token 经济学可感知化，见 2.1-3） | kimi `cache-hint-controller.ts` | 无，随 2.1 一起做 | 🟡 P1 |
| 4.6 | rewind 前 diff 预览面板 | minimax `tui/features/session-mutation/rewind-preview-panel.ts` | `rewind.rs` 有快照，缺预览 UI | 🟢 P2 |
| 4.7 | alt-screen 滚动区全文搜索 | minimax/kimi `alt-screen-search.ts` | pager scrollback 需核对是否已有 | 🟢 P2 |
| 4.8 | paste-burst：无 bracketed paste 的终端里"8 字符+120ms 快打+Enter"降级为换行 | kimi `pi-tui/src/paste-burst.ts` | Windows 老终端有真实场景 | 🟢 P2 |
| 4.9 | 终端主题自动检测 | qwen `detect-terminal-theme.ts` | 已有 embed 颜色适配，可选 | 🟢 P2 |

---

## 5. 不建议引入（负面清单）

- **内置模型目录/厂商 preset**：用户定调，维护不起。兼容性靠 1.1/1.4 的"方言自学习+用户自填覆盖"实现。
- **Agent Teams / 看板多代理**（qwen `agents/team/`）：体量大、单用户 CLI 场景收益低。
- **best-of-N + judge max 模式**（MiMo `session/max-mode.ts`）：成本模型不适配按量付费；机制本身留档备查。
- **Dream/Distill 完整自我改进闭环**（MiMo `session/auto-dream.ts`）：grok memory 已有 dream 机制，不必照搬 SQLite 轨迹库。
- **换渲染栈**（SolidJS/@opentui/Ink）：ratatui 生态已深耕（inline/textarea/markdown/diff 四 crate），不动。

## 6. 落地顺序建议

三批 P0 全部**不动架构、单 crate 落点**，可独立提交：

1. **第一批（防 400 套装，适配性地基）** ✅ 已落地：1.2 reasoning 回写/泄漏清洗（清洗为实验开关默认关）+ 1.3 协议边界消息合并（边界归一单点，见 1.3 实现注记）+ 1.1 方言自学习。
2. **第二批（token 经济学主菜）** ✅ 已落地：2.1-1 前缀不变量测试（含 mimo fork 不变量移植）→ 2.1-2 确认上游已带断点注入（tool 断点有意不加，槽位论证见 2.1-2 注记）→ 2.1-3 断裂检测（检测器+wire 面；闲置过期提醒对话框随 4.5 P1）→ 2.2 交接文档摘要 prompt。
3. **第三批（体感）** ⏳ 未动：3.1 提交边界重试 + 2.3 spill-to-disk（按原 compression-plan 推进）。

P1/P2 按 §3/§4 表内级别随迭代带入；每项引入后在本文勾选并注明落点 PR/commit。

### 6.1 实验特性 vs 正式特性分类（本批引入）

**直接上正式（默认启用，无需开关）**：
- 1.1 方言自学习：入站 `reasoning` 解析是纯增益（此前这些端点的 reasoning 整体丢失）；出站回放仅在端点"说过别的方言"后才生效，默认仍 `reasoning_content`，与 kimi 生产语义一致。
- 1.3 连续 user 合并：只改写本就 400 风险的序列，合并确定性，长端行为等价。
- 2.1-1/2.1-3/2.2：测试、纯检测器、提示词——无用户可见行为风险。
- `cache_break_calls` 等 LOCAL 字段：additive wire 字段，默认 `0`，不参与计费。

**实验特性（`experimental` 配置节，默认全关，按模型开启）**：
- `thinking_tag_scrub`（1.2）：唯一改变可见输出的特性——`<think>` 清洗采用 qwen 的 toggle-anywhere 语义，会把正文字面 `<think>` 吞进推理通道；且 onset 判定在"响应以字面 tag 开头"时误分类。收益（脏输出清洗）与风险（吞正文）都真实存在，交给用户按端点决定。
- 配置形态：`[model.<id>.experimental] thinking_tag_scrub = true`（ModelEntryConfig → ConfigModelOverride → ModelEntry → SamplerConfig → SamplingConfig 全链路 serde 默认关，子代理/模型切换自动继承）。节内可继续加字段，新实验特性不再扩表结构。

---

## 7. 增补评估（2026-10-11：crush / goose / zcode + 边缘仓甄别）

> 背景：父目录 `D:/CODE/ai/` 下三个首批未覆盖的 coding agent（crush / goose / zcode）本轮调研入库，
> 四个边缘仓（DCP 插件 / rpiv-mono / deepseek-harness-codearts / openagents）甄别归档。
> 机制笔记：[notes/crush.md](notes/crush.md) / [notes/goose.md](notes/goose.md) / [notes/zcode.md](notes/zcode.md)。
> 口碑口径（用户拍板）：**opencode 与 MiMo-Code 社区口碑下降**，其设计仅作机制参照、不再作为对齐对象，后续刷新优先级降低。
> 「已有」判定均为本轮 grep/读码实际核实（锚点随条目给出），防止误报。

### 7.1 候选清单（按优先级）

| # | 特性 | 来源 | 现状对照 | 落点 | 级别 |
|---|---|---|---|---|---|
| 7.1.1 | **模型自选压缩工具 + 裁剪策略族**：模型调 `compress` 自选范围；dedup（同工具+同参数只留最近一次输出）、purge-errors（出错工具超 N 轮只删大输入保错误信息）；双阈值 nudge（超上限注入 context-limit、超下限开 reminder，锚点集合防重复） | opencode-dynamic-context-pruning（`lib/strategies/deduplication.ts`、`purge-errors.ts`、`lib/messages/inject/inject.ts`） | fork 已有落点：canonical context edit journal 直记 replace_visible/hide_visible（workflow-pi-port P1 已落地）+ microcompaction P1 待做（§2.2）——DCP 策略族即 microcompaction 的策略层现成参照。注意两点：①其「请求时替换、不动历史」路线与 fork journal 直记路线的取舍（DCP 自曝缓存代价 85% vs 90% hit rate）；②嵌套摘要（新压缩与旧压缩重叠时旧摘要嵌入新摘要） | `xai-grok-compaction` history 模块 + context edit ops | 🟡 P1 |
| 7.1.2 | **egress 外泄检测 inspector**：工具参数出口扫描敏感数据外发（如密钥随 curl/bash 外传）并进审批决策 | goose `src/security/egress_inspector.rs`（统一 ToolInspector 管线 `tool_inspection.rs:11`） | fork 出站脱敏只盖遥测/产品事件面（`xai-grok-secrets/src/sanitizer.rs`）；workspace permission 的 exec_risk 有环境配置扫描但无密钥外发模式库——工具参数出口是真空白 | `xai-grok-tools` ToolBridge 输出侧 + workspace permission 规则库复用 | 🟡 P1 |
| 7.1.3 | **turn-context 回合预算自感知**：每回合注入 `<turn-context>`（时间/cwd/compaction 状态/剩余 turn 预算），模型见预算见底自主收敛 | goose `src/agents/moim.rs:6,37`（≥32k 上下文才启用） | fork `/goal` 有 token_budget + completion classifier，但普通回合无预算自感知；纯 prompt 组装层改动 | prompt 组装层（agent definition / system prompt sections） | 🟡 P1（小件） |
| 7.1.4 | **amend-workflow：主 agent 中途修订运行中的工作流**（description/resolve/retune/source 四面） | zcode `core/src/tool/handlers/amend-workflow*.ts` | fork `xai-workflow` 有 journal 重放/双预算/await_user，运行中脚本无修订通道；amend 可走 await_user 同族 host call（脚本在 await 点收到修订值） | `xai-workflow/src/host.rs` await_user 旁 + 新 host call | 🟡 P1 |
| 7.1.5 | **跨模型评审门（advisor）**：零参工具把**整个当前会话**交给更强 reviewer 模型，返回 plan/correction/stop 结构化裁决 | rpiv-mono `packages/rpiv-advisor/` | fork 子代理按 definition 建新会话收 task prompt，无「全量会话作输入 + 结构化裁决回灌」评审流；BYOK 双钥匙场景价值高（一个账号跑活、强模型把关） | `xai-grok-subagent-resolution`（评审型子代理 + 会话注入模式） | 🟢 P2 |
| 7.1.6 | **跨生态会话历史导入**（Claude 原生会话入库） | zcode `packages/services/src/session/claude-native/` | fork 有外部 agent 会话**元数据**发现（`xai-grok-foreign-sessions`）+ /import-claude 配置面，无会话历史导入 | `xai-grok-foreign-sessions` 扩展 | 🟢 P2 |
| 7.1.7 | **rewind 对照补**：检查点带 diff hunk、策略四态判定（active_chain/file_only/fork_required/unavailable 显式化不可回退态） | zcode `contracts/src/rewind/index.ts` | fork rewind 分支树 undo 已落地（T1/T2a/T3）；picker 面对照补 diff 预览（另见 §4.6 minimax 同款）与不可回退态提示 | `pager/app/dispatch/rewind.rs` | 🟢 P2 |
| 7.1.8 | **权限 alwaysAsk：工具级强制审批不可被模式放行绕过** | zcode `core/src/permission/service.ts` | fork headless denial continue 已有；审批模式与工具级强制审批的优先级语义做一轮对照 | `xai-grok-tools` ToolRequirement 面 | 🟢 P2 |
| 7.1.9 | **Channels：MCP server 反向推送触发 agent 回合** | crush `internal/backend/channels.go:25-45`（`--channels` opt-in，push 永不丢弃） | fork MCP 仅为 client 拉模式；IM/监控→agent 集成场景有价值，依赖用户需求再立项 | `xai-grok-mcp` + 会话入口 | 🟢 P2 |
| 7.1.10 | **workspace hook 信任摘要**：hook 按「工作区身份 + 声明摘要」信任审查，摘要变化即失效 | zcode `core/src/hooks/workspace-hook-*.ts` | fork hooks 七事件 fail-open，无声明变更失效机制；安全硬化小件 | `xai-grok-hooks` | 🟢 P2 |
| 7.1.11 | **sigstore 供应链自更新校验**（trust-root + bundle 验证后才替换二进制） | goose `goose-cli/src/commands/update.rs:19` | fork 自更新默认关（五期）；启用自更新前值得补的底座 | `xai-grok-update` | 🟢 P2/留档 |
| 7.1.12 | **会话导出 HTML/Markdown** | goose `src/session/export_html/`、`export_markdown.rs` | **已有**：`/export`（Markdown 导出文件/剪贴板，`pager/src/slash/commands/export.rs`）+ `/transcript` + `/share`；HTML 仅为格式增量 | — | ✅ 已有 |
| 7.1.13 | **LSP 工具对照补**：call_hierarchy / rename / replace_symbol | crush `internal/agent/tools/lsp_*.go`（8 工具） | fork LSP 模块已全（`xai-grok-tools/src/implementations/lsp/`，hover/symbols/diagnostics/format/restart 等，`features.lsp_tools` 默认关）——只差个别工具；随 LSP 特性开箱评估一并定 | `xai-grok-tools/src/implementations/lsp/` | 🟢 P2 |
| 7.1.14 | **checks / REVIEW.md 约定审查**：`.agents/REVIEW.md` 派生审查项、每项 check 子代理执行 | goose `src/checks/mod.rs` | fork skills + subagents 组合可低成本仿制（一个读 REVIEW.md 派任务的 slash/skill） | skills/slash 层 | 🟢 P2 |
| 7.1.15 | **formal-proof 状态空间枚举**（对 compact/fork/队列行为组合做枚举验证） | zcode `packages/formal-proof/` | 测试基建思想留档；rewind/压缩回归夹具设计可借鉴 | 测试侧 | 🟢 留档 |

### 7.2 不移植（负面清单增补）

- **crushrc = Bash 配置语言**（crush）：TOML 体系已深耕，表达力收益不值得第二配置语言。
- **声明式 provider 一 JSON 一网关编译进二进制**（goose）：§5「内置模型目录/preset」同判，维护不起。
- **SmartApprove LLM 判只读免批**（goose）：fork 已有 tree-sitter 命令静态分析全套（确定性三态 fail-closed），LLM 判定只添不确定性。
- **ACP 反向 provider（把 Claude Code/Codex 当模型用）/ Telegram 网关 / P2P roaming / 桌面语音**（goose）：产品边界外。
- **dynamic-workflow 模型写 TS 工作流**（zcode）：codemode 同判（§5 D1：双运行时破坏 journal 确定性）；其「编译器静态分析（污染不动点+时序走查）」思想留档，Rhai 侧若做工作流校验可参照。
- **model-option-map DSL**（zcode）：TOML per-model 覆盖（`[model.<id>]`）已覆盖同域。
- **闲时任务票据（off-peak）**（zcode）：依赖服务端取号/排队/核销。
- **状态机 op 管线架构重写**（goose）：fork actor 模型已深耕，推倒不值。
- **prompt-trajectory 录制器**（zcode）：unified.jsonl + limit-probe 已覆盖同域取证面。

### 7.3 本轮核实的「fork 已有」防误报清单（外部同款特性已在 fork 落地）

| 外部特性 | fork 现状锚点 |
|---|---|
| 流空闲超时（zcode stream-idle-timeout） | `conversation_collect_with_idle_timeout`（sampler `client.rs:2382-2415`，主链 300s、side call 可短） |
| 空补全重试（zcode compat 层） | `SamplingError::EmptyResponse` 分类 + retry 判定（sampler `retry.rs:267`、`actor/request_task.rs:840`） |
| 工具循环检测（goose tool_monitor / crush loop_detection） | `doom_loop.rs` / `stream_classify.rs` |
| 大工具输出落盘（goose large_response_handler） | 六期工具输出压缩 + §2.3 spill-to-disk 已落地 |
| bash 静态分析族（crush safe.go 白名单 / zcode bash-*.ts） | `xai-grok-workspace/src/permission/` 全套（2026-10-03 勘误后确认） |
| skills 兼容扫描 `~/.claude/skills`（crush） | `/import-claude` + skills watcher（`claude_import.rs:389`、`extensions/skills.rs:236`） |
| 插件市场（zcode 插件商店） | `xai-grok-plugin-marketplace` |
| 会话自动命名（crush/goose LLM 起名） | 未逐项核实，随 TUI 轮对照 |

### 7.4 边缘仓甄别结论

- **opencode-dynamic-context-pruning**：DCP 插件——「模型自选范围压缩 + 双阈值 nudge + 请求时占位符」，7.1.1 主参照；⚠️ 开发已放缓（作者转向 Sleev），机制仍有效。
- **rpiv-mono**：pi 扩展集——值得深挖限两包：advisor（7.1.5）与 rpiv-workflow（每阶段独立会话 + 谓词路由 + failure-memos，workflow-pi-port「编排上下文膨胀」缺口的补充参照）；其余包浅尝即可。
- **deepseek-harness-codearts**：deepseek-harness 的登录/provider 运营插件（11 家国服网关路由 + 积分/账号池/打码链），与 /usage 供应商主题同域，但打法是逆向客户端协议吃套餐额度，无可搬机制——浅尝即可。
- **openagents**：多 agent 网络协作平台（Agent Network / Mods），非单机 coding agent——与本项目无关。
- **only-cc-lite**：Headroom 抽取的上下文压缩库（用户口径：非 coding agent，不立项）。

### 7.5 落地顺序建议（本批）

1. 7.1.1（microcompaction 策略层）与 §2.2 既有 P1 合并推进——DCP 策略族直接充当设计输入。
2. 7.1.2 egress 检测为随手小件（tools 出口 inspector）；~~7.1.3 预算注入~~（已撤回，见 §7.6.2）。
3. 7.1.4 amend-workflow 随 workflow-pi-port P2 后续切片评估。
4. P2 项随各自主题轮（TUI 轮 / rewind 轮 / 更新轮）顺带对照，不单独立项。

### 7.6 逐项终评：移植必要性与收益（2026-10-11 二轮，应用户要求挨个过）

> 判定口径：「必要性」= fork 真实缺口 × 本机使用场景出现频率；「收益」= 受益面与频率；「成本」含
> 编译级联与测试面。结论三档：**做**（排期）/ **缓**（挂条件，等触发信号）/ **不做**（留档或不做）。
> 另登记本轮衍生的 LOCAL 提案 `/lsp`（7.6.1）。

#### 7.6.1 衍生 LOCAL 提案：`/lsp` 斜杠命令（workspace 级 LSP 工具启停）

- **行为**：`/lsp on|off|status` — 启停当前 workspace 的 LSP 工具族注册；仅允许在**新会话或 /clear 后**的会话执行，带历史回合时拒绝并提示。用户 2026-10-11 发起。
- **为什么限会话边界**：工具清单在 agent definition 构建时定格（`agent_ops.rs:4535` 按 `Feature::LspTools` 决定注册）；会话中途改清单 = 后续请求 tool 声明面变化 → 前缀缓存从工具段起失效 + 模型对工具集的假设漂移。与 ④T2 延迟工具发现（search_tools 渐进披露 + 公告流）是两个方向：那里保留声明连续性，这里是整族启停——按拍板走「会话边界切换」，最简单且零声明态包袱。
- **现状缺口**：LSP 工具族已全但由 `[features] lsp_tools` 全局控制（默认关，env `GROK_LSP_TOOLS`，remote settings 可覆盖）——粒度是用户级，没有按 workspace 的开关，也没有运行时交互入口。
- **设计草案**：
  - 持久化推荐 **A：grok-home 按 workspace 记忆的 feature 覆盖**——复用 `xai-grok-config/src/paths.rs` 的 `encode_cwd_dirname`（cwd 编码键）做 `~/.grok/feature-overrides/` JSON（同 active-sessions/limit-probe 家族的锁保护文件模式），agent definition 构建时叠加读取；workspace 粒度天然成立。
  - 备选 B：直写用户级 `[features] lsp_tools`——config 回写面现无先例（`/lang` 等均为会话态），且作用域全局，与「当前工作空间」诉求不符。
  - 优先级注记：workspace 覆盖与 remote/managed settings 的优先级要定（建议 remote > workspace 覆盖 > 用户 config 默认，保持 managed 层优先惯例）。
  - 门控实现：命令执行时检查当前会话回合数（或 /clear 标记），非全新会话拒绝。
  - 落点：pager `slash/commands/lsp.rs`（新）+ shell feature 叠加点（`is_feature_enabled` 旁）+ grok-home 存储；编译级联 = pager + shell。
- **状态**：提案成立，待排期；建议并入 TUI 便利件轮，与 7.1.13（LSP 工具补齐）联动——先有开关和用户，再谈加工具。

#### 7.6.2 逐项终评表

| # | 候选 | 必要性 | 收益 | 成本 | 结论 |
|---|---|---|---|---|---|
| 7.1.1 | DCP 策略族（dedup/purge-errors/nudge）→ 并入 microcompaction | **高**——长会话上下文压力是日常主痛点，replace_visible/hide_visible 落点已备 | **大**——所有长会话受益；纯策略层风险低 | 中（compaction crate + 测试；改写点之后的缓存失效需与 byte-equal 不变量测试协调） | **做**（并入 microcompaction 设计，第一顺位） |
| 7.1.2 | egress 外泄检测 | 中——BYOK/多网关场景密钥多，风险真实但触发低频 | 中——低频高价值的安全兜底 | 小中（tools 出口 inspector + 正则模式库，可复用 `xai-grok-secrets`） | **做**（小件随手轮） |
| 7.1.3 | turn-context 预算注入 | 低中——`/goal` token_budget 已覆盖目标场景，普通回合预算自感知是锦上添花 | 小中——长会话收敛改善，但每回合固定 token 开销 | 小（prompt 组装） | **不做**（2026-10-11 用户先准后撤，最终拍板去掉） |
| 7.1.4 | amend-workflow | 中——运行中修订是真实痛点，但 workflow 使用尚在早期、频率未知 | 中——编排体验跃升 | 中高（host call + await 语义 + journal 事件 + 测试） | **缓**——workflow-pi-port P2 T2 之后再排 |
| 7.1.5 | advisor 跨模型评审门 | 中低——BYOK 双钥匙用户是子集；子代理 model override 可近似一半 | 中——强模型把关正确率 | 中（全量会话注入 + 结构化裁决回灌，耦合 spawn 链路） | **缓**——看 ④T2 落地后 subagent 面再定 |
| 7.1.6 | 跨生态会话历史导入 | 低——迁移期一次性需求，元数据发现 + /import-claude 已覆盖主场景 | 小 | 中（Claude 会话格式解析 + 入库映射） | **不做**（真需要时一次性脚本） |
| 7.1.7 | rewind diff 预览 + 不可回退态显式化 | 中——rewind 分支树刚落地，预览是 UX 完善面（§4.6 minimax 同款） | 小中——回退前可预视，减少误操作 | 小（`xai-grok-pager-diff` 可复用，picker 面已有） | **做**（rewind 轮收尾件） |
| 7.1.8 | alwaysAsk 不可被模式绕过 | 低中——已有参数级 ToolRequirement + hooks deny，语义增强属硬化 | 小——防误配置兜底 | 小 | **缓**——先对照语义差距，可能已有等价物 |
| 7.1.9 | Channels MCP 反向推送 | 低——IM/监控触发场景本机未出现 | 小——场景特化 | 中（反向通道 + 会话入口 + 安全面） | **不做**（留档，有真实集成需求再立项） |
| 7.1.10 | workspace hook 信任摘要 | 低中——hooks fail-open 信任模型弱，但威胁模型是本机项目；folder_trust 已有项目信任机制 | 小 | 小中 | **缓**——核对 folder_trust 覆盖面后可能部分等价 |
| 7.1.11 | sigstore 自更新校验 | 低——自更新默认关（五期） | 小 | 中（sigstore Rust 依赖引入成本不小） | **不做**（真启用自更新时 SHA256 先行） |
| 7.1.12 | 会话导出 HTML/MD | — | — | — | ✅ **已有**（`/export` Markdown + `/transcript` + `/share`；HTML 出现需求再说） |
| 7.1.13 | LSP 工具补齐（call_hierarchy/rename/replace_symbol） | 取决于 LSP 工具启用后的使用反馈（features.lsp_tools 默认关、尚无反馈） | 小中——重构场景有用 | 小（lsp 模块已全，三个薄封装） | **缓**——随 `/lsp` 提案落地后看使用反馈 |
| 7.1.14 | checks / REVIEW.md 约定审查 | 低中——skills + subagent 可手工组合 | 小——约定化便利 | 小（一个内置 skill 读 REVIEW.md 派子代理） | **缓**——按 skill 而非机制做，随手可做 |
| 7.1.15 | formal-proof 状态空间枚举 | — | — | — | 🗂 **留档**（测试基建思想，rewind/压缩夹具设计可借鉴） |

#### 7.6.3 二轮结论汇总（2026-10-11 用户逐项拍板后更新）

- **做（3 + 3 提案）**：7.1.1 并入 microcompaction（第一顺位）、7.1.2 egress 检测（小件）、
  7.1.7 rewind 预览（收尾件）；提案 = `/lsp`（7.6.1）+ **`/auth` 供应商配置面板与 `/stats`
  API 成本列（同日新立项，设计文档
  [`../provider-onboarding-api-cost-todo.md`](../provider-onboarding-api-cost-todo.md)，
  参照实现 = 用户 modelq 仓库）**。
- **缓（6）**：7.1.4 / 7.1.5 / 7.1.8 / 7.1.10 / 7.1.13 / 7.1.14（各挂触发信号，见 §7.6.2 表）；
  其余用户表态「不太有感知」，维持缓档不主动推进。
- **不做 / 已有 / 留档（6）**：7.1.3（用户撤回）、7.1.6、7.1.9、7.1.11（不做）；7.1.12（已有）；
  7.1.15（留档）。
