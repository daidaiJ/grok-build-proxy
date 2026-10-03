# 移植路线图（选型定稿 2026-10-03）

> **状态：四特性施工合 main（2026-10-03 统一发版 v1.0.41）。** ① rewind 分支树 undo
> （T1/T2a/T3 落地，T2b 未做）② headless `--non-interactive-denial continue`（P1
> 勘误：静态分析上游已有，实做面收窄）③ canonical context edit ④ 工具面 allowlist
> （T1 落地，T2 未做），均已合 main；两随手件未动，见 §6 交接台账。三个移植预研专题
> （[workflow-pi-port](workflow-pi-port/README.md) / [kimicode-port](kimicode-port/README.md)
> / [step-code-port](step-code-port/survey.md)）的候选特性按
> 「coding + agent work 价值 × 国模适配性」定序，本文是**施工顺序与交接台账的唯一
> 权威**；各特性的设计细节仍以各自分文档为准，本文不复述。
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

### 2026-10-03 · 任务① 开工（feat/local-rewind-branch-undo）

- **当前状态**：按 [rewind-branch-undo.md](kimicode-port/rewind-branch-undo.md) 开工，
  分支 `feat/local-rewind-branch-undo`（自 main `9908362`）；T1（L1 记录与回放语义）
  先行，`cargo check` + 相关 lib 过滤测试过绿再进 T2。
- **接手指引**：进度看该分支 `git log` + 本台账后续条目；中途中断按 §5-2 先补
  "中断点"条目再续作。设计锚点全在 rewind-branch-undo.md §1/§4（当日 grep 复核过）。

### 2026-10-03 · 任务① 阶段完成交接（T1/T2a/T3 本机完成，待 CI/活回合收口）

- **当前状态**：分支 `feat/local-rewind-branch-undo`（自 main `9908362`），四个提交：
  开工台账 `78f37f5` → T1 `2057bfe`（RewindMarker SwitchEdge 化 + 分支树 fold +
  两阶段 replay）→ T2a `357a529`（redo 通道端到端 + 分支面/边界预计算 +
  abandoned_branches 响应）→ T3 `1bba307`（picker ↩/⚠ 标注 + RewindTarget 贯穿 +
  to_branch execute）。**未合 main、未打 tag。**
- **关键证据**：`cargo check -p xai-grok-shell/-p xai-grok-pager --all-targets` 全绿；
  pager `ctest.sh --lib rewind` 63/0、全量 lib **10032/0**；shell rewind 过滤 58 过
  /25 挂（25 全为 `acp_session_tests/support.rs:287` AbsPathBuf("/tmp") Windows
  环境族，该 crate 不在白名单的既有原因，非本改引入）；兼容门 = 既有
  filter/collect 测试全过（旧格式输出逐条一致）+ 新增 5 用例（redo 恢复/未知分支
  回退/serde 兼容/两阶段等价/redo 端到端 replay）。
- **未验证事项 / 遗留**：① T2b（向前 rewind 统一走 replay 重建，消除双路径）未做；
  ② redo 端到端 + pty 回归（rewind_after_compaction_with_missing_checkpoint）待
  Linux CI / 专用会话；③ 活回合 TUI 抽查（/rewind → 切回弃分支 → 编辑重发 →
  regeneration 遥测）；④ boundary 文案 i18n；⑤ fork-feature-inventory 已回填。
- **详情指针**：PATCHES.md「rewind 分支树 undo」节（改动面全景，多属上游同步线
  **同步后必须重放**）；设计锚点 kimicode-port/rewind-branch-undo.md。
- **接手者第一步**：合 main 前先推分支跑 Linux CI（build.yml + 补跑 shell 侧
  rewind 用例）；或先做 ③ 活回合抽查（`ctest.sh` 白名单外需 `GATE_FORCE=1`）。
  中断恢复按 §5-2。


### 2026-10-03 · 任务② 完成交接（headless non-interactive-denial continue，feat/local-shell-command-analysis）

- **当前状态**：分支 `feat/local-shell-command-analysis`（自 main `8d82c36`），代码提交
  `6b1bc29`。**重大勘误收口**：step-code-port P1 的核心（tree-sitter typed AST + 危险
  命令规则 + fail-closed 三态 + 分类器 findings）fork 在 2026-09-23 上游同步后**全都有**
  （`xai-grok-workspace/src/permission/`），预研"grep 0 命中"判定作废，survey D1 与
  inventory 已勘误。真实缺口仅剩无人值守收口一件：headless 非交互遇审批阻塞原样
  Cancelled→终止回合；现新增 opt-in `--non-interactive-denial continue` 转成
  PolicyDeny 失败工具结果，agent 换路续跑（对齐 Step-Code 同名语义）。
- **关键证据**：`cargo check -p xai-grok-shell -p xai-grok-pager -p xai-grok-pager-bin`
  0 error；shell 单测 3+3（resolve_unattended_denial 纯函数 + StartupHints 解析回退）、
  pager headless init hint 测试 1 条，GATE_FORCE=1 窄过滤全绿。
- **未验证事项 / 遗留**：① headless 活跑 A/B（stop vs continue 两档对比，需真实
  触发一次权限提示）；② fork/worktree 子会话不经 initialize startupHints，continue
  不覆盖（已知边界，PATCHES.md 十八期登记）；③ subagent 端到端继承未活测（链路已接）。
- **详情指针**：PATCHES.md 十八期（改动面 + 上游重放清单）；survey D1 勘误节；
  inventory §2 工具体系行已回填。
- **接手者第一步**：随统一发版合 main；活跑验证可后置到发版后的新客户端。
  中断恢复按 §5-2。
### 2026-10-03 · 任务③ 完成交接（canonical context edit，feat/local-workflow-context-edit）

- **当前状态**：分支 `feat/local-workflow-context-edit`（自 main `8d82c36`），代码提交
  `904dc9a`。pi ContextEditEntry 语义落地：`replace_visible(seq, digest)` /
  `hide_visible(seq)` journal 直记 + spawn payload 哈希耦合 + host_service 可见面替换；
  脚本可见 `r.seq`。与原型的偏差（journal 直记 vs host_call 通道）已记录于 PATCHES
  十九期——seq 分配/重放/divergence 语义等价。**兼容关键**：context_edits 空时不
  序列化，旧 journal 重放 hash 零漂移（有测试断言 indirectly 兜底）。
- **关键证据**：xai-workflow lib 66/0（含 4 个新引擎用例：payload 注入/重放不重跑/
  编辑 divergence/hide 占位符）；shell workflow::host 过滤 4/0；两 crate cargo check 全绿。
- **未验证事项 / 遗留**：① 结构化输出不可替换（v1 边界，PATCHES 登记）；② 占位符
  格式等真实场景校准（设计 §5-1）；③ 活回合 token 曲线验收（设计验收 1）未做；
  ④ `context_with_system` 对应物（落点三）按设计未纳入本项，挪 P2' 评估。
- **详情指针**：PATCHES.md 十九期；设计锚点 workflow-pi-port/canonical-context-edit.md。
- **接手者第一步**：随统一发版合 main；活回合验证可后置。中断恢复按 §5-2。

### 2026-10-03 · 任务④ 阶段完成交接（工具面 allowlist T1 落地，search_tools T2 待做，feat/local-deferred-tool-exposure）

- **当前状态**：分支 `feat/local-deferred-tool-exposure`（叠层于 ③ 分支之上——
  AgentOpts 同文件演进，合 main 时按 ③→④ 顺序）。
  `AgentOpts.tools` 白名单全链路（workflow payload/hash → host_service →
  SubagentRuntimeOverrides → apply_child_tool_policy 裁剪，capability 只减不增）
  已落地并验证。**T2 = defer_tools + search_tools 延迟发现未做**：涉及新工具实现 +
  子 agent 会话内声明态 + 国模能力门（动态改声明面的模型支持未核实，路线图明确
  要求先立能力门），按切片拆出。
- **关键证据**：xai-grok-subagent-resolution 95/0（含 2 新用例）；xai-workflow
  67/0（含 payload 传递 + divergence 用例）；shell workflow::host 过滤 4/0。
- **未验证事项 / 遗留**：T2 三件套（search_tools 工具 / 会话声明态 + resume 原子性 /
  国模能力门）；fan-out token 曲线 A/B（验收 1）等 T2 一起做对照实验。
- **详情指针**：PATCHES.md 二十期（含 T2 施工约束全文）；设计锚点
  workflow-pi-port/deferred-tool-exposure.md。
- **接手者第一步**：T2 开工前先对照 fork-feature-inventory 与 `search_tool`
  现有实现（builder.rs ToolSearch 映射）定 D 判定；国模能力门先行。中断恢复按 §5-2。

### 2026-10-03 · 合 main/推送发版中断点（接手会话登记）

- **现场核对（git log/status/diff）**：前会话已完成三笔 merge（① `aae86a9` /
  ② `a56cab0` / ④ `f66364a`，③ 线性落在 main），main 领先 origin 16 提交，
  未推送、未打 tag。工作区遗留 2 行未提交补线：`handle_request.rs` /
  `agent_rebuild_tests.rs` 给 `apply_child_tool_policy` 调用点补 `allowed_tools`
  第 4 参——④ 合并后签名 4 参已入库而 shell 调用点仍 3 参，main HEAD 编译不过；
  系前会话验证全链路时未及提交（用户确认另一会话已验证）。
- **本轮动作**：补线按分支纪律收编进 `feat/local-deferred-tool-exposure`
  （`607fb6e`）再合 main（`67f0b21`）；`cargo check --workspace --all-targets`
  门禁通过后推送 + tag `v1.0.41` 发版，完成后另追加完成交接条目。
