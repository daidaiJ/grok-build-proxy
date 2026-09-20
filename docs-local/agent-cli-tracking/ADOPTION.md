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
