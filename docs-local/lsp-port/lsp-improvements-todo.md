# LSP 特性改进施工规划（lsp-improvements-todo）

> **状态：规划完成、未开工**（2026-10-12 立项，随 v1.0.53 前后定稿）。
> 来源：[`README.md`](README.md) §7 改进评估（对照 crush / opencode / qwen-code
> 三家的缺口矩阵与顺位拍板）。基线：`main` `4b70762`（/lsp workspace 启停已落地，
> PATCHES 三十四期）。
> 分支拟：T1 `feat/local-lsp-diagnostics-op` / T2 `feat/local-lsp-call-hierarchy` /
> T3 `feat/local-lsp-rename`；各期独立发版或合并发版视工期定。

## 0. 顺位总览

| 期 | 特性 | 价值 | 成本 | 结论 |
|---|---|---|---|---|
| T1 | 诊断按需拉取 op（`diagnostics` / `workspaceDiagnostics`） | 高——模型重构后当回合自验「还有什么错」，闭环从被动等 reminder 变主动确认 | 小——backend `read_diagnostics` 已实现，纯暴露层 | **做，第一顺位** |
| T2 | call hierarchy（prepare / incoming / outgoing） | 高——重构前摸调用面，替代 grep+read 低效循环 | 中——三步协议 + 陈旧防护 | **做，第二顺位** |
| T3 | rename（写路径） | 中高——rust-analyzer 批量改名替代模型手改漏改 | 中偏大——写审批 + WorkspaceEdit 落盘 + 回滚 | **做，第三顺位（独立切片）** |
| — | code actions / 热重载 / 多根 / per-server disabled | 见 README §7.3 #4–#8 | — | **缓/不做**（各挂触发信号） |

## 1. 现状锚点（施工前实测，2026-10-12）

- **统一工具**：`xai-grok-tools/src/implementations/grok_build/lsp/mod.rs`（ToolId
  `"lsp"`，`is_read_only: true` + `ToolScope::Read`）；操作枚举
  `implementations/lsp/types.rs:108` `LspOperation`——现有 6 op：GoToDefinition /
  FindReferences / Hover / GoToImplementation / DocumentSymbol / WorkspaceSymbol。
- **backend trait**：`implementations/lsp/types.rs:61-79`——`dispatch(&LspToolInput)`、
  `drain_diagnostics(timeout)`、`read_diagnostics(&[PathBuf]) -> Vec<FileDiagnosticEntry>`
  （**T1 的核心落点已存在**）、`notify_file_changed` / `notify_file_event`。
- **诊断被动回路**：`reminders/lsp_diagnostics.rs`（编辑/patch 事件 → drain → 150ms
  去抖注入 + answer 版本跟踪）；`implementations/lsp/diagnostics.rs`（publish/install/
  confirm_unchanged 状态机）。**模型侧无按需拉取 op**。
- **生命周期**：`implementations/lsp/restart.rs`（crash monitor + attempts/max_restarts/
  backoff + lifecycle_id 竞态防护 + 重启后文档 replay）；manager 五态通知。
- **配置/门禁**：`~/.grok/lsp.json` + `<cwd>/.grok/lsp.json` + 插件贡献
  （`implementations/lsp/config.rs`）；folder_trust 过滤（shell agent_ops 装配点）；
  `/lsp` workspace 启停（feature-overrides，决策核
  `shell agent/config.rs resolve_lsp_tools_with_override`）。
- **文档同步**：`implementations/lsp/documents.rs` / `workspace_open.rs`（didOpen
  管理与版本）；`watched_files.rs`（client-side didChangeWatchedFiles，防 OS watcher
  风暴）。

## 2. T1：诊断按需拉取 op

### 改动点

1. `types.rs`：`LspOperation` 增 `Diagnostics`、`WorkspaceDiagnostics`；`LspToolInput`
   复用 `file_path`（Diagnostics 必填）与新增可选 `limit`（workspaceDiagnostics 截断，
   缺省 100——对齐 qwen 语义）。
2. `dispatch.rs`：两个 op 分派——Diagnostics 直调既有
   `LspBackend::read_diagnostics(&[path])`；WorkspaceDiagnostics 聚合各 server 已跟踪
   文件（不主动扫盘发现新文件——对齐 qwen「neither discover more documents」边界），
   按 server 分组格式化 + limit 截断标注。
3. `grok_build/lsp/mod.rs`：description_template 增两句 op 说明（含「查询返回的是
   server 当前已发布面，编辑后请重新查询」的口径）。
4. 结果格式化：沿用 `format.rs` 风格（severity 前缀 + range + message + source）。

### 边界与语义拍板

- 查询**不触发** didSave/didChange 重放（被动同步面已由 documents.rs 管理）；目标文件
  尚未 open 时走既有 ensure/open 路径（read_diagnostics 调用链内已有）。
- `workspaceDiagnostics` 只聚合「已跟踪文档」（本连接 delivered 过的），不承诺全工作区
  ——诚实边界写进工具描述，防止模型把空结果当「全库无错」。
- 空结果显示为「clean（server=<name>）」而非纯空串（对齐 qwen「Successful empty
  diagnostics still display as clean」）。

### 验收标准

- 夹具单测：Diagnostics 单文件（有错/无错/未 open 文件自动 open）、WorkspaceDiagnostics
  聚合 + limit 截断标注、op 序列化名 camelCase（`workspaceDiagnostics`）。
- 真实 server 手验：rust-analyzer 开本项目，人为制造编译错误 → `lsp diagnostics` 返回
  → 修复 → 重查 clean；`workspaceDiagnostics` 在多文件错误场景返回分组结果。
- 回归：lsp 模块既有单测全绿；`ctest.sh -p xai-grok-tools --lib lsp`（tools crate 是否
  在白名单需查 win-whitelist.txt，不在则 GATE_FORCE）。

## 3. T2：call hierarchy（prepare / incoming / outgoing）

### 改动点

1. `types.rs`：`LspOperation` 增 `PrepareCallHierarchy`、`IncomingCalls`、
   `OutgoingCalls`；`LspToolInput` 增可选 `call_hierarchy_item`（incoming/outgoing 必填，
   为 prepare 回包原样透传的 JSON）。
2. `dispatch.rs`：三步协议——prepare（位置参数）→ item；incoming/outgoing（item 参数）。
   async-lsp 的 `CallHierarchyItem` / `CallHierarchyIncomingCall(s)` 类型可用性需在
   开工首日核实（缺则本地 serde 结构体补齐，仅取 name/kind/uri/range 字段）。
3. `format.rs`：calls 结果格式化（调用方 uri + range + name，fromRanges 摘要）。
4. **陈旧防护（qwen 教训，验收硬项）**：incoming/outgoing 前对 item.uri 做版本核对
   （documents.rs 的已交付版本 vs 当前盘面由既有 notify 链路维护）；版本不一致时返回
   「stale——请对当前位置重新 prepare」的可执行错误，**绝不把陈旧偏移结果冒充「无调用」**。
   v1 不做 qwen 的 HMAC 出处签（fork 单连接模型下版本核对已覆盖主风险，留档）。

### 验收标准

- 夹具单测：prepare→incoming/outgoing 全链（mock backend 回放固定 JSON）、stale 拒绝
  路径、item 透传字段不丢。
- rust-analyzer 实测：对本仓库 `resolve_lsp_tools_for_cwd` 做 prepare → incoming（应含
  agent_ops 调用点）→ outgoing；编辑目标文件后 traverse 返回 stale 提示。
- 工具描述更新 + 既有 op 回归。

## 4. T3：rename（写路径，独立切片）

### 改动点

1. **工具形态（待定决策 D1）**：
   - 方案 a（推荐）：独立 `lsp_rename` 工具（crush 同款独立化）——`is_read_only: false`
     只作用于新工具，既有 `lsp` 工具审批面零变化；
   - 方案 b：`lsp` 工具整体转 non-read-only——声明面省一个工具，但导航六 op 的免审批
     语义被拖累，扩大化。
2. 协议：`textDocument/rename`（newName 参数）→ `WorkspaceEdit` 解析
   （`changes` 与 `documentChanges` 两形态 + textEdit 范围替换）。
3. **落盘与审批**：多文件编辑走 fork 既有写文件审批链（参数级 `ToolRequirement` /
   hooks / 沙箱语义对齐 EditFile）；**全批原子预检**——任一目标路径不可写（只读位/
   沙箱拒绝）则整单拒绝不落盘（对齐 write_model_entries 全批原子惯例）；跨文件失败
   中途处理策略（回滚已写 vs 全拒）为拍板点 D2（倾向：先全量读内存再统一写，写阶段
   失败即报错并列出已写文件）。
4. 未打开文件的 didOpen 顺序（rename 前确保 open）与 workspace/edit 权限协同。

### 验收标准

- 夹具：多文件 WorkspaceEdit 两形态解析、全批原子预检拒绝路径、rename 结果摘要
  （N 文件 M 处）。
- 审批面：非 YOLO 模式下 rename 触发审批；deny 后零落盘。
- rust-analyzer 实测：本仓库对 `LspOperation` 枚举变体改名，验证注释/调用点全量更新。

## 5. 随手件（随任一期搭车，不单独立项）

- `/lsp status` 增强：显示 per-server 就绪态（backend 五态已在 manager）——需 pager↔shell
  通路（ext method 或 meta 扩展），成本小中；等 T1 开工时顺手评估。
- 工具 description 的 i18n 与 op 清单同步（每期必做项，防描述与实现漂移）。

## 6. 待定决策汇总

| # | 决策点 | 默认倾向 | 拍板状态 |
|---|---|---|---|
| D1 | T3 rename 独立工具 vs lsp 工具整体转写 | 独立工具（a） | 未拍板 |
| D2 | T3 多文件写中途失败的语义（回滚 vs 报错列已写） | 先全量读内存再统一写，写阶段失败报错列出已写 | 未拍板 |
| D3 | T1 workspaceDiagnostics 的 limit 缺省值 | 100（对齐 qwen） | 未拍板（可默认执行） |
| D4 | T2 陈旧防护强度（版本核对 vs HMAC 出处签） | v1 版本核对，HMAC 留档 | 未拍板（可默认执行） |

## 7. 与既有文档的关系

- 缺口矩阵与顺位论证：[`README.md`](README.md) §7（本文是其施工化）。
- ADOPTION 7.1.13（LSP 工具对照补）的 call_hierarchy/rename/replace_symbol 三件中，
  前两件由本文 T2/T3 覆盖；replace_symbol（crush 特有，编辑指定符号）随 T3 后评估。
- qwen-code 工程教训的两条硬约束（先准入后 spawn / 陈旧不冒充无结果）已内化进
  T1/T2 验收。
