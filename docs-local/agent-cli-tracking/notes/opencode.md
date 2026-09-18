# opencode 研究笔记

> 仓库: https://github.com/anomalyco/opencode （原 sst/opencode，已迁移至 anomalyco）
> 本地克隆: `D:/CODE/ai/opencode`（分支 dev）· 分析基准 commit: `5f9d9187`（2026-09-18）
> 语言栈: TypeScript / Bun / Effect-TS · 架构血缘: 自研（client/server + SolidJS TUI + 自研 llm 协议层）
> 分析日期: 2026-09-19 · 范围: TUI 层与 agent 通用优化（桌面/Web/控制台/企业版未覆盖）
> 引入评估见 [../ADOPTION.md](../ADOPTION.md)

## 架构总览

TypeScript/Bun monorepo，Effect-TS 风格的后端（`packages/opencode`+`packages/core`）与 client/server 分离：server 暴露 HTTP API + SSE 事件总线，TUI（`packages/tui`）基于 **SolidJS + @opentui**（终端上的 solid 渲染器），通过 SDK 订阅 SSE 并批处理渲染。LLM 层独立为 `packages/llm`（自研协议适配：anthropic-messages / openai-chat / openai-responses / gemini / bedrock），上层经 Vercel AI SDK provider 抽象（`provider/provider.ts`）。元数据来自 models.dev。

## A. TUI 设计

- **渲染架构**：SolidJS 响应式 + @opentui（`packages/tui/package.json:55-63`），细粒度信号更新而非全量重绘；`packages/tui/src/context/sdk.tsx:56-80` 将 SSE 事件入队并按 16ms 窗口**批处理 emit**，保证一帧一次渲染（注释明言 "batch all event emissions so all store updates result in a single render"）。
- **流式输出**：消息按 part 组件渲染（`packages/tui/src/routes/session/index.tsx` 的 `AssistantMessage/UserMessage`），markdown 用内置解析配置（`packages/tui/src/parsers-config.ts`）；滚动定位按可见子元素 y 坐标计算（index.tsx:378-405）。
- **工具调用与 diff 展示**：revert/diff 文件列表带 +/- 行数着色（index.tsx:1240-1250），diff 换行模式可切（`diff_wrap_mode`）。
- **输入处理**：消息队列可视化（`QUEUED` 标记，index.tsx:1387-1450）、中断标记 "interrupted"（:1568）；有独立 win32 终端层 `packages/tui/src/terminal-win32.ts`。
- **通信**：SSE（`EventSource`，sdk.tsx:18-21）+ 本地 server。
- **长输出**：后端截断落盘——`packages/opencode/src/tool/truncate.ts:19` 返回 `{truncated, outputPath}`，提示语直接教模型用 Task/Grep 处理全量文件，避免父上下文爆炸。

## B. agent 通用优化

1. **稳定性**：`packages/opencode/src/session/retry.ts` — 指数退避（`delay`:47），优先读 `retry-after-ms`/`retry-after` 头，限流错误正则识别（:39），`retryable()` 按错误体/头判断。SSE 断流由 TUI 侧 abort/重订阅处理（sdk.tsx:83）。
2. **压缩**：`packages/opencode/src/session/overflow.ts` — `usable()` 以 model.limit.context 减 reserved（默认 20k buffer，可配 `compaction.reserved`），`isOverflow` 自动触发（`compaction.auto:false` 可关）。核心逻辑在 `packages/opencode/src/session/compaction.ts`：PRUNE_MINIMUM 20k / PROTECT 40k，工具结果截到 2000 字符（skill 工具豁免），保留近期 2k–15k token 的 turn；摘要由 `packages/core/src/session/compaction.ts` `buildPrompt` 生成——`<prior-summary>`+`<conversation>` 结构、锚定式增量摘要（旧摘要丢弃，冲突以新对话为准）、用**当前主模型**摘要。压缩后旧工具结果标 "[Old tool result content cleared]"，消息 ID 不变，天然保护前缀缓存。
3. **Subagent**：`packages/opencode/src/agent/agent.ts` — agent 定义 = 代码内置（build/general/explore，mode: subagent/primary）+ 配置/Markdown 覆盖（:284），甚至支持 LLM 生成 agent 配置（:408）。`packages/opencode/src/tool/task.ts` — 支持前台/`background=true` 异步（完成后自动通知，禁止轮询），`task_id` 可**续接同一子会话**；权限经 `agent/subagent-permissions.ts` 派生隔离；子结果作为 tool result 回传父上下文。
4. **会话记忆**：JSON 文件存储 `storage/session/{info,message,part}/**/*.json`（`packages/opencode/src/storage/storage.ts:97-165`），按 ID 分目录。resume/fork（`session.ts:691` fork 可从任意 messageID 分叉）、children 支持子代理会话树。**文件回滚**：`packages/opencode/src/snapshot` + `session/revert.ts:70-73` — snapshot.track/restore/patch revert 并生成 diff，TUI 展示回滚 diff。规则文件：`session/instruction.ts:61-122` 加载全局/项目 AGENTS.md（兼容 CLAUDE.md，祖先目录取首个匹配避免叠加）。
5. **前缀缓存**：这是显式设计——`packages/llm/src/cache-policy.ts`：默认 "auto" 策略注入 `cache_control` 断点于**最后一个 tool 定义、最后一个 system part、最新 user 消息**三处（注释引 LangChain 等实践论证 tool-loop 命中率）；仅对支持 inline hint 的协议（anthropic/bedrock）启用，OpenAI 隐式前缀缓存跳过；undefined→auto（1.25x 写 vs 0.1x 读的成本论证写在注释里）。`provider/transform.ts` 另有 `applyCaching` 按 model 处理。token 统计含 cache.read/write（overflow.ts:43）。

## C. 国产模型适配

- **Provider 架构**：元数据走 models.dev（`packages/core/src/models-dev.ts`），npm 包映射表在 `provider/provider.ts:114-132`；自定义 provider 走 openai-compatible + `options.baseURL/endpoint/apiKey`（provider.ts:363-365），auth 模块管理 key。自研协议层 `packages/llm/src/protocols/openai-compatible-chat.ts` 处理方言。
- **Reasoning**：`provider/transform.ts` — OpenAI Responses 请求 `reasoning.encrypted_content` 实现无状态多轮（:20-23）；DeepSeek 特判：assistant 消息**必须**带 reasoning part，为空也要补（:303-315）；interleaved-capability 模型把 reasoning 文本写入 `providerOptions.openaiCompatible[field]`（即 `reasoning_content`/`reasoning_details`），空也回传（:329-346）。
- **国产专门代码**：`session/system.ts:46-49` — Kimi 系（id 含 kimi/moonshot 或 provider 为 moonshotai-cn）换用专属系统提示 `session/prompt/kimi.txt`；`transform.ts:29-38` isKimiFamily 按 providerID/model/域名（api.kimi.com 等）识别。DeepSeek reasoning 修复如上。未发现 MiniMax/GLM/MiMo 专门分支，但通用 openai-compatible + models.dev 路径覆盖。

## 最值得借鉴的设计

1. **`packages/llm/src/cache-policy.ts` 的声明式缓存策略**：把"在哪打断点"从玄学变成带成本论证的一等策略层（auto=三断点），按协议能力门控注入，手工 hint 可覆盖——任何自研 agent harness 都可直接抄这套抽象。
2. **压缩即记账**（overflow.ts + compaction.ts）：阈值=模型上下文−保留 buffer，工具结果 2000 字符截断+白名单豁免、近期 turn 保 2k–15k token、增量锚定摘要——压缩后消息 ID 稳定，兼顾缓存命中，闭环完整。
3. **超长输出落盘委派**（truncate.ts）：截断不是丢信息，而是存文件并把"用 explore 子代理/Grep 处理"写进 tool result，把上下文压力转化为 task 工具的用法，与 subagent 机制联动，是上下文经济的聪明解法。
