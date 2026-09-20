# `/stats` 窗口化 + 时间窗 option + TUI 英文残留 i18n（TODO）

> LOCAL 特性待办。分支 `feat/local-stats-modal-i18n`，基线 `86a6e1d`（= 当时 `main`）。
> 调研快照 2026-09-20；下文行号仅作定位参考，施工时**以文本搜索为准**（同步提交会让行号漂移）。
> 施工顺序建议 T1 → T2 → T3：T1 最小且独立，T2 依赖 T1 的数据层，T3 与前两者无耦合可随时插入。

## 现状调研结论（三件事的共同起点）

- `/stats` 实现：`crates/codegen/xai-grok-pager/src/slash/commands/stats.rs`
  （`StatsCommand`，`slash_meta!` 在 L19-25，`run()` 在 L27-36，`format_usage_report` L40-59，
  `format_usage_row` L62-89，`fmt_tokens` L92-105）。
- 数据源：`grok_home/cache/model-usage.jsonl`，由 shell 每次成功推理追加采样，
  经 `xai_grok_tools::model_usage_ledger` 读写（`load_samples` / `aggregate`）。
- 时间窗常量：`crates/codegen/xai-grok-tools/src/model_usage_ledger.rs:15-17`
  已有 `WINDOW_5H_MS` / `WINDOW_WEEK_MS` / `WINDOW_MONTH_MS`，**没有 day**。
- 当前输出：纯文本，`CommandResult::Message(report)` 进 scrollback，**不是**窗口。
- 当前参数：`takes_args: false`，命令不吃参数。
- 窗口框架已有现成先例：`views/modal_window.rs`（框架）+ `views/usage_modal.rs`
  （3 标签页）+ `views/agents_modal.rs`；打开入口在 `app/dispatch/status.rs`
  （`open_usage_info_modal` L42，设 `ActiveModal::UsageInfo` L111），
  modal 枚举与标题映射在 `views/modal.rs`（`ActiveModal`，标题 L713），
  事件分发在 `app/modals.rs`（L434/L489/L1563/L1640+/L2360）。
- `/usage` 的**带参数命令参考模板**：`crates/codegen/xai-grok-pager/src/slash/commands/usage.rs`
  ——`slash_meta!` 声明 `takes_args: true`（L46-51）、`suggest_args` 提供补全项（L69-84）、
  `run()` 里 `match arg` 分支 + 未知参数报错文案（L86-107）。**T1 照抄这个模式。**

---

## T1：`/stats` 加 option，分别查看 5h / day / week / month

### 目标

`/stats` 支持可选参数选择单个时间窗；不带参数时保持现有"全窗口"输出（向后兼容）。

- `/stats` → 全部四个窗口（现状 + 新增 day）
- `/stats 5h` / `/stats day` / `/stats week` / `/stats month` → 只输出该窗口
- 未知参数 → `CommandResult::Error`，文案风格对齐 `/usage`
  （`Unknown argument: {arg}. Use /stats [5h|day|week|month]`）

### 改动点

1. `crates/codegen/xai-grok-tools/src/model_usage_ledger.rs`
   - 在 L15-17 的常量组里补 `pub const WINDOW_DAY_MS: u64 = 24 * 60 * 60 * 1000;`。
   - 该文件已有 `aggregate` 的窗口测试（L225-265），照样式补一个 day 窗边界用例
     （`now - WINDOW_DAY_MS - 1` 应排除）。
2. `crates/codegen/xai-grok-pager/src/slash/commands/stats.rs`
   - `slash_meta!`：`takes_args: false` → `true`，`usage` 改成
     `"/stats [5h|day|week|month]"`。描述同步英文原文（i18n 表见 T3 规约）。
   - `run()`：解析 `args.trim()`，映射到窗口枚举后调用改造过的渲染函数。
   - `format_usage_report`：现在硬编码三窗循环（L42-46）。改成接受"窗口集合"参数
     （`Option<Window>` 或 `&[Window]` 切片），保持纯函数以便测试。
   - 新增 `suggest_args`，给 `5h`/`day`/`week`/`month` 四项补全，抄 `usage.rs:69-84` 的 `ArgItem` 结构。
   - **别名**：建议 `5h` 同时接受 `5h`/`5hr`？——不要自作主张，只做 `5h`/`day`/`week`/`month` 四个，
     加别名须在验收里写明理由。
3. 现有测试 `stats.rs` L154-205（`usage_report_aggregates_windows_and_models`）断言三窗口标题
   都在输出里——改造后**必须**同步更新：拆成"默认全窗"+"单窗"两个用例，让失败能自报用例名
   （遵守 AGENTS.md 第 9 条：禁止单个 `#[test]` 打包多场景）。

### 验收标准

- `cargo check -p xai-grok-pager` 通过。
- `/stats 5h` 输出只含 5h 段；`/stats` 输出含四段且 day 段位置合理。
- 未知参数返回 Error 且不 panic。
- 单元测试覆盖：单窗过滤、默认全窗、未知参数、day 边界。

---

## T2：`/stats` 输出改为 Grok 自制窗口样式（对齐 agents / usage limit 面板）

### 目标

把 `/stats` 从 scrollback 纯文本改成**打开的模态窗口**，视觉与交互对齐现有
`agents_modal` / `usage_modal`（同样的边框、标题、页脚快捷键、滚动）。

推荐做法：**新增独立 modal**（`ActiveModal::StatsInfo`），不塞进 `usage_modal`
——后者的 3 个标签页语义是"当前会话用量/限额/会话信息"，与"按模型的历史聚合"不同源，
混在一起会让 `usage_modal` 的 state 加载逻辑（异步 fetch_nonce，见
`app/dispatch/tests/billing.rs:41`）与纯本地 jsonl 读取耦合。

### 改动点

1. 新文件 `crates/codegen/xai-grok-pager/src/views/stats_modal.rs`
   - 抄 `views/usage_modal.rs` 的结构：`State` 结构体 + `render()` + `route_key()` +
     `Outcome` 枚举（`Close` / `Changed` / `Unchanged` / `CopyText(String)`）。
   - 底部复用 `views/modal_window.rs` 的框架（边框/标题/页脚），不要手画框线。
   - 内容布局建议：每个 model 一个"卡片"块（model id 作小标题，tokens/cache/perf 分行对齐），
     时间窗用窗口内**标签页或分节标题**呈现（若做标签页，直接抄 `usage_modal` 的 tab 切换）。
   - 数字宽度对齐：`fmt_tokens` 已有 `823`/`45.2k`/`3.05m` 三档，卡片内建议右对齐。
2. `crates/codegen/xai-grok-pager/src/views/modal.rs`
   - `ActiveModal` 加变体 `StatsInfo { state }`。
   - 标题映射处（L713 附近，`ActiveModal::UsageInfo { .. } => tr("Usage")` 旁）补
     `ActiveModal::StatsInfo { .. } => tr("Model usage")`。
   - 注意同文件 L681 的 `|` 模式列表与其它 `match active_modal` 处要一并补分支。
3. `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs`
   - 抄 `open_usage_info_modal`（L42）新增 `open_stats_info_modal`，在 L111 附近设
     `ActiveModal::StatsInfo`。数据是本地同步读取，**不需要** `fetch_nonce` 那套异步。
4. `crates/codegen/xai-grok-pager/src/app/modals.rs`
   - 在 modal 事件分发（L434 / L489 / L1563 / L1640+ 的 `UsageInfo` 分支附近）补
     `StatsInfo` 的处理：键盘路由、滚动、关闭、复制。
   - L2360 附近的渲染分支同样要补。
5. `slash/commands/stats.rs`
   - `run()` 返回值从 `CommandResult::Message(report)` 改为
     `CommandResult::Action(Action::ShowStats)`（参照 `usage.rs:92` 的
     `Action::ShowUsage`），再到 `app/actions.rs` 注册该 Action 并路由到 (3) 的 open 函数。
   - **决策点**：是否保留纯文本回退？建议保留一个 `/stats text` 或直接删掉文本路径
     ——须在实现时定夺并在提交信息里写明。若删除，`format_usage_report` 可退化为
     modal 内部的数据准备函数。

### 验收标准

- `cargo check -p xai-grok-pager` 通过。
- 手动验证：`/stats` 弹出窗口，边界/标题/页脚与 `/usage` 面板观感一致；Esc 关闭；
      `/stats month` 打开时定位到 month 窗（若 T2 做成标签页）。
- 视口自适应：终端宽度 80 / 120 / 200 列下不溢出、不错位（本仓库已有 footer 宽度计算的
  前车之鉴，见 `ui-i18n-plan.md` 里 "Settings"/"No matches for " 的布局计算条目）。
- 若删除了文本路径，确认没有其它地方依赖 `format_usage_report` 的字符串契约（grep 全仓）。

---

## T3：TUI 中残留英文说明的 i18n 扫尾

### 现状

`docs-local/ui-i18n-plan.md` 的 P0–P4 计划**已全部完工**（翻译表 1338 → 1417 组），
该计划自述"收尾"。所以本任务是**计划外的漏网扫尾**，不是续做 P5。

### 机制（勿另起炉灶）

- 翻译表：`crates/codegen/xai-grok-pager/src/slash/i18n.rs` 的 `translations()`，
  **英文原文为键**，无键则透传英文。
- 渲染：`crate::slash::i18n::tr(&'static str)` / `tr_str(&str)`。
- 切换：`/lang`；`GROK_LANG=en` 改启动默认；测试构建整体旁路查表。
- 分界口径沿用 `ui-i18n-plan.md`：只译**用户可见的说明性文案**
  （标题/标签/分节头/字段标签/快捷键图例/空态/帮助段/占位符/徽章状态）；
  **不译**日志、调试 HUD、协议字符串、运行时数据、被逻辑比较当键的字符串本身、
  markdown 正文、一次性命令结果/错误消息。

### 施工方法（关键：先扫描再动手，禁止逐个盲改）

1. **零编译静态扫描先行**（遵守 AGENTS.md 第 2、8 条：编译最贵）。
   仓库已有扫描工具先例 `docs-local/win_scan.py`；仿写一个 i18n 扫描脚本或直接用 grep：
   - 候选：`views/`、`slash/commands/`、`app/` 下含英文单词的字面量，
     且**未被** `tr(` / `tr_str(` 包裹、**不在** `#[cfg(test)]` 模块内、**不在**注释/日志宏内。
   - 输出按文件分组 + 行号 + 字符串，形成一份清单。
2. 把清单按 `ui-i18n-plan.md` 的四条通用规约分类（存键/渲染翻译、比较键不译、
   `format!` 模板拆固定段、多行整段成键）。
3. **分批施工**：每批 ≤3 个视图文件（沿用该计划第 8 条的并发上限），
   改完统一合并进 `i18n.rs`，脚本查重，然后一轮编译验证。
4. `PATCHES.md` 登记第十四期（当前最新为十二期，见该文件）——文档里写明"计划外扫尾"。

### 已知的高概率漏网区（调研时的观察，施工前用扫描结果复核）

- `slash/commands/` 下的**一次性命令输出**（如 `stats.rs` 现在的 `tr()` 混用）：
  注意本计划口径是"一次性命令结果不译"，但 `/stats` 的**标签**类文案（`tr("input")` 等）
  已进表，改造 T2 时新增的窗口文案（标题/页脚/列头）**必须**进表。
- `views/modal_window.rs` 框架自身的 chrome 文案（若 P0-P4 只改了各 modal 的内容而漏了框架）。
- `views/status_line/`、`views/prompt_widget/`、`views/dock/`、`views/list_pane/` 的
  状态词与提示。

### 验收标准

- 扫描脚本对同一批文件二次运行，未包裹英文候选项数量归零（或仅剩已书面豁免的项）。
- `cargo check -p xai-grok-pager` 通过。
- `cargo test -p xai-grok-pager --lib slash::i18n` 通过（翻译表纯数据测试）。
- 抽查 `/lang zh` 下切换 2-3 个界面，无残留英文说明。

---

## 通用验证与纪律（三件事都适用）

按 `AGENTS.md` 第 4 条（Windows 测试门禁，MANDATORY）：

- 编译验证（快，先跑）：`TMP='D:\cargo-tmp' TEMP='D:\cargo-tmp' cargo check -p xai-grok-pager -p xai-grok-tools`
- 行为验证（按模块过滤，避免全量）：
  `scripts-local/ctest.sh -p xai-grok-pager --lib slash::`
  以及 `scripts-local/ctest.sh -p xai-grok-tools --lib model_usage_ledger`
- **禁止裸跑 `cargo test`**；一次性分析确需裸跑用 `GATE_FORCE=1` 并事后恢复。
- 新确认的 Windows 环境族必挂测试 → 先登记 `docs-local/win-skip.txt`（带根因注释），
  不逐个排查。
- 改共享 crate（`xai-grok-tools`）会级联重编：T1 的常量新增放最前一次做完，
  不要"改一行 → 全量"循环。
- 每个任务独立提交（绿一块走一块，AGENTS.md 第 10 条）；提交前
  `gofmt` 等价物为 `cargo fmt -p <改动的 crate>`，但注意本仓库 .go 无关，
  Rust 侧按 `rustfmt.toml`；Windows 行尾为 CRLF，避免整文件重写 diff。
- 文件落在 `docs-local/`，上游同步不带；对同步文件（`views/`、`slash/` 等）的改动
  按第 6 条在 `docs-local/PATCHES.md` 登记，便于同步后重放。

## 待定决策（实现时须先定，不要默默选一个）

1. T2 是否保留纯文本输出路径（`/stats` 直接给窗口 + 提供 `/stats text` 回退？）。
2. T2 的窗口内多时间窗呈现：标签页 vs 单列分节 vs 一次只显示一个窗（配合 T1 参数）。
3. T1 的参数命名：`5h` 是否也要接受 `5hr`/`5hour` 之类的别名。
