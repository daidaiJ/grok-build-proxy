# zcode 研究笔记

> 仓库: ZCode（Z.ai/智谱官方 AI 编程工作台开源仓）· 本地克隆: `D:/CODE/ai/zcode`
> 语言栈: TypeScript monorepo（pnpm）· 血缘: Z.ai 官方产品，交互/概念体系（hooks 事件名、SKILL.md、agent frontmatter、rewind、plan mode）与 Claude Code 高度同构但为自研实现，非 fork；依赖 Vercel AI SDK（带仓库 patch）；捆绑 superpowers 插件（MIT）
> 分析基准 commit: `29628c9`（2026-09-24，快照仓库单提交，v3.14.3）· 分析日期: 2026-10-11
> 范围: 特性设计面（不逐文件流水账）；行号均实际读过源码

## 定位与血缘

Z.ai（智谱海外品牌）/BigModel 的官方 AI 编程工作台：桌面（Electron）+ 浏览器 Web + 终端三形态，单一 `zcode` 命令分流（无参→TUI，`--web`→Web，其余→Agent CLI），v3.14.3，Apache-2.0。商业闭环 = 官方 Coding Plan 订阅（内置 GLM-5.3 / GLM-5.3-Flash），模型请求统一改发 `zcode.z.ai` 平台网关。

## 架构速览

- **双层 monorepo**：`packages/`（desktop/web/server/services/ui/shared/rpc/provider…）+ `apps/zcode-cli/`（Agent 运行时，零生产依赖 CLI bundle）。
- **CLI 内部分层**：`contracts`（zod 契约与端口接口）→ `core`（runtime/turn-machine/tool/permission/subagent/workflow/memory/compact）→ `adapters`（模型执行与协议兼容，49 文件）→ `bootstrap`（装配：auth/skills/插件/自定义命令/`zcode-protocol-v4`）→ `tui`（OpenTUI + React 19）。
- **独立库**：`dynamic-workflow`（纯函数工作流引擎）+ `dynamic-workflow-runtime`（沙箱 harness）+ `node-repl-host` + `model-option-map` + `i18n`。
- **工程治理**：`architecture-policy.yaml` 声明模块边界（单文件 ≤400 行、禁环、禁深导入）+ `scripts/architecture/architecture-check.mjs` 带 baseline；`DESIGN.md` 是写给 coding agent 的 UI 设计系统规约。

## 特性清单

### 会话 / 持久化 / 分支
- **Rewind 检查点** — 检查点载荷带 diff hunk，scope 分 conversation/workspace/both；策略判定四态 `active_chain/file_only/fork_required/unavailable`，且 compact-aware（压缩边界不阻断回退）（`apps/zcode-cli/packages/contracts/src/rewind/index.ts`；`core/src/runtime/methods/rewind.ts` 等）。
- **会话 fork + 稳定 fork 边界** — 独立 fork 方法与「稳定边界」预计算，配合正式验证器（`core/src/runtime/methods/session-fork.ts`、`stable-fork-boundary.ts`）。
- **SQLite 会话库** — `~/.zcode/cli/db/db.sqlite`；tasksDatabase 版本化迁移（provider-selection v2、official-glm-selection v3）。
- **跨生态会话互通** — 导入 Claude 原生会话（`packages/services/src/session/claude-native/`）、识别 OpenCode Go 端点；会话分享走「公开投影 + 附件 + 确认时间校验」。

### 上下文管理
- **双层压缩** — auto compact 双预算策略（legacy/preflight-v1）、token 源可切 `estimate/provider_usage`（用 provider 回报的 cache read/write 精确判定）、连续失败上限 3；**microcompact 按工具名白名单清理旧 tool result**（阈值 0.9 ratio、保留最近 5 条、最小节省 256 token）（`core/src/compact/policy.ts`、`microcompact.ts`）。
- **system-reminder 三轴建模** — 每条 reminder 按「投递通道 × 生命周期 × provider 可见性」三维类型化，通道含 request_prefix/tool_result/history_continuity/mid_turn_event 等（`core/src/system-reminder/source.ts`）。
- **上下文 sections 化组装** — identity/env/skills listing/memory/workflow-actor 等动态段（`core/src/context/sections/`）。
- **记忆子系统** — CLI 默认开、桌面默认关；提取走独立只读记忆 agent loop（仅 Read/Grep/Glob 三工具 + 工具策略门）、按项目根持久化、manifest 扫描式 recall（`core/src/memory/memory-agent-loop.ts`、`recall/manifest.ts`）。

### LLM 协议兼容
- **compat fetch 包装层（国模/网关适配核心）** — Anthropic 流与 thinking JSON 归一、OpenAI Responses JSON 修补、strict tool schema、空补全重试、流空闲超时、重试预算档位（「无上限」只放宽放弃条件，退避曲线/可重试分类不动）（`adapters/src/model/anthropic-stream-compat.ts`、`openai-responses-json-compat.ts`、`retry-budget.ts`、`stream-idle-timeout.ts` 等）。
- **官方 Coding Plan 网关改写** — 按「协议+host+端口+路径」精确匹配官方 anthropic 端点，URL 重写为 zcode.z.ai 网关路径，方法/正文/鉴权头原样透传；用户自建 provider 不受影响（`adapters/src/model/official-coding-plan-gateway.ts`）。
- **model-option-map DSL** — 每模型的 reasoningLevel/maxOutputTokens 映射写成 DSL，模型创建时编译一次、每请求只绑定冻结值，以有序 JSON merge patch 注入请求体（`packages/model-option-map/src/option-maps.ts`）。
- **内置 provider 目录远程化** — 版本化 provider 配置（revision 30）从远端同步/物化/缓存。

### 子 agent / 编排
- **dynamic-workflow（最大亮点）** — 模型写 TS 工作流脚本：编译器对内嵌 facade `.d.ts` 做 typecheck，收集 ask/actor/world-read/join 四类站点，做**污染不动点 + 时序走查的静态分析**并产出诊断；执行在「子进程 + vm cell + NDJSON host bridge」沙箱，SQLite journal 支持回放（`apps/zcode-cli/packages/dynamic-workflow/README.md`、`core/src/workflow/scheduler.ts`）。
- **workflow 修正工具族** — **amend-workflow**（description/resolve/retune/source 四面）让主 agent 可中途修订运行中的工作流（`core/src/tool/handlers/amend-workflow*.ts`）。
- **子代理体系** — 内置 Explore/general-purpose + markdown frontmatter 自定义 profile；借用式 MCP port、运行中消息 steering、完成通知回灌、per-agent 持久记忆（`core/src/subagent/`）。

### 工具体系
- **权限四模式** — build/edit/plan/yolo（headless `--prompt` 未指定 `--mode` 默认 yolo，`cli/src/run.ts:42`）；声明 `alwaysAsk` 的工具**不能被模式放行绕过**；plan-mode 独立策略（`core/src/permission/service.ts`）。
- **bash 静态分析族（约 20 文件）** — argv 级只读判定（git/flags/process/text 分家族文件）、命令解析三态、cwd 策略、gh 限速、git 运行时安全（`core/src/tool/handlers/bash-*.ts`）。
- **读取时间线追踪** — read-file-state 记录文件读取状态供 edit 一致性校验；background bash 生命周期与输出落盘；webfetch 预批准清单。
- **node_repl 沙箱 host** — 每次 `js` 调用全新 kernel，仅 `node:*` 内建 + 技能根下 `file://` 可导入；与 Computer Use 共享 host（`apps/zcode-cli/packages/node-repl-host/`）。

### TUI / 扩展生态
- **OpenTUI + React 19 终端渲染** — shiki 语法高亮 + web-tree-sitter；subagent 侧栏透视图、workflow 时间线卡、排队输入、selection 模式、审批面板。
- **插件商店** — 唯一官方市场（内置播种 + CDN sha256 校验 zip）；manifest 含 skills/commands/mcpServers/userConfig，`${user_config.key}` 展开。
- **hooks 七事件** — 与 Claude Code 同名；workspace hook 按「工作区身份 + 声明摘要」信任审查，**摘要变化即失效**（`contracts/src/hooks/index.ts`、`core/src/hooks/workspace-hook-*.ts`）。
- **MCP 双来源** — 工作区级 + 插件级自动连接，OAuth 自动刷新。

### headless / 自动化
- **非交互运行** — `--prompt --mode --attach` + toolDisallowlist + resume（`cli/src/run.ts:178,517`）。
- **闲时任务（Off-Peak）票据化** — 服务端取号/排队/核销，「创建即取号」（`packages/services/src/session/offPeakTask.ts`）。
- **桌面 automation cron** — 间隔任务/校验/恢复。

### 工程与测试
- **formal-proof 状态空间枚举器** — 对 compact/fork/goal/消息队列/query 编辑的产品行为组合做枚举验证（`packages/formal-proof/`）。
- **prompt-trajectory 录制器** — OpenAI 协议轨迹录制/派生，离线检视 prompt 组装与 model-io（`apps/zcode-cli/tools/prompt-trajectory/`）。
- **内置原生搜索二进制** — 预编译搜索工具随发行包分发（含 Windows PE 校验）。

## 独特亮点（别家少见）

1. **dynamic-workflow 全链路**：模型写 TS 脚本经「编译器静态分析（污染不动点 + 时序走查）→ vm 沙箱 → journal 回放」的完整严格化管线。
2. **model-option-map DSL**：模型参数适配从硬编码升级为可编译、可远程下发的映射语言。
3. **formal-proof**：把压缩/fork/队列等核心会话行为做成状态空间枚举验证，测试基建级投入。
4. **闲时任务票据体系**：把「错峰跑任务」做成服务端取号/排队/核销的产品化能力。
5. **system-reminder 三轴类型化**：reminder 的通道/生命周期/provider 可见性显式建模。
6. **架构即代码**：policy yaml + check baseline + knip 组成可机器执行的边界治理（≤400 行/文件写进全局约束）。
7. **双端记忆策略分化**：同一记忆子系统按入口（CLI 开/桌面关）差异化默认值。

## 版本与活跃度

- 版本 **v3.14.3**（根 `package.json`；README 更新日志 2026-09-23）。
- 本地克隆为**快照仓库：单提交 `29628c9`（2026-09-24）**，无法从 git 评估提交频率；官方产品在营（zcode.z.ai 网关、官方插件市场 CDN、Feishu/Discord 社区），**活跃维护中**。

## 存疑 / 仅文档宣称

- Computer Use 在开源仓是不可用占位（`packages/zcode-cua/index.js:1` 起返回 "not available"，与 NOTICE.md 一致）。
- 桌面定时任务、闲时任务的完整服务端行为在客户端仓内只能看到接口面，服务端实现不在本仓。
