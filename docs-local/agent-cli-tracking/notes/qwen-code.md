# qwen-code 研究笔记

> 仓库: https://github.com/QwenLM/qwen-code
> 本地克隆: `D:/CODE/ai/qwen-code` · 分析基准 commit: `85631a3d`（2026-09-14）
> 语言栈: TypeScript / Ink+React 19 · 架构血缘: gemini-cli 深度 fork（packages/core = agent 引擎，packages/cli = TUI）
> 分析日期: 2026-09-19 · 范围: TUI 层与 agent 通用优化（vscode/desktop/web/channels 等未覆盖）
> 引入评估见 [../ADOPTION.md](../ADOPTION.md)

## ① 架构总览

gemini-cli 深度 fork 的 TypeScript monorepo。`packages/core` 为 agent 引擎：统一 contentGenerator 层多协议适配（OpenAI/Anthropic/Gemini/Qwen-OAuth），声明式 provider preset 体系，自带 subagent/Agent-Teams 工作流运行时、Auto-Memory 索引、三级阈值 compaction 与显式 prompt cache。`packages/cli` 基于 Ink/React 19 渲染，带逐帧流式合并与渲染高度感知的 markdown 增量提交。

## ② A. TUI 设计（packages/cli）

- **双渲染栈**：主栈 Ink 7 + React 19（`packages/cli/package.json:75-88`），同时引入 `@opentui/core/react 0.5.8` 备用 TUI 内核（`packages/cli/src/ui/opentui/`，含无障碍纯文本/a11y 屏幕阅读器输出）——对上游 gemini-cli 的显著改造。
- **流式渲染**：`packages/cli/src/ui/hooks/use-llm-stream.ts`（~2000 行）+ `use-frame-coalesced-flush.ts`——把每帧多个流式 chunk 合并为一次 flush；提交给 MarkdownDisplay 时按**渲染行高度**整块 commit（`use-llm-stream.ts:1924-1936`），围栏代码块用 `splitFencedMarkdown` 闭合/重开 fence 保持语法块合法——比上游"整段重渲"更精细。
- **工具调用展示/长输出折叠**：`toolResultDisplayCompaction.ts`、`toolUseSummary.ts`（core 侧预压缩展示），`use-show-tool-call-args.ts` 控制参数展开；`AnsiOutput.tsx`/`useAnimatedScrollbar` 处理终端输出。
- **主题**：14 套内置主题含 qwen-dark/light、ANSI-only、no-color（`packages/cli/src/ui/themes/`），`detect-terminal-theme.ts` 自动检测终端色。
- **输入/中断**：`InputPrompt.tsx` + bracketed paste、mouse tracking、wake-repaint hook（后台唤醒重绘），外壳事件经 AppContainer 统一分发。

## ③ B. Agent 通用优化（packages/core）

**B1 稳定性**
- 重试：`utils/retry.ts` + `utils/retryPolicy.ts`——指数退避+jitter，单次退避上限 5min；仅 429/529 允许"持久重试"不受 maxAttempts 限制，500 明确排除；尊重 `Retry-After` 头（minimum 模式且不加抖动），并处理 setTimeout 溢出（clamp 到 2^31）与 DashScope `Throttling.AllocationQuota`（HTTP 429 但属配额型、走有界重试）——分类非常细。
- SSE/流解析：`core/openaiContentGenerator/streamingToolCallParser.ts` 专门处理分块 tool-call JSON 的残缺 chunk；`taggedThinkingParser.ts` 兜底 `<think>` 标签泄漏（`converter.ts:1194-1279` 有生产事故回放测试）。
- 循环检测：`services/loopDetectionService.ts`（~1700 行），同内容 chunk 超过 `CONTENT_LOOP_THRESHOLD` 判定循环，同时服务 daemon turn-loop guard。

**B2 上下文压缩**
- `services/chatCompressionService.ts`（1349 行）：三级阈值梯（warn→auto→hard），`AUTOCOMPACT_BUFFER=13k`、`SUMMARY_RESERVE=20k`（注释直接对标 claude-code autoCompact 参数）；阈值=min(百分比， effectiveWindow−buffer)，小窗口自动降级。压缩输出 token 预算 = 剩余窗口−安全边距。auto 连续失败熔断。
- 微压缩：`services/microcompaction/microcompact.ts`——只清老工具结果内容（占位 `[Old tool result content cleared]`），不动对话骨架；`compactionInputSlimming.ts` 在压缩前瘦身输入。

**B3 Subagent / Agent Teams**
- 定义：`.qwen/agents/*.md` YAML frontmatter，schema **逐字段镜像 Claude Code 2.1.168**（`subagents/agent-frontmatter-schema.ts:10`），含 permissionMode/mcpServers/hooks/executor 解析；另有内置 agent 注册表（`builtin-agents.ts`）与代码式运行时（`agents/runtime/agent-core.ts`、agent-headless）。
- fork 查询：`agents/forkedAgent.ts` 双路径——带 cacheSafeParams 时走 LlmChat 单轮共享父 prompt（**保前缀缓存**），否则 AgentHeadless 多轮全工具。
- Agent Teams：`agents/team/`——TeamManager + leaderPermissionBridge（leader 权限桥接）+ mailbox 异步消息 + board-tasks/看板锁 + promptAddendum，是真正的多代理协作（成员可独立模型路由、plan 审批）。结果回传用 XML `<analysis>` 标签结构化解析（`agents/subagent-result.ts`），workflow 编排器支持 journal/snapshot/预算控制（`agents/runtime/workflow-*.ts`）。

**B4 会话记忆**
- 持久化：**JSONL**（`services/sessionService.ts:427-930`，`<uuid>.jsonl`，原子写 `utils/atomicFileWrite.ts`，写者租约 `session-writer-lease.ts` 防并发损坏）；PR sidecar `.pr.json`。
- 回滚：`services/fileHistoryService.ts` 按回合快照文件（成功备份 vs 失败捕获分开记账，损坏快照降级报告）；另有 git worktree 会话隔离（`gitWorktreeService.ts`）与 conversation branch（`utils/conversation-branches.ts`）。
- Auto-Memory：`src/memory/` 完整子系统——turn 后由 extraction agent 抽取记忆（`extract.ts`/extractionAgentPlanner）、indexer 重建托管索引、forget/dream（梦境式离线整理）、learn-skill-agent 把经验固化为 skill；与 QWEN.md/AGENTS.md 一起计入 always-on 上下文预算并在 config.ts:4727 告警。

**B5 前缀缓存（亮点）**
- `core/openaiContentGenerator/prefix-caching.ts`：对 `openai`/`qwen-oauth` 协议启用，注入 DashScope 风格 `prompt_cache_options:{mode:implicit|explicit,ttl}` 与消息级 `prompt_cache_breakpoint`（显式缓存设 2 个断点，cache key 前缀 `qwen-code:`）；显式模式仅对 gpt≥5.6 官方端点开启。forked agent 专门传 cacheSafeParams 共享前缀。
- cached token 记账：`converter.ts:1365-1402` 兼容 `prompt_tokens_details.cached_tokens` 与顶层 `cached_tokens` 两种方言，归一为 `cachedContentTokenCount`；Anthropic 侧兼容 `n_input_tokens` 历史格式（`anthropicContentGenerator/usage.ts`）。

## ④ C. 国产模型适配（重点，Qwen 官方项目）

- **协议抽象**：`AuthType` 6 值（`utils/auth-type.ts:8-15`：openai / openai-responses / qwen-oauth / gemini / vertex-ai / anthropic），每协议一个 contentGenerator 子目录（`core/openaiContentGenerator|anthropicContentGenerator|geminiContentGenerator|llm-content-generator`），统一收敛到 `llm-chat.ts`/`baseLlmClient.ts`。
- **Provider preset 体系**：`providers/types.ts` 声明式 ProviderConfig（protocol/baseUrl 选项/envKey/ModelSpec 元数据/modelsEditable），内置 preset：阿里系 3 档（coding-plan/standard/token-plan，均 DashScope 域名）+ deepseek/moonshot/minimax/zai/grok/openrouter/modelscope 等（`providers/presets/`）；`model-discovery.ts` 运行时拉 `/models` 推荐列表。Ollama/vLLM 经自定义 provider（openai 协议 + 自由 baseUrl）+ converter 内 ollama 特判接入。
- **Qwen OAuth**：`qwen/qwenOAuth2.ts` 完整 RFC 8628 device_code 流（免费额度），device_code 按 BrandedSecret 防泄漏、token 管理器共享（`sharedTokenManager.ts`），过期提示 /auth 重登。
- **模型参数映射**：ModelSpec 携带 `capabilities.reasoning{thinking,toggleOnly,disableField:'enable_thinking'}`、contextWindowSize（qwen3.5-plus 达 1M）、modalities（`presets/alibaba-coding-plan.ts:22-35`）；`models/modelConfigResolver.ts` 按 provider 解析。
- **reasoning_content**：converter 全链路处理——读取 `reasoning_content`/`reasoning` 双字段、回写 assistant 消息防 400（`converter.ts:817-828`）、`<think>` 标签泄漏清洗、thinking token 统计补算（converter.ts:1377）。deepseek provider 特判 `thinking:{type:'disabled'}`。
- **tool-call 方言**：per-provider 请求转换器（`core/openaiContentGenerator/provider/` 下 default/deepseek/mistral/fireworks/cerebras/mimo 等，各带测试），流式残缺 tool-call 由 StreamingToolCallParser 容错。

## ⑤ 最值得借鉴的设计

1. **显式前缀缓存断点注入**（prefix-caching.ts + forkedAgent cacheSafeParams）：把 prompt cache 当一等公民——请求层注入 cache key/断点、子代理复用父前缀、cached token 跨方言归一展示。国产 API 显式缓存能力普遍存在但少有 CLI 显式利用。
2. **渲染高度感知的流式 markdown 增量提交**（use-llm-stream）：按实际渲染行数整块 commit 并围栏闭合，兼顾流畅、正确性与性能，比 gemini-cli 上游的整段重渲染体验明显更好。
3. **分类重试策略**（retry.ts/retryPolicy.ts）：把 429 细分为"可无限退避的限流"与"配额耗尽的有界重试"，结合 Retry-After 精确等待与 setTimeout 溢出防护，是生产级 API 健壮性的样板。
