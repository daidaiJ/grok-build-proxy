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
