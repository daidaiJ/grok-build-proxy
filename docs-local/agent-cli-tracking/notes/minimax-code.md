# minimax-code（MiniMax Code / MCode）研究笔记

> 仓库: https://github.com/MiniMax-AI/minimax-code
> 本地克隆: `D:/CODE/ai/minimax-code` · 分析基准 commit: `30dd6f27`（2026-09-19）
> 语言栈: TypeScript / pnpm · 架构血缘: **自研产品层 + vendored pi 生态**（pi-mono 在 `third_party/`，TUI engine 为 pi-tui 0.84.2 受控 fork）
> 分析日期: 2026-09-19 · 范围: TUI 层与 agent 通用优化（browser-core/local-runtime 细节/mcode-tools-host 未覆盖）
> 引入评估见 [../ADOPTION.md](../ADOPTION.md)

## ① 架构总览

TypeScript pnpm monorepo，核心深度复用 earendil-works/pi 生态（pi-mono 作为 workspace 子包 vendored 在 `third_party/`，TUI engine 也是 pi-tui 0.84.2 的受控 fork，见 `packages/tui/src/tui/engine/UPSTREAM.md`）。自研部分：pi-turn-runner（agent 循环）、agent-modules（context-manager、system-reminder、skills、permission 等）、local-runtime-v2（会话/turn 服务）、TUI 产品层。云能力（多模态生成、web search）走 MiniMax "Matrix" 托管后端。

## ② A. TUI 设计（packages/tui）

- **渲染架构：自研布局引擎（pi-tui fork），非 React/Ink**。`packages/tui/src/tui/engine/`：`layout.ts`/`layout-node.ts` 组件树 + diff 重绘，组件 `engine/components/`（box、markdown、editor、scroll-view、select-list…）；`BASELINE.json` 精确记录每个文件的上游 hash，`LOCAL_CHANGES.md` 记录 fork 增量，且有 `scripts/verify-tui-engine-baseline.mjs` 可校验——**供应链式 UI fork 管理，亮点**。产品代码只允许 import `public.ts`。
- **Markdown 增量渲染**：`engine/components/markdown.ts` 基于 `marked` + 自定义 Tokenizer（严格删除线、行内/块级 LaTeX token，未闭合公式按 pending 处理），配合 `engine/latex.ts` 渲染；流式 delta 经 `agent-core/src/event-bridge/`（bridge.ts、display-sanitize.ts）转成 UI 事件。
- **工具调用/diff**：转写展示在 `tui/features/transcript/panel.ts`；文件回滚前 diff 预览在 `tui/features/session-mutation/rewind-preview-panel.ts`。
- **输入队列与中断**：`tui/features/queue/panel.ts`（queued/paused/failed 状态、重试/编辑/丢弃）+ `paused-send-panel.ts`（中断时用户可选"插队发送"还是"清空队列"）。编辑器带 undo-stack、kill-ring、east-asian-width 宽度处理。
- **长输出/检索**：`engine/tui-alt-screen.ts` + `alt-screen-search.ts` 支持滚动区全文搜索；`components/truncated-text.ts` 截断。
- **与核心通信**：TUI 通过 `runtime/port.js` 端口抽象接 local-runtime-v2（ACP 协议 `@agentclientprotocol/sdk`，`tui/src/acp/`），使用量经 `application/response-usage.ts`、`session-cache-metrics.ts` 投影。

## ③ B. Agent 通用优化

**1. 稳定性（`agent-core/src/pi-turn-runner/llm-retry.ts`）**
- `withLLMRetry`（:190）包住 provider StreamFn：默认 5 次重试、1s→30s 指数退避+抖动、总时限 120s（:43-48），优先尊重 `Retry-After`/`retry-after-ms` 头（:897）。
- **提交边界设计（亮点）**：preflight 阶段缓冲事件流，直到出现第一个可见 delta（text/thinking/toolcall delta）才"提交"该物理请求（:458-474）；此前失败可无痕换请求重试，之后失败不再静默重放，避免重复输出。BYOK provider 宽松分类、官方 provider 严格（:19-21,463）。
- 错误分类：`shared/src/llm-error-classifier.ts` 归一化为 16 种 kind（rate_limited/tpm/usage_limit/credits/content_filter/overloaded/empty_response…），对外只暴露脱敏文案（:170-187），每次物理请求/逻辑调用双观测器上报 usage 与 errorKind。

**2. 上下文压缩（`agent-modules/context-manager/`）**
- `manager.ts`：作为 `PiBeforeLlmCallHook` 在每次 LLM 调用前评估；触发阈值 `settings.ts:29` = contextWindow×0.9（注释指 M3 512K/1M 双模式）；按 sessionId 加锁；`SkipReason` 枚举细化（消息太少、低于阈值、无安全切点、插件 hook 推迟、摘要为空等）。
- 摘要复用 pi-agent-core 的 `generateSummary`，压缩产物作为 CompactionSummaryMessage 注入；local-runtime-v2 另有 `turn-system/compaction/algorithm/`（tool-result-archiver、history-reduction、todo/assistant 节奏控制、checkpoint-format）做工具结果归档与多档策略。

**3. Subagent 机制**
- 角色固化：`shared/src/subagent-roles.ts` 定义 canonical 三角色 **explore（只读侦察）/ worker（限定交付）/ verifier（独立验收）**，含 whenToUse prompt 文案与保留名清单；`agent-tools/src/desktop/local-task.ts`、`local-task-control.ts`、`task-verification.ts` 负责派发与结果回传；`agent-modules/background-task/` 提供后台运行/输出存储。隔离靠独立 session + 角色约束的工具白名单（canonical-tool-policy.ts）。

**4. 会话记忆**
- 持久化在 local-runtime-v2 `service/session-system/`：files/repo 分层存储 + `fork/` 分支 + `diffs/service.ts` 工作区 diff 记录；`messages/history/` 有 canonical-history-materializer 与 stale-compaction-repair（修复历史里失效的压缩标记）。回滚走 session-mutation 的 rewind 预览。
- AGENTS.md：`agent-modules/system-reminder/src/providers.ts:443` 仅 git workspace 注入 bootstrap 提示。

**5. 前缀缓存**
- 一等公民：usage 四桶 input/output/cacheRead/cacheWrite 全链路贯通（llm-retry.ts:73-88、local-runtime-v2 token-usage.ts），TUI 侧 `application/session-cache-metrics.ts` 计算会话级 **cacheReadRatio** 展示；pi-ai 有 `providers/openai-prompt-cache.ts` 专门做 OpenAI 兼容缓存优化；压缩阈值取 0.9 明确为"尽量少动前缀保 cache"。

## ④ C. 模型适配

- **MiniMax 官方**：`config/src/config.ts:1530+` 内置模型目录——M3（512K/1M 双 contextWindowOptions + 变体 thinking: disabled/adaptive、file API 能力位）、M2.7/M2.7-highspeed（200K，reasoning+tool_call）。托管网关按 cn/en×test~prod 预设 baseURL（`PRESET_BASE_URLS`）。pi-ai 的 models.generated.ts 另含 minimax-m2.x（Bedrock 路径）。tool-call 方言与 thinking 解析由 vendored pi-ai 统一（openai-completions/responses 双 API）。
- **BYOK**：`config/src/byok-config.ts` —— 内置 `minimax`（托管）与 `minimax_api`（用户自有 key，保留 id 不可被用户配置遮蔽，:275-291）+ `custom_provider:*` 任意 OpenAI 兼容 baseURL；模型元数据来自 **models.dev 目录快照**（`shared/src/models-dev.ts`，cn 区用 MiniMax CDN 镜像防墙）。
- **国产厂商**：无专门方言处理，靠 models.dev 目录 + provider preset 顺序（`local-runtime-v2/.../provider-presets.service.ts:27`：cn 区推荐 zhipuai、deepseek、moonshotai-cn、openai、anthropic），DeepSeek/Qwen 词表用于本地 token 估算（local-runtime token-counter-adapters）。
- **oauth-core**：通用 OAuth 客户端 + 凭据存储（file-store、原子写、跨进程锁、命名空间迁移）。
- **oauth-lease-protocol**：`contracts.ts` 定义 broker 式**授权租约协议**（status/lease/unauthorized 三方法、capability 43 位串、minValidityMs、generation 计数）——把账号订阅 token 安全地租给子进程/插件宿主，避免长期凭证扩散。

## ⑤ 最值得借鉴的设计

1. **流重试的"提交边界"**：llm-retry.ts 先缓冲、见到首个可见 delta 才绑定物理请求，失败重试对用户零感知且不产生重复/截断文本；配合 Retry-After 解析与 16 类错误归一，是各家 agent 里最完备的流级重试实现之一。
2. **UI fork 的供应链管理**：对 pi-tui 引擎保留 BASELINE.json 逐文件 hash + LOCAL_CHANGES 增量清单 + 可执行校验脚本，既吃到上游维护又保住自有改造可审计——任何"fork 开源组件"的项目都可复制。
3. **缓存友好的一等指标**：从 provider 四桶 token 到 TUI 展示的 cacheReadRatio 全链路类型化，压缩阈值明确为前缀稳定性服务；对按缓存计价的国产 API（MiniMax/DeepSeek）尤其划算，值得作为 agent 框架标配。
