# MiMo-Code 研究笔记

> 仓库: https://github.com/XiaomiMiMo/MiMo-Code
> 本地克隆: `D:/CODE/ai/MiMo-Code` · 分析基准 commit: `50cd7139`（2026-09-19）
> 语言栈: TypeScript / Bun · 架构血缘: **opencode 深度改造 fork**（Effect 骨架沿用，`packages/opencode/src` 约 1/3 目录为 MiMo 新增；已把 `@opencode-ai/core` 替换为自建 `@mimo-ai/shared`，TUI 为自有 SolidJS 一套，非上游 tui 包）
> 分析日期: 2026-09-19 · 范围: TUI 层与 agent 通用优化（desktop/console/web/slack 等未覆盖）
> 引入评估见 [../ADOPTION.md](../ADOPTION.md)

## ① Fork 关系结论

目录骨架、Effect 风格、config/agent/tool 体系均沿用 opencode，但 `packages/opencode/src` 内约 1/3 目录（actor、task、team、workflow、inbox、cron、memory、metrics、llm-server、session/checkpoint-* 等 20+ 文件）为 MiMo 新增，已重度分叉，远超"换皮 fork"。

## ② MiMo 独有设计

**1. 持久记忆系统**（`packages/opencode/src/memory/` + `session/checkpoint.ts`）
- 存储：markdown 文件树 `<data>/memory/{global|projects|sessions|cc}/<id>/<key>.md`，用 SQLite FTS5 建 BM25 全文索引（`memory/service.ts:102`，`fts-query.ts` 做分词）。类型含 memory/checkpoint/progress/notes/feedback 等（`memory/paths.ts:5`）。还可选索引 Claude Code 的 `~/.claude/projects` 记忆（`paths.ts:57` parseCcPath，`memory.cc_index` 配置）——直接兼容竞品记忆格式，是亮点。
- 写入：checkpoint writer 子代理按模板写 MEMORY.md/checkpoint.md（`session/checkpoint-templates.ts`），写开关集中在 `memory/write-gate.ts`（`memory.disable_write`，默认开）。
- 检索：模型经 `memory` 工具（`tool/memory.txt`）BM25 搜索，`service.ts:83` 用"相对得分下限 0.15"过滤常见词噪声，搜索前惰性 reconcile。
- 跨会话注入：project scope 的 MEMORY.md 即持久项目理解；`project/bootstrap.ts` 启动时 reconcile。
- **自我改进**：`session/auto-dream.ts` 每 7 天自动 spawn "Dream"会话（`agent/prompt/dream.txt`）从 SQLite 轨迹库 consolidate 记忆；每 30 天 "Distill"（`distill.txt`）把重复工作流固化为 skill/subagent/command——即 README 的 "self-improvement"。
- 与 AGENTS.md 无直接耦合：AGENTS.md 走上游 instruction 体系，memory 是独立数据目录树。

**2. 前缀缓存优化**（`session/llm-request-prefix.ts`）
- 核心是 **byte-equal invariant**：`buildLLMRequestPrefix` 把 system + tools + inheritedMessages 的构造收敛为单一函数，父 runLoop 与 checkpoint-writer fork 共用同一构建路径，保证请求前缀逐字节一致→前缀缓存命中（文件头注释明确说明）。
- system 尾部内容（环境/格式/skill reminder/instruction 文件）由调用方定序固定，支持 `prebuiltSystem` 冻结 system 禁止再生成。
- 无显式 99%/95% 数字的代码；统计侧：`metrics/event.ts` 上报 `cached_read_tokens`，TUI `app/src/components/session/session-context-metrics.ts` + `session-context-tab.tsx` 展示 cacheRead/cacheWrite token。跨会话命中靠 project-scope 稳定记忆注入 + 稳定 system 构成。

**3. 模型路由 / smart orchestration**（`session/max-mode.ts`，436 行）
- 不是按成本自动路由，而是 **best-of-N + judge**：`MAX_MODE_AGENT="max"`，每步并行跑 5 个 propose-only 候选（schema-only 工具、不执行），再由独立 judge 模型择优（`judge()` at max-mode.ts:267，prompt "You are a judge selecting the single best next step"），judge 失败兜底候选 0。
- 另有 `session/prompt/orchestrator.txt` 编排 agent，指示子代理崩溃后用 `session send` 续接而非重建。任务分类器 `session/classify.ts` 是**步进循环**的步骤分类（final/continue/think-only/invalid…），非成本路由。

**4. Cascade subagent resume（50cd713 提交）**
- 机制：主会话 Resume 时 best-effort 恢复"registry-idle 且有恢复候选"的同会话非主 actor（`actor/registry.ts`、`actor/spawn.ts`），生命周期走 ActorExecution + runTurn + 父通知，cancel epoch 绑定、准入超时中断；缺 ForkContext 在源头失败。running/pending 子代理绝不自动接管。
- **honest failure terminals**：不掩盖子代理失败——错误终态如实落为 terminal（`session/message-error.ts`），且面向模型的恢复提示只建议"发 continue/重新 spawn"，不提用户层概念（提交信息自述）。

**5. 上下文压缩**：大幅改造而非沿用。`session/compaction.ts` 与上游参考版 diff 达 903 行：新增 `COMPACTION_TAIL_BUDGET=40k`、工具结果 8k 截断、文件 manifest、tail-shrink 元数据；配套 `budgeted-read.ts`、`checkpoint-context.ts` 的"重建尾折叠为活动日志"（`llm-request-prefix.ts` 注释）。

## ③ 国产模型适配

- **provider/transform.ts** 有成体系的国产模型分支：按模型 ID 调整比例/参数（qwen 0.55、glm-4.6/4.7、minimax-m2、kimi-k2 系列，transform.ts:1210-1238）；强制开启 thinking 以获得 `reasoning_content`（kimi-k2.5/qwen3/deepseek-r1，transform.ts:1720-1741，注明 kimi-k2-thinking 除外）；moonshot/kimi provider 特判（:1989）。
- **每模型 system prompt**：`session/prompt/{deepseek,glm,kimi,minimax,anthropic,gpt,beast,trinity}.txt` 按模型族切换提示词。
- 自定义 OpenAI 兼容 provider：上游 models.dev 目录（`provider/models.ts`，`Flag.MIMO_MODELS_URL` 可换源）+ `OPENAI_COMPATIBLE_NPM` 路径（provider.ts:1362 对 deepseek 等 ID 有专门补全逻辑）。
- 未发现小米自家 MiMo 系模型的内置模型清单（代码中 "mimo" 只出现在包名 `@mimo-ai/*` 与产品名），接入仍走通用 OpenAI 兼容配置。

## ④ 最值得借鉴的设计

1. **buildLLMRequestPrefix 的"字节级前缀一致"不变量**：把 system/tools/消息序的构造收敛到一个纯函数，父代理与 fork 共用，使前缀缓存命中成为结构性保证而非调参运气——这是把 prompt caching 做成工程约束的范本。
2. **Dream/Distill 自我改进闭环**：以 SQLite 会话轨迹为"真相源"、markdown 记忆为"索引/缓存"，定期自动 spawn 会话做 consolidation 与工作流固化，且记忆目录直接兼容 Claude Code 格式，冷启动即有数据。
3. **best-of-N + judge 的 max 模式**：候选用 schema-only 工具 propose-only 并行、失败单候选可弃、judge 失败兜底候选 0——在单代理循环里低风险引入"多路采样择优"，代价模型清晰（overhead 只记 metric 不进上下文）。
