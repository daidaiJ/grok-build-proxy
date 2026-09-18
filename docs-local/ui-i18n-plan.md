# UI 说明文案中文化 i18n：分级多阶段施工计划

> LOCAL 特性。上游同步不会带走本文件（docs-local/ 不同步），但会冲掉同步文件里的施工改动；
> 每阶段落地后在 docs-local/PATCHES.md 登记一笔，便于同步后重放。
> 调研快照基于 commit 49f41780（2026-09-17），文中行号仅作定位参考，施工时以文本搜索为准。
> 已完成（此前提交）：斜杠命令描述/用法/占位符、shortcuts_bar、modal_window 框架、
> agents_modal、persona_detail。
> 已完成（九期补丁 2026-09-18）：P1 常用弹窗与帮助（settings_modal / shortcuts_help /
> usage_modal / mcps_modal / tutorial，翻译表 449 → 917 组），见 PATCHES.md 九期。
> 已完成（十期补丁 2026-09-18）：P2 集成管理弹窗（extensions_modal + workflows_picker_rows /
> memory_modal / feedback_modal / import_claude_modal，翻译表 917 → 1138 组），见 PATCHES.md 十期。
> 已完成（十一期补丁 2026-09-18）：P3 仪表盘与代理面板（dashboard chrome/row/render/peek +
> tasks_pane + agent 页脚补键 + agent_status + goal_detail + workflows，翻译表 1138 → 1338 组），
> 见 PATCHES.md 十一期。勘误：render.rs 生产区实际到 3639 行（#[cfg(test)] mod tests 在 3640 起，
> 1104/1950 只是两个 cfg(test) 辅助函数），本表"仅 1–1103 行"为调研误差，施工已按真实边界完成。
> **已完成（十二期补丁 2026-09-18）：P4 低感知扫尾（19+1 个视图/命令文件接线 + 翻译表
> 1338 → 1417 组），见 PATCHES.md 十二期。P0–P4 全部完工，本计划收尾。**

## 机制与口径

- 翻译表：`crates/codegen/xai-grok-pager/src/slash/i18n.rs` 的 `translations()`，英文原文为键；
  代码用 `crate::slash::i18n::tr(&'static str)` / `tr_str(&str)` 渲染时查表，无键透传英文。
  `/lang` 切换；`GROK_LANG=en` 改启动默认；测试构建查表整体旁路（渲染测试语言稳定）。
- 本计划只覆盖「用户在界面上看到的说明介绍性文本」：标题、标签页、分节头、字段标签、
  快捷键图例/页脚、空态、帮助/说明段、占位符、徽章状态标签。
- 不做：一次性命令结果/错误单行消息、日志、调试 HUD、协议字符串、运行时数据
  （路径/名字/对话正文）、被逻辑比较当键的字符串本身（只译显示侧）。

## 通用施工规约（每阶段都遵守）

1. 状态存英文键、渲染时翻译：字符串会进 state 且被重新渲染或参与比较的，存原文，
   渲染出口 `tr_str()`；一次性直接渲染的字面量可就地 `tr()`（agents_modal 先例）。
2. 比较/匹配键绝不包翻译：peek.rs `panel.response_type == "Working"`、agent.rs
   `fold_label == Some("expand")`、turn_status 的 `starts_with("Ask: ")`、goal_detail
   `ago == "just now"`、subagent_catalog 的 match 键——只译展示侧。
3. `format!` 模板：固定片段成键，运行时插值保留；计数模板整键进表（"{n} plugins" 类）。
4. 多行说明段（long_help、PRIVACY_BANNER_DESC、EMPTY_PLAN_PLACEHOLDER）整段一个键。
5. 带对齐空格的文案（"worktree "、" · Admin Managed"、"Workspace  "）译文保宽或同步改布局计算。
6. 来自 registry / 外部 crate 的动态串（settings registry 的 meta.label/description、
   FeedbackType::label() 等）在渲染出口 `tr_str()`，并把原文补进表。
7. 不动 `#[cfg(test)]` 模块；按文案分段断言点击热区的测试（privacy_banner），改文案须同步测试。
8. 施工流程：并发派遣子代理 ≤3（全局约束，超出分批）→ 各代理只改自己负责的视图文件并回报
   (en, zh) 键值对 → 主会话统一合并进 i18n.rs → 脚本查重 →
   `TMP='D:\cargo-tmp' TEMP='D:\cargo-tmp' cargo check -p xai-grok-pager` 一轮修复 →
   i18n 纯数据测试 `cargo test -p xai-grok-pager --lib slash::i18n` → 独立提交（绿一块走一块）。
9. 术语表保持一致：agent 代理 / persona 人格 / subagent 子代理 / session 会话 / plugin 插件 /
   skill 技能 / hook 钩子 / workflow 工作流 / memory 记忆 / context 上下文 / usage 用量 /
   plan 计划 / rewind 回退 / turn 轮 / token token / MCP server MCP 服务器。

## P0 交互必经（每个会话都撞上）｜约 185 条

| 范围 | 体量 | 要点 |
|---|---|---|
| views/modal.rs 命令面板 | L ~40 | 面板条目 label 是 String 存 state → 渲染出口 tr_str；按钮 label() 返回 &'static str 可 tr；"save & send" 等组合文案新键；约 36 条面板条目名 |
| views/permission_view.rs | S ~10 | 模式编辑帮助/占位符（"type a command pattern to allow…"、"⚠ very broad"）；页脚多段 Span 拼接对动作词单独 tr |
| views/question_view.rs | S ~3 | "Type your answer here"、截断提示 "Ctrl-F to expand"、自由输入行固定标签 "Other"（进 state → 构造处 tr_str） |
| views/plan_approval_view.rs | S ~5 | EMPTY_PLAN_PLACEHOLDER 整段多行成键；"Waiting on plan approval" 等状态标签 |
| views/elicitation_view/{render,state}.rs | M ~14 | render 处直接 tr（标签/按钮/等待文案）；state 的 3 个标题模板与 4 条 URL 校验错误取固定片段成键 |
| views/welcome/mod.rs（启动屏） | L 100+ | 信任确认多行说明、认证流程文案、菜单项（"Import Claude settings"/"Resume session"…）、"Yes, proceed"/"No, quit"、相对时间词族（moments/minutes/hours ago；注意 "just now" 与 goal_detail 是跨文件比较键，需联动）；多行说明逐行成键 |
| views/privacy_banner.rs | S ~8 | 多行说明段整段成键；LEGAL 链接分段带点击热区且有测试按段断言，译文分段需同步校准 |
| views/session_picker.rs + session_picker_surface.rs | M ~16 | 字段标签（ID/CWD/Created/Updated/…）push 处 tr；"Extended search results…" 头部；"Open session" 共用 const 一处改 |

## P1 常用弹窗与帮助｜约 160 条

| 范围 | 体量 | 要点 |
|---|---|---|
| views/settings_modal/{render,state}.rs | M ~35 | 页脚快捷键图例 ~15 条、Tip 提示、过滤空态、行值徽章、校验错误、"Settings" 标题；registry 动态串（meta.label/description/category.label()/choice.description）渲染出口 tr_str 且原文补进表；"No matches for " 前缀宽度参与布局计算需同步 |
| views/shortcuts_help.rs | L 60+ | 7 个分类标题、多行 long_help 常量（粘贴/撤销/历史/搜索）、弹窗标题、页脚；施工前先核查 ActionRegistry 的 short_help/long_help 渲染链路是否已接 tr，未接则一并补 |
| views/usage_modal.rs | M ~25 | 3 个标签页（Context usage/Usage limit/Session info）、页脚、加载/空态、format 模板（金额/URL/错误取前半句成键）；注意剪贴板复制文案与屏显共用字符串的取舍 |
| views/mcps_modal.rs | M ~10 | 分组标题模板（Managed by grok.com (n)/Plugin: x (n)/Local (n) 整键）、说明行、6 个状态徽章 label() |
| views/tutorial.rs + tutorial_docs.rs（仅代码内 title/blurb） | M ~30 | INTRO_LINES 引导语、弹窗标题、页脚、format 前后缀拆分；tutorial_docs 的 go_deeper 标题参与 `==` 匹配（改键须同步 docs 索引）；markdown 正文不在本计划 |

## P2 集成管理弹窗｜约 135 条

| 范围 | 体量 | 要点 |
|---|---|---|
| views/extensions_modal.rs | L 60+ | 6 个标签页（Hooks/Plugins/Marketplace/Skills/Workflows/MCP Servers）、状态过滤（All/Enabled/Disabled）、表单标签/占位符、徽章（[policy]/[disabled]/[installed]…）、分组头（Project/User/Bundled/Server/…）、页脚与动作键图例；大量 format 模板（"{n} plugins"、"(count) skills"、"Select a {noun} row to {verb}." 整句进表，disable/enable 成对）；行数据构建期进 state → 渲染出口 tr_str |
| extensions_modal/workflows_picker_rows.rs | S ~6 | 空态句、字段标签（path/when to use）；行字段 String 需渲染侧 tr_str |
| views/memory_modal.rs | M ~25 | 标题 "Memory"、分组节头（Global/Workspace/Sessions，进 state）、搜索占位、空态、页脚 ~13 条、相对时间 "{m}m ago" 族（format_modified 出口 tr_str） |
| views/feedback_modal/{mod,render,drafts,enum_picker}.rs | M ~40 | 标签页（Write/Drafts）、trace 选项、确认/删除问句、空态、页脚 ~14 条；error 是 Option<String> 存 state → 存英文键渲染点 tr_str（含多条多行长句）；枚举 label 来自 xai_grok_feedback crate → tr_str + 补表 |
| views/import_claude_modal.rs | S ~8 | 标题、快捷键图例、"Enter import {n}" 拆分 |

## P3 仪表盘与代理面板｜约 165 条

| 范围 | 体量 | 要点 |
|---|---|---|
| views/dashboard/{chrome,row,render,peek}.rs（生产区） | L ~75 | chrome 按钮/chip（awaiting/working/idle/done/failed，拼进 " {count} {label}"）；row 状态词存 state → 渲染 tr_str；render 生产区仅 1–1103 行（1104 起全是测试勿动）；peek 活动标签（"Thinking"/"Working" 等被 `==` 比较——存键渲染翻译）、工具标签、"(thinking)" 类注记、空态/选项兜底字面量；"agent(s)" 复数中文合并 |
| views/tasks_pane.rs | M ~15 | GroupKind::label 分组头（Workflows/Subagents/Tasks/Watchers）、" (next in …)"/" (due now)" 调度说明、三段 Span 拼接的空态帮助行 |
| views/agent.rs 页脚 + views/agent_status.rs | M ~19 | HintItem 标签直通渲染（hide done/navigate/reorder/queue/…）；agent_status 的 chip/phase 标签（Failed/Interrupted/Verifying/Planning/…）；pause_label() 定义在枚举 impl 处，顺带排查 |
| views/goal_detail.rs | L ~40 | 状态标签（Active/Failed/Budget Limited/…）、事件行（match 键是 "goal_created" 等 kind 字符串勿动，译值侧）、大量 "Status: "/"Budget: " 类标签行与 format 模板、页脚 |
| views/workflows.rs | M ~25 | 标题 "Workflow Runs"、空态两行、分节头 "Phases"、页脚图例、状态说明行；plural(n, noun) 中文合并；run.status.replace('_', " ") 是协议状态词不译 |

## P4 低感知扫尾（打包一轮）｜约 35 条

jump(2)、queue_pane(4)、todo_pane(4)、subagent_catalog_pane(3，"Roles" 标签+空态)、
new_worktree_dialog(2)、managed_connectors_wait(4)、welcome/hero_box(1: "Changelog" 两处)、
workspace_mode(3)、btw_overlay(1)、location(2)、session_title(2)、dock/mod(8: 标签页+tab_hint)、
list_pane/render(6: " Copied!"、输入前缀)、block_viewer/mod(接线为主+补 "limit: " 等少数键)、
picker.rs(接线为主+补 "No matches")、context_bar(1: "MAX %")、credit_bar(~12，若确认常显则提前至 P1)、
commands 层漏网：screen_mode_switch.rs:45（成对语句漏译一条）、theme.rs "auto (follow system)"、
debug.rs 子命令说明(3)、expand.rs 提示(1)、mode_support 的 why: 字段(6)。
hint 类多数键已在表中，只是渲染处没接 tr_str——以接线为主，工作量小。

## 砍掉 / 决策项（默认不施工）

- **doctor 诊断文案**（src/diagnostics/，L，>60 条，多行续行拼接多）与 **tips 面板**
  （src/tips/，9 文件 ~2100 行）：故障/引导场景才出现，体量大、插值多，收益/成本差。
  真要做各占一整轮，单独立项。
- **markdown 正文**（docs/tutorial/*.md 9 篇、docs/user-guide/*.md 29+ 篇）：内容级翻译，
  且都是同步文件，上游同步会冲掉（PATCHES 重放成本高）。除非明确要求，不做。
- release-notes 远端内容、一次性命令结果/错误消息：不在范围。
