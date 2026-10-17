# opencode v2 TUI（anomalyco/opencode）× 本仓库 特性移植预研

> **状态：复核完成，候选定稿为「移植优选」（2026-10-17 用户拍板：除通知音频/原生
> toast 与键位配置层外全部立项为移植优选；施工顺位随 roadmap 拍板）。**
> 调研快照：2026-10-17；opencode dev 分支 `22e771f`（v2.0.32，2026-10-16 发版；
> 2.0 于 2026-10-13 发布）。本地克隆 `D:\CODE\ai\opencode`；opencode 侧事实 =
> `packages/tui/src` 逐文件读码 + 2.0.x CHANGELOG 逐条过筛。
> fork 基线：main（2026-10-17，v1.0.61 后）；fork 侧事实全部本机 grep/读码反查
> （含上轮口头推荐误报 6 项的系统性撤销，台账见 §3），人天估算 = C。
> 口径：opencode 社区口碑下降仅作机制参照（agent-cli-tracking/ADOPTION.md
> 2026-10-11 口径），本文只取 **TUI 呈现层**；全部候选为模型无关纯客户端特性
> （国模适配性 ★）。1.x 机制面判定（压缩/缓存断点/超长输出落盘/revert）已在
> [agent-cli-tracking/notes/opencode.md](../agent-cli-tracking/notes/opencode.md)
> 完成吸收评估，本文不重复。

## 对象是什么

opencode 2.0 把 TUI 从「单会话聊天窗」升级为「多会话工作台」：多会话 tabs +
停靠面板 + worktree 工作区 + 通知层 + TUI 插件插槽，TUI 成为后台服务
（`opencode service`）的客户端之一，会话独立于 TUI 进程存活。渲染栈 =
SolidJS + OpenTUI（Zig TUI 框架），与本仓 ratatui 架构不同——因此只移植
**特性设计**，实现一律按本仓落点重做。

## TL;DR — 移植优选五件（+1 可选）

1. **diff 呈现面组合**（per-turn 改动文件聚合 + `/diff` 停靠面板）——地基最全
   （hunk-tracker / pager-diff / block_viewer / timeline rail），第一顺位。
2. **worktree 管理补全**（列表 / 会话切入 / 删除安全检查）——创建侧
   `/fork --worktree` 已有，只补管理面。
3. **连续工具调用分组**（活动组折叠 + 运行态组头）——可读性件。
4. **prompt stash 面板**（多条目按需暂存）——单槽自动 stash 已有，最小件。
5. **会话内 tabs rail（可选）**——dashboard 已覆盖大半，挂触发信号再拍板。

**排除项**（拍板记录见 §2）：通知音频 + 原生 OS toast、键位配置层（用户拍板
2026-10-17 不立项）；TUI 服务化、TUI 插件插槽整套（架构/生态错配）。

## 1. 移植优选候选

| # | 特性 | opencode 锚点 | fork 缺口证据 | 落点 | 规模 |
|---|---|---|---|---|---|
| 1 | diff 呈现面组合 | v2.0.30：changed-files-by-turn（per-turn 改动文件 + 行数 + 可展开 diff，#55064，desktop 端呈现、数据面在 core）；`/diff` 停靠面板（#47152/#55574/#55780：侧栏停靠 + `f` 全屏切换 + 窄终端自适应） | 全仓 grep `changed_files`/`files_changed` 0 命中；`slash/` 无 diff 命令 | 见 §1.1 | T1 3–5 + T2 4–7 人天 |
| 2 | worktree 管理补全 | `component/dialog-workspaces.tsx`（项目 worktree 列表 + 当前标记 + 删除）、`dialog-worktree-name.tsx`（slug 生成名）、`dialog-workspace-file-changes.tsx`（删除前改动检查） | 创建/分叉已有（`/fork --worktree` + `views/new_worktree_dialog.rs` + dashboard `Ctrl+w`）；列表/切入/安全删除缺 | 见 §1.2 | 2–4 人天 |
| 3 | 连续工具调用分组 | `routes/session/grouping/session.ts`（reasoning/exploration/activity 组）+ `activity-summary.ts`（组头实时 "Running command…"）+ `group-view.tsx` | 逐 block 折叠已有（`scrollback/block.rs`）；多调用分组缺 | 见 §1.3 | 3–5 人天 |
| 4 | prompt stash 面板 | `prompt/stash` + `component/dialog-stash.tsx`（多条目 + 首行预览 + 相对时间）；`prompt/draft-stash.ts`（per-tab 草稿，取走即销毁） | 自动**单槽** stash 已有（`app/acp_handler/interactions.rs:260-290`）；手动多条目缺 | 见 §1.4 | 1–2 人天 |
| 5 | 会话内 tabs rail（可选） | `component/session-tabs.tsx` + `context/session-tabs-model.ts`（自适应布局 + 未读 badge + tab-pulse 辉光 + Option+↑/↓ 切 tab / Shift 跳未读）+ `context/session-retention.ts`（LRU 逐出） | 进程内多 agent + dashboard 已有；agent 视图内常驻 tab 条缺 | 见 §1.5 | 3–5 人天 |

### 1.1 diff 呈现面组合（第一顺位）

- **设计要点**：两个互补呈现面共用一套数据。① per-turn 聚合：每完成回合产出
  「改动文件 + 行数（±）」行，可展开成 diff——把「这轮改了什么」从翻
  transcript 变成一眼可见。② `/diff` 会话工作区级停靠面板：文件树 + 补丁浏览
  常驻侧栏，`f` 在停靠/全屏间切换，窄终端自动降级布局。
- **fork 现状与地基**：`xai-hunk-tracker`（hunk 跟踪 + agent/外部归因，数据面
  就绪）、`xai-grok-pager-diff`（hunk 构造）、`views/block_viewer`（全屏内容
  查看器，模块注释明写「Execute and edit viewers will be added in later
  phases」——edit viewer 正是本项的展开 diff 容器）、`views/timeline.rs`
  （per-turn tick rail）、`views/dock/mod.rs`（prompt 上方分区停靠原语，
  `dock_enabled` 门控）。
- **切片**：T1 per-turn 改动文件聚合（timeline rail 或 dock 分区呈现，展开进
  block_viewer edit viewer）；T2 `/diff` 停靠面板（文件树 + 补丁 + `f` 切换）。
- **拍板点**：D1 呈现位（timeline rail 扩展 vs dock 分区 vs 独立右栏）；D2
  归因口径（仅 agent 改动 vs 含外部改动——hunk-tracker 已有归因，建议沿用）；
  D3 diff 源（working tree 快照 vs 会话累计）。
- **验收**：T1 = 跑一个改 3 文件的回合，timeline 出现该回合条目、行数与
  `git diff --stat` 一致、展开进 viewer；T2 = `/diff` 开合、`f` 往返、窄终端
  不破版、i18n 0 挂。

### 1.2 worktree 管理补全

- **设计要点**：opencode 的 worktree 面是三件套——列项目全部 worktree（当前
  标记）、创建（slug 自动生成名，可重生成）、删除（先弹该 worktree 的
  file-changes 检查，有未合并改动需显式确认/替换目标）。fork 的创建/分叉侧
  已齐（`/fork --worktree` 询问 modal + `new_worktree_dialog.rs` + dashboard
  `Worktree Ctrl+w` + `views/location.rs::worktree_badge`），缺的是**管理面**。
- **拍板点**：D1 切换语义——活会话迁移 cwd 是大改（会话 spawn 定 cwd），建议
  落「新会话从 worktree 列表选」（dashboard/new 对话框加列表位），活会话迁移
  不做；D2 删除安全检查口径（脏工作区判定走 `xai-gix-status`）。
- **验收**：列表含当前标记；从列表新建会话落进所选 worktree；删除含改动
  worktree 时弹检查且默认拒删。

### 1.3 连续工具调用分组

- **设计要点**：把一段连续的探索类工具调用（bash/read/grep…）归入一个可折叠
  组，组头显示运行态摘要（"Running command…"）与条数，完成后收拢；思考块同
  理成组。解决长 transcript 里「十几个 bullet 淹没正文」的可读性问题。
- **fork 现状**：逐 block 折叠已有（thinking 自动折叠 / toolcall bullet /
  收尾自动收起 / `collapsed_edit_blocks` 外观缓存），**组**这一层缺。运行态
  摘要数据已有现成来源——`agent.resolve_turn_activity()`（通知层
  `app_view.rs::update_notifications` 在用）。
- **拍板点**：D1 分组粒度（仅探索类连续段 vs 全部工具）；D2 与既有 bullet
  折叠的关系（组头取代组内 bullet 或叠加）；D3 默认折叠策略（运行中展开/
  完成后收起，对齐 opencode）。
- **验收**：连续 5+ 工具调用的回合收成单组、组头实时活动文案、展开回逐块
  视图无损、与 `/expand`/选择/搜索不冲突（回归 scrollback 既有套件）。

### 1.4 prompt stash 面板

- **设计要点**：手动 `/stash` 把当前 composer 草稿存入多条目栈；面板列出首行
  预览 + 相对时间，选取恢复（取走即删防旧稿遮新输入）。opencode 另有 per-tab
  在途草稿（切换会话自动保存/恢复）——fork 的自动单槽
  （`permission_stashed_prompt`/`casual_stashed_prompt`，取走即清）已覆盖该
  语义，本项只补**多条目手动栈 + 面板**。
- **拍板点**：D1 持久化（进程内存 vs grok-home 落盘跨进程，建议内存起步）；
  D2 入口（斜杠 + 面板，键位随既有 modal chrome 页脚）。
- **验收**：存 3 条 → 切会话往返 → 面板选取恢复正确、取走即删、i18n 0 挂。

### 1.5 会话内 tabs rail（可选，价值待定）

- **设计要点**：agent 视图顶部常驻会话 tab 条：自适应布局、未读 badge +
  完成脉冲、per-tab spinner、`Option+↑/↓` 切 tab / Shift 跳未读、标题溢出
  marquee。
- **fork 现状**：进程内多 agent + dashboard（分组/peek/attach/dispatch/搜索/
  布局持久化）+ tasks pane 已覆盖「多会话并行 + 切换 + 状态感知」的大部分；
  缺的只是 agent 视图内的常驻条与未读指示。未读/活动数据面可复用 dashboard
  `row_activity` 的 badge 口径。
- **判定**：布局偏好件，建议挂触发信号（真实多会话并行使用抱怨）再拍板；
  若做，先出 rail chrome + 未读面的设计草案。

## 2. 排除项（拍板记录）

| 项 | 排除理由 |
|---|---|
| 通知音频 + 原生 OS toast | 用户拍板 2026-10-17 不立项。fork 通知层事件面/条件/协议/focus 门控已齐（见 §3），缺的只是声音与系统 toast；Windows Terminal 自身 bell 处理可兜底，价值边际小 |
| 键位配置层（用户可配 keybinds + leader 键） | 用户拍板 2026-10-17 不立项。palette/动作注册表已有，配置层等真实诉求 |
| TUI 服务化（后台服务客户端 + 重连 + 远程配对） | 架构错配：fork 是 embedded 单进程；journal + resume 已是降级等价物 |
| TUI 插件插槽整套（discovery/watch 热重载 + `Slot` 渲染位） | fork 插件生态在 agent 层（hooks/marketplace）；"panel slot" 思想已被 `views/dock` 体现 |
| 动画细节层（title-shimmer/tab-pulse/fade-in 等） | OpenTUI 投入红利，ratatui 下非必需；`appearance/config.rs` `AnimationConfig`（wave_rows）已有动画面 |

## 3. 复核撤销台账（防误报）

上一轮口头推荐未经系统反查即列为「可移植」，经逐条反查撤销 6 项；下表为本轮
确认 **fork 已有** 的 opencode v2 TUI 对应物（锚点为本仓 file:line），后续预研
开工前先对照本表与 fork-feature-inventory §4：

| opencode v2 特性 | fork 已有证据 |
|---|---|
| `/copy` 最新回复 | `pager/src/slash/commands/copy.rs`（更强：`/copy [N] [file]` 第 N 条 + 剪贴板不可达文件回退） |
| Command palette（fuzzy 全命令） | `pager/src/actions/defaults.rs:790` `ActionId::CommandPalette`（registry.all() 可搜 + 分类） |
| 终端 tab 标题 + OSC 9;4 进度 | `pager/src/app/app_view.rs:5920`（`notifications/progress.rs:20-21` OSC 9;4 + Ghostty 15s keep-alive `notifications/mod.rs:15`；parked 清除 `app/dispatch/queue.rs:4069`） |
| Attention 通知层核心 | `pager/src/notifications/`：`NotificationEventKind` 5 事件 + `NotificationCondition`/`NotificationHook`/`NotificationMethod`（Auto/Osc9/Osc99/Osc777/Bel）+ `FocusTracker` + `SleepInhibitor` + `TitleManager` + tmux 模块 + 审批批去重 |
| 多会话（进程内多 agent + 切换面） | `app.agents` 多 top-level agent；`views/dashboard/mod.rs`（按状态分组 + peek/attach/dispatch + 搜索 + `xai-grok-dashboard-store` 布局持久化） |
| worktree 会话分叉 | `/fork --worktree`（`slash/commands/fork.rs`）+ 询问 modal + `views/new_worktree_dialog.rs` + dashboard `Worktree Ctrl+w`（`views/dashboard/chrome.rs:2`） |
| transcript 块折叠 | `scrollback/block.rs`（thinking 自动折叠 / toolcall bullet / 收尾自动收起 / groupable 块无空隙）+ `collapsed_edit_blocks`（`acp/tracker.rs`） |
| 时间线侧栏 | `/timeline`（`slash/commands/timeline.rs`，per-turn tick rail，渲染在 `views/timeline.rs`） |
| 草稿暂存（自动单槽） | `app/acp_handler/interactions.rs:260-290`（permission/casual 双通道 stash，取走即清） |
| transcript 虚拟化（mount-budget） | `scrollback/render.rs` 窗口化渲染（skip_rows + `VisibleBlockGeometry`）——OpenTUI 需自建滚动缓冲才需要解决，ratatui 架构下不需要 |
| release highlights（升级后一次性展示） | `/release-notes`（`slash/commands/release_notes.rs`）+ `xai-grok-announcements` |
| 停靠原语 / 动画配置 / prompt 队列 / steer | `views/dock/mod.rs`；`pager-render/src/appearance/config.rs` `AnimationConfig`；`xai-prompt-queue`；`xai-interjection-core` |

**方法论教训**：`collapse`/`stash`/`palette` 等词首轮 grep 均命中无关行
（blocking-waits 收拢、审批草稿、动作枚举），抽查式 grep 会同时产生漏判与
误判；候选判定必须按「slash 命令表全量 + views 目录清单 + 通知/布局模块」
系统反查后再下结论。

## 4. 施工顺位建议

1（T1 可先行，地基最全）→ 2 → 3 → 4（规模递增）；5 挂触发信号。各特性开工前
按惯例对照 fork-feature-inventory（本文已回填其 §4）；完工后按 roadmap §6
交接协议登记。

## 来源

- opencode：本地克隆 `D:\CODE\ai\opencode`（dev `22e771f`，v2.0.32）；
  `packages/tui/src` 读码 + 仓库 `CHANGELOG.md` v2.0.x 条目（2026-10-17）。
- fork 侧锚点：本文 §1/§3 所列 grep/读码复核（2026-10-17，基线 main v1.0.61 后）。
- 关联文档：`agent-cli-tracking/notes/opencode.md`（1.x 机制面）、
  `agent-cli-tracking/ADOPTION.md`（口碑口径）、`fork-feature-inventory.md` §4、
  `port-roadmap.md`（施工顺位权威）。
