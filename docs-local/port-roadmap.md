# 移植路线图（选型定稿 2026-10-03）

> **状态：选型定稿，未开工。** 三个移植预研专题（[workflow-pi-port](workflow-pi-port/README.md)
> / [kimicode-port](kimicode-port/README.md) / [step-code-port](step-code-port/survey.md)）
> 的候选特性按「coding + agent work 价值 × 国模适配性」定序，本文是**施工顺序与
> 交接台账的唯一权威**；各特性的设计细节仍以各自分文档为准，本文不复述。
> 基线：main `6e83214`（2026-10-03）。

## 1. 选型结论

| 序 | 特性 | 来源 | 分支拟名 | 规模（C 级） | 国模适配性 |
|---|---|---|---|---|---|
| ① | `/rewind` 分支树 undo（并入 branch summarization） | kimicode-port P1 + step-code-port P2（D2 落实） | `feat/local-rewind-branch-undo` | 5–8 人天 | ★ 模型无关，零适配风险 |
| ② | bash 命令 AST 静态安全分析（三态判定） | step-code-port P1 | `feat/local-shell-command-analysis` | 3–5 人天 | ★ 模型无关；无人值守 + 便宜国模场景价值放大 |
| ③ | canonical context edit | workflow-pi-port P1 | `feat/local-workflow-context-edit` | 4–7 人天 | ★ 中性偏正（摘要 agent 惯用法在国模成本下更可承受） |
| ④ | 延迟工具声明（三档 exposure） | workflow-pi-port P2 + kimicode-port P2（D2 三参照合并定稿） | `feat/local-deferred-tool-exposure` | 3–5 人天 | ⚠️ 收益大但国模风险最高，必须带 capability 门 |
| 随手 | 不完整流恢复注入（projection-only 续作指令） | step-code-port P3 | 挂靠 sampler 顺手件 | ≤1 人天 | ★ 国产网关/聚合器流中断直接对症 |
| 随手 | 子 agent 结果信封 + next_step 映射 | kimicode-port P3 | 挂靠 workflow 子 agent 面 | 2–4 人天（可拆） | ★ stop_reason→next_step 结构化契约替代弱模型自由判断 |

未选入：workflow-pi-port P2'（cache warming，待 ③④ 落地后评估）、P3（classifier，
依赖上游模型接入）、step-code-port P4（env 标记 + idle watchdog，fork 侧 spawn 链路
未核实）、P5/P6（cron 表达式 / ultraloop 入口，按需随手）。

## 2. 选型理由（摘要）

1. **① rewind 分支树 undo — coding 价值最高且国模适配性完美**。纯客户端特性，
   任何模型受益；原型已推翻"缺 journal"前提（`updates.jsonl` 本就是 append-only +
   `RewindMarker`），真实差距只剩"分支可往返 + 旧分支点可见 + 边界预计算"，
   prompt 放回编辑器已有完整等价物。并入 step 的 branch summarization（切走分支
   自动摘要 + summary-overflow 降源）后即完整会话分支树——三个专题里唯一的
   主会话 coding 体验级特性。kimicode-port D1 随本选型**拍板为分支树路线**。
2. **② bash 静态安全分析 — 无人值守安全基座**。真正值钱的是
   `analysisIncomplete` 不冒充"安全"的三态语义，而非解析器本身。国模放大效应：
   无人值守回合通常选便宜国模，命令生成可靠性弱于一线模型，现有参数级
   `requires_expr` 挡不住命令内容；便宜模型诱发放开审批跑长任务，安全下限恰是
   最缺的一块。
3. **③ canonical context edit — workflow 上限直接解锁**。迭代式 workflow 的
   prompt 线性膨胀是编排面唯一硬缺口（编排原语本身领先 pi）；走 host call +
   可见面编辑不碰 journal 重放确定性，设计已闭环。
4. **④ 延迟工具声明 — agent work 收益最大、国模风险也最大**。声明面瘦身对国产
   供应商 prompt cache（DeepSeek/GLM/Kimi 按缓存计费）有直接成本收益；但
   `search_tools` 自主发现对弱模型 agentic 纪律要求高（原型头号风险即
   "deferred 发现率"）。施工定稿三个国模适配决策一并锁死（见 §3）。

## 3. 国模适配约束（施工时必须落进设计/验收）

- **④**：① 只有 `dynamically_loaded_tools === true` 能力位的模型走 deferred，
  其余回落 direct 全量声明（kimi capability 门，"按能力裁剪请求而不是发出去等
  报错"）；② 首版用 kimi 公告流（`<tools_added>/<tools_removed>`）+ 按名展开，
  **不上 BM25**（按名认领比自主搜索对国模可靠；pi 的 `describeNamespace`/
  description 参与排名三条约束仍吸收进设计）；③ 描述静态化 + 变更才追加
  （prompt cache 友好，0.99.2 #10212）；④ kimi 历史剥离
  （`stripDynamicToolContext`）一并吸收。
- **②**：`analysisIncomplete` 三态语义按模型无关安全下限验收，主验收场景 =
  headless/无人值守 + 非一线模型驱动 bash。
- **随手-信封**：结构化 `stop_reason → next_step` 映射按"弱模型补强"定位验收。

## 4. 施工顺序与依赖

1. 顺序 **① → ② → ③ → ④**；随手件-流恢复注入可随 ② 之前任何时候搭车
   （独立、便宜、与国产网关现状强相关）。
2. **① 建议紧跟一次上游同步后开工**：rewind 相关文件多属上游同步线，
   紧跟同步降低 PATCHES 重放成本（rewind-branch-undo.md §8 依赖注）。
3. **④ 开工前先做三参照合并定稿**（kimicode-port D2）：pi（search_tools/静态化
   三约束）+ kimi（公告流/历史剥离/capability 门）+ step（branch summarization
   属 ①；工具面裁剪无 step 侧参照）合并写回
   [deferred-tool-exposure.md](workflow-pi-port/deferred-tool-exposure.md) 后再动工。
4. 每个特性**开工前**按 [fork-feature-inventory.md](fork-feature-inventory.md)
   复核「已有」判定（防把 fork 已有设计当缺口）；
   **完工后**把新澄清的行回填进其 §3/§5。

## 5. 交接协议（MANDATORY）

多任务跨会话推进，会话可能在任意任务之间中断。因此每个任务（含随手件）必须
产出台账条目，写入 §6 交接台账（**只追加，禁止改写/重排已有条目**）：

1. **任务完成时**（合 main / 发版 / 收口任一节点），追加一条"完成交接"，六项缺一不收：
   1. 当前状态一句话（分支 / tag / 是否已合 main / release run）；
   2. 关键证据（测试结果数字、验收项逐条对照——对照各设计文档 §验收标准）；
   3. 未验证事项 / 遗留（活回合验证、待定决策等）；
   4. 详情指针（设计文档小节、PATCHES.md 期号、release 页）；
   5. 下一个接手者的第一步动作；
   6. 若该任务是四特性之一：fork-feature-inventory 回填是否已完成。
2. **任务中途中断时**（会话断了/被掐了），接手会话开工前先在台账追加一条
   "中断点"条目：做到哪一步、哪些验证没跑、工作区有无未提交改动——然后再继续。
   接手者**禁止凭记忆直接续写**：先 `git log` + `git status` + 工作区 diff 核对
   实际进度，再对照本表对应行的分支拟名找分支。
3. **重大里程碑**（feature 合 main + 打 tag 发版）另在 AGENTS.md「Handoff 摘要」
   加一行指针指向本台账对应条目（客户端可见状态以 AGENTS.md 为准）。
4. 每个任务继续沿用既有纪律：代码走 feature 分支、验证绿一块提交一块
   （AGENTS.md 调试时间成本原则第 10 条）、LOCAL 补丁登记 PATCHES.md。

## 6. 交接台账（只追加）

（暂无——任务 ① 开工后在此追加第一条"开工"或"中断点"条目。）
