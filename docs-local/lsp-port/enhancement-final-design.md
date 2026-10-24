# LSP 增强最终设计与交接（2026-10-24）

> **状态：P0 + P1 已落地（2026-10-24，分支
> `feat/local-lsp-diagnostics-op`，PATCHES 五十六期）；P2–P5 未开工。**
> 本文是 [`README.md`](README.md)（三家调研 + /lsp
> v1–v2.2 施工）与 [`lsp-improvements-todo.md`](lsp-improvements-todo.md)
> （T1/T2/T3 施工规划）之上的**最终整合**：补入社区口碑实现两轮检索
> （Claude Code 原生 LSP / Serena / zircote-lsp-tools / aider）与
> zcode、opencode v2 的方案补充，给出开工顺位与核心设计。
> 基线：`main` `f1f916f`（v1.0.67）。分支沿用 improvements-todo 命名：
> T1 `feat/local-lsp-diagnostics-op` / T2 `feat/local-lsp-call-hierarchy` /
> T3 `feat/local-lsp-rename`；P0 随 T1 同分支搭车。

## 1. 本轮新增调研结论（相对前两份文档的增量）

### 1.1 参照实现全景（含新增四家）

| 实现 | LSP 形态 | 核心方案（对本项目有意义的部分） | 社区口碑要点 |
|---|---|---|---|
| **Claude Code**（新增，闭源未入前轮扫描） | 原生 code intelligence 插件（2026 正式化，12+ 语言） | 统一单 `LSP` 工具 ~9 op（含 **call hierarchy + incoming/outgoing**）；**每次编辑后自动回喂诊断**；per-language 插件注册，配置 schema 与 qwen `.lsp.json` 同构（startupTimeout/maxRestarts/trustRequired） | 好评最集中＝编辑后立即看到诊断；**头号抱怨＝模型默认走 Grep 不用 LSP**；次级抱怨＝改完不查诊断、工具链式使用差；工程坑：工具偶发未注册（#27498）、Windows rust-analyzer 检测失败（#16084） |
| **Serena**（新增，MCP 工具包，口碑很高） | 符号级 MCP 工具（`find_symbol` 带 body / `get_symbols_overview` / `find_referencing_symbols`） | **符号级取上下文省 token**：1 次调用拿"定位+函数体"，免两跳；压缩语法大纲 | 批评＝模型对精确文本查找也反射性用 LSP（多付调用延迟）；共识决策矩阵：**精确文本一律 grep，LSP 只做语义操作** |
| **zircote/lsp-tools**（新增，社区执法层） | Claude Code 插件 | 用 hooks/提示词强制 LSP 优先工作流（改码前先定位、重构前分析影响、改完诊断验证） | 它的存在本身即信号：**靠模型自觉不可靠，约束要么进描述要么进 hook** |
| **aider**（新增，反方立场） | ❌ 故意不做 | tree-sitter（grep-ast）+ grep；理由＝LSP 脆弱、语言相关、per-language 进程管理负担 | 2026 年实践已淘汰其一半论据（CC 插件化 + fork 已有重启/退避/检测链路）；**"grep 是一等公民、LSP 是增强"的定位仍然正确** |
| qwen-code | 统一 `lsp` 工具 14 op | 热重载增量 reconcile / 查询时磁盘同步 / call hierarchy HMAC 出处签 / 先准入后 spawn | （原调研已覆盖，不变） |
| crush | 8 工具直出 | 懒启动 canAutoStart + **recentlyUnavailable 负缓存**；诊断 settle 双计时器 | （原调研已覆盖，不变） |
| opencode **v1** | builtin+config 合并、per-(root,server) 多实例、编辑回喂 | （原调研已覆盖，但**仅适用于 v1**，见 §1.2） | — |
| opencode **v2** | **❌ 运行时已砍** | 见 §1.2 | — |
| zcode | ❌ 无 LSP 子系统 | 见 §1.3 | — |

### 1.2 opencode v2 砍掉 LSP：事实、原因与教训（重要修正）

原调研对照表里 opencode「✅ 成熟」的结论**只对 v1 成立**。实测本地 clone
（`D:\CODE\ai\opencode`，dev `22e771f`，2026-10-17，package.json 2.0.32）：

- **全仓（packages/，含 .tsx）零 `textDocument/didOpen`**——v1 的
  `packages/opencode/src/lsp/` 整个模块在 v2 重写（新 `packages/core` +
  共享后台服务架构 + Bun→Node + 新 server API/SDK）中没有移植；
- 仅存留：config schema（`packages/schema/src/config/lsp.ts`：
  command/extensions/disabled/env/initialization）、两处 UI 配置面
  （app settings `project-lsp.ts` / gui-extensions `configured-lsp.ts`）、
  以及 edit/write 工具里的 TODO——`packages/core/src/file-mutation.ts:131`
  「TODO: Notify LSP and collect diagnostics after LSP runtime exists」、
  `tool/plugin/edit.ts:205` 同文；
- 官方迁移文档原话（opencode.ai/v2/docs/migrate-v1）：*"V2 accepts and
  preserves `lsp` configuration, but it does not run language servers, expose
  LSP tools, or produce LSP diagnostics. Replace workflows that depend on
  those capabilities with the project's lint, typecheck, or compiler
  commands."*（配置兼容保留，运行时砍掉，过渡指引＝用 lint/编译命令替代）；
- 截至 v2.1.2（2026-10-21 changelog）无运行时回归条目；docs 页
  （`packages/web/src/content/docs/lsp.mdx`，34 内置 server 表）仍是 v1
  旧文未清，勿据此判定 v2 有 LSP。

**为什么砍**（官方口径 + 证据推断）：v2 核心从"每会话进程"改为"单一共享
后台服务"（多客户端工作台：tabs/停靠面板/worktree），v1 LSP 运行时与旧
架构耦合深（per-(root,server) 实例、会话内 documents 状态、effect 运行时），
重写首版没带上；为用户兼容保留配置接受面，过渡期建议编译器/lint 命令，
运行时留 TODO 分批重建。社区反应：v2 回归抱怨（r/opencode "Opencode 2 vs
v1.19.0"），社区 fork 以"保留 LSP"为卖点。

**教训三条（进本项目工程纪律）**：
1. LSP 是横切运行时（文档会话 + 工具面 + 诊断回喂 + 文件监听），架构重写
   后重建成本极高——fork 的 `LspBackend` trait + manager 隔离层是它在上游
   同步中幸存的原因，**上游同步时该模块的 LOCAL 补丁重放优先级最高**；
2. 诊断时序是成熟实现也会栽的坑（opencode v1 时代
   [#12288](https://github.com/anomalyco/opencode/issues/12288)
   "diagnostics skipped or delayed"，2026-02 closed）——fork 的 150ms 去抖 +
   `confirm_unchanged` 状态机方向正确，T1 的 settle 等待建在同一状态机上；
3. opencode v1/v2 对「默认关」vs fork「默认开」——fork 更激进，必须有懒启动
   对冲（见 §3 P2），否则重演 eager bootstrap 内存尾部。

### 1.3 zcode：无 LSP 子系统，但有可借鉴的管线设计

本地 clone（`D:\CODE\ai\zcode`）确认无 LSP 实现（与前轮扫描 ❌ 一致），
相关设计只有一处：**受控文件变更管线**——Write/Edit 工具提交
`WriteFileIntent` → `FileMutationService` 统一负责读缓存校验、mtime/content
比对、diff 生成、file history 和 **LSP/IDE 通知槽位**
（`apps/zcode-cli/docs/design/v2/tool/02-write.md:130`；实现状态表标
`file history / LSP / IDE 通知 | missing`）。

对 fork 的意义：fork 已有等价物（编辑/patch 事件 →
`reminders/lsp_diagnostics.rs` 诊断回喂 + edit 工具事件链），zcode 的设计
印证了"所有文件变更收敛到单一管线再通知 LSP"是正确形态——fork 无需改动，
但 T3 rename 的多文件落盘**必须走既有编辑工具事件链**（而不是绕开直写），
否则诊断回喂会漏 rename 的结果。

### 1.4 社区口碑对本项目判断的背书与修正

- **统一单工具多 op 形态被 CC 背书**（CC 也是统一单工具 ~9 op，不是 crush
  的 8 工具直出）——fork 现形态不变；
- **T1/T2/T3 顺位全部被 CC 原生 op 清单背书**（诊断回喂好评最集中、
  call hierarchy 在原生 ~9 op 内、rename 是 aider issue #1107 用户点名的
  头号用例）；
- **新增缺口**：描述层路由引导（针对"模型不用 LSP"与"滥用 LSP"两大抱怨）
  ——原缺口矩阵没有这一行；
- ** Serena 的符号体一步取**是原矩阵外的可选增量（挂触发，见 §3 P5）。

## 2. 最终开工顺位（定稿）

| 顺位 | 项 | 内容 | 成本 | 依据 |
|---|---|---|---|---|
| **P0** | 工具描述路由指南 | `lsp` 工具 description_template 加 grep-vs-LSP 路由 + navigate-then-edit + 改完自验口径 | 极小（纯描述） | CC 头号社区抱怨的 fork 侧最轻解法；随 T1 同分支同发 |
| **P1** | T1 诊断按需 op | `diagnostics` / `workspaceDiagnostics`（improvements-todo §2 + §3.1 两点设计补丁） | 小 | backend `read_diagnostics` 已有；CC 好评背书 |
| **P2** | 懒启动 | crush 式 canAutoStart + recentlyUnavailable 负缓存 | 中 | **从「挂触发」上调**：默认开 + eager bootstrap 内存尾部是现实问题；CC 的 env 门起步形态也印证保守路径价值 |
| **P3** | T2 call hierarchy | prepare/incoming/outgoing + 版本核对陈旧防护（improvements-todo §3 原案） | 中 | CC 原生收录；grep 无替代品 |
| **P4** | T3 rename | 独立 `lsp_rename` 工具 + 先读后写 + 原子预检（improvements-todo §4 原案，D1/D2 按默认倾向） | 中偏大 | aider #1107 点名；写路径必须独立切片 |
| **P5** | 挂触发三件 | ① 符号体一步取（Serena 验证）② Windows builtin 检测回归用例（**便宜，建议随任一期搭车**）③ zircote 式 hook 执法层 | 小–中 | ①②成本小建议搭车；③等 P0 描述层实测不够再立项 |

原「不做/缓」判定全部维持：热重载 reconcile（零回合热加载已消解）、多根
monorepo、per-server disabled（v2.1 勾选清单已等效）、code actions（等 T1
落地看闭环）。另加一条不做：**工具多拆分**（crush 式 8 工具）——CC/qwen/
fork 三方一致选统一单工具。

## 3. 核心设计（各期实现要点与拍板）

### 3.1 P0 描述路由指南（随 T1 搭车）

`grok_build/lsp/mod.rs` description_template 追加路由段（i18n 两语同步），
口径草案：

- 精确文本/字面量/错误信息定位 → 用 grep（快、零依赖、不占 server 资源）；
  **不要**用 lsp 做字符串搜索；
- 找全部引用、调用链、影响面评估、符号大纲 → 用 lsp（语义准确，grep 会漏
  重载/宏/别名）；
- 导航到定义后**先读目标代码再改**（goToDefinition/documentSymbol +
  read_file，不要凭名字盲改）；
- 修改接口/签名后**当回合用 `diagnostics` op 自验**，不要等下一回合 reminder。

### 3.2 P1 T1 诊断按需 op（improvements-todo §2 全案 + 两点补丁）

原案照执行（op 枚举增 `Diagnostics`/`WorkspaceDiagnostics`、复用
`read_diagnostics`、tracked-docs-only 聚合、limit 缺省 100、空结果
"clean (server=x)"、诚实边界写描述）。**两点设计补丁**（原 todo 未含）：

1. **pull-only server 分流**：`implementations/lsp/pull.rs` 已有 pull-model
   诊断通路（为 Roslyn 类只答不推的 server）。`Diagnostics` op 对 push 型
   server 读 store；对 pull-only（或 push 超时且 server 支持拉取）走
   `PullDiagnostics::request` 主动问一次再回——判定复用 pull.rs 既有的
   能力探测（capability 不足不够，还需实测过无推送）。
2. **返回前 settle**：目标文件近期被编辑过时，短窗（~300ms）等待诊断版本
   稳定再返回（crush settle 思想），建在 `diagnostics.rs` 既有
   DiagnosticsStore/Answer 状态机上；避免"查询结果反映编辑前一瞬"。
   超时按现状返回并在结果尾注「编辑后可能尚未稳定，请重查」。

### 3.3 P2 懒启动（新切片）

- 触发点：会话内首次对可检测语言的文件执行工具调用 / 读写时拉起对应
  server（crush `canAutoStart` 思想），替代现行 eager bootstrap；
- **负缓存**：spawn 失败的 server 记 recentlyUnavailable（进程级 TTL），
  TTL 内同语言文件访问不再重试（防每文件白探测）；
- `/lsp status` 兼容：未拉起的 server 标「未启动（懒启动，首次访问时拉起）」，
  已拉起走现有五态；`/lsp` 面板 `e` 键显式启用 = 跳过懒启动立即拉起
  （保留用户强制语义）；
- 兜底：`/lsp on` workspace 覆盖 + 显式 enable 的组合下行为 = 现状
  eager（不改语义，只改缺省路径）。

### 3.4 P3 T2 / P4 T3（improvements-todo 原案不变，摘要背书）

- T2：三步协议 + item 透传；**验收硬项 = incoming/outgoing 前 item.uri 版本
  核对，不一致返回「stale——请重新 prepare」可执行错误，绝不把陈旧偏移结果
  冒充"无调用"**（qwen 教训；HMAC 出处签 v1 不做留档）；
- T3：D1 独立 `lsp_rename` 工具（不拖累六 op 免审批）；D2 先全量读内存再
  统一写、写阶段失败报错列出已写文件；全批原子预检；**落盘走既有编辑工具
  事件链**（§1.3 教训：保诊断回喂不漏）。

### 3.5 P5 搭车件

- **Windows 检测回归用例**（强烈建议随 P1 搭车）：builtin.rs 五语言 PATH
  检测（PATHEXT 语义）各一条单测锁行为——CC 在 Windows 栽过 rust-analyzer
  检测（#16084），一次写完防回归；
- 符号体一步取（挂触发）：`documentSymbol`/`workspaceSymbol` 加可选 body
  附件（限单符号限长度，Serena `find_symbol include_body` 等效）——实测
  模型 documentSymbol→read_file 两跳没问题就不做；
- hook 执法层（挂触发）：P0 描述层实测后模型仍频繁绕过 LSP 再立项
  （参照 zircote/lsp-tools：改码前定位 / 改完诊断验证两个 hook 点）。

## 4. 验收标准汇总

每期的模块级验收沿用 improvements-todo §2/§3/§4 各自清单；全期公共验收：

- 工具 description 与实现不漂移（每期同步 i18n 两语 + op 清单）；
- `ctest.sh -p xai-grok-tools --lib lsp`（白名单不在则 GATE_FORCE）+ 涉及
  crate check 0 error；
- 真实 server 手验（rust-analyzer 开本仓库）每期一条主链路用例；
- 上游同步重放注意：`implementations/lsp/` 属 fork 重镇，PATCHES.md 登记
  每期落点清单（§1.2 教训 1）。

## 5. 待拍板决策（更新版）

| # | 决策点 | 默认倾向 | 状态 |
|---|---|---|---|
| D1 | T3 独立工具 vs 整体转写 | 独立 `lsp_rename` | 未拍板（可默认执行） |
| D2 | T3 多文件写中途失败语义 | 先读内存后统一写，失败列已写 | 未拍板（可默认执行） |
| D3 | T1 workspaceDiagnostics limit 缺省 | 100 | 未拍板（可默认执行） |
| D4 | T2 陈旧防护强度 | v1 版本核对，HMAC 留档 | 未拍板（可默认执行） |
| D5 | 符号体一步取做不做 | 挂触发（两跳实测够就不做） | 未拍板 |
| D6 | 懒启动缺省 vs 保留 eager 开关 | 缺省懒启动 + config `[lsp] eager = true` 逃生口 | 未拍板 |
| D7 | hook 执法层 | 挂触发（P0 实测后定） | 未拍板 |

## 6. 与既有文档的关系

- [`README.md`](README.md) §7.2 缺口矩阵：opencode 一行仅对 v1 成立（v2 已
  砍运行时，见 §1.2）；新增「描述层路由引导」缺口行（§1.4）。
- [`lsp-improvements-todo.md`](lsp-improvements-todo.md)：T1/T2/T3 施工
  细节仍以其为准；顺位以本文 §2 为准（新增 P0/P2，T1 前移的 P2 懒启动
  插在 T1 之后 T2 之前）；T1 设计以本文 §3.2 为准（含 pull.rs 分流与
  settle 两补丁）。
- [`../../agent-cli-tracking/ADOPTION.md`](../agent-cli-tracking/ADOPTION.md)
  §7.6.1：已全部落地（v1–v2.2），无剩余条件。
