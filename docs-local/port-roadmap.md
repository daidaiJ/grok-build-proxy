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

### 2026-10-03 · Linux CI 首跑暴露 redo 回放缺口 + 修复（接续上条中断点）

- **现场**：tag `v1.0.41` 首推后 build run `37111186613` linux job 挂 2 例
  （`test_replay_redo_switches_back_to_abandoned_branch` /
  `test_replay_two_phase_path_matches_streaming_path_for_forward_markers`）；
  windows job 与 release run `37111199780` 全绿（release 曾以修前树发布 4 资产，
  下载量 0）。本机 Windows 复跑同值复现 → 纯确定性语义缺口，非平台差异；根因
  是两用例名不含 "rewind"，此前的 `--lib rewind` 过滤从未执行过它们，Linux CI
  为其首跑。
- **根因两处**：① 末端截断按全局 prompt 计数对齐 target，而 redo 的 target
  是目标分支时间线坐标——复活分支自身 prompt（P4）被削掉；② fold 输出喂给
  状态机时丢失全文件 promptIndex 幻影上下文（编号 chunk 在被裁分支上），
  幻影尾巴被误计为真 prompt（reached 1≠0）。
- **修复**（分支 `feat/local-rewind-branch-undo` 提交 `6e3c858`，合 main
  `aae18d3`）：fold At 落点认活跃链上的现成分叉；pointer 路径预种全文件幻影
  上下文 + redo 路径跳过末端截断、reached 固定 target。验证：shell replay::
  27/0、rewind 过滤 58 过/25 挂（全为已知 AbsPathBuf Windows 环境族，与基线
  一致）、pager rewind 63/0 无回归。
- **发版动作**：沿 v1.0.39 覆盖先例删 tag `v1.0.41`（release 转 draft）→ 重指
  修复后 main 重推，触发新 release/build run；资产同名覆盖、URL 不变。

### 2026-10-03 · 四特性统一发版完成交接（v1.0.41，tag 重指后双绿）

- **当前状态**：main `b811cfb`，tag `v1.0.41` 重指至修复后树（annotated，tag 消息
  即 release body），release run `37115871788` linux/windows 双 job success 出
  4 资产、build run `37115862185` 双 job success（此前挂的 2 用例转绿）；
  release 页已恢复发布（draft → published，沿 v1.0.39 先例，同名覆盖）。
- **关键证据**：`cargo check --workspace --all-targets` 0 error/0 warning（补线
  后全树两轮）；shell replay:: 27/0、rewind 过滤 58 过/25 挂（全为已知
  AbsPathBuf Windows 环境族，与基线一致）、pager rewind 63/0；Linux CI build
  双 job success。
- **未验证事项 / 遗留**：① T2b（replay 重建统一双路径）；② headless 活跑 A/B
  （stop vs continue 两档对比）；③ 活回合 token 曲线验收；④ T2 三件套
  （search_tools 工具 / 会话声明态 + resume 原子性 / 国模能力门）+ fan-out
  token 曲线 A/B；两随手件（流恢复注入 / 子 agent 结果信封）未动；活回合 TUI
  抽查（/rewind 切回弃分支 → 编辑重发 → regeneration 遥测）随新客户端。
- **fork-feature-inventory 回填**：①②③④ 均已回填（见各任务交接条目）。
- **详情指针**：PATCHES.md rewind 节 + 十八～二十期；release 页 tag v1.0.41。
- **接手者第一步**：更新本机客户端到 1.0.41 做活回合抽查；下一特性按 §4 顺序，
  随手件-流恢复注入可随时搭车；④ T2 开工前先做三参照合并定稿（§4-3）。


### 2026-10-04 · 接手会话开工登记（遗留任务滚动推进）

- **接手内容**：v1.0.41 交接遗留五件——①T2b（replay 重建统一双路径）、②headless
  活跑 A/B、③活回合 token 曲线、④T2 三件套（search_tools/声明态 resume/国模门）、
  随手件-流恢复注入（P3）。推进顺序：随手件-P3（独立便宜）→ ①T2b → ④T2（先
  三参照合并定稿）→ 活跑类（②③，需新客户端）。每任务收口即在台账追加条目。
- **开工核验**：`git status` 干净（4 个未跟踪文件为已知暂存物）；分支
  `feat/local-rewind-branch-undo`（tip `6e3c858`）与 `feat/local-deferred-tool-exposure`
  （tip `607fb6e`）与上条交接一致；main `20470cd`（发版后新增三条 docs 提交）。
### 2026-10-04 · 遗留①T2b 完成交接（replay 重建统一双路径，feat/local-rewind-branch-undo）

- **当前状态**：分支 `feat/local-rewind-branch-undo`（tip = 本条目提交），未合 main
  （随本会话滚动收口统一发版）。`handle_rewind` 双路径已消除：一律 journal replay
  重建；replay 状态机补全保真（工具调用/结果 item + 用户消息图片 parts），重建
  结果对齐 resume 侧 ChatReducer 输出。
- **关键证据**：`ctest.sh -p xai-grok-shell --lib session::helpers::replay`
  （GATE_FORCE）26/26（含新增
  `replay_keeps_tool_calls_and_results_full_fidelity` /
  `replay_rewind_marker_discards_partial_tool_step`）；整测试二进制编译 0 警告。
- **未验证事项 / 遗留**：acp_session rewind_cross_compaction / rewind_synthetic_turn
  族本机 win-skip（族R），统一路径的行为回归依赖 Linux CI（发版时盯 build run）；
  活回合 /rewind 抽查随新客户端。
- **语义变化（有意）**：目标在 compaction 点之上时重建基准从"内存截断"变为
  "checkpoint + raw 重放"；replay 失败不再回退截断。详见 PATCHES.md 二十二期。
- **详情指针**：PATCHES.md 二十二期；rewind-branch-undo.md §4-L2。
- **fork-feature-inventory 回填**：无新澄清行（T2b 属设计内施工）。
- **接手者第一步**：发版收口时合 main → Linux CI 盯 rewind 族 → 活回合抽查。
### 2026-10-04 · ④T2 开工登记（defer_tools + search_tools 延迟发现，feat/local-deferred-tool-exposure）

- **状态**：设计定稿已完成（§2.2 三参照合并，用户同日加拍板：**声明模式必须
  可配置**——`tools.declaration_mode = "direct" | "deferred"` 全局项，默认
  direct=传统全量声明，deferred 开启仍受能力门约束；三层可配 global → server
  exposure → 子代理 AgentOpts）。分支已快进到 main `20470cd`，设计文档定稿版
  已带入工作区（原提交在 feat/local-stream-recovery-injection 47447f0）。
- **施工切片计划**（每片绿一块提交一块）：
  1. 管道片：`AgentOpts.defer_tools`（serde default false，old-journal hash
     稳定）→ `SubagentRuntimeOverrides` / `EffectiveRuntimeConfig` →
     `apply_child_tool_policy` 新参（defer 时保留面 = allowlist，其余记入
     deferred 集）→ host_service:564 同层接线（镜像 T1 allowed_tools 模式）。
  2. 能力门片：SamplerConfig per-model `supports_dynamic_tools`（默认 false
     = 回落 direct；先例 `supports_backend_search` config.rs:111）+ host_service
     降级 info 日志。
  3. 运行时片（核心）：search_tools 内建工具 + 子代理会话声明态
     （ToolBridge shared_resources `State<DynamicToolExposure>`，先例
     ReportedTaskCompletions）+ 每轮声明面过滤（agent.rs `tool_definitions`
     按声明态过滤，注册不裁剪）+ 公告流（声明变更追加 system-reminder，
     tools_added 名单）+ resume 原子性（声明态与工具面同批重建）。
  4. 面板片：/mcp 展示模式与生效层级 + i18n。
- **关键入口点（已探明）**：`apply_child_tool_policy`
  （subagent-resolution/definition.rs:231，调用点 handle_request.rs:789）；
  `AgentOpts`（xai-workflow/src/host.rs:93 tools 字段旁）；host_service
  allowed_tools 透传在 host_service.rs:564。
- **接手者第一步**：按切片 1 开工；切片 3 前先读
  `agent/subagent/handle_request.rs` spawn 流与 `agent.rs tool_definitions()`
  每轮声明面构建，确认声明态过滤的落点（每轮 vs 构建时）。


### 2026-10-04 · ①T2b CI 红灯回退登记（接本日完成交接条目，接手会话中断点）

- **现场**：v1.0.42 build run `37174064017` linux job 红——`rewind_synthetic_turn_tests`
  族 6 用例全挂（windows 绿；release run `37174082853` 仅产资产不受影响）。
  根因：T2b 统一路径锚点 = "journal 即真相"，而该族 6 夹具只播种内存对话
  （updates.jsonl 为空），replay 重建自然清空——语义锚点冲突，非实现 bug。
- **处置**：main 上外科手术回退 T2b 三个代码文件（replay.rs / rewind.rs /
  storage/mod.rs 还原至 47447f0 版），PATCHES 二十二期与本条目保留为记录；
  tag `v1.0.42` 重指至回退后 main（沿 v1.0.39/41 先例）。
- **重做清单（下会话，分支 feat/local-rewind-branch-undo 保留全部 T2b 提交）**：
  1. 重写 6 夹具为 journal+内存一致（镜像 rewind_cross_compaction_tests.rs 的
     `write_compacted_session_fixture` 写法：SessionUpdateEnvelope 逐行写
     updates.jsonl + 唯一 session id），marked 变体 chunk 带 promptIndex meta、
     unmarked 变体不带。
  2. 两个非平凡点必须先推导再动手：① `rewind_to_start_keeps_only_preamble`
     的 preamble system_reminder（journal 以 UserMessageChunk 持久化，replay 的
     hostTurn/phantom 折叠语义决定它是否可重建——若不可重建，期望值需改为
     不含 reminder 并在台账登记语义变化）；② unmarked 回退计数
     （`truncate_target` 的 target-1 前提是 preamble 计入 live 计数，与
     marked 夹具的 index-0 起点矛盾——先读 live 侧 prompt_index 分配再定）。
  3. 本机验证走 WSL（该族在 win 因族R夹具根因 skip）：`wsl -e bash -c "cd
     /mnt/d/CODE/ai/grok-build-proxy && cargo test -p xai-grok-shell --lib
     rewind_synthetic"`，首跑含全量编译预算。
  4. 绿后重复合 main + 重指 tag。
- **工作区状态**：干净（本条目提交后）。

### 2026-10-04 · v1.0.42 发版完成交接（tag 重指后双绿）

- **当前状态**：main `cdd2bfc`，tag `v1.0.42` 重指一次（首指 build 红于 rewind
  族 → T2b 回退后重指，沿 v1.0.39/41 先例）；release run `37175711015` /
  build run `37175702302` 双 job success，release 页 published + 4 资产。
- **本版内容**：随手件-流恢复注入（PATCHES 二十一期）；④T2 设计定稿文档
  （含用户可配置拍板）；①T2b 未随版（回退，重做清单在案）。
- **关键证据**：build run Linux 85 套件全绿 0 FAILED，rewind_synthetic_turn
  族回退后全部 ok；本机 workspace check 0 error/0 warning。
- **未验证事项 / 遗留**：①T2b 夹具重做（清单见回退条目）；④T2 四切片施工；
  ②headless 活跑 A/B；③活回合 token 曲线；随手件-子 agent 结果信封；
  流恢复提示活回合抽查（等客户端更新 1.0.42）。
- **接手者第一步**：按 roadmap §6 ④T2 开工条目切片 1 动工；或先做 ①T2b
  夹具重做（WSL 验证）。

### 2026-10-11 · 兄弟 agent 二批调研收口（crush / goose / zcode 入库 + 边缘仓甄别）

- **结果**：父目录 coding agent 全量盘点收口。首批五家（opencode/MiMo/qwen/minimax/kimi）已有
  agent-cli-tracking 笔记；本轮新增 crush / goose / zcode 三份机制笔记
  （notes/crush.md、notes/goose.md、notes/zcode.md），README 扩为八家对比（对象表 + 增补批次
  对照节），引入评估新增 ADOPTION.md §7（候选 15 条：P1 四条 = DCP 策略族并入 microcompaction、
  egress 外泄检测、turn-context 预算注入、amend-workflow；P2/留档 11 条；负面清单增补 9 条；
  「fork 已有」防误报 8 条均 grep/读码核实）。
- **用户口径**：only-cc-lite 非 coding agent 剔除；opencode 与 MiMo-Code 社区口碑下降，仅作
  机制参照不再对齐（README 口碑注记 + ADOPTION §7 口径）。
- **甄别归档**：DCP 插件（7.1.1 主参照，作者转向 Sleev 开发放缓）、rpiv-mono（advisor +
  rpiv-workflow 两包值得深挖）、deepseek-harness-codearts（逆向协议运营插件，浅尝）、
  openagents（多 agent 网络平台，无关）、only-cc-lite（压缩库，不立项）。
- **接手者第一步**：推进 ADOPTION §7.5——优先把 DCP dedup/purge-errors 策略族并入既有
  microcompaction P1 设计；P2 项随各自主题轮顺带对照，不单独立项。

### 2026-10-11 · 二轮逐项终评 + /lsp 提案登记（接同日调研收口条目）

- **结果**：应拍板要求对 ADOPTION §7.1 全部候选逐项评估必要性与收益（§7.6），并登记衍生
  LOCAL 提案 `/lsp`（workspace 级 LSP 工具启停，仅新会话//clear 后可用；设计草案在
  §7.6.1，持久化推荐 grok-home 按 cwd 编码键的 feature 覆盖文件，复用 encode_cwd_dirname）。
- **核查修正**：/export Markdown 导出 + /transcript + /share 已存在 → 7.1.12 转「已有」；
  /clear 已注册；config 无项目作用域回写先例（/lang 均会话态）→ /lsp 走 grok-home 存储路线。
- **二轮结论**：做 = 7.1.1（DCP 策略族并入 microcompaction，第一顺位）+ 7.1.2（egress
  检测小件）+ 7.1.7（rewind 预览收尾件）+ /lsp 提案；缓 ×7（各挂触发信号，见 §7.6.2 表）；
  不做/已有/留档 ×5。
- **接手者第一步**：/lsp 提案待排期（建议 TUI 便利件轮，与 7.1.13 联动）；7.1.1 随既有
  microcompaction P1 设计推进。

### 2026-10-11 · 三轮拍板：/auth 面板 + /stats API 成本立项；7.1.3 撤回

- **拍板**：① 7.1.3 turn-context 预算注入先准后撤，最终**去掉**（ADOPTION §7.6.2 已改判不做）；
  ② 其余缓档候选用户「不太有感知」，维持挂触发信号不主动推进；③ **新立项两件**（设计文档
  docs-local/provider-onboarding-api-cost-todo.md，设计完成未开工）：
  /auth 供应商配置面板（协议→baseUrl→SK→自动检索模型→models.dev/OpenRouter 目录回填参数，
  仅与默认值不同才写）+ /stats「缓存写」列（OpenAI 兼容网关恒 0）替换为 API 美元成本
  （per-model prompt/output/cache_read × OpenRouter pricing，合计行 + 三窗联动）。
- **参照实现**：用户 modelq 仓库（D:/CODE/ai/openrouter-cli）——OpenRouter /api/v1/models 免鉴权
  pricing（client.go:128-152）、models.dev api.json TTL 缓存（modelsdev.go:207,263-291）、
  置信匹配序 exact→canonical→vendor（helpers.go:32-35）；fork 侧镜像 managed prefetch 合并
  先例（shell config.rs:3420-3455）。
- **关键现状锚点**：ApiBackend 三值（sampling-types types.rs:1094）；stats 缓存写列喂
  cache_creation_tokens（stats_modal.rs:455-465，chat-state usage.rs:80）；账本已有 per-model
  input/output/cached_read 拆分——成本渲染层可算，账本零改动；config 回写面无先例（T3 需新增）。
- **待拍板**：SK 存储默认（建议 env_key）、config 回写策略（重序列化 vs 定点插入）、首启
  onboarding、目录源优先级、status line 成本位——见文档「待拍板决策」节。
- **接手者第一步**：按文档分期 T1（目录取数模块 + modelq 同款 JSON 夹具）开工，T1 完成后
  特性 B（/stats 成本列）可先行落地（依赖 T1），特性 A（/auth 面板）随后。

### 2026-10-11 · 四轮拍板：首启默认 BYOK 引导 + /login 改名 /grok（接三轮条目）

- **拍板**：① 零配置（无订阅/无任何模型配置）首启默认提供 BYOK `/auth` 交互继续使用，
  推 Grok 认证的现状废止；② 原 `/login`（Grok 订阅登录）改名 `/grok`。
- **根因与落点（已探明）**：零配置时 `build_auth_methods` unpinned 分支仅通告 `grok.com`
  （shell `agent/auth_method.rs:85-87`），pager 以 `auth_methods.first()` 取启动元数据 ⇒ 首启必落
  Grok OAuth。改法 = 零配置默认推荐位给 BYOK 面板、OAuth 降级为面板内选项；ACP method 通告
  保持不变（编辑器客户端依赖），只动自家人默认首屏。改名波及 5 引用点 + i18n（清单见
  provider-onboarding-api-cost-todo.md「拍板增补」节），建议保留 `login` 隐藏别名一个周期。
- **分期更新**：新增 T0 小件（改名 + 首启默认路径改向）可先行独立发版；T2 /auth 面板落地后
  引导位从占位提示升级为直进面板。
- **接手者第一步**：T0 两件（改名 + 零配置分支）按文档落点施工；或按三轮条目先做 T1 目录模块。

### 2026-10-11 · 五轮拍板：不保留别名 + 交接下会话分期施工（接四轮条目）

- **拍板**：`/login`→`/grok` **不保留别名**（硬切换，引用点一次性改净，含 ACP 命令列表与
  全部测试断言）；四轮条目中「保留隐藏别名一个周期」的建议作废。
- **交接**：本会话（兄弟 agent 二批调研 → 逐项终评 → /auth + /stats 成本立项 + 首启 BYOK
  拍板）已交接，下会话从 T0 开始分期施工。
- **指针**：交接卡 `.handoff/provider-onboarding-api-cost.md`（任务单/约束/拍板原文/文档地图）；
  AGENTS.md Handoff 摘要已置顶对应块；设计文档
  `docs-local/provider-onboarding-api-cost-todo.md` 为施工唯一权威。
- **接手者第一步**：读交接卡 §6 任务单，T0 两件（/grok 硬改名 + 零配置首启默认 BYOK 引导）
  开工，分支 `feat/local-grok-rename-byok-first`（或按拆分自定）。

### 2026-10-11 · T0 施工完成登记（/grok 硬改名 + 零配置首启 BYOK 引导，feat/local-auth-onboarding）

- **改动**：两件 T0 落地。① `/login`→`/grok` 硬改名（不保留别名）：命令 meta/注册
  （`slash/commands/grok.rs`，LoginCommand→GrokCommand）、shell 保留名单
  （`session/slash_commands.rs`，"login"→"grok"）、用户可见文案 6 处（`login/error.rs`
  provider_login_message ×2、shell `compaction.rs` 抑制通知 ×2、pager `session_list.rs`
  NoOauth toast、`session_event.rs` ReAuthRequired）、`manager/remedy.rs` advice
  文案（分诊时发现的设计文档遗漏波及点）、i18n 描述对、全部测试夹具/断言
  （registry/mod/acp_command/inspect/slash_commands_tests/prompt/task_result 等）。
  ACP 侧 method id `grok.com` 与通告机制不动（编辑器客户端兼容）。
  ② 零配置首启默认 BYOK 引导：shell `build_auth_methods` 新输入
  `byok_recommended`（调用点 `acp_agent.rs` 按 unpinned ∧ 无 SK ∧ 无 token ∧ 无
  企业 OIDC ∧ 无 auth_provider_command ∧ 非 disable_api_key_auth 计算），grok.com
  method 挂 `meta.byok_recommended`（wire 兼容，id 不变）；pager
  `startup_auth_metadata` 提取 → `AcpConnection`/`AppView.byok_recommended` →
  welcome Pending 首屏改向：BYOK 配置指引（指向 `~/.grok/config.toml` 的
  `[model.*]` + base_url + api_key/env_key）为默认推荐位，「使用 Grok 订阅登录」
  降级为 `l` 菜单项；auth 错误出现时错误占回消息位。T2 面板落地后引导位升级进面板。
  `default_auth_method_id` 语义审过：零配置仍为 None（走通告的登录 method），无需改动。
- **验证**：`cargo check -p xai-grok-pager -p xai-grok-shell -p xai-grok-login
  --all-targets` 0 error；pager 门控 lib：slash 568/0、welcome 226/0（含 3 个新
  BYOK 渲染用例）、acp 905/0（含 byok_recommended 提取新用例）、session_event
  100/0、agent_view 904/0、dispatch::tests 1732/0；shell GATE_FORCE 过滤：
  auth_method 28/0（含 3 个新 meta 用例）、slash_commands 105/0、inspect 41/0、
  compaction 75/0；login GATE_FORCE 过滤：error 11/0、manager::remedy 10/0。
  i18n 扫描新键全部入表 0 挂。
- **分诊副产物**：win-skip.txt 新登记族S（acp_session compaction/goal 三族，
  `/tmp` 夹具 NotAbsolute，同族N根因）与族T（xai-grok-login 铸造/锁/管道 29 例，
  counting_provider 用 POSIX shell 命令当铸造命令，Windows 无 POSIX shell）。
  新撞到的 `/log` 前缀测试用例（slash_menu_enter）改 `/gr`。
- **后续**：合 main 后按 `vX.Y.Z` 发版；T1 开工吸收六轮拍板（价格缓存 TTL 1 天起、
  只留官方价格、/auth 面板参照既有配置面板 + 模型列表关键词过滤勾选）。

### 2026-10-11 · T0 发版 + T1/特性B 施工完成登记（模型目录 + /stats 成本列，feat/local-model-catalog）

- **T0 发版**：`feat/local-auth-onboarding` 已合 main（merge `8651bd2`）随 tag
  **`v1.0.48`** 发版；release run `38115162480` 双 job success、4 资产发布。
  main build run `38115162565` Linux 挂一例上游原生用例
  （persistence `reset_title_to_auto_adopts_in_flight_generation_as_auto`，
  8 秒墙钟轮询超时；T0 未触碰该文件，v1.0.47 同 run 绿）——failed job 已重跑
  观察，判定 flake 与否见下条补记。
- **改动**：T1（目录取数模块）+ 特性 B（/stats 成本列）一期落地，实现记录见
  PATCHES.md 二十九期（新 crate xai-grok-model-catalog、shell 扩展
  `x.ai/modelCatalog`、pager 进程级 store + 三渲染面换列 + 合计行）。
  六轮拍板全部落地：TTL 缺省 1440 可配（clamp 5–20160）、只留官方价
  （OpenRouter pricing / models.dev cost）、OpenRouter 主 models.dev 兜底。
- **验证**：见 PATCHES.md 二十九期（catalog 29/29；shell 5/5；pager stats 36 +
  catalog 30 + tools fmt_cost 6；三 crate check --all-targets 0 error；
  i18n cache write 键 0 残留）。
- **后续**：T2/T3（/auth 面板交互 + config 写回，T3 前定回写策略拍板 2）→
  T4 i18n 扫尾；面板落地后 T0 引导位从占位提示升级进面板。

- **补记（同日晚）**：① v1.0.48 build run `38115162565` 首挂的 persistence
  `reset_title_to_auto…` 在 failed-job 重跑通过（shell lib 7096/0），判定计时
  flake；② 重跑暴露 T0 改名漏网断言：集成测试
  `tests/external_auth_expired_credential.rs`（`cfg(unix)`，本机跑不了）仍断言
  `/login`，三处字面量已改 `/grok` 随本分支合入（PATCHES 二十八期补记）。

- **发版收口（同日晚）**：`feat/local-model-catalog` 合 main（merge `ea3c594`）随
  **`v1.0.49`** 发版；release run `38120737264` / build run `38120737274` 双 job
  success、4 资产非草稿。build 全绿同时收口两件：T0 断言补修（unix-only 集成测试）
  在 Linux 验证通过；v1.0.48 首挂的 persistence 用例复判为计时 flake（连续绿）。

### 2026-10-11 · T2/T3 施工完成登记（/auth 面板 + 目录回填 + config 写回，feat/local-auth-panel）

- **改动**：/auth 五步向导（协议→baseUrl→SK 掩码→模型列表过滤勾选/手填回退→
  确认写回）+ shell 扩展 `x.ai/providerModels` + `write_model_entries`（toml_edit
  定点写回 `[model.<id>]`，注释/未知段保留，flock 守卫，全批原子）+ welcome 门
  `a` 菜单直进面板（第二宿主）。七轮拍板全部落地（SK 明文直写 / id 用户缩写
  唯一冲突即拒 / toml_edit 定点编辑）。实现记录 PATCHES.md 三十期。
- **回填语义**：目录值与新建模型兜底不同才写——context_window 兜底 200_000
  同值省略；max_completion_tokens 缺省=不发送、有值即写；目录未命中不写值。
  写回后 `[model.*]` 经既有 ModelsChanged 热重载，`/model` 即选即用。
- **验证**：见 PATCHES.md 三十期（auth_modal 13 / welcome 226 / slash 568 /
  dispatch::tests 1732；shell model_entry_writes 7 / provider_models 6 /
  slash_commands 105；双 crate check --all-targets 0 error 0 warning；i18n 0 挂）。
- **发版收口（同日）**：`feat/local-auth-panel` 合 main（merge `54e6389`）随
  **`v1.0.50`** 发版；release run `38126950148` / build run `38126944660` 双 job
  success，release 页 4 资产非草稿。活回合验证待新客户端（五步实跑 + /model
  热加载 + SK 不落 transcript/日志抽查）。
- **后续**：T4 i18n 扫尾（面板字符串已带 zh，剩余无）——实质挂起；
  T5 特性 C（能力门控）独立小件待开工；发版待用户拍板（活回合验证后）。

### 2026-10-11 · 活回合三修复登记（目录裸 id 匹配 + /auth 提示行贴底 + 写回后 /model 不刷新，fix/local-catalog-bare-ids-auth-panel）

- **改动**：① 目录源换序（models.dev 主、OpenRouter 兜底）+ 原厂白名单过滤
  （21 项，转售 provider 整库不留）+ 匹配策略扩展（剥壳/分隔符变体/部署戳容忍
  三路 84 档 + canonical 自证平局）+ `~/.grok/prices.cache` 磁盘缓存与宁旧勿空
  回退；② /auth 提示行固定贴面板底（对齐全局面板惯例）；③ 写回成功后 pager 显式
  发 `x.ai/internal/reload_models`（embedded 无 config watcher，v1.0.50 的热重载
  文案为空头支票）。**勘误上条**：「写回后经既有 ModelsChanged 热重载」在
  embedded 模式不成立（watcher 仅 run_leader 装配），本次以显式触发修复。
  实现记录 PATCHES.md 三十一期，issue 与拍板二轮 8–13 见
  issues/2026-10-11-catalog-bare-ids-and-auth-panel-fixes.md 与 design doc。
- **验证**：model-catalog lib 38/38；shell GATE_FORCE model_catalog 7/7 +
  ext_method_reload_models 1/1；pager GATE_FORCE auth_modal 14/14 +
  dispatch::tests::auth 25/25；双 crate check --all-targets 0 error；i18n 基线不变；
  匹配仿真（线上真实两源）本机 10 id 8 命中全原厂价，2 miss 均 sensenova（豁免）。
- **发版**：随 **`v1.0.51`** 发版（merge `14561c0`，tag 已推；release run
  `38135151264` / build run `38135151234` 双 job success，release 页 4 资产非草稿已核；
  本机客户端已部署 `grok 1.0.51 (14561c01201f)`，旧版留 `grok2.exe.v1.0.50.bak`）。
- **遗留**：活回合验证（/stats 成本列三模型、/auth 五步写回后免重启 /model、
  prices.cache 断网回退）；T5 特性 C（能力门控）仍待开工。

### 2026-10-12 · /lsp 施工完成登记（workspace 级 LSP 工具启停，feat/local-lsp-workspace-toggle）

- **落地**：接 2026-10-11 二轮条目登记的 /lsp 提案（§7.6.1）。调研先行——同类真有
  LSP 的只有 crush / opencode / qwen-code 三家（其余五仓两轮扫描 0 命中；only-cc-lite
  非 coding agent、MiMo-Code 按用户口径不取参照），深读结论与思想吸收判定见
  [`docs-local/lsp-port/README.md`](lsp-port/README.md)；qwen-code 只读 `/lsp`
  状态面板先例 + 其热重载设计「启用旗标不做运行时切换」印证会话边界拍板。
  语义：`/lsp on|off|status|clear`，grok-home `feature-overrides/<encoded-cwd>.json`
  存储，优先级 pin > env > workspace 覆盖 > config > remote > default，写入过
  「零用户回合」新会话门，生效点 = session spawn 的 definition 构建。
- **验证**：shell GATE_FORCE workspace_overrides 4/4 + lsp_tools 3/3；pager lsp 6/6
  + translations_table 1/1；双 crate check --all-targets 0 error 0 warning；
  i18n 扫描 exit 0。实现记录 PATCHES.md 三十四期。
- **遗留**：① 活体 TUI 验证（/lsp on → /clear → 工具清单现 lsp；status 三态；
  带历史回合拒绝）；② 后续思想吸收判定表在 lsp-port/README §5（热重载增量
  reconcile / call_hierarchy HMAC / per-root 多实例均挂触发信号缓）；
  ③ 7.1.13 LSP 工具补齐（call_hierarchy/rename/replace_symbol）随使用反馈再排。

### 2026-10-12 · LSP 改进规划立项登记（lsp-port 专题续期）

- **产出**：[`docs-local/lsp-port/lsp-improvements-todo.md`](lsp-port/lsp-improvements-todo.md)
  ——/lsp 落地当日对三家（crush/opencode/qwen-code）缺口矩阵评估后拍板顺位：
  T1 诊断按需 op（backend `read_diagnostics` 现成，小件）→ T2 call hierarchy
  （三步协议 + 陈旧防护）→ T3 rename（写路径独立切片）；code actions/热重载/多根
  等 4–8 号候选挂触发信号缓。文档含现状锚点、各期改动点/验收标准、待定决策
  D1–D4（D3/D4 带默认倾向可默认执行）。
- **接手者第一步**：T1 开工（分支拟 `feat/local-lsp-diagnostics-op`）；开工首日
  核实 async-lsp 的 CallHierarchy 类型可用性（T2 前置）。

### 2026-10-13 · /lsp v2 开箱即用施工完成登记（默认开 + builtin 检测 + status 数据面 + 零回合热加载，feat/local-lsp-builtin-servers）

- **落地**：接上两条 /lsp 条目，回应用户三条不满 + 两条追加拍板。①
  `Feature::LspTools` 默认翻 true（registry 层，关闭路径不变）；② builtin
  五语言检测（tools `implementations/lsp/builtin.rs`：rust-analyzer / gopls /
  pyright→basedpyright→pylsp 优先序 / jdtls / clangd；PATH 探测不 spawn 二进制；
  同名 + 扩展双重 gap 合并，config 恒压 builtin），canonical 入口
  `load_servers_with_builtins_sourced` 四处统一（spawn / inspect / workspace
  handle / status）；③ `/lsp status` 换数据面：`x.ai/lspStatus` 扩展（门禁
  六层来源 + server 清单 + 会话后端快照走 `LspBackend::status_snapshot()` 新
  trait 方法），pager `format_status` 渲染、异步回包落 scrollback——improvements
  todo §5 随手件核销；④ **推翻 v1「会话边界切换」语义**：零回合会话
  `/lsp on|off` 写完覆盖立即 `x.ai/lspApply` 热加载（`SessionCommand::
  ReloadLspHarness` → `AgentRebuildSpec::build_agent_with_lsp` 只换 LSP 注入
  面，复用 model-switch 零回合重建簿记 + `HarnessRebuild` 工具清单重广播；
  turn_count>0 拒绝）。后端组装收敛 `MvpAgent::build_lsp_backend` 单入口。
- **验证**：cargo check shell / pager --all-targets / tools / workspace
  0 error；tools lib builtin 4/4；pager lsp 8/8（status effect 参数 +
  format_status 三态 + 门控四例）；shell GATE_FORCE config lsp 3/3 +
  workspace_overrides 4/4；i18n 扫描 exit 0。GATE_FORCE tools lsp 过滤实锤既有
  Windows 环境族：mock e2e 40 挂（mock server 是 Python 脚本 spawn 不兼容，
  非本次回归）。实现记录 PATCHES.md 三十五期；设计正文 lsp-port/README §8。
  **v1.0.54 同 tag 两轮补修**（详录 PATCHES 三十五期补修节）：① CI 暴露
  `ensure_started_background` 无 runtime panic（默认开 × 同步测试夹具 × runner
  clangd）→ try_current fail-safe；② **ptyctl 真客户端活体验出 v1 遗留 bug**：
  pager fresh 门拿输入召回历史当回合判据，生产恒拒（/session-info 轮次=0 仍
  报「已有历史回合」）→ 移除 pager 门，零回合权威判定收敛 shell turn_count。
  活体验证（v1.0.54 资产 1c12a2e 本机部署）：status 数据面（默认开来源 +
  rust-analyzer/clangd 双 builtin 检测 + 后端就绪态）、lsp 工具注册 +
  clangd documentSymbol 真实应答、带历史拒绝——全部通过；热加载回路因 ①②
  两修打断，随终版资产补验。本机 rust-analyzer 是 rustup 垫片（组件未装，
  `rustup component add rust-analyzer` 可修），检测→spawn→init 失败摘除→
  status 如实显示「未运行」整链行为正确。
- **遗留**：① 活体 TUI 验证（rust 仓起会话 `lsp` 工具自动注册 + rust-analyzer
  拉起；/lsp on 零回合热加载；带历史拒绝；status 面板）；② 默认开 + eager
  bootstrap 的内存尾部（rust-analyzer）——懒启动为后续切片，挂真实抱怨触发；
  ③ T1 诊断按需 op 仍是 improvements todo 第一顺位。

---

### 2026-10-13 主题目录扩容（feat/local-theme-catalog-port）——11 套社区色板移植自 opencode

- **状态**：施工完成待验证合入。从父目录同类 agent 吸收社区好评主题配色：捐献者
  opencode 主题库（32 JSON 资产），移植 Catppuccin Mocha/Latte（第二浅色）、
  Dracula、Gruvbox Dark、Nord、One Dark、Everforest、Kanagawa、Vesper、
  Night Owl、Solarized Dark 共 11 套（与既有 6 主题零重叠，全 truecolor 门控）。
- **落点**：pager-render `theme/` 新 ×9 文件 + mod.rs/cache.rs/syntax.rs 接线；
  pager settings defs 两 catalog + i18n 中文描述 + settings_modal 测试分发 +
  mermaid `theme_is_dark` 纳入 Latte。全部 ~60 槽位逐一映射（编译器强制完整），
  donor 直采 + 色系内手调派生（文件头注明），donor 设计原貌保真。
- **验证**：cargo check render/pager --all-targets + ctest 门控测试（render lib
  theme:: + pager settings/theme/doctor 套件）+ ptyctl 活体抽查（`/theme` 切换
  catppuccin-mocha 渲染）。
- **遗留**：其余 21 套 opencode 主题（synthwave84/flexoki/zenburn/monokai 等）
  同模式可增量移植，未纳入本次。

---

### 2026-10-14 `/lsp select` server 勾选面板（feat/local-lsp-server-picker）——检测 → 勾选 → 确认 → 热加载

- **状态**：施工完成、验证中/已验证（以本条落盘时的测试轮结果为准）。背景：三十五期
  builtin 检测默认全量装配，用户当前项目未必全都要；本条给 LSP server 面加 per-workspace
  禁用清单 + 勾选面板交互（用户 2026-10-14 提出）。
- **语义拍板（默认执行，未单独征询）**：① 持久化 = grok-home
  `feature-overrides` 增 `lsp_disabled_servers` 禁用清单（与 `/lsp on|off` 同文件同层，
  机器本地不落项目仓库）；② 语义 = 只记排除项（取消勾选的才写盘），空清单 = 清除覆盖
  恢复全量跟随；③ 生效面 = `build_lsp_backend` 单入口过滤（config/builtin/plugin 来源
  一视同仁），零回合会话经 `x.ai/lspApply` 热加载；④ 入口 = `/lsp select`（别名
  选择/勾选），`/lsp status` 增已禁用标注与提示行；⑤ 面板需在会话内打开（挂 agent
  视图模态槽），welcome/dashboard 提示先进会话。
- **落点**：shell `workspace_overrides`（新键读写 + 幂等写 + 文件整删惯例）、
  `agent_ops.rs build_lsp_backend`（禁用过滤）、`extensions/lsp_status.rs`（disabled
  标志）；pager `actions.rs`（LspConfigureRequest::Select / Effect select 位 /
  TaskResult 携出参 / Action::LspServersConfirm）、`views/lsp_server_picker.rs`（新
  模态）、`views/modal.rs` / `app/modals.rs` / `dispatch/router.rs` / `task_result.rs`
  接线、`slash/commands/lsp.rs`（select 解析 + status 渲染）、i18n 5 键。
- **验证**：cargo check shell/pager --lib 0 error（首查 rustc 0xc0000005 增量缓存
  损坏，清 incremental 重查通过）；ctest 门控 shell `workspace_overrides` 6/6、
  pager `lsp` 过滤 14/14；i18n 扫描 exit 0。
- **遗留**：① 真机 TUI 活体验证（/lsp select 面板开合、取消勾选 jdtls 后
  status 显示已禁用、零回合热加载生效）；② 面板初始勾选取自 status 快照，取数
  与确认之间 server 环境变化无检测（低风险，记录）；③ 首次检测到新 server 时的
  主动提示（是否弹面板）未做，走用户主动 /lsp select。

### 2026-10-14 `/lsp` 面板化——统一操作面板（feat/local-lsp-panel）——三十七期 select 的交互收敛

- **状态**：施工完成。用户 2026-10-14 追加：把 `/lsp` 的分离子命令（status 文本 /
  on|off / select 面板）收敛为单一面板操作——启用关闭、状态显示、自动发现（启用
  时）、手动勾选确认都在面板内完成。
- **语义拍板（默认执行，未单独征询）**：① 裸 `/lsp`（及 panel/面板）= 打开统一
  面板；`status` 保留文本回显，`on|off|clear` 保留直接子命令，旧 select 入口并进
  面板（兼容别名）；② 面板内 `e`/`t` = 启用/关闭翻转（本地态先翻转保反馈，
  dispatch 写 workspace 覆盖 + `x.ai/lspApply` 热加载，**面板保持打开**）；
  ③ Enter 确认沿用三十七期禁用清单语义（全集 − 勾选集）并关面板；④ 面板渲染整份
  lspStatus 快照（门禁来源 + 覆盖行 + server 勾选清单带运行态/命令探测 + 会话
  后端行），禁用时 warning 色提示「按 e 启用，自动检测」。
- **落点**：pager `slash/commands/lsp.rs`（Panel 解析 + label 抽共享）、新
  `views/lsp_panel.rs`（替代 lsp_server_picker）、`modal.rs`（LspPanel 变体）、
  `modals.rs`（ToggleEnabled 路由）、`router.rs` / `task_result.rs` /
  `actions.rs` / `effects/mod.rs`（select 旗标改名 panel + Panel 臂）、i18n 16 键。
  **shell 侧零改动**。
- **验证**：cargo check pager --lib 0 error 0 warning；ctest 门控 pager `lsp`
  过滤 16/16（新增 panel 取数旗标/无会话提示/回包开面板例）；i18n 扫描 exit 0。
- **遗留**：① 真机 TUI 活体验证（面板开合、e 翻转后 status 同步、勾选确认热加载）
  ——随三十七期同批补；② 三十七期遗留 ②③（取数与确认间环境变化无检测、新
  server 主动提示）继承不变。

### 2026-10-14 `/lsp` 面板二次收敛（同分支 feat/local-lsp-panel；v1.0.57 tag 重指重发）——usage 同款 chrome + 子命令全撤 + Clear 全量重置

- **状态**：施工完成。用户对第一版面板三点反馈全收：① 面板太小/风格与 usage
  modal 不对齐 → 改走 `views/modal_window` 公共 chrome（usage/stats 同尺寸档位
  0.65/100，标题边框 + 七快捷键页脚 + 关闭按钮，内容随光标滚动）；② option
  子命令残留 → `/lsp` 只开面板，on/off/status/clear/select 一律拒绝，
  `LspConfigureRequest::Status` 与 `format_status` 整体移除，`panel` 旗标删除；
  ③ 补面板内 `c` 键清覆盖：Clear 升级全量重置（lsp_tools 覆盖 + server 禁用
  清单串联清除）。
- **拍板**：不新增 v1.0.58，v1.0.57 tag 原地重指重发（用户明确指示；沿
  v1.0.39 / v1.0.51 先例，注意删 tag 曾把 release 转 draft，需重传恢复发布）。
- **落点**：`views/lsp_panel.rs`（chrome 化重写 + c 键 + label fns 迁入）、
  `slash/commands/lsp.rs`（panel-only 解析）、`actions.rs`（Status 删 + 旗标删）、
  `router.rs`（Clear 串联重置）、`task_result.rs` / `effects/mod.rs`（恒开面板）、
  `modals.rs`（ClearOverrides 路由 + 新渲染签名）、i18n 2 键。shell 零改动。
- **验证**：cargo check pager --lib 0 error 0 warning（三连 rustc 0xc0000005 打在
  未触碰 crate，清 incremental + CARGO_INCREMENTAL=0 + -j 4 后通过）；
  GATE_FORCE pager `lsp` 14/14；i18n 扫描 exit 0。
- **遗留**：真机 TUI 活体验证（面板开合/chrome 渲染、e 翻转、c 重置、勾选确认
  热加载）随新客户端补。
### 2026-10-14 Codex 品牌主题（feat/local-codex-theme）——OpenAI Codex CLI 真实 UI 配色移植

- **状态**：施工完成待验证合入。捐献者 = 本地 clone `D:\CODE\ai\codex`
  （codex-rs/tui style.rs / color.rs / markdown_render.rs / 内置 tmTheme），
  取 ChatGPT 蓝体系（UI_ACCENT #63A8F8、BLUE_100 #A4CDFB、warning 琥珀
  #C4A767、secondary 60% blend、user_message 白 12% 叠底、tmTheme 暗前景
  #D0D0D0、diff 绿红 #00AF00/#FF5F5F）；**用户拍板无障碍配色不要**——6 个
  256 色 `.tmTheme` 变体（金/橙 accent）整族弃用。
- **落点**：同三十六期八处接线 +1 变体（`Codex = 18`）：theme/codex.rs 新文件
  （~60 槽位 + 签名测试）、mod.rs/cache.rs/syntax.rs、settings defs 两 catalog、
  slash i18n +1、settings_modal tests 分发、doctor 夹具 17→18、user-guide
  06-theming 表 18 行。mermaid `theme_is_dark` 浅色负名单制零改动。
  实现记录 PATCHES.md 三十九期。
- **验证**：cargo check render/pager --all-targets 0 error 0 warning + GATE_FORCE
  合批测试（render theme 179/0 + pager 526/0；settings 单独 380/380 对齐基线）+
  i18n 扫描 0 新挂；合批首轮一挂为既有全局主题测试隔离 flake（native-lock 测试
  × settings preview 并行互踩，重跑自愈，详见 PATCHES 三十九期运行备注）；
  ptyctl 活体抽查随新客户端。
- **遗留**：opencode 余 21 套社区主题同模式增量移植（继承上条）；
  Latte 浅色下 mermaid 活体抽查（继承上条）。

### 2026-10-14 Ayu 主题（feat/local-ayu-theme）——qwen-code 主题定义移植（叠 codex 分支）

- **状态**：施工完成待验证合入。捐献者 `D:\CODE\ai\qwen-code`
  （`packages/cli/src/ui/themes/ayu.ts`）；**用户拍板避免同质化**，逐候选筛查
  后唯一引入 Ayu（暖橙日落深色，填空位），qwen-dark（ayu 克隆）/shades-of-
  purple（第三紫）/github-dark（同族）/浅色系/ansi 系全部剔除，结论沉淀于
  PATCHES 三十八期。`Ayu = 19`，别名 `ayu-dark`。
- **落点**：同三十九期八处接线；theme/ayu.rs 新文件（~60 槽位 + 签名测试）；
  doctor 夹具 18→19；user-guide 表 19 行；README/AGENTS 同步。
- **验证**：cargo check render/pager --all-targets 0 error 0 warning + GATE_FORCE
  合批测试（render theme 180/0 + pager 526/0；settings 单独 380/0 对齐基线）+
  i18n 扫描 0 新挂；附带根除三十七期记过的 settings_modal 全局主题竞态 flake
  （59 个对照测试统一持 test_lock，fix 提交 f1de484 先行）；ptyctl 活体抽查
  随新客户端。
- **合并顺序**：codex（三十九期）→ ayu（四十期），分支叠建无冲突。
- **遗留**：opencode 余 21 套同模式增量移植时同样先过「同质化」筛
  （qwen-code 全库已筛毕，仅 ayu 入选）；Latte 浅色 mermaid 活体抽查（继承）。

### 2026-10-14 模型组（跨供应商降级链）+ subagent 模型/effort 配置（feat/local-subagent-model-group）

- **状态**：施工完成（PATCHES 四十一期），随 Ayu 主题分支（三十九/四十期）一起合并
  发版（用户指示同批交付）。设计文档 `docs-local/subagent-model-group-todo.md`
  （六拍板点按建议值落地，偏差见其「施工实况」节）。
- **落点**：shell `agent/model_group.rs`（进程级冷却运行态 + 失败签名分类器，新模块）、
  `agent/config.rs`（`[model_groups.<name>]` 三类型 + Config 字段 + resolve_subagents
  冲突 warn）、`config/mod.rs`（`[subagents.model_groups]`/`[subagents.efforts]`）、
  `subagent/mod.rs`（解析链组分支 + 组条目 effort 能力门/variants 路由 +
  `ModelGroupBinding` 三元组返回）、`subagent/handle_request.rs`（efforts
  fill-if-none + 失败标冷却 unified 事件）、`mvp_agent/subagent_spawn.rs`（ctx 三新字段）。
- **v1 语义**：降级 = spawn 级（失败条目进程冷却 → 后续 spawn 顺延下一可用条目）；
  首字节前会话内 failover 与 transcript notice 留 v2。
- **验证**：cargo check shell --all-targets 0 error；新测 8 例（model_group 5 +
  config 2 + 组链优先级 1）；Linux CI shell 套件随合并后 build 验证。
- **遗留**：v2（首字节前 failover/粘性、transcript notice、主循环绑组 G4）；
  活体验证（真实三家 glm-5.3-flash 组断一家的自动顺延）随发版后新客户端。

### 2026-10-15 Deep Dark 主题（feat/local-claude-theme-port）——Claude Code 泄露源码 darkTheme 移植

- **状态**：施工完成（PATCHES 四十二期），待合并发版。预研与判定（dark ✓ /
  light ✓ 次选 / daltonized 可选 / ansi ✗）在
  [`docs-local/claude-theme-port.md`](claude-theme-port.md)，light 与
  daltonized 两档映射草案已备、未施工。
- **落点**：`theme/deep_dark.rs`（~64 槽映射，donor 直采 + 色系内派生）+
  Ayu 期同款九处接线（mod/cache/syntax/defs×2/i18n/settings_modal 测试/
  doctor 夹具 19→20/user-guide 表）；命名 `deep-dark`（用户拍板，别名
  `claude-dark`/`cc-dark`），bg_base 派生拍板 #141414。
- **验证**：cargo check render/pager --all-targets 0 error 0 warning；
  GATE_FORCE render theme 175/0、pager theme 76/0、settings 380/0、
  doctor 42/0；i18n 扫描 exit 0（rustc 0xc0000005 两轮，清 incremental 后绿）。
- **遗留**：light / daltonized 两档移植（映射草案在预研文档）；活体 TUI
  抽查（`/theme deep-dark` 切换 + 256 色 PTY 隐藏 + config 持久化）随发版后
  新客户端。

### 2026-10-15 空闲时自动压缩主上下文（feat/local-idle-auto-compact）——实验特性 idle_auto_compact

- **状态**：施工完成（PATCHES 四十三期），已合 main 待发版。需求原话：空闲时
  自动压缩主上下文，默认不启用，配置文件开启实验性特性（参考 Codex/Claude
  auto-compact）。
- **落点**：`session/acp_session_impl/idle_compact.rs` 新模块（3s 防抖 + busy
  select! 中止 + `run_compact_only` 复用）；`compaction_config.rs`
  `idle_compact_in_flight`（inline/preflight 检查互斥）；`run_loop.rs` 成功回合
  空闲点 spawn；`util/config/resolve/features.rs` 解析器（env
  `GROK_IDLE_AUTO_COMPACT` > `[features] idle_auto_compact` > 默认关）。
- **验证**：cargo check shell --all-targets 0 error 0 warning；新测 5 例；config_docs
  行校验绿。CI 四轮收口（run 38768559297→38773838791→38823680111→38894033624，
  修复链与根因见 PATCHES 四十三期补记）；夹具 cwd 修复后本机模块 42/42、
  win-skip 族S 撤销本模块条目。随 v1.0.60 发版。
- **遗留**：活回合验证（真会话开 flag 看 fired/aborted 事件与下一条 prompt 的
  TTFT 改善）随新客户端；配置无 per-session 覆盖（跟随全局开关）。

### 2026-10-15 视觉桥 T1/T2/T3+D8（feat/local-vision-bridge）——纯文本主模型的图像转写

- **状态**：施工完成（PATCHES 四十四期），已合 main 待发版。对照
  vision-bridge/plan.md D1–D8 全落地；D9 二级（400 反应式摘除转桥）为 v2 增量未做。
- **落点**：`session/acp_session_impl/vision_bridge.rs`（决策 + 披露文案 + 测试）、
  `util/config/resolve/vision.rs`（D3 授权解析）、config 三层 accepts_images +
  ACP meta + ModelsManager 访问器、turn/interjection/transcribe 三接线（D4
  失败降级）、tool_calls 四口子 + D6 预算、`XaiSessionUpdate::VisionBridgeNotice`
  三面（pager/headless/rewind-window）、model-catalog accepts_images 解析 +
  prices.cache v3 + 启动预热回填（toml_edit 定点）。
- **验证**：cargo check shell/pager/model-catalog --all-targets 0 error 0 warning；
  model-catalog 41/41；shell GATE_FORCE 决策矩阵/D1 缺省表/config_docs 行校验绿；
  pager 36/36；i18n 0 挂；win-skip 族W/X/Y 登记（Linux CI 验证）。CI 决策测试
  LocalSet 修复（`4a2ddbf`）后全绿，随 v1.0.60 发版。
- **遗留**：D9 二级反应路径（400 签名表按供应商归档，v2）；活回合验证（真实
  VL 转写质量、BYOK 剥图告知、工具图预算耗尽路径）随新客户端；backfill 无
  单测（依赖用户 config 面，unified 日志可观测）。

### 2026-10-15 模型选择器目录详情展示——立项记录（未开工）

- **状态**：用户口头需求（通过缓存的模型信息，在选模型时右边显示模态/支持思考/
  上下文长度/美元价格），仅立项不施工。文档
  [`docs-local/model-picker-catalog-details-todo.md`](model-picker-catalog-details-todo.md)
  （数据源盘点：CatalogMatch 四字段全就位；设计草案 + 4 拍板点 + T1/T2 分期，
  估 1–2 人天）。

### 2026-10-17 /stats 压缩标签页换月度统计（feat/local-stats-month-tab）——4 时间窗收口

- **状态**：用户需求「/stats 面板的压缩换成月度统计」落地：工具输出压缩账本
  标签页退场，第 4 标签页改为月度用量统计——恢复 df81acf 原始月窗（滚动
  30 天，`WINDOW_MONTH_MS`，`RETAIN_MS=35d` 本为其预留）。`StatsTab` 包装枚举
  删除，`Window` 直驱 4 标签页（5h/day/week/近一月）。压缩账本数据面不动
  （headless `grok stats --json` 照旧）。PATCHES 四十五期；随 v1.0.61 发版。
- **遗留**：活体 TUI 抽查（新客户端 /stats 看 4 标签页 + `/stats month`
  直跳 + 极简文本报表月窗段）；自然月（本月 1 号起）口径如需再立项。

### 2026-10-17 /model picker 右侧目录详情列（feat/local-model-picker-catalog-details）——T1 收口

- **状态**：用户拍板「右边空白区域显示模态/上下文/思考 + 定价」落地。行右侧
  `right_label` 列（原 description 常空）改显目录四字段 token 串
  （`文本 256k reasoning $3/$15`），未知段省略、负缓存显式 `–`；本地 meta 恒
  优先（上下文/模态/思考），价格只来自目录。ArgPicker 渲染时查进程级 store
  （回包落地下一帧自动补显），picker 打开经新 `Action::FetchModelCatalogForPicker`
  触发取数（FetchSessionList 先例）。**活体验证补强**：dashboard `/model` 下拉
  （用户日常所见面）走 `SuggestionRow` 复制 ArgItem.description——
  `build_model_items` 改为详情 token 前置拼进 description；pager 侧 store 由
  「模型列表到达即预取」`prefetch_effect()` 加热（三处 lifecycle/load 安装点 +
  `x.ai/models/update` 通知路径，补齐 dashboard 下拉永远冷 store 的缺口）。
  纯 pager 面，无 shell 改动。PATCHES 四十六期。
- **验证**：cargo check 四 crate --all-targets 0 error；GATE_FORCE 四 crate lib
  批量过滤 65/65 + 注入单测；i18n 扫描 exit 0；ptyctl 活体（release bin 实测：
  `/model` 下拉右列出「文本 1M 推理 $0.02/$0.4 · Volcengine Ark coding plan…」，
  模态/上下文/思考/价格四字段 + 原描述齐活）。
- **遗留**：T2 剩余（窄终端主动折叠、stale-while-revalidate 标注）未见需要；
  活体 TUI 抽查随新客户端。
- **验证坑（已入 memory）**：TUI 活体必须编 `-p xai-grok-pager-bin`——bin 在
  独立 composition-root crate，`-p xai-grok-pager` 只编 lib，旧 exe 会静默
  顶替（本次实证 mtime 停在 10-14，五轮「验证」无效）。

### 2026-10-17 命令提示跟随实际二进制名（feat/local-resume-hint-bin-name）——收口

- **状态**：用户报「`grok --resume <id>` 提示未同步发行名 grok2」，要求任意
  改名自适应。新 `app::cli::current_binary_name()`（argv[0] 文件名，剥 `.exe`，
  缺省 grok）；修 `session_title_resolve`（resume 提示）与 `startup_failure`
  （`grok leader kill`）两处用户可见命令示例。diagnostics 别名族
  （`grok doctor fix` / `grok wrap ssh` + doctor 写 shell 配置的 alias）影响
  配置产物面，单独立项 open（issues/2026-10-17-binary-name-in-command-hints.md，
  P3）。PATCHES 四十八期。
- **验证**：cargo check --all-targets 0 error；ctest pager lib
  `display_name_from_argv0` 1/1。
- **遗留**：diagnostics 别名族适配（open，见上）；活体触发 resume 报错路径
  随新客户端。

### 2026-10-17 视觉桥 D9 tier-2 反应式摘图转桥（feat/local-vision-bridge-d9）——收口

- **状态**：v1.0.60 视觉桥遗留的 D9 第二级落地：400 且高置信「不支持图像输入」
  签名（中英网关方言归一，Api 400/415）→ 采样器既有 RetryWithImageStrip 归
  ServerRejected → shell 挂在请求上的转写钩子（eligibility 与前置门同构：只挂
  accepts_images==true + D3 授权 + 描述模型识图）把剥掉的像素转成 D5 envelope
  注入重试请求；失败臂按 D4 占位继续。PayloadHeuristic（413/连接重置）维持纯
  剥图；置信不足的 400 一律不动。成功发 transient VisionBridgeNotice +
  unified 日志。PATCHES 四十七期；实现细节与取舍见
  [`docs-local/vision-bridge/plan.md`](vision-bridge/plan.md) D9 施工记录节。
- **验证**：sampling-types lib 3/3 + sampler lib 26/26 + 集成 test_actor 2/2
  （假服务器全链路：签名 400 → 转写 → 重试体含 envelope 不含像素）；
  shell vision_bridge 纯函数 6/6（决策矩阵 6 例为已登记 /tmp 夹具环境族，
  Linux CI 验证）；win-skip 族W 补登记 recap_display_only_tests。
- **遗留**：D9 活体验证（真实「能力位说 true 上游拒图」场景：notice、转写质量、
  重试时延）随新客户端；三级拓扑的第三级（桥不可用终态告知）四十四期已覆盖，
  无新增。

### 2026-10-17 DeepSeek 官方 API 余额上 /usage 面板（feat/local-deepseek-balance-usage）——收口

- **内容**：供应商套餐用量第六家（用户点名，官方接口
  https://api-docs.deepseek.com/zh-cn/api/get-user-balance）。provider-usage 增
  `UsageProviderId::DeepSeek` + `UsageWindowKind::Balance`（余额是金额非百分比，
  金额进 note；`granted_balance>0` 附 `· grants`，`is_available=false` 追加
  `· unavailable`）+ `providers/deepseek.rs`（`GET /user/balance` Bearer，
  origin 跟随 deepseek.com base_url host）+ base_url 匹配 `api.deepseek.com`；
  pager 块标题分流（DeepSeek → 余额）+ 余额窗渲染（无 bar 无 unknown 行）+
  i18n「余额」。shell 扩展零改动（通用管线吃第六家）。判定边界：Ark /
  Command Code / ClinePass 的 deepseek 系列 id base_url 各指网关不命中。
- **验证**：cargo check 三 crate --all-targets 0 error 0 warning；
  provider-usage lib 63/63（+9）、pager usage_modal 25/25（+1）、i18n 扫描
  exit 0；活体验证（ptyctl 真 TUI + 真 API，release bin 核对 mtime）：官方
  1017 面板「余额 — DeepSeek / 余额 / ¥99.89 / 查询于 10-17 19:50」，切回 Ark
  回归 SuperGrok 块零变化。merge `25fa7be`。
- **遗留**：随下次发版；`grants`/`unavailable` 标记活体未采到（本机余额无赠金），
  夹具单测覆盖。PATCHES 四十九期。

### 2026-10-18 DeepSeek hosted-search 供应商分流（feat/local-deepseek-hosted-search-gate）——收口

- **内容**：handoff 任务单 ② responses 通道（用户拍板「分期」，anthropic 通道
  另立项排队）。`SessionActor.hosted_search_vendor` 档位（spawn 按 base_url
  host 判定、模型切换同 `supports_backend_search` 更新点随采样配置更新），
  `resolve_hosted` 在 Deepseek 档剥 xAI 专有 `XSearch` 声明；漏斗唯一性核实
  （turn/side_call/recap/compaction 全走 `hosted_tools_for_turn`），sampler /
  sampling-types / agent / pager 零改动。host 判定复用 provider-usage
  `provider_for_base_url` 先例。启用仍由用户显式 `supports_backend_search = true`；
  user-guide 26/11 随更。merge `60ab08a`，commit `dd29bde`。
- **验证**：cargo check shell --lib 0 error；ctest（GATE_FORCE）
  `hosted_search_vendor` 4/4；测试夹具 SessionActor 直构 11 处补缺字段；
  合并树三 crate check 0 error。
- **遗留**：活体验证（v4-pro 条目 + `supports_backend_search = true` 全链路，
  看 x_search 不再出现在请求）随新客户端；anthropic 通道（flash 可用，
  messages 流 `server_tool_use` 块解析）另立项。PATCHES 五十期。

### 2026-10-18 目录思考位回填 reasoning_efforts（feat/local-catalog-reasoning-backfill）——收口

- **内容**：handoff 任务单 ③。施工前现场重抓双源载荷推翻「目录无档位」旧
  dump 结论（OpenRouter `reasoning.supported_efforts` / models.dev
  `reasoning_options` 原厂条目均带档位），按用户指示升级目录驱动零手写、
  静态表方案作废。`CatalogEntry.reasoning_efforts`（归一化解析）+
  `backfill_model_capabilities` 单遍历三字段 + `prices.cache` v4；红线 = 只在
  键缺失时回填、显式键（含 false/空表）永远赢、目录 reasoning=true 才写、
  档位按枚举过滤全未知不写；幂等 + 逐字段 unified 日志。用户本机 glm-5.3-flash
  （Ark）已手工补两行，另两条 glm 条目随特性自动补。D8「backfill 无单测」
  遗留补齐（`_at(path)` 参数化 + 7 用例）。merge `1fbc991`，commit `7a740c0`。
- **验证**：model-catalog lib 43/43（+2）；ctest（GATE_FORCE）
  `backfill_capability` 7/7；合并树三 crate check 0 error。
- **遗留**：活体验证（删测试条目思考位 → 启动看回填日志与 /effort 菜单）
  随新客户端；hot 生效语义沿 D8（下次启动）。PATCHES 五十一期。

### 2026-10-21 zcode 3.16.2 CLI 刷新（调研入库，ToolSearch 参照）

- **内容**：父目录 zcode 从 v3.14.3（`29628c9`）拉到 v3.16.2（`3c5b2276`，3 个提交）。旗舰 =
  ToolSearch/DeferredToolCall（`off/auto/native/local` 四模式、常驻名单 + `shouldDefer` +
  MCP `alwaysLoad`、select/关键词搜索、公告流、compact 后提升、包装执行与冷恢复），已作为
  workflow-pi-port P2 T2 第四参照写入
  [`deferred-tool-exposure.md`](workflow-pi-port/deferred-tool-exposure.md) §2.3；其余 CLI 改进
  与内置 skill/plugin 清单（superpowers 14 / zcode-guide 6 / skill-creator / plugin-creator /
  office 四件）见 [`agent-cli-tracking/notes/zcode.md`](agent-cli-tracking/notes/zcode.md)，
  移植定级见 [`agent-cli-tracking/ADOPTION.md`](agent-cli-tracking/ADOPTION.md) §8。
- **验证**：纯文档；`notes/zcode.md` 的路径与行号均在本机 zcode 3.16.2 源码实读；未改 fork 代码。
- **遗留**：ToolSearch 的 Windows/Linux 与真实缓存命中 zcode 自报未验；superpowers 三件技能
  A/B 未做；FillWorkflowHole / Bash 图片输出 / MCP 资源订阅 / LLM selector 待触发信号。

### 2026-10-22 LLM API 异常响应落盘（`[api_dump]`）+ 压缩传输重试对齐

- **内容**：`feat/local-llm-api-dump`。新 `xai-grok-sampler/src/api_dump.rs`：
  非 429 采样失败按日期写 `$GROK_HOME/dump/<date>/<ts>-<kind>-<backend>-<mode>-<model>-<req>.json`
  + `index.ndjson` 一行（16 MiB 轮转、每日 200 文件上限、凭据头/敏感 query 脱敏、
  正文与 SSE 前缀按配置截断）；六个发送点（chat/responses/messages × stream/nonstream）
  全部接线。压缩路径对齐主回合：`is_transport_failure` 判 `Transient` 且 detail 含
  `reqwest error stream: ` / `request error: `，`ShellCompactionSampler` 客户端改
  `Mutex<OaiCompatClient>` 并在传输失败后的下一次尝试换 `force_http1` 新客户端，
  重试 3→5。配置 `[api_dump]`（`enabled` 缺省 true、三个上限）；env
  `GROK_API_DUMP=0` / `GROK_API_DUMP_DIR`。
- **验证**：sampler lib 300/300；shell `session_compact` 49 passed/1 ignored；
  `--features config-docs` `config_docs` 10/10；pager `docs::` 19/19；三 crate
  check 0 error 0 warning。活体（release 产物 + 本机 mock）：400 落盘且
  `authorization` = `[REDACTED]`、429 无记录、`enabled=false` 与 `GROK_API_DUMP=0`
  均不建目录、`GROK_API_DUMP_DIR` 换点生效。
- **发版**：PR #2，merge `6054081`，tag `v1.0.66`；release run `39702900602`
  双 job success、4 资产非草稿；本机部署 `grok 1.0.66 (6054081530b1)`，
  回滚点 `grok2.exe.1.0.65-6fc83fa.bak`；用户 config 已启用 `[api_dump]`。
- **遗留**：dump 无自动清理（按日目录只增不减，需人工或后续加保留策略）；
  `index.ndjson` 轮转只保留一代；`stream.frames` 上限内保留原始 SSE 文本，
  大 payload 场景磁盘占用需按 `max_files_per_day` 兜底。PATCHES 五十四期。

### 2026-10-22 压缩失败根因收口（docs：compaction-failure-rca.md）

- **内容**：把两起压缩失败（09-23 半答并行 `tool_calls` 400 → `b40cb95`；10-19
  Command Code 链路抖动 + 压缩「3 次重试 + 单客户端」→ `fc0e899`）合并成一份根因
  记录 [`compaction-failure-rca.md`](compaction-failure-rca.md)：现场工件字段、
  上游严格度对照（DeepSeek 400 / MiMo 200）、67 次传输重试的精确拆分、
  `unified.jsonl` 覆盖起点坑、取证 playbook、四条遗留。归档文
  `archive/compaction-dangling-toolcall-400.md` 按约定追加「后记」指向新文；
  PATCHES 五十四期与 AGENTS.md 待办索引同步加链。
- **验证**：纯文档；文中 file:line 锚点与两起会话工件的数值均在本机实读复核
  （09-23：439 条 / 26 工具 / attempts=1；10-19：176 条 / attempts=3 /
  dangling=0；retry_state 68 条 = 67 传输 + 1 条 429）。
- **遗留**：见文末四条（压缩重试参数未对齐主回合 15 次指数退避、two-pass 单次尝试、
  孤儿 `ToolResult` 未在压缩 prep 兜底、400 诊断文案仍显示成 sampler build failed）。

### 2026-10-24 LSP 增强最终设计定稿（docs：lsp-port/enhancement-final-design.md）

- **内容**：LSP 增强交接定稿——在前两份调研（lsp-port README 三家对照 +
  improvements-todo T1/T2/T3 规划）之上补两轮社区口碑检索（Claude Code 原生
  LSP code intelligence 插件 / Serena / zircote-lsp-tools / aider 反方立场）
  与 zcode、opencode v2 方案补充，整合为 P0–P5 开工顺位：P0 工具描述路由
  指南（随 T1 搭车）→ P1 T1 诊断按需 op（加 pull.rs 分流 + settle 两补丁）→
  P2 懒启动（从「挂触发」上调）→ P3 T2 call hierarchy → P4 T3 rename →
  P5 挂触发三件（符号体一步取 / Windows 检测回归用例 / hook 执法层）。
  **重要修正：opencode v2 重写砍掉了 LSP 运行时**（官方迁移文档原话 + 本地
  clone 全仓零 didOpen 实证），原调研「opencode ✅ 成熟」仅对 v1 成立；
  zcode 无 LSP 子系统（仅 FileMutationService 通知槽位设计，missing）。
- **验证**：纯文档；关键断言一手核实——opencode clone `22e771f`（2.0.32）
  全仓 grep 零 `textDocument/didOpen`、edit/write 工具 TODO 原文实读
  （`packages/core/src/file-mutation.ts:131`）、官方 migrate-v1 文档与
  changelog（至 v2.1.2 无 LSP 回归条目）webReader 抓取、opencode #12288
  经 gh api 核实（2026-02 closed，v1 时代）。
- **遗留**：P0–P5 全部未开工（设计定稿）；D1–D7 待拍板（均给默认倾向，
  可默认执行）；fork 侧 lsp crate 现状锚点已复核（`LspOperation` 仍 6 op、
  `read_diagnostics` 就绪、pull.rs 已存在）。

### 2026-10-24 LSP P1 诊断按需 op + P0 描述路由指南（feat/local-lsp-diagnostics-op）

- **内容**：enhancement-final-design.md 定稿首期施工。T1 两 op
  （`diagnostics`/`workspaceDiagnostics`）直读 per-server store：单文件
  带版本感知 settle（fresh 1s / stale 300ms / pull 2s，超时尾注重查）+
  pull-only 分流（Roslyn 类 server 主动 will_answer 一次）；workspace
  聚合 tracked-only + limit 100 截断 + 诚实边界注。P0 = 工具描述路由段
  （grep-vs-LSP 分工 / navigate-then-edit / 改完当回合自验）。P5 搭车 =
  builtin 五语言检测 + PATHEXT 语义回归用例。
- **附带收获**：lsp 套件首次本机跑（crate 从未进 win-whitelist），44 挂
  分族收口——mock 脚本 Windows 文本模式 stdout（`\r\r\n` 帧头）、pull
  mock 盘符路径 marker 错位、**restart.rs replay_tracked_documents 的
  `file://` 剥离真 product bug**（Windows 上 server 重启后 tracked docs
  全部静默丢弃）。前两族修 mock，第三族修产品（Url::to_file_path）。
- **验证**：GATE_FORCE `ctest.sh -p xai-grok-tools --lib lsp::` 119/119；
  cargo check 0 error；i18n 扫描 exit 0。watched_files out-of-workspace
  glob 的 Windows 误命中登记 win-skip 族AA（挂触发，未修）。
- **遗留**：真实 server 手验随发版活体；P0 描述层实测后定 hook 执法层
  （D7）；落点清单在 PATCHES 五十六期（上游同步重放用）。

### 2026-10-24 LSP P2 懒启动 + P4 rename 写路径（feat/local-lsp-lazy-start → feat/local-lsp-rename）

- **内容**：enhancement-final-design.md 中高价值两项按评估顺位落地。P2 =
  per-server 懒启动（manager 拆 ensure_server_started 原语 / adapter per-server
  状态机 + 60s spawn 失败负缓存 / notify_file_event 读写触发 / reminders 去
  eager / shell build_lsp_backend 增 eager 参数：spawn false、lspApply true）。
  P4 = 独立 `lsp_rename` 写工具（WorkspaceEdit 两形态解析 + 文件操作拒执行 +
  UTF-16 偏移应用器越界/重叠整批拒 + 原子预检 + D2 失败列已写 + 写后
  notify_file_event 保诊断回喂）。
- **验证**：GATE_FORCE lsp:: 122/122 → 130/130（两期共 +11 用例）；tools+shell
  check 0 error。
- **顺位裁定记录**：P3 call hierarchy 经价值评估降级挂触发（findReferences +
  grep 覆盖八成场景，等「findReferences 不够用」的真实信号再立项）；P5 两件
  （符号体一步取 / hook 执法层）维持挂触发（后者等 P0 描述层实测，D7）。
- **遗留**：真实 server 手验（懒启动面板 + rename 全量改 `LspOperation` 变体
  名）随发版活体；rename 的 diff 面板接入与审批活体留 v2/活体；落点清单见
  PATCHES 五十七/五十八期（上游同步重放用）。

### 2026-10-24 LSP 四项（P0/P1/P2/P4）合 main 随 v1.0.68 发版（交接卡收口）

- **内容**：三分支顺序叠放（五十六→五十七→五十八期）按交接卡
  `.handoff/lsp-enhancements.md` 收口——`feat/local-lsp-rename` tip `9987be0`
  fast-forward 合 main（三分支线性，一次 ff 即按序含入），PR #3 作为发版前
  Linux CI 验证面；tag `v1.0.68` 发版。
- **CI 与发版**：PR build run `40080132346` 双 job success；main build run
  `40081680780` / release run `40081685940` 双 job success、4 资产非草稿；
  Windows zip sha256 `58b75c66…` 与 .sha256 文件一致，发行产物
  `grok 1.0.68 (9987be08d72e)`；本机部署 D:\tool-cli\grok2，回滚点
  `grok2.exe.1.0.67-8019d08.bak`。
- **遗留（活体随新客户端）**：诊断主链路 / workspaceDiagnostics、懒启动
  （spawn 不拉起 / 首触达拉起 / e 键 / 负缓存）、rename 全量改枚举名 +
  审批面、restart 回放恢复（Windows）——清单见交接卡 §12；P3/P5 挂触发不变。

### 2026-10-24 缓存命中率与稳定性优化（cache-strategy-port）开工——拍板 + probe 门通过 + P1/P2 施工

- **内容**：交接卡 `.handoff/cache-hit-optimization.md` 接手。拍板 D-a…D-d 按
  建议值落档（P2 做 / 复用 `[compaction.pruning]` enabled + per-model 能力位 /
  P3·P3' 随批顺带 / 顺位本条目）。**probe 门通过（硬前置）**：四象限两连发
  实验（DeepSeek 官方 chat_completions / responses / messages + Command Code
  网关主通道）全部接受中部 system 且最新快照生效；放行矩阵 = cc/resp 放行、
  messages 暂不放行（fork 折叠转换层 + 端点降级双原因），结果回填
  dsh-port context-engineering-design.md §3.2。施工 = P1 in-history 能力位
  （`system_prompt_update`）+ P2 追加式裁剪改写（`prune_mode = "append"`，
  失效标记块替代原地改写），分支 `feat/local-dsh-in-history`。
- **验证**：probe 零成本（53–92 prompt tokens/请求）；代码验证随分支提交补记。
- **施工实况（同日补记）**：P1+P2 已落地（PATCHES 五十九期，分支
  `feat/local-dsh-in-history` 提交 `1fbad9c`）——能力位
  `system_prompt_update`/`prune_mode` 全链接线（use_proxy 先例 6 构造点 +
  4 派生点）、in-history 双字段 wire 态（head 冻结 + last_snapshot 追加判定，
  修复单字段设计 position 0 换字节的断崖错误）、append 裁剪标记 call-id 键控
  类别无关。验证：chat-state 407/407 / sampler 300 / sampling-types 314 /
  pager docs 19 / shell override 15 全绿，shell --all-targets check 0 error。
  **发版**：PR #4 CI 绿（`reset_title_to_auto…` flake 重跑过）合 main
  （`e86015c`）随 tag **`v1.0.69`** 发版——release run `40101135537` 双 job
  success、4 资产非草稿，sha256 `094aede4…` 一致，本机部署
  `grok 1.0.69 (e86015c19194)`（回滚点 `grok2.exe.1.0.68-9987be0.bak`）。
  main build run `40101077754` 同 flake 当日第 3 次挂（重跑绿）——**立案线已到**。
- **遗留**：limit-probe A/B 活体（触发点后首轮 cached_tokens 比对）随新客户端；
  messages backend in-history 需先改折叠转换层；P3 sidecar 串行化 + 归因头
  probe 视批次余量。

### 2026-10-27 /stats 原生选择面撤回（用户拍板不做面板内框选）

- **内容**：撤销六十一期给 /stats 加的「面板期间释放鼠标捕获」。v1.0.69 实测
  暴露出 `[✗]` 关闭按钮失效与拖选越出面板（终端原生选择无法裁剪，会带上背后
  对话正文）两个副作用。用户拍板「不要支持可选复制，简单点、不容易出问题」。
  删 `AppView::stats_modal_open()` 与 `sync_native_selection_mouse()` 的
  stats 分支；auth 原始 URL 面板机制不动；`y` 只复制当前标签页保留。
  分支 `fix/local-stats-modal-revert-native-select`，PATCHES 六十二期。
- **验证**：cargo check pager --all-targets 0 error；ctest `stats_modal`
  18/18（删 1 例原生选择面用例、新增 1 例 `[✗]` 点击回归，
  `y_copies_only_the_active_tab` 在）。
- **遗留**：活体复核 `[✗]` / 滚轮 / 标签点击随新客户端。
- **发版**：tag `v1.0.70`（`6515ad9e`）release run `40459325505` 双 job
  success、4 资产非草稿；main build run `40459298820` 双 job success；本机
  部署 `grok 1.0.70 (6515ad9e6c24)`（`D:\tool\cli`，回滚点
  `*.1.0.69-ac859850.bak`）。

