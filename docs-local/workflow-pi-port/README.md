# workflow × pi 特性移植 — 预研选型

> **状态：预研选型完成，未开工。** 两个选型特性（canonical context edit / 延迟工具声明）
> 已有设计原型，见同目录分文档；其余缺口已定性、给出做/不做的决策记录。
> 调研快照：2026-09-30；fork 基线 `95aad87`（main）；pi 基线 `earendil-works/pi`
> `0.99.1`（2026-09-29 发布，`0.99.0` 为 codemode+MCP 大版本）。

## 用途与范围

回答一个问题：**给本仓库的 `xai-workflow`（Rhai 编排引擎）移植 pi 近两个月的新特性，
能不能提高上限、移植哪些**。只覆盖与 workflow/编排相关的主线特性（pi `0.84.0`–`0.99.x`，
2026-07 至 2026-09）；主题/终端体验/登录体系类不调研。

可信度分级：

- **A（实测/读码核实）**：fork 侧全部结论（读的本地代码，锚点随文）；pi 侧特性语义
  （读的官方 CHANGELOG 与 docs/cli.md、docs/mcp.md 原文）。
- **B（官方文档转述）**：pi 的实现细节如 BM25 排名、QuickJS 沙箱约束、1 MiB 输出通道
  ——来自 changelog/doc 描述，未读 pi 源码验证。
- **C（推断）**：移植后的收益估算、人天粗估。

## 结论（TL;DR）

1. **底层编排原语不需要从 pi 借任何东西**：`agent()/parallel()/journal 重放/预算
   reserve-release/await_user` 这套是本仓库领先 pi 的地方（pi 的 subagent 无确定性
   resume 语义）。
2. **真正限制 workflow 上限的是三件事**，对应三个缺口：
   - 编排上下文膨胀（→ **canonical context edit**，P1，`canonical-context-edit.md`）
   - 子 agent 工具面全量声明（→ **延迟工具声明 / exposure 分级**，P2，
     `deferred-tool-exposure.md`）
   - 成本（→ prompt cache warming，P2，sampler 层，与 workflow 解耦）
3. **codemode 缺口最大但不移植**：QuickJS 会引入第二脚本运行时，破坏 journal hash 的
   确定性模型（见决策记录 D1）；可吸收的是两个思想（按分支持久 KV、大输出旁路通道）。

## pi 近两月特性 × fork 现状对照矩阵

| pi 特性 | 版本 | fork 现状 | 判定 |
|---|---|---|---|
| MCP（stdio + streamable HTTP + OAuth） | 0.99.0 | `xai-grok-mcp` 基于 rmcp 全有：双传输、OAuth discovery+浏览器授权+刷新、401 重试、进程组收割（`servers.rs`）；会话内 `/mcp` | ✅ 已有 |
| codemode（QuickJS 模型写 JS 调工具） | 0.99.0 | 无。唯一脚本是 Rhai，且只在 workflow 引擎（编排者写、非模型写、无工具调用） | ❌ 不移植（D1） |
| tool_search / 延迟工具声明 | 0.99.0 | 有 `ToolSearch→search_tool` 兼容映射（`builder.rs:3016` 测试）但那是 allowlist 场景；缺整个"延迟声明"维度 | ⚠️ **选型 P2** |
| 工具 exposure 分级（direct/model-only/codemode/deferred/hidden） | 0.99.0 | `tool_name.rs` 只有 admission 校验，无 exposure 维度，MCP 工具全量声明 | ⚠️ 并入 P2 |
| virtual models（按请求路由物理模型） | 0.99.0 | `AgentOpts.model/effort` per-agent 已覆盖主场景 | 不需要 |
| classifier models（Jev next-token 分类） | 0.99.0 | 无对应物 | P3，等 P1/P2 落地后有场景 |
| extension 运行时 API（ctx.executeTool/nestedCalls 等） | 0.99.0 | hooks/plugins 是事件钩子，非运行时可编程 | 不移植 |
| canonical session context（ContextEditEntry） | 0.87.0 | compaction 是"重写历史"路线，与 journal 重放确定性相斥 | ⚠️ **选型 P1** |
| prompt cache warming | 0.86.0 | 无；已有 `docs-local/prompt-cache-notes.md` 实测地基 | P2，sampler 层独立做 |
| cache-friendly 动态工具加载 / constrained sampling | 0.84.0 | 无 | 观察，随 P2 顺带评估 |

## 决策记录

- **D1 — codemode 不原样移植**：引入 QuickJS = 第二脚本运行时。journal 的
  `request_hash(kind, payload)` 重放一致性（`engine.rs:268 host_call`）建立在
  "脚本是确定性函数"之上，双运行时让确定性证明面翻倍且无对应收益。
  吸收两个思想：① `store()/load()` 按会话分支持久 KV —— 可做成 workflow host fn，
  比 `write_scratch_file` 语义更精细（按 seq 分支隔离）；② bash 结构化结果的
  1 MiB 原始输出 + `full_output_path` 旁路 —— 配合 `docs-local/tool-output-compression-plan.md`。
- **D2 — exposure 分级不照抄 pi 五档**：fork 没有 codemode 档的需求（D1），收敛为
  三档 `direct / deferred / hidden`，见 `deferred-tool-exposure.md`。
- **D3 — canonical context edit 不走 compaction 通道**：两者语义相反（重写历史 vs
  历史不动），强行复用会把 compaction 的熵带进 journal 重放，见
  `canonical-context-edit.md` §3。

## 分期路线（拟）

| 期 | 内容 | 分支拟名 | 规模（C 级粗估） |
|---|---|---|---|
| P1 | canonical context edit（workflow host fn + prompt 构建层可见面） | `feat/local-workflow-context-edit` | 净开发 4–7 人天 |
| P2 | 延迟工具声明 + exposure 三档（`AgentOpts` + host_service 通道） | `feat/local-deferred-tool-exposure` | 净开发 3–5 人天 |
| P2' | prompt cache warming（sampler 层，与 workflow 解耦，参考 pi 成本感知刷新） | 独立专题，暂不立项 | 待 P1/P2 落地后评估 |
| P3 | classifier 廉价评审节点（`classify` host fn + 模型能力位） | 暂不立项 | 依赖上游有此类模型接入 |

## 来源

- pi CHANGELOG：`packages/coding-agent/CHANGELOG.md`
  （github.com/earendil-works/pi，`0.99.0`/`0.87.0`/`0.86.0`/`0.84.0` 条目）
- pi docs：`docs/cli.md`（Enable codemode / How codemode works / Tool search）、
  `docs/mcp.md`、`docs/session-format.md#contexteditentry`、`docs/extensions.md#tool-exposure`
- pi.dev/changelog（0.99.0 摘要页）

## 分文档索引

- [`canonical-context-edit.md`](canonical-context-edit.md) — P1 设计原型：不改写历史的
  可见上下文编辑（journal 化的 context-edit 事件 + prompt 构建层应用）
- [`deferred-tool-exposure.md`](deferred-tool-exposure.md) — P2 设计原型：三档 exposure +
  BM25 延迟声明（`AgentOpts` 扩展 + host_service 传递通道）
