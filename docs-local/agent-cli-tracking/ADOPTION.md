# 引入评估：五个 agent CLI 的设计如何落到 grok-build

> 范围定调（2026-09-19）：**不引入内置模型目录/厂商 preset**（维护不起）。只评估三件事：
> ① **协议接口兼容性**（OpenAI 兼容层的方言鲁棒性）；② **token 经济学**（前缀缓存/压缩/上下文成本）；③ **TUI 易用性与便利性**。
> 外部路径均为各仓库相对路径，机制细节见 [notes/](notes/) 分仓库笔记；现状证据来自 2026-09-19 对本仓库的摸底。

## 0. 现状基线（评估前提）

本仓库已完善：三协议流式（ChatCompletions/Responses/Messages，`xai-grok-sampler/src/stream/`）、指数退避+Retry-After 重试（`xai-grok-sampler/src/retry.rs`）、断路器（`crates/common/xai-circuit-breaker`）、循环检测（`doom_loop.rs`/`stream_classify.rs`）、压缩（`crates/common/xai-grok-compaction`，85% 阈值，`xai-compaction-transcript` 段落落盘）、会话持久化/搜索/记忆（`xai-chat-state` JSONL、`xai-session-search` FTS5、`xai-grok-memory` embedding+dream）、ratatui 流式 markdown（`xai-grok-markdown/streaming.rs` checkpoint 增量）。

薄弱点：① reasoning/方言处理偏 xAI 单一后端；② 前缀缓存**只有统计没有请求侧设计**（`sampling-types/types.rs:538` cached_tokens 有，注入/稳定性无）；③ 工具输出压缩仅一期简化版；④ TUI 便利件（队列状态、折叠展开、缓存提示）缺。**以下所有建议都对准这四点，不碰架构。**

---

## 1. 协议接口兼容性（适配性）

### 1.1 reasoning 方言自学习 ⭐ P0
- **来源**：kimi-code `packages/kosong/src/providers/reasoning-key.ts`（notes/kimi-code.md §C）——三方言 `reasoning_content`/`reasoning_details`/`reasoning`，入站按优先级扫描，出站**逐端点学习回放**（端点说什么方言就回什么方言），可显式 pin。
- **为什么**：不硬编码厂商映射，天然兼容任意 OpenAI 兼容网关/vLLM 版本漂移——正是"不做 preset 也要兼容性好"的正解。
- **落点**：`xai-grok-sampling-types`（reasoning 字段已有）加方言枚举 + sampler 层按 (endpoint,model) 记忆观察结果。

### 1.2 reasoning 回写防 400 + `<think>` 泄漏清洗 ⭐ P0
- **来源**：qwen-code `packages/core/src/openaiContentGenerator/converter.ts:817-828`（assistant 消息回写 reasoning 防严格端点 400）、`taggedThinkingParser.ts` 兜底 `<think>` 标签泄漏（有生产事故回放测试）；opencode `provider/transform.ts:303-315` DeepSeek 特判——assistant 必须带 reasoning part，**为空也要补**，interleaved 模型空 reasoning 回传（:329-346）。
- **落点**：`xai-grok-sampler/src/stream/` collect 层 + chat-state 历史回放路径。

### 1.3 协议边界消息合并 ⭐ P0
- **来源**：kimi-code `packages/kosong/src/providers/merge-user-messages.ts`（notes/kimi-code.md B2）——压缩后连续 user 消息在严格 provider 会 400；在协议边界合并连续 user 消息，并保证并行 tool-result 同 turn。
- **为什么**：grok 的 `replace_history` 压缩/回滚路径随时可能产出非法消息序列，这是埋在压缩功能里的雷。
- **落点**：`xai-chat-state` replace_history 出口 + sampler 请求构造入口，双保险。

### 1.4 thinking 开关按协议编码 + 能力位挂模型条目（用户自填） 🟡 P1
- **来源**：kimi-code `kosong/src/catalog.ts:419-429` thinking off 编码按协议区分；qwen-code ModelSpec 的 `capabilities.reasoning{thinking,toggleOnly,disableField}`（`presets/alibaba-coding-plan.ts:22-35`）。
- **做法**：不做内置目录——用户在配置里写模型条目时允许携带 `protocol/reasoning/context_window` 覆盖字段（参考 kimi `refreshProviderModels.ts` 的自定义 provider 凭据+清单刷新思路），缺省值按协议给安全默认。

### 1.5 流式残缺 tool-call chunk 容错对照 🟡 P1
- **来源**：qwen-code `streamingToolCallParser.ts`（分块 tool-call JSON 残缺 chunk）。
- **做法**：与 `xai-grok-sampler/src/stream/collect.rs` 对照做一轮 gap 分析，缺则补（不重写）。

---

## 2. token 经济学

### 2.1 前缀缓存四件套 ⭐ P0（本报告最高优先级）
1. **字节级前缀一致不变量**：MiMo-Code `session/llm-request-prefix.ts`（notes/mimo-code.md ②2）——system+tools+消息序构造收敛为单一纯函数 `buildLLMRequestPrefix`，父循环与 fork 共用，`prebuiltSystem` 冻结 system。**落点**：sampler 请求构造纯函数化 + 单测断言两次构造 byte-equal。这是把缓存命中从"调参运气"变成"工程保证"。
2. **显式断点注入（按协议门控）**：opencode `packages/llm/src/cache-policy.ts`（notes/opencode.md B5）——auto 策略=三断点（最后 tool 定义/最后 system part/最新 user 消息），仅对支持 inline hint 的协议（Anthropic messages）注入，OpenAI 隐式缓存跳过，成本论证写在注释（1.25x 写 vs 0.1x 读）。kimi-code `kosong/src/providers/anthropic.ts:352-1083` 同款实现可对照。**落点**：Messages 协议流即插即用。
3. **断裂检测 + 闲置过期提醒**：kimi-code `apps/kimi-code/src/tui/controllers/cache-hint-controller.ts`（notes/kimi-code.md B5）——cache read 环比跌幅 <95% 报 `cache_break_detected`；闲置后 resume/提交时提醒缓存已过期。**落点**：grok 已有 usage 账本（`chat-state/usage.rs` cached_tokens + cache_creation_tokens），只差检测器和 UI 提示。
4. **cacheReadRatio 上 status line**：minimax-code `tui/application/session-cache-metrics.ts`（notes/minimax-code.md B5）。**落点**：本地补丁已扩过 status line 字段（PATCHES.md），顺手。

### 2.2 压缩升级
- **交接文档式摘要 prompt** ⭐ P0：kimi-code `agent/fullCompaction/compaction-instruction.md`（notes/kimi-code.md B5③）——保真最近意图/已运行命令与结果/已决未决/前向计划，跟随会话语言。**纯提示词替换，成本最低收益最直接**，落点 `xai-grok-compaction` 摘要模板。
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

1. **第一批（防 400 套装，适配性地基）**：1.2 reasoning 回写/泄漏清洗 + 1.3 协议边界消息合并 + 1.1 方言自学习。
2. **第二批（token 经济学主菜）**：2.1-1 前缀字节一致不变量 → 2.1-2 Messages 协议断点注入 → 2.1-3 断裂检测 → 2.2 交接文档摘要 prompt（提示词级，随手）。
3. **第三批（体感）**：3.1 提交边界重试 + 2.3 spill-to-disk（按原 compression-plan 推进）。

P1/P2 按 §3/§4 表内级别随迭代带入；每项引入后在本文勾选并注明落点 PR/commit。
