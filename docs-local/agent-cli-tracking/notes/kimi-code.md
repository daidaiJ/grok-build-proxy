# kimi-code（Kimi Code CLI）研究笔记

> 仓库: https://github.com/MoonshotAI/kimi-code
> 本地克隆: `D:/CODE/ai/kimi-code` · 分析基准 commit: `02d829e1`（2026-09-19）
> 语言栈: TypeScript / pnpm · 架构血缘: 自研（pi 生态 TUI + 自研 agent-core-v2/kosong/minidb/transcript）
> 分析日期: 2026-09-19 · 范围: TUI 层与 agent 通用优化（remote-control/kap-server/node-sdk 等未覆盖）
> 引入评估见 [../ADOPTION.md](../ADOPTION.md)

## ① 架构总览

`kosong`：LLM 抽象层，含各协议 provider（kimi/anthropic/openai/google-genai）与模型 catalog；`agent-core-v2`：agent 核心（DI 仿 VSCode 架构，agent 循环/压缩/undo/subagent/session 持久化）；`pi-tui`：终端 UI 框架（与 MiniMax Code 同源）；`minidb`：纯 Node 嵌入式 KV 库（Redis+SQLite 杂交，WAL/快照/二级索引），作 persistence 后端；`transcript`：会话消息存储/回放/分页；`oauth`：Kimi 登录与凭据管理；`klient`：客户端契约层。产品层在 `apps/kimi-code/src/tui/`（组件、controllers、渲染）。

## ② A. TUI 设计

- **渲染架构**：pi-tui 自研布局引擎（`packages/pi-tui/src/layout.ts`、`layout-node.ts`），双屏模式（`tui-main-screen.ts`/`tui-alt-screen.ts`，主屏保留滚动历史，alt-screen 带搜索 `alt-screen-search.ts`）。终端能力协商：kitty keyboard protocol 探测（`terminal.ts:17-25`）、bracketed paste（`terminal.ts:184`）。
- **Markdown 增量渲染**：`packages/pi-tui/src/components/markdown.ts:338-383` — 按文本+宽度做渲染结果缓存（cachedText/cachedWidth/cachedLines），流式时每帧只重渲染变化文本；流中自动修剪残缺代码围栏防闪烁（:252）。产品层 `apps/kimi-code/src/tui/components/markdown/` 另有 mermaid ASCII 渲染（`mermaid-art.ts`）。
- **工具调用/diff 展示**：`apps/kimi-code/src/tui/components/messages/tool-call.ts` — 默认折叠、Ctrl+O 展开（:583,787）；按工具分渲染器（`tool-renderers/`）；thinking 有独立组件（`thinking.ts`），agent 并行进度用"蜂群"估算器（`agent-swarm-progress-estimator.ts`）。
- **输入处理/IME**：终端侧用 kitty protocol + bracketed paste 解决粘贴误提交；另有启发式 `paste-burst.ts`（`packages/pi-tui/src/paste-burst.ts`）——无 bracketed paste 的终端中，检测 8 字符以上快速连打后跟 Enter（窗口 120ms），把 Enter 降级为换行防误提交。IME 依赖 kitty 协议下 composition 期间不触发提交，最近 commit 只修了 vscode 端。
- **中断/队列**：`packages/agent-core-v2/src/agent/loop/loop.ts:38,209` — loop 维护 `queue: UserEntry[]`，支持 `steer()`（运行中转向，`steerIfActive`），状态机在 `loop/machine/`。

## ③ B. Agent 通用优化

**1. 稳定性**：`agent/llmRequester/llmRequesterService.ts:284-535` — 统一退避重试（`_base/utils/retry` 的 `retryBackoffDelay`），支持 `KIMI_CODE_INFINITE_RETRY` 无限重试；429 解析 `Retry-After` 头（`kosong/src/providers/openai-common.ts:151-158`），区分配额耗尽（不可重试）与限流（可重试）；kimi 配额错误单独分类（`kosong/src/providers/kimi-errors.ts`）。

**2. 上下文压缩**：`agent/fullCompaction/strategy.ts:19-27` — 触发阈值 85% 上下文（可按模型 `compactionTriggerRatio` 覆盖），block 比率防止新请求超限，保留最近 4 条消息/≤20% 尺寸；溢出时最多 3 次递进压缩。摘要 prompt（`compaction-instruction.md`）为**"交接文档"式**：要求保真最近请求意图、已运行命令与结果、未决问题、前向计划，且跟随会话语言。压缩后用户消息连续导致严格 provider 400，由 `kosong/src/providers/merge-user-messages.ts` 在协议边界合并（含并行 tool-result 必须同 turn 的规则）。

**3. Subagent**：`session/subagent/` — `spawn.ts` 支持三种来源：独立 profile（`subagent_type`）、模型覆盖、以及 **fork 当前上下文**（一次性快照注入，注入提示明告"这不是你的历史，是参考材料"，:17）。结果回传为 `summary` 字符串（`subagent.ts` 的 `AgentRunCompletion`）；工具结果回传前统一截断+落盘：`agent/toolResultTruncation/toolResultTruncationService.ts:46-77` — 超限文本写 spill 文件，消息中只留指针+尾部保留。

**4. 会话记忆**：持久化三层——`transcript` 包（消息历史/分页/回放）+ `persistence/backends/minidb`（WAL+CRC 崩溃恢复+快照 compaction 的单文件 KV）+ fs 后端。resume/fork：`subagent/spawn.ts`（fork 与 resume 互斥校验）。文件回滚：`agent/undo/undoService.ts` — 对话级 undo（`ContextUndone` 事件，按 turn 切割 `computeUndoCut`），各工具注册 undo participant（`contextMemory/conversationUndoParticipants.ts`）。指令加载：`agent/profile/context.ts:79-214` — 收集 `AGENTS.md`、`.kimi-code/AGENTS.md`、`~/.agents/AGENTS.md` 多级；且 `agent/agentsMdReminder/` 会在工具调用触达未注入路径或文件变更时动态注入提醒。无 auto-memory 机制。

**5. 前缀缓存**：Anthropic 路径显式注入 `cache_control: ephemeral` 到最后消息块与最后一个工具（`kosong/src/providers/anthropic.ts:352-1083`）；Kimi/OpenAI 走自动缓存，仅解析 Moonshot 专有顶层 `cached_tokens`（`openai-common.ts:213-222`）。**亮点**：`apps/kimi-code/src/tui/controllers/cache-hint-controller.ts` — **缓存断裂检测器**：某步 cache read 相比上一步骤跌 <95% 即报 `cache_break_detected`（识别模型/effort 切换导致）；session 长时间闲置后 resume 或闲置后提交时弹出"缓存已过期"对话框，可拦截提交提醒用户。

## ④ C. 模型适配

- **thinking/reasoning**：`kosong/src/providers/reasoning-key.ts` — 三方言统一（`reasoning_content`/`reasoning_details`/`reasoning`），入站按优先级扫描，出站按 `ReasoningKeyDialect` **逐端点学习回放**（"对方说什么方言就回什么方言"），可显式 pin。Kimi reasoning 模型的 max_tokens 与 thinking 共享预算的特殊处理（`kimi.ts:59-60`）。
- **协议目录**：`kosong/src/catalog.ts` — 接入 models.dev 的 api.json 作模型清单（`KNOWN_WIRE_TYPES: anthropic/openai/kimi/google-genai/openai_responses/vertexai`），自动过滤 embedding/deprecated/alpha 模型；每模型带 `reasoningKey`、`supportEfforts`、`offEffort`、`alwaysThinking`、`protocol` 覆盖（如 gateway 用 anthropic 协议）。thinking off 编码按协议区分（:419-429）。
- **国产厂商**：无 DeepSeek/Qwen/GLM/MiniMax 专门处理——靠通用 OpenAI 兼容 provider + reasoning-key 自适应方言覆盖（DeepSeek 的 `reasoning_content` 正是默认值）。
- **OAuth**：`packages/oauth/src/oauth.ts` + `device.ts` — 标准 device flow（`/api/oauth/device_authorization` + `device_code` 轮询 `/api/oauth/token`），含 token 刷新事务（`oauth-token-transaction.ts`）、自定义 provider 凭据注册（`provider-credential.ts`、`custom-registry.ts`）、模型清单动态刷新（`refreshProviderModels.ts`）。

## ⑤ 最值得借鉴的设计

1. **ReasoningKeyDialect 方言自学习**（`reasoning-key.ts`）：不硬编码厂商映射，而是观察每个端点实际使用的 reasoning 字段名并回放，天然兼容任意 OpenAI 兼容网关与 vLLM 版本差异——比枚举厂商列表鲁棒得多。
2. **缓存断裂检测 + 闲置提示**（`cache-hint-controller.ts`）：从 usage 的 cache read 骤降反推 cache break，并在 resume/闲置提交两个最可能断缓存的时机主动提醒用户——把 token 成本优化做成了可感知的 UI 机制，各家 CLI 均未见。
3. **压缩"交接文档"式摘要 prompt**（`compaction-instruction.md`）：要求模型区分"已决/未决"、保真命令与结果、给前向计划，并跟随会话语言；配合 spill-to-disk 的工具结果截断（消息留指针、全文落盘可回取），上下文管理链路完整且有理论自洽性。
