# kimi-code / zcode / codex 的插件与内置技能：对本机场景的移植评估（2026-11-13）

> 范围：只评估**插件/内置技能**（不改代码），目标 = 稳定性 + 体验。
> 场景：Windows + gash、DeepSeek Flash 主模型（多供应商 relay）、MCP 五台
> （codebase-memory/codegraph/context7/ssh/websearch）、重度中文文档与排障、
> pstack 插件。基线：fork v1.0.74 + 本机已有 45 个用户技能、13 个 bundled 技能。

## 0. 结论

- 三家真正新增、且贴合本机痛点的只有三件：**一个 grok 自诊断/配置技能**（吸收
  zcode-guide 六件 + kimi mcp-config 的诊断部分）、**systematic-debugging**
  （superpowers 改写）、**skill-creator 的评估闭环**（并入现有 create-skill）。
- 其余大多与 fork 已有能力重复（`/review`、`/goal`、`/doctor`、`grok mcp doctor`、
  create-skill/create-workflow、compat 导入），或属于平台绑定（office/CUA/站点）。
- 技能目录本身有成本：当前 50 条、约 16.7k 字符进首条 user 消息。新技能建议 ≤3 个；
  同时值得做一次目录瘦身（9 个 `principle-*` 与多套 review 类技能重叠）。

## 1. 三家实际有什么

### kimi-code（2.5.x，内置 catalog）

| 技能 | 内容 | 对 fork |
|---|---|---|
| `mcp-config` | OAuth 登录 + 三层 mcp.json（global / project-root / project-local）编辑、信任提示 | 大部分已有（grok 原生 OAuth + `grok mcp add`；guide 07 有同类说明）；**诊断决策树**值得吸收 |
| `update-config` | 配置编辑流程 | 与自诊断技能合并 |
| `custom-theme` | 自定义主题 | 低频率；fork 已有 19 套主题，跳过 |
| `check-kimi-code-docs` | 自查产品文档 | 换成 fork 的 user-guide 即可，不单独做 |
| `import-from-cc-codex` | 从 Claude Code/Codex 导入配置 | fork 已有 compat 发现，跳过 |
| `write-goal` | 结构化目标 | fork `/goal` 已有独立证据复核，更强，跳过 |
| `sub-skill.review` / `consolidate` | 技能清单分层重组（review 提案 → consolidate 执行 + 备份） | fork 无 `has-sub-skill` 加载支持；**思路**用于手工瘦身 |

另：仓库内 `.agents/skills/`（review-pr、write-pr、gen-docs、translate-docs、
sync-changelog 等）是开发 kimi 自身的技能，不随产品分发；`review-pr`/`write-pr`
的流程可借一页，但 fork 已有 `/review` 与 handoff。

### zcode（v3.17.2 / 3.18）

| 资产 | 内容 | 对 fork |
|---|---|---|
| `zcode-guide` 6 技能 | commands / hooks / mcp / plugins / skills / 配置指南，全部是"先查证再改"的自诊断流程 | **最贴合**：fork 有 `grok inspect`、`grok mcp doctor`、`/mcps`、`/doctor`，但缺 agent 侧决策树。建议改写成一个 grok-doctor 技能 |
| `superpowers` 14 技能 | TDD、systematic-debugging、writing/executing plans、requesting/receiving review、verification、git worktrees、finishing branch、dispatching agents、brainstorming 等 | 多数已有对应：tdd / design / execute-plan / review / verify-this / defect-detective / blast-radius / handoff / grill-me。**systematic-debugging** 与 **finishing-a-development-branch** 是缺口 |
| `skill-creator-plugin` | 技能编写 + 2–3 个真实测试提示 + 轨迹评审闭环 | fork 的 `create-skill` 缺"评测"一步，值得并入 |
| `plugin-creator-plugin` | 插件脚手架/本地市场测试 | 用户几乎不写插件，跳过 |
| office 四件 / dynamic-workflows / visualize / browser-use / CUA / zcode-site | — | 用户已禁 office；workflow 有 create-workflow；其余平台绑定，跳过 |

### codex（本地 `codex-src/codex-main`）

| 资产 | 内容 | 对 fork |
|---|---|---|
| `review-agent` | 只读、defect-first 的评审 agent，P0–P3 分级 | 已有 `/review` + thermo-nuclear-review + defect-detective，重复 |
| `skill-creator` | 技能设计原则（假设模型已能干、渐进披露、发现成本、脚本化） | 原则层与 `skill-design-principles` 重合；**评测/脚本**两节可并入 |
| `skill-installer` | 从 openai/skills 或任意 GitHub 仓库安装技能 | 供应链风险；用户技能已很多，跳过（除非要做技能市场） |
| `openai-docs`（含 mcp-diagnostics 参考） | 自产品文档 + 模型选择 + MCP 诊断 | 自产品文档可由 fork user-guide 替代；MCP 诊断页并进 grok-doctor |
| `imagegen` | 图片生成 | fork 有 image_gen 工具与游戏资产技能，跳过 |

## 2. 建议（按性价比）

### P1：`grok-doctor` 自诊断技能（新写，约 120 行 + 3 个 references）

吸收：zcode-guide 六件的问题分类 + kimi mcp-config 的三层配置/信任 + codex
mcp-diagnostics 的"先查文档再改"纪律。内容按故障面分四页：

1. **配置**：改前备份；`grok2 inspect` 看 Config Warnings；枚举值一律
   snake_case（`in_history`，不是 `in-history`）；`[consent]` 这类 pager 拥有的
   节是已知假警告；改完新会话生效。
2. **MCP**：`grok mcp doctor [name] [--json]`；`/mcps` 里 `r` 刷新；服务名规则
   （首字符字母/下划线、禁 `__`）——doctor 通过但工具不到模型时先查这个；
   stdio 的 PATH（gash 场景）vs http 超时/OAuth；`~/.grok/mcp_credentials.json`。
3. **模型/供应商**：`[api_dump]` 取证、`unified.jsonl` 事件、缓存命中骤降的
   检查顺序（换模型/工具集/压缩/日期）。
4. **技能/插件/hook**：技能目录位置、`skills disabled`、plugin 加载失败、
   hook 信任。

触发词写清："配置改了不生效 / MCP 连不上 / 工具不见了 / 模型报错 / 缓存突变"。
成本：目录里一条（约 60 tokens），正文按需加载。

### P2：`systematic-debugging`（改写 superpowers 版）

四阶段：复现 → 分族/隔离（A/B、compile-cost 规则）→ 根因证据（unified.jsonl /
dumps / 夹具）→ 修复 + 回归。与现有技能分工：`tdd` 负责先写测试，`verify-this`
负责证明结论，`defect-detective` 负责审计；本技能只补"排障顺序与证据纪律"，
并把仓库既有规则（先分族、ctest 门控、不接 tail 管道）写进去。

### P2：把评估闭环并进 `create-skill`

从 codex skill-creator / zcode skill-creator-plugin 取两点：① 新建/改动技能后，
用 2–3 个真实提示跑一遍，把结果与轨迹一起看；② 技能正文保持短，细节进
`references/`。不新增技能，改现有 bundled skill 的正文。

### P3（可选）：`finishing-a-development-branch`

分支收尾清单（测试门控 → PATCHES/docs → tag/发版 → 清理）。AGENTS.md 已把这些
写成硬规则，只有当 AGENTS.md 瘦身、规则外移时才值得单独做。

## 3. 明确不建议

- **review-agent / requesting-code-review / receiving-code-review**：`/review`、
  thermo-nuclear-review、defect-detective 已覆盖。
- **brainstorming**：grill-me / designer 已覆盖。
- **write-goal**：`/goal` 自带独立证据复核，强于 Kimi 版。
- **import-from-cc-codex**：fork 已有 Claude/Cursor/Codex compat 导入。
- **skill-installer**：供应链风险，且当前目录已偏大。
- **sub-skill 层级**：fork 技能加载器不支持父子包；改成一次性人工瘦身
  （把 9 个 `principle-*` 合并成 1–2 个，review 类技能去重）。
- **office xlsx / CUA / browser-use / zcode-site / custom-theme**：平台或低频率。

## 4. 目录成本账

当前 50 条技能、16.7k 字符（约 5k tokens）在首条 user 消息里，cached 后每轮成本
可忽略，但占用上下文并影响技能选择。新增 3 个技能约 +0.3–0.5k tokens；如果同时
把 `principle-*` 九条合并为两条，净省约 1k tokens/轮，且选择更清晰。建议成对做：
**加 3 减 7**。

## 5. 来源

- kimi-code：GitHub raw `packages/agent-core-v2/src/features/skill/catalog/builtin/`
  （`mcp-config.md` 已读；其余按目录名与 `docs/zh/customization/skills.md` 归类）、
  `.agents/skills/`。
- zcode：`docs-local/agent-cli-tracking/notes/zcode.md`（superpowers 14 /
  zcode-guide 6 / skill-creator / plugin-creator 的逐条判定）。
- codex：本地 `codex-src/codex-main/codex-rs/skills/src/assets/samples/`
  （review-agent、skill-creator、skill-installer、openai-docs）与 `docs/skills.md`。
- fork 现状：`~/.grok/docs/user-guide/04-slash-commands.md`（`/doctor`、`/goal`）、
  `07-mcp-servers.md`（`grok mcp doctor`、命名规则、OAuth）、技能清单
  （45 user + 13 bundled）。
