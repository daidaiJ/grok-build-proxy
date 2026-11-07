# zcode 研究笔记

> 仓库: ZCode（Z.ai/智谱官方 AI 编程工作台开源仓）· 本地克隆: `D:/CODE/ai/zcode`
> 语言栈: TypeScript monorepo（pnpm）· 血缘: Z.ai 官方产品，交互/概念体系（hooks 事件名、SKILL.md、agent frontmatter、rewind、plan mode）与 Claude Code 高度同构但为自研实现，非 fork；依赖 Vercel AI SDK（带仓库 patch）；捆绑 superpowers 插件（MIT）
> 分析基准 commit: `29628c9`（2026-09-23，v3.14.3，2026-10-11 首轮）→ `3c5b2276`（2026-10-20，v3.16.2，2026-10-21 刷新）→ `a9fe401e`（2026-11-03，v3.17.2，2026-11-07 刷新）
> 范围: 特性设计面（不逐文件流水账）；行号均实际读过源码

## 定位与血缘

Z.ai（智谱海外品牌）/BigModel 的官方 AI 编程工作台：桌面（Electron）+ 浏览器 Web + 终端三形态，单一 `zcode` 命令分流（无参→TUI，`--web`→Web，其余→Agent CLI），v3.16.2，Apache-2.0。商业闭环 = 官方 Coding Plan 订阅（内置 GLM-5.3 / GLM-5.3-Flash），模型请求统一改发 `zcode.z.ai` 平台网关。

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

## 2026-10-21 刷新：v3.14.3 → v3.16.2（CLI 面）

> 刷新方式：`git pull --ff-only origin main`，`29628c9` → `3c5b2276`（3 个提交）。
> 版本轨迹：v3.15.1（`aac47556`，2026-10-10）→ 2026-10-19 源码同步（根版本 3.16.1）→ 2026-10-20
> 源码同步（根版本 3.16.2，CLI 工作区 `apps/zcode-cli` 0.17.0）。公开快照已不再是单提交：文件数
> 7,060 → 18,266，CLI 包 1,518 → 5,248 个文件，`docs/`、测试与 `third-party/` 随快照入库；因此
> CLI 包 diff 的 61 万行增量主要是快照补齐，本节只列读过源码/设计文档的实质 CLI 变更。

### 旗舰：ToolSearch / DeferredToolCall（3.16.x 首次可见）

> 直接对口 fork workflow-pi-port P2 T2（延迟工具声明 + `search_tools`）。实现细节、失败教训与
> 与 fork 设计原型的逐条差异见
> [`../../workflow-pi-port/deferred-tool-exposure.md`](../../workflow-pi-port/deferred-tool-exposure.md) §2.3。

- **模式**：CLI `--toolsearch-mode auto|native|local|off`，缺省 **off**；只允许配 `--prompt` /
  `--target` / TUI，启动时定型、不写 session/用户配置，子 Agent 继承
  （`packages/cli/src/run.ts:446,526,534`）。App 设置另有 `auto|local|off` 三值：新建/冷恢复读取、
  热恢复保持原值（`docs/design/v2/tool/19-tool-search.md`「App Settings 模式」）。
- **Native**：仅 Anthropic Messages；请求带 `defer_loading: true` 与
  `{type:"tool_reference",toolName}` 内容块，beta header `advanced-tool-use-2025-11-20`；Coding Plan
  默认启用（`packages/adapters/src/model/tool-search.ts:7-12`）。
- **Local**：与 provider 无关；两次 compact 之间固定发送常驻工具 + `ToolSearch` +
  `DeferredToolCall`，搜索命中的 schema 在真实 tool_result 位置展开成 `<functions>` 文本，
  **不向 tools 数组加 schema**；成功 compact 后把已发现的 deferred 工具提升为普通 tools。
- **deferred 判定**：常驻名单 Agent/AskUserQuestion/Bash/Edit/Glob/Grep/Read/ReportFindings/
  Skill/ToolSearch/Write；其余内置工具在 metadata 声明 `shouldDefer: true`；MCP 默认延迟，
  server 可配 `alwaysLoad`（含 `_meta["anthropic/alwaysLoad"]`），每工具可给 `searchHint`。
  判定入口 `packages/core/src/tool/search/exposure.ts:42-49`（排除控制工具/provider 原生/alwaysLoad）。
- **搜索**：`ToolSearch(query, max_results=5)`；支持 `select:A,B` 精确选择、关键词、`+required`
  必含词、MCP 原始/规范化短名；按名称分词与 coarse 段、searchHint、description 加权排序，
  稳定去重（`packages/core/src/tool/search/search.ts:25-110`）。
- **公告**：声明态变化时追加 `<system-reminder>` 风格附件，正文只含 added/removed/readded
  名称增量；metadata 持久化增量、compact 后按保留历史重建；删除按完整作用域目录判断，
  超过 30 个名称按 MCP server 分组摘要，并带 ambient context 提示；不推断 MCP 连接状态
  （`exposure.ts:146-220`）。
- **执行包装**：`DeferredToolCall {name,input}` 严格 schema；调度前解包为目标调用，复用同一
  callID 与普通调度的准入/审批/Hook/超时/取消/结果/媒体/turn-stop；只接受当前请求允许的
  deferred 目标，常驻/控制/provider 原生工具拒绝并提示直接调用；内部历史存目标名 +
  `modelCallWrapper` 标记，请求出口恢复原始包装（`packages/core/src/tool/deferred-call.ts:15-60`）。
- **持久化/冷恢复**：发现集合只从真实成功的 `tool_reference` 结果与有效 compact 检查点推导，
  不存 schema 快照；公告正文与增量 metadata 随隐藏 synthetic 历史持久化，fork/rewind/resume
  原位恢复。修复过的坑：公告在请求投影之后生成会反复累积进主历史；compact 只保留尾部时
  全量 metadata 与增量正文不一致；deferred 转 alwaysLoad 后误报删除（代码注释
  `exposure.ts:134-175`）。
- **验证口径**：设计文档自报 43 文件/678 项定向回归、macOS Local CLI/PTY E2E 5/5；明确未跑
  Windows/Linux 与真实缓存命中，模拟 provider 成功不等于真实模型能力。

### 其他 CLI 改进（按面归类）

| 面 | 变更 | 落点 | 对 fork 的意义 |
|---|---|---|---|
| 工具 | **ApplyPatch** 结构化多文件补丁（3.15.1） | `core/src/tool/handlers/apply-patch.ts`；`handlers/index.ts:86` 仍注释未注册 | ✅ fork 已有 Codex `apply_patch`（`xai-grok-agent/src/config.rs:345`、registry `types.rs:747`），不移植 |
| 工具 | **FillWorkflowHole**：运行中的 dynamic-workflow 停在留白，模型补函数体后继续，run id/journal/子 agent 不变 | `core/src/tool/handlers/fill-workflow-hole.ts:299`；描述 `fill-workflow-hole-description.ts` | 🟢 P2 候选；fork 是 Rhai + journal，可借「留白 host call / 只补一支 / 诊断回填文件」的交互形态，不能直接搬 TS 编译 |
| 工具 | **ReplyToChannel**：飞书/Lark 会话内补发消息与原生 @ | `core/src/tool/handlers/reply-to-channel.ts` | ❌ 不移植（服务端票据 + 渠道身份） |
| 工具 | **Bash 图片输出**：stdout data URL 经 magic 校验 + 缩放后作为 image block，失败回退文本 | `core/src/tool/handlers/bash-image-output.ts` | 🟡 P2 候选；fork 的视觉桥已覆盖 read_file/PDF/工具图，Bash 截图输出是缺口 |
| 工具 | **guarded 命令族**：cmd/find/git/windows-delete 等统一审批生命周期 + 参数调整 | `core/src/tool/handlers/guarded/` | 🟢 参考；fork 已有 tree-sitter 三态 + 审批，重点看 Windows 删除命令覆盖 |
| Bash | **Shell provider**：session 持久化选择（POSIX `$SHELL` / Windows Git Bash / CMD）、startup source、cwd capture、background 不更新 cwd | `adapters/src/exec/bash-shell-provider.ts`、`docs/bash-shell-provider.md` | 🟡 P2 参考；fork 已有 Windows shell 探测，缺「session 快照 + 失效回退 + 变更提示」 |
| Bash | **AST 权限语义**：`unbash` 纯 TS AST + 命令级 safeFlags，动态结构 fallback open；prefix registry 与 readonly 分层 | `docs/bash-tool-ast-permission-semantics.md` | ✅ fork 已有 tree-sitter 静态分析；其 data-driven safeFlags/policy table 可作规则库参照 |
| MCP | **资源订阅**：`(server,uri)` 引用计数，0→1 订阅 / 1→0 退订，重连重放，`resources/updated` 分发 | `adapters/src/mcp/resource-subscriptions.ts:7` | 🟡 P3；fork 已有 resources list/read + list_changed，缺 subscribe/updated |
| MCP | **MCP Apps / UI 工具**：页面沙箱提供的工具 `app__server__tool`，页面关闭即失效；sampling/elicitation 桥 | `core/src/mcp/app-tools.ts:47`、`bootstrap/src/zcode-protocol/mcp-ui/` | ❌ 不移植（终端无托管页面宿主）；`alwaysLoad` 元数据可并入 deferred |
| 记忆 | **Dream + LLM recall selector**：24h 最小间隔 + 10min 扫描节流 + 锁；selector 每次最多 5 条、粘性会话、对 user/project 画像保守 | `core/src/memory/dream.ts:18,40`、`recall/selector.ts:70` | ✅ dream 已有（`xai-grok-memory` + `memory_dream.rs`）；LLM selector 是 🟡 P2 参考（fork 走 embedding/MMR） |
| 协议 | **Endpoint routing / request security / account retry**：按 provider 端点路由、传输保护与请求观测、401 PAT 刷新一次并校验 scope 指纹 | `adapters/src/model/endpoint-routing-edition/`、`request-security.ts`、`account-request-retry.ts` | 🟢 P2/留档；多与官方账号体系耦合，fork 的 BYOK 只需吸收「重试前刷新凭据 + scope 校验」 |
| 会话 | **Session execution residency lease / protocol v4**：多端共享 runtime、输入准入、canonical intent replay、highspeed metrics | `bootstrap/src/session-execution/residency-lease.ts`、`zcode-protocol-v4/` | 🗂 留档；fork leader 模式已有同域问题，无直接移植面 |
| 上下文 | **Agent listing delta / runtime reminder 持久化 / workflow blocked reminder** | `core/src/agent/agent-listing-metadata.ts`、`runtime/methods/runtime-reminder-persistence.ts` | 🟢 P3 参考；fork subagent listing 可按增量公告补 |
| 插件 | **官方插件 seed 体系**：13 个新插件包、`requiredSeedPaths` 完整性校验、内容 hash 缓存、`defaultEnabled`、i18n listing、`userConfig` 占位符 | `bootstrap/src/app/official-plugin-definitions.ts`、`bundled-plugins.ts` | 🟢 P2/P3 参考；fork 已有 marketplace，缺「随包内置 + 完整性校验 + 默认启用」这套 |
| 工程 | **`skills-lock.json`**：外部技能按 source + computedHash 锁定 | 仓库根 `skills-lock.json` | 🟢 P3 参考；fork 的 bundled/plugin skills 无锁文件 |
| 工程 | architecture policy（≤400 行/文件、禁环）、formal-proof、prompt-trajectory | `architecture-policy.yaml`、`packages/formal-proof/`、`tools/prompt-trajectory/` | 🗂 留档；formal-proof 思想可借 rewind/压缩夹具 |

### 内置技能与官方插件（随 CLI / 官方市场分发）

> 区分三类：① `packages/bundled-skills`（随 CLI 必装、不可卸载，目前只有 dynamic-workflows）；
> ② 官方插件（市场 seed + 默认启用位）；③ `superpowers` 第三方插件包（MIT，随仓库源码提供）。
> 技能格式与 fork 的 SKILL.md 体系兼容，**技能正文可移植，插件壳/UI 面多不可移植**。

| 资产 | 内容 | 许可 | 判定 | 说明 |
|---|---|---|---|---|
| `superpowers-plugin` 5.1.0 | 14 个开发方法论技能 + SessionStart hook（`using-superpowers` 注入） | MIT（Jesse Vincent） | 🟡 **P1 技能候选** | brainstorming / writing-plans / executing-plans / TDD / systematic-debugging / requesting+receiving-code-review / verification-before-completion / subagent-driven-development / dispatching-parallel-agents / using-git-worktrees / finishing-a-development-branch / writing-skills；与 fork 技能体系同构，hook 的平台分支需改；`using-superpowers` 的「每次会话先读」语义要先评估 |
| `skill-creator-plugin` 0.1.0 | 技能编写/测试/迭代循环，含触发描述写法与渐进披露 | MIT | 🟢 P2 可吸收 | fork 已有 `create-skill`，缺「2–3 个真实测试提示 + 结果与轨迹一起评审」的评估闭环 |
| `plugin-creator-plugin` 0.1.1 | 插件 manifest/脚手架/本地测试市场/安装 handoff | MIT | 🟢 P2 可吸收（需改写） | fork 有 `xai-grok-plugin-marketplace`，缺面向作者的 skill；UI 步骤（Plugin Marketplace → Add）要换成 fork 的 `/plugin` 面 |
| `zcode-guide-plugin` 0.3.0 | 6 个自诊断技能：commands/hooks/mcp/plugins/skills + 配置指南 | MIT | 🟡 P2 候选 | 与 fork `/doctor`、`/mcps`、`/plugin`、`/lsp` 面高度对应；文案与命令需改成 grok |
| `bundled-skills/dynamic-workflows` | 1,607 行 SKILL + 794 行 patterns + 1,110 行 examples，TS 工作流作者契约 | 随仓库 Apache-2.0 | 🗂 不直接移植 | Rhai API 不同；可借「拓扑图册 / 留白 §15 / report salvage / 后台 run」的章节结构重写 fork 的 workflow skill |
| `visualize-plugin` 0.1.0 | 对话内联 HTML/Gen UI 可视化 | Apache-2.0 | ❌ 不移植 | 依赖桌面 Gen UI surface |
| `documents` / `pdf` / `presentations` / `spreadsheets` 0.2.x | docx/pdf/pptx/xlsx 生产技能 + 查看面板 | Apache-2.0 | 🟡 部分重叠 | fork 已内置 docx/pdf/pptx 技能；xlsx 技能可单独评估；查看面板不适用 |
| `browser-use-plugin` 0.5.1 | headless CDP + node_repl 浏览器自动化 | MIT | 🗂 留档 | fork 有 web 工具/MCP，无 node_repl 宿主；场景重叠、成本高 |
| `zcode-cua-plugin` 0.6.5 | Computer Use SDK/skill（依赖 node_repl host） | Apache-2.0 | ❌ 不移植 | 桌面 GUI 自动化，终端场景外 |
| `android-emulator` / `ios-simulator` | 移动端构建/模拟器 MCP + 技能 | MIT | ❌ 不移植 | 平台特化，重负载 |
| `image-search-plugin` 0.1.1 | 官方搜图 HTTP MCP | Apache-2.0 | ❌ 不移植 | 绑定 zcode 服务端鉴权 |
| `restore-legacy-sessions-plugin` 0.1.0 | 旧 ZCode 会话恢复 | MIT | ❌ 不移植 | 产品迁移特化 |

### 移植判定汇总（本轮）

- **P1（直接进现有专题）**：ToolSearch/DeferredToolCall → workflow-pi-port P2 T2 第四参照；
  `superpowers` 14 技能 → skills-only 引入评估（先挑 TDD/systematic-debugging/verification 三件做 A/B）。
- **P2（排期候选）**：FillWorkflowHole 交互形态、Bash 图片输出、MCP 资源订阅 + `alwaysLoad`、
  zcode-guide 诊断技能、skill-creator 评估闭环、plugin-creator（改写 UI）、LLM memory selector。
- **P3/留档**：官方插件 seed 完整性体系、agent listing 增量公告、residency lease、
  skills-lock、architecture policy、formal-proof。
- **不移植**：ReplyToChannel/Feishu、MCP Apps/Gen UI 视觉面、office 查看面板、CUA、
  Android/iOS 模拟器、image-search、restore-legacy-sessions、off-peak 票据。

### 版本归因（避免把旧特性当新增）

- 3.15.1（`aac47556`）：ApplyPatch、FillWorkflowHole、ReplyToChannel、memory dream、protocol v4、
  官方插件包与插件 seed 体系、UI 插件/Gen UI。
- 3.16.x（`0a1fc56d` / `3c5b2276`）：ToolSearch/DeferredToolCall、session execution/residency
  lease、off-peak runtime、tool-result-settlement、channel-reply 模块化、插件市场更新。
- 早于 3.14.3 已有：Bash 图片输出（基线提交即存在）、bash AST 权限。

## 2026-11-07 刷新：v3.16.2 → v3.17.2（CLI 面）

> 决策摘要与待办台账见 [`../../zcode-317-port/README.md`](../../zcode-317-port/README.md)；
> 本文只做机制逐条与源码锚点（单一事实源）。

> 刷新方式：`git pull --ff-only origin main`，`3c5b2276` → `a9fe401e`（2 个提交：
> `a60ba6a7` 无版本标题 + `a9fe401e` v3.17.2）；新增 tag v3.17.1 / v3.17.2；根版本
> 3.17.2、CLI 工作区 0.17.1。diff 5,315 文件 / +226,212 / −613,001 行，仍是公开
> 快照重排为主（git 明确告警 rename detection 被跳过），**用 `git ls-tree` 双版本树
> 交叉核对才判定新增资产**，别信 A/D 标记——本轮插件包没有发生目录迁移。

### 协议 / 请求形态（对 fork 最直接）

| 机制 | 落点 | 对 fork 的意义 |
|---|---|---|
| **provider-builtin-tool-compat**：GLM 服务端内置工具（`analyze_image`/`webReader`）把多轮合成单条 assistant 下发成 markdown 伪装调用头 + 结果副本，SDK 会丢块、模型「照着文字假装调用工具」；按结构改写成原生 `mcp_tool_use`/`mcp_tool_result`（`server_name: zcode-provider-builtin`）并删重复 markdown | `adapters/src/model/provider-builtin-tool-compat/compat.ts:14-23,96-162,229-271`；真实脱敏流 fixture `adapters/tests/fixtures/provider-builtin-analyze-image.sse`（glm-5.3） | 🟡 **条件性 P1**；纯协议变换零 IO。fork 的 `server_tool_use` 是 headless 里 Grok 原生 web_search 内联块解析，不是这类「伪装调用改写」；BYOK 接国模网关时可用 `[api_dump]` 取证再决定 |
| **provider-tool-rounds**：服务端执行的内置工具按轮切分（独立 reasoning/text、工具卡 Scheduled→Started→Result、`providerExecuted:true`、回放 `assistant(text,tool_use)+tool(result)`、未收口轮补错误）；`compactGroupKey` **必须取持久化 assistant 消息 id**（相邻响应会复用 tool call id），有轮次的响应**不挂 tokens**（服务端 usage 是多轮合计，会高估一倍） | `core/src/runtime/provider-tool-rounds/CONTRACT.md:1-37`、`rounds.ts:15-26,47-79` | 🟢 **P1 参照**；三处不变量（分组键、回放形态、usage 归属）直接决定压缩后请求一致性，对准协议方言 + 前缀缓存 |
| **requestCapabilities / foldReminders**：仅 `anthropic-messages` 把 system-reminder 折进 tool result，OpenAI 兼容协议保持独立 user 消息 | `adapters/tests/request-capabilities.test.ts`、`core/src/runtime/helpers/provider-request-messages.ts:61,85-87,339`、`core/tests/fold-reminders-tool-error.test.ts` | 🟢 **P1 参照**；按协议固定请求形态，避免同一会话中途变形破坏前缀 |
| **post-compact-file-restore**：压缩后按 `readFileState` 从新到旧重读磁盘补回文件内容（≤5 文件 / 单文件 5k / 总 50k tokens），跳过其他写入者与排除路径 | `core/src/runtime/helpers/compact-post-reminders.ts:27-30,44-56`、`core/tests/post-compact-file-restore.test.ts` | 🟢 **P1 参照**；fork 已有压缩 + read 一致性面，这是压缩闭环的必要配套 |

### 运行时状态 / 工具

- **tool-search 3.17.x 有实质改动（不是新协议）**：新增 managed 模块 `core/src/tool/search/`（CONTRACT.md + contract/module）；`description.ts` 增约 90 行 `renderToolSearchAnnouncement`，公告改为**以 metadata 为唯一事实源**渲染 `RuntimeAttachmentEntry`，模式切换只重渲染文案、不替换历史任意文本；新增 `hiddenNames` 区分「本轮隐藏」与「MCP 断连」，收紧关键词正则语义（明确不支持正则）；`exposure.ts`/`history.ts` 相应重构。→ 给 fork workflow-pi-port P2 T2 补两条施工坑，见 [`../../workflow-pi-port/deferred-tool-exposure.md`](../../workflow-pi-port/deferred-tool-exposure.md)。
- **interrupt-marker**：用户停止后给模型的固定标记按请求阶段 / 工具阶段区分，被取消工具统一改写 `USER_STOPPED_TOOL_RESULT_CONTENT`，冷恢复按 source 还原（`core/src/runtime/methods/interrupt-marker.ts:11-16,24-36,43-69`）。
- **hook `onError`**：**不是第八个事件**，是 `PreToolUse` 的新配置字段 `HookOnError = allow|deny`，缺席 = fail-open；仅 hook 自身运行出错时生效，`deny` 等价退出码 2；用户中断 / 未信任工作区 hook / async 不受影响（`core/src/hooks/types.ts`、`runner.ts:219,513-526`、`configured-runner.ts:112,169`、`core/tests/hook-on-error.test.ts`）。七事件枚举未变。
- **runtime-context-announcements**：compact 后的上下文重述与公告（`core/src/runtime/methods/context-refresh.ts:261,294,311`）。
- **session-features header**：会话级能力标签 `Set` 只增不减（compact/rewind/冷恢复不撤销、子会话不继承、workspace scope 不发），出站写 `x-zcode-session-features`（排序去重、每标签 `^[a-z0-9_]{1,64}$`、总长 ≤256、非法整头省略），provider 静态同名头一律剔除；标签可从历史 best-effort 推导（`docs/design/v2/model/session-features-header.md:1-217`、`core/src/runtime/session-features/session-features.ts:47-62,202-236`）。架构通用，词表绑官方统计口径 → 🟡 设计参照。
- **工具 handler 本轮全是 M、无新增文件**：`handlers/index.ts` 注册 `cronPauseSelfToolEntry`、注释掉 `replyToChannelToolEntry` 默认注册、子 runtime 移除 `RUN_INTROSPECTION_TOOLS`；`tool-search.ts`/`deferred-tool-call.ts` 仅 2–6 行小改。

### MCP / 会话 / 多端（多为官方形态绑定）

- **workspace-mcp-trust**：`inspect/decide/allows/guard` 四方法，工作区级 MCP 声明精确摘要比对 + 授权原子写 + **连接准入重验 + `guard` 包住整个 lease**，未知/陈旧/损坏默认拒绝（`bootstrap/src/workspace-mcp-trust/CONTRACT.md:1-16`、`contract.ts:6-15`）→ 🟢 安全内核可吸收，与 §7.1.10 workspace hook 信任摘要同族。
- **session-lifecycle**：`unloaded/starting/ready/stopping/faulted` 五态 + 每 session 唯一写者 + 并发矩阵 + 清理失败进 faulted（`docs/design/v2/session-runtime-lifecycle.md:1-216`、`bootstrap/src/session-lifecycle/controller.ts:47-67,142-207`）→ 🗂 绑 app-server/V4，TUI 单进程只需借状态机。
- **mcp-node-fallback**：仅 Electron Node 下给 stdio MCP 注入 node/npm/npx shim（PATH 末尾追加 + 两个 env var，Windows `Path`/`PATH` 大小写归一）→ ❌ Rust 原生 TUI 无此运行时。
- **official-command-endpoint**（UDS/命名管道 + 每 session token 窄代理，只放行 `/api/v1/sites`，CLI 不持凭证）/ **local-ttft**（V4 CommandInbox 计时）/ **input-prediction-sidecar**（手机远控 ghost hint，近 6 轮、3 候选、confidence≥0.8、20s 超时）/ **conversation-quote-context**（飞书/Bot 引用材料，不混 canonical 正文）→ ❌ 全绑官方服务端与多端。
- **node-repl-call-ledger**：每次 `js` 调用随机 token，facts 只由主线程 broker 写入，组装结果先删模型可写 `_meta` 再回写，防 Worker 冒充（`docs/design/v2/tool/20-node-repl-call-ledger.md`、`node-repl-host/src/call-ledger/ledger.ts`）→ ❌ fork 无 node_repl 宿主。
- **dynamic-workflow** 仅静态分析增强：`causality-order-bindings.ts` 解析「经参数应用」的回调绑定，把不动点合并的回调换成当前内联调用实际绑定的函数，支撑递归预计算；配 6 个 `shared-helper-*` 图 fixture → 🗂 绑 TS AST，Rhai 侧只留档。

### 工程治理：CONTRACT.md 契约四件套（本轮最值得拿走的东西）

全仓 `34 A + 23 M` 个 `CONTRACT.md`。每个 managed 模块 = `CONTRACT.md`（**只记类型与测试无法表达的不变量**：owner、状态单一事实源、异步时序）+ `contract.ts`（唯一公开入口：branded id / zod schema / typed commands/events/errors / port 接口）+ `module.ts`（`id / requires / provides / publicEntrypoints`，注释要求与 policy.yaml 同步）+ `contract.example.ts` + `contract.test.ts`（CLI/CUA 例外保留，App 单测已退役）。三层机器校验：① `architecture-policy.yaml` 登记 roots/requires/publicEntrypoints/layers/layerOrder/owner，全局阈值 `maxFileLines 800`、`maxContractLines 300`、`maxPublicMethods 12`、`forbidCycles`、`forbidDeepImports`、`newFilesEnforcedSince 2026-09-10` + 精确路径 exceptions；② `scripts/architecture/policy.mjs` + `violations.mjs` 实现 13 条规则（module-dependency / deep-import / cycle / layer-direction / missing-module-artifact / manifest-policy-mismatch / disable-count 等），`.architecture-baseline.json` 只压存量、**新增违规 blocking**；③ `.husky/pre-push` → `verify:pre-push` → `test:architecture` + `architecture:check --changed`，另有 `pnpm architecture:adopt <id> --root <dir>` 从真实 import 推断 requires 并 scaffold 四件套，`.agents/skills/architecture-governance/SKILL.md` 把「先契约后代码」写成 agent 工作流。→ 🟢 **跨语言可移植**：Rust 侧用 trait/newtype/typestate 承载 `contract.ts` 的公开面、`pub(crate)` + workspace `Cargo.toml` 天然实现 publicEntrypoints/禁深导入，把依赖白名单、方法数上限、循环依赖做成自建 architecture-check + baseline，CONTRACT.md 当与代码同目录的「人类不变量清单」。

### 插件 / 技能生态（本轮几乎没动）

- **`zcode-site-plugin` 是唯一真正新增的插件包**（50 文件全 A，`git ls-tree` 双版本树核对；`3c5b2276` 无此路径，其余 16 个插件包已在 `apps/zcode-cli/packages/`）：`zcode-site` v0.4.2，1 技能 `site-deploy`（5 references + 4 templates）+ 7 个 `/site-*` 薄命令 + MCP `sites` + Sites UI 面板；`site-deploy` 规定 `.zcode-site/stage/` + `start.sh` → `zcode-site check` → `deploy`，云运行时硬绑 Debian 12/x86-64、Node 24/Python 3.12、`0.0.0.0:3000`、90s 健康检查、包 ≤200MB，错误码含 3611/3612/3608（内容审核/付费 badge）；`src/server/api.ts` 的 `origin` 直注 "zcode-server origin"，带官方鉴权头 `com.zcode/official-mcp-auth`。❌ 不移植，可借的只有「stage + 本地 check + 上传前显式确认」交互。
- **技能数量零变化**：superpowers 14（MIT）、zcode-guide 6（MIT）、bundled-skills 1（dynamic-workflows）两版完全一致；根与 CLI 的 `skills-lock.json` blob hash 未变；`docs/superpowers/specs/2026-10-24-project-management.md` 是桌面「设置→项目」页规格，**不对应新技能**。20 个 M 中最大的是 `bundled-skills/.../dynamic-workflows/SKILL.md`（+21/−6）；`zcode-guide/diagnosing-commands` 把 `!\`...\`` 从「执行并粘贴」改成「重写为让模型用 Bash 工具执行的指令」（对应 `custom-command-shell-expansion.ts` 删除）。office 四件 / visualize / browser-use / CUA / 移动端插件均只有 1–2 行修订或版本 bump。
- **官方插件 seed 体系**：`official-plugin-definitions.ts:431-471` 唯一新增项 `zcode-site` v0.4.2（`defaultEnabled: true`、`requiredSeedPaths` 10 项）；`bundled-plugins.ts` 的 `findMissingOfficialPluginSeedPaths()` 差集非空即抛 `ZCODE_PLUGIN_SEED_INCOMPLETE`、拒绝写缓存并回退旧版缓存，逐文件 SHA-256 比对 + `.zcode-plugin-seed.json` marker + 临时目录 rename 原子替换 + 备份回滚 + 15s 共享 seed lock + 顶层白名单裁剪 + hook 脚本 0755 → 🟢 上一轮 §8.1.10 的「缺件拒装」有了完整实现参照。
- **新增斜杠命令**仅 `zcode-site-plugin/commands/` 的 7 个 `/site-*`；`config/provider/regression/`（`providers.example.json` + `test-list.json`）与 `tools/provider-regression/`（26 文件，带四件套）是模型回归夹具（TEXT-001/TOOL-001/REASON-001、repetitions、timeoutMs、concurrency），不是新配置体系。

### 移植判定汇总（本轮）

- **P1（并入既有专题，不另立项）**：tool-search 3.17 公告/投影修正 → workflow-pi-port P2 T2 第五参照；provider-tool-rounds + foldReminders + post-compact-file-restore → 协议兼容 / 压缩轮；`onError` fail-open/deny + interrupt-marker → hooks/stop 小件轮；**§8.4 的 superpowers 三件 A/B 仍未做**（本轮核实 fork 里 `superpowers` 只出现在插件市场测试 fixture，无 bundled 技能）。
- **P1/P2（条件触发）**：provider-builtin-tool-compat（BYOK 网关真出现 markdown 伪装内置工具时，用 `[api_dump]` 取证）、workspace-mcp-trust 的 `allows`/`guard` 内核（与 §7.1.10 合并）、CONTRACT.md 契约治理（工程资产，成本中等但长期收益最大）、官方 seed 完整性门禁。
- **🗂 留档**：session-features header 架构、session-lifecycle 五态机、causality-order-bindings、provider-regression 夹具。
- **❌ 不移植**：zcode-site-plugin 全家、mcp-node-fallback、official-command-endpoint、local-ttft、input-prediction-sidecar、conversation-quote-context、node-repl-call-ledger、桌面/远程/数据同步那一大堆 `docs/issues`（101 个新文档里 CLI 设计文档只有 5 个）。

### 版本归因（避免把旧特性当新增）

- 3.17.1 / 3.17.2（`a60ba6a7` / `a9fe401e`）：provider-builtin-tool-compat、provider-tool-rounds、session-features header、workspace-mcp-trust、session-lifecycle、CONTRACT.md 契约治理铺开、zcode-site-plugin、tool-search 公告重构、input-prediction/local-ttft、dynamic-workflow 因果绑定分析。
- 早于 3.17 已有：ToolSearch/DeferredToolCall、FillWorkflowHole、ApplyPatch、Bash 图片输出、插件 seed 框架、superpowers 14 技能、skills-lock。

## 版本与活跃度

- 版本 **v3.17.2**（根 `package.json`；CLI 工作区 `apps/zcode-cli/package.json` 0.17.1；README
  更新日志 2026-11-03）。
- 公开快照：v3.14.3（`29628c9`）→ v3.15.1（`aac47556`）→ 2026-10-19 / 10-20 两次
  `chore: sync ZCode source`（`0a1fc56d` / `3c5b2276`，v3.16.2）→ 2026-10-30 / 11-03
  （`a60ba6a7` / `a9fe401e`，v3.17.1 / v3.17.2）；仓库已从单提交快照变为持续同步，
  文件数 7,060 → 18,266 → 5,315 文件变更的一次大重排（+226k / −613k 行，勿按行数估特性）。
- 官方产品在营（zcode.z.ai 网关、官方插件市场 CDN、Feishu/Discord 社区），**活跃维护中**。

## 存疑 / 仅文档宣称

- Computer Use 在开源仓是不可用占位（`packages/zcode-cua/index.js:1` 起返回 "not available"，与 NOTICE.md 一致）。
- 桌面定时任务、闲时任务的完整服务端行为在客户端仓内只能看到接口面，服务端实现不在本仓。
- ToolSearch 的 Native 依赖 Anthropic Messages 与 zhipu Coding Plan 账号；设计文档自报未跑
  Windows/Linux 与真实缓存命中，Local 模式在普通 BYOK 国模上的收益与误用率未量化。
- `superpowers-plugin` 在仓库内有完整 14 技能源码与 MIT LICENSE，但不在 CLI bootstrap 的
  官方插件 seed 表里；它的分发走插件市场，不能当作「CLI 首启即装」的内置项。
- 插件 UI / Gen UI / office 查看面板只在桌面与 Web 形态可用；CLI TUI 只能消费其中的技能正文。
- 3.17.x 的 provider-builtin-tool-compat / provider-tool-rounds / session-features 的证据主要是
  仓库内单测、脱敏 SSE fixture 与设计文档，**未跑真实多网关 BYOK**；「服务端内置工具按轮回放」
  的收益只在官方 Coding Plan 链路上自证，别家国模网关是否下发同形态需 `[api_dump]` 现场取证。

## 刷新方法

```bash
git -C D:/CODE/ai/zcode fetch --all --tags
git -C D:/CODE/ai/zcode pull --ff-only origin main
git -C D:/CODE/ai/zcode log --oneline <old>..HEAD
git -C D:/CODE/ai/zcode diff --stat <old>..HEAD -- apps/zcode-cli
```

快照同步会让 diff 极大（10-21 那次 61 万行、本次 +226k/−613k 行），先按
`apps/zcode-cli/packages` 的新增目录/文件与 `docs/design` 归类，再读源码；不要按 diff 行数估
特性量。git 会因文件数过多跳过 rename detection（告警 `renameLimit`），**判定「某资产是否新增」
必须 `git ls-tree -r <old> | grep <path>` 与 `<new>` 交叉核对**，只看 A/D 标记会把目录重排误判成
新插件。版本号看根 `package.json`，CLI 工作区版本看 `apps/zcode-cli/package.json`。
