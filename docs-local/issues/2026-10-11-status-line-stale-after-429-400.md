# 429/400 后状态行不再更新

- **状态**：fixed（P1 显示侧守卫 + P2 归因键统一已合 main、随 **`v1.0.45`** 发版；**TUI 活体复现未做**，见文末「结论（二）」）
- **施工**：已完成主体（2026-10-10；merge `350874c`、tag `v1.0.45`；P3 未做）
- **现场补充（用户口述，2026-10-10）**：行形态 `model id | ✓ n ✗ m | ttft · tps`
  （用户澄清：**数值是占位，只看样式**），关键是**无 tokens / cache / think 段**，
  之后无论多少轮都不变
- **严重度**：P2（请求本身可用，状态行显示冻结在旧内容，quota/usage 读数失真）
- **发现方式**：2026-10-11 用户实机使用中发现（口头报告）
- **环境**：`main` @ `583ac36`（v1.0.44），Windows 本机 fork

## 现象

状态行在遇到 **429** 或 **400** 错误后，**再次发起请求时不再更新**——行内容
停在出错前的旧状态（冻结），后续正常响应也不刷新。用户原话：

> grok build proxy 的状态行在遇到 429 或者 400 错误后再次请求状态不会更新了

## 复现步骤

> ⚠️ 本节原计划的两项取证**已被文末「结论」节接手**（来源确认为 Builtin 默认行；场景确认为
> 上游网关 429/400 终止性失败），下一会话按「结论」§复现与验证计划执行即可，不必再走本节。

待补。取证时需要先确认两件事：

1. 状态行来源是 **Builtin segments**（默认）还是**自定义脚本 command**
   （`[status_line]` 配置外部命令）——两条刷新链路完全不同（见下）。
2. 429/400 的具体场景（免费额度 paywall 429 / 参数 400 / 上游网关 400），
   以及错误后是否走过重试。

## 状态行刷新链路（现状梳理）

行内容两条来源，刷新触发点：

- **Builtin**：会话状态快照 `XaiSessionUpdate::SessionStatus` → 写
  `agent.status_context` + `app.refresh_status_line_now()`
  （`crates/codegen/xai-grok-pager/src/app/acp_handler/session_notification.rs:1326,1342`）。
- **Command**：外部脚本 run-slot 状态机（Running/Abandoned/Superseded/Idle），
  `begin_command_run` / `finish_command_run` /
  `supersede_command_run`（`crates/codegen/xai-grok-pager/src/app/status_line.rs:312,347,416`）。
- 共同触发点：`TaskResult::PromptResponse`（成功与错误都走）
  → `app.refresh_status_line_for(agent_id)`
  （`crates/codegen/xai-grok-pager/src/app/dispatch/task_result.rs:870`）；
  刷新间隔定时器 `note_status_line_refresh_due`；resize supersede。

## 排查线索（未证实假设，按可疑度排序）

1. **刷新源门**（`status_line_policy.rs:203`）：`refresh_status_line_for`
   仅当行 `source` 与响应 agent 匹配才刷新——错误路径若令 source 漂移/分离
   （如行归属被改写到别的 agent），后续所有 refresh 全部 no-op，行冻结。
2. **Command 行 run-slot 卡死**（`status_line.rs:319` /
   `status_line_policy.rs:158`）：`begin_command_run` 在
   `command_in_flight` 时直接返回 `None`；若错误路径吞掉了本次 run 的
   finish 事件，slot 永不落回 `Idle`（仅剩 deadline watchdog
   `abandon_if_past_deadline` 或 resize supersede 兜底），之后每次
   `begin_command_run` 都被 in-flight 门挡掉 → 行永停旧内容。与用户描述
   「再次请求不更新」最吻合。
3. **刷新间隔失败静默**（`status_line.rs:28,398-409`）：RefreshInterval
   触发的失败在 `REFRESH_FAILURES_TO_PAINT = 3` 次之前**静默保留旧答案**；
   若 429/400 后脚本持续失败，表现即「不再更新」（但这是带间隔定时器的
   慢性冻结，与「再次请求」的即时性描述吻合度稍低）。
4. **status_context 快照断流**（`session_notification.rs:1326`）：错误响应
   不推 SessionStatus 快照；若 429/400 后服务端快照流中断，Builtin 行的
   usage/quota 段自然停在最后一次快照——需确认快照恢复后是否恢复刷新。

## 本地补丁嫌疑

**先排除再定性**：出口代理补丁 `a4e8883` / `76891f3`（模型级 egress proxy
opt-in + 自动探测，v1.0.44）改动过 HTTP 请求路径，429/400 恰好是网络层响应
——需用 bypass 直连复测一次，区分「上游自带」还是「代理路径副作用」。
对照 `PATCHES.md` 二十三/二十四期与 `SOURCE_REV`。

## 取证计划

1. 本地假端点复现：强制先 429/400 再 200，观察状态行行为与
   `unified_log`（`turn.complete` 等）。
2. 若为 Command 行：抓 run-slot 状态迁移（begin/finish/supersede/abandon），
   验证线索 2。
3. 若为 Builtin 行：抓 SessionStatus 快照序列，验证线索 4。
4. 结论回填本文件（追加「结论」小节），自修则登记 `PATCHES.md`。

## 关联

- `docs-local/status-line-perf-wiki.md`（状态行机制文档）
- pigo 侧同域观察：状态行统计口径对齐 grok 的 O1 件（pigo-fork wiki
  implementation-plan 优化池）——若根因在上游状态机，pigo 的 O1 对齐设计
  需避开同款坑。

---

## 结论（2026-10-10 定位完成，未施工；交给下一会话）

**定位结论**：根因在 **LOCAL 面**（状态行 scoped 用量投影 + LOCAL ✗ 端点健康计数器），
**不在**上游状态机，也**不在**出口代理补丁。前面 4 条「未证实假设」全部不成立（见
[§已排除](#已排除下一会话不必重做)）。这一节的每个锚点都已对着当前 `main` 核实过。

### 根因（一句话）

状态行**按「当前模型的配置 id」精确取账本条目**来画 `✓/✗` 与 tokens/cache/think 段，
而**成功调用写进「上游回显的 model id」条目**、**终止性失败计数写进「配置 id」条目**
—— 取账本的键和记账的键不是同一套。两种拼写只要不同（本机实测：配置侧
`glm-5.3-flash`，回显侧 `glm-5-3-flash`），精确命中就落在**只有 ✗ 的影子条目**上，
该条目此后永远接不到成功用量 → 行上 `✓ n ✗ m` 冻死，且 tokens / cache / cache-misses /
think **四段整段消失**（它们要求 `model_calls > 0`）。

**为什么"必然从一次 429/400 之后开始"**：影子条目**只由终止性失败创建**
（`record_main_loop_failure`，由 429/400 这类 `log_terminal_failure` 路径触发）。
在那之前配置侧拼写在账本里根本没有条目，`scoped_usage` 走的是「当前模型查不到 →
退回全会话合计」的兜底，行是活的；第一次终止性失败把影子键建出来，行从此冻在这条影子条目上。
这也解释了用户口述行里**没有 tokens 段**：影子条目 `model_calls == 0`，
`window_totals = usage.filter(|t| t.model_calls > 0)` 直接把它滤成 `None`。

### 证据

**A. 账本实证：本机三处「成功与失败分裂到两个键」的影子键（读 `usage.json` 与 `summary.json`）**

| 会话 | `summary.current_model_id`（配置侧） | 账本键（`usage.json` → `session.modelUsage`） |
|---|---|---|
| `D:\CODE\ai\pgo-fork` / `01a123e9-…`（2026-10-10，本 bug 现场） | `glm-5.3-flash` | `glm-5-3-flash` = **148 calls**；`glm-5.3-flash` = **0 calls** |
| `D:\CODE\ai\grok-build-proxy` / `01a0ceb3-…`（2026-09-23） | `glm-5.3-flash` | `xiaomi/mimo-v2.6-flash` = 1；`glm-5-3-flash` = 83 |
| `D:\CODE\ai\websearch-mcpserver` / `01a0fa8a-…`（2026-10-02） | （`grok-4.6-build` 会话） | `grok-4.6` = **0 calls**；`grok-4.6-build` = 25 |

**0 成功调用的键只可能来自 `record_main_loop_failure`**（成功路径必写 1 次 `model_calls`），
所以前两行是"影子键"机制的直接指纹；第三行同理（`grok-4.6` 0 calls）。

**B. 代码链：两个键各自的来源（锚点以当前 `main` 为准）**

1. 成功键 = **上游回显 id**：`xai-grok-sampler/src/stream/chat_completions.rs:140`
   （`model = chunk.model.clone()`，取首个 chunk 的 wire `model`）→ `:401 model_id: Some(model)`
   → 落到 assistant item；`xai-grok-shell/…/sampler_turn.rs:2245`
   `record_model_call_usage(response.assistant().and_then(|a| a.model_id.clone()), …)`
   → `xai-chat-state/src/actor/mutations.rs:449` `model_key = 回显 id`（空则退回配置 id）
   → `usage.rs:156 record_main_loop_call(&model_key, …)`。
2. 失败键 = **配置 id**：`…/sampler_turn.rs:1167`
   `self.chat_state_handle.record_model_call_failure(None)`（在 `log_terminal_failure`（`:1165`）里，
   429/400 终止路径）→ `mutations.rs:475` `model_key = sampling_config.model`（None 兜底）
   → `usage.rs:177 record_main_loop_failure(&model_key)`，**只加 `failed_model_calls`，不加 `model_calls`**。
3. 显示侧按键取值：`xai-grok-shell/…/status_line.rs:127 scoped_usage(ledger, config.model)` ——
   `by_model.get(id)` **精确优先**，其次 `key.ends_with("/{id}")`（c66d29e 的 qualified 兜底），
   都不中才退回 `ledger.totals`；命中即 `ledger.totals = 命中条目`。
4. 段的可见性：同文件 `:183` `window_totals … .filter(|t| t.model_calls > 0)`（tokens/cache/think）
   与 `:263` `api_calls`（`model_calls > 0 || failed_model_calls > 0`）。

**C. 时间线（现场会话 `01a123e9`，与 `~/.grok/logs/unified.jsonl` 对齐）**

- 03:44:21 `session/new` 建会话（全量日志窗口内只有 `session/new`，无 load/resume/attach）。
- 04:19:05.882 与 04:19:09.054：两条 `shell.turn.inference_failed`（`kind: rate_limited, status_code: 429`）
  + 两条 `turn.terminal_failure` → 配置侧键 `glm-5.3-flash` 被建成 ✗ 计数条目（`✓ 0 ✗ 2`）。
- 04:19:05 之前的 turn 1 里 106 次调用全部记在回显键 `glm-5-3-flash`。
- 05:33:52 之后的 turn 4 又有 42 次调用，**也全部记在 `glm-5-3-flash`** → 影子条目再也不动，
  行冻死在 `✓ 0 ✗ 2` + 无 tokens 段。

**D. 形态吻合**：用户口述的行形状是「`model | ✓ n ✗ m | ttft · tps`」（数值为占位）——没有
tokens / cache / think 段，正是 `model_calls == 0` 命中条目的渲染结果。

### 已排除（下一会话不必重做）

| 原假设 | 排除依据 |
|---|---|
| 出口代理补丁（`a4e8883`/`76891f3`）副作用 | 本 bug 在账本/投影层，不在 HTTP 层；同会话换模型/直连同样触发 |
| `gateway_enabled` / `status_line_enabled` 被永久关断 | 生产写入点只有 attach/reconnect 窗口（`session_setup.rs:996` 关 → `:1553` 开；`agent_ops.rs:2577` 断连）——全量日志窗口内 `session.setup.phase` **132 条全是 `session/new`**，无 attach/重连，写入点从未经过 |
| 线索 1「刷新源门」（`status_line_policy.rs:203` source 漂移） | source 就是 `active_view.agent_id()`，而行的渲染判定用的是同一值；漂移会让行变 Reserved（空），与"内容冻住"不同 |
| 线索 2「command 行 run-slot 卡死」 | 本机 `config.toml` 无 `[ui.status_line]` → 走内置默认 items（`xai-grok-status-line/src/config.rs:149` `DEFAULT_ITEMS`），无 command / refresh_interval 路径 |
| 线索 3「刷新间隔失败静默」 | 同上：无 command 行 → `refresh_interval()` 恒 `None`，不存在 `REFRESH_FAILURES_TO_PAINT` 路径 |
| 客户端不重算（快照→`refresh_status_line_now`→重算链有闩锁） | 逐段读过：`status_context` 只写不空、`status_line_frame` 判定自洽、无永久早退；行冻住不是因为"不重算"，而是**每次重算出来的数字本来就不变** |
| 发射器任务挂死/退出 | `emit_loop` 的 await 点只有 signals / chat-state 两次 actor 往返，两个 actor 的 run 循环体内**无 await**（不会卡）；`emit_loop` 另有单测（`status_line_tests.rs:211,237`） |
| 快照序列化/解析静默丢弃 | 会整条丢快照 → 行会变空或只剩 model 段；现场行有 ✓/✗/perf 段，形态不符 |

### 修复方案

**P1 显示侧（必做，小改，`xai-grok-shell`）** —
`…/status_line.rs:127 scoped_usage`：命中条目为**纯 ✗ 影子**（`model_calls == 0`）时**不采用**，
退回全会话合计。精确条件建议：
`命中条目.model_calls == 0 && ledger.totals.model_calls > 0` → 不 scoped。
（这样"单模型会话只有失败"的既有行为不变；失败计数仍在 `totals` 里，`✗` 不会丢。）

**P2 归因侧（根治影子键成因，`xai-chat-state`）** —
`mutations.rs:475 record_model_call_failure` 的 `None` 兜底键从 `sampling_config.model`
改为「最近一次成功回显的 model id」：在 `record_model_call_usage` 成功路径里记一个
`last_echoed_model_id`，失败时优先用它，**从未回显过**才退回 `sampling_config.model`。
这样 `✓` 与 `✗` 落同一个键，P1 退化为安全网（老会话/历史账本仍需要它）。
唯一调用点 `sampler_turn.rs:1167` 传 `None` 是刻意的（失败时没有回显 id），补状态在 actor 里最小侵入。

**P3（可选，需拍板，别先做）** —
把"模型身份"收成一个：① 拼写容忍匹配（`.`/`-` 归一后比对，能直接救老账本），
风险是可能并掉本该区分的两个条目（如 provider 前缀条目与裸条目）；② 客户端把 catalog key
一起下送（LOCAL 已有 `model_display_name` overlay 通道），shell 按 catalog key 记账/取账。
建议先落 P1+P2，用活体与账本复核后再判要不要 P3。

### 回归保护（两次历史相关修复不能破）

- **`b68a036`「按当前模型 scoped」**：`status_usage_scopes_to_the_current_model`
  （`status_line_tests.rs:36-46`）必须保持绿——`m-b`（1 成功 + 1 失败）**不许**被 P1 一起退回全会话合计，
  否则别的模型消耗又漏进行里，老 bug 回归。
- **`c66d29e`「provider-qualified echo → 后缀匹配」**：`status_line.rs:129-144` 的
  `ends_with("/{id}")` 与测试 `status_usage_matches_gateway_qualified_echo_by_suffix`
  （`status_line_tests.rs:80-105`）必须保持绿；P1 的"纯 ✗ 不采用"判定要放在**后缀匹配之后**，
  不能把 qualified 命中一起否掉。
- **`status_usage_model_known_only_through_failures_reads_zeroed_calls`（`:48-57`）把现行为写成了期望**：
  P1 必须按新语义改写它（老注释"the failure itself surfaces through api_calls"只在单模型会话成立），
  改写时在测试里写明原因。
- `status_usage_without_a_model_keeps_cross_model_totals`（`:60`）、
  `status_usage_unknown_current_model_keeps_cross_model_totals`（`:69`）保持绿。
- LOCAL ✗ 计数器语义（"endpoint health，不涉 token/成本"）不变：P2 只改它**落哪个键**。

### 复现与验证计划（下一会话按此顺序）

1. **最便宜的决定性回归测试（先做，零活体）**——shell 单测按现场账本造夹具：
   `record_main_loop_call("glm-5-3-flash", …)`（1 次即可，给足 token）+ `record_main_loop_failure("glm-5.3-flash")` ×2，
   然后 `scoped_usage(ledger, Some("glm-5.3-flash"))`：修复前断言现值（`model_calls == 0`、`failed == 2`、
   无 token 窗口），修复后断言退回合计（`model_calls == 1`、tokens 段可见）。
   成本提示：该 crate（`xai-grok-shell`）**不在** `docs-local/win-whitelist.txt` → 需
   `GATE_FORCE=1 scripts-local/ctest.sh -p xai-grok-shell --lib status_line`（一轮 shell 全量编译）。
   备选：把 `scoped_usage` 下沉到白名单 crate（`xai-grok-status-line` / `xai-chat-state`），
   P1/P2 的测试就能同一轮跑完。
2. **P2 测试（`xai-chat-state`，白名单内，便宜）**：成功一次（回显 `x`）→ 失败一次（`None`）
   → 断言 ✗ 落在 `x` 键；从未回显过 → 断言落在 `sampling_config.model`。
3. **活体复现（可选，用于核对 perf 段与屏幕）**：假端点（先 429/400 再 200）+
   `PAGER_BINARY=D:\tool-cli\grok2\grok2.exe` 走 `xai-grok-pager-pty-harness`
   （`ScriptedResponse::json(429, …)` 已可用），或用 ptyctl 驱动真实 TUI 截图核对。
4. 落 `PATCHES.md`；按分支纪律走 `fix/local-status-line-shadow-key` 之类 feature 分支（main 只收文档）。

### 未决

- **用户给的样例行只说明形状，数值是占位**（2026-10-10 用户澄清）：`model id | ✓ n ✗ m |
  ttft · tps`，可用的信息只有两点 —— ① 行上**没有 tokens / cache / think 段**（= 命中条目
  `model_calls == 0`），② 行此后不动。因此**不要**拿样例行里的 ✓/✗ 数值去反推是哪个会话；
  现场会话（`01a123e9`）按账本应投影为 `✓ 0 ✗ 2`，形状一致。
- **perf 段是否也冻**：按代码它**不受模型 scoped**（`status_line.rs:285 build_turn_perf` 读 signals +
  `last_turn_api_duration_ms` + 最后一轮 usage），本应逐调用变化；而用户说整行都不动。
  样式数值不可用 ⇒ 从报告本身判不出 perf 是否也冻，需活体验证：若复现时 perf 段也在冻，
  说明还有第二条机制，届时按"快照是否还在到"再查一次。
- 本 bug **不在上游状态机**，因此 pigo 的 O1（状态行统计口径对齐）直接对齐**修后**语义即可，
  不必预留"同款坑"的规避设计；但 O1 若自己实现按模型分桶，**必须让成功与失败落同一个桶**（就是本文根因）。
- 本 bug **不在上游状态机**，因此 pigo 的 O1（状态行统计口径对齐）直接对齐**修后**语义即可，
  不必预留"同款坑"的规避设计；但 O1 若自己实现按模型分桶，**必须让成功与失败落同一个桶**（就是本文根因）。

## 结论（二）：修复落地（2026-10-10，分支 `fix/local-status-line-shadow-key`）

状态 → `fixed`（P1+P2 已施工、单测全绿；**活体 TUI 复现未做**，见文末「未做完」）。

### 改动

1. **P1 显示侧守卫**（`xai-grok-shell/…/acp_session_impl/status_line.rs` `scoped_usage`）：
   候选条目 `model_calls == 0`（纯 ✗ 影子）**且全会话有已完成调用**时不采用，继续试
   provider-qualified（`ends_with("/{id}")`）候选，都不中才退回**全会话合计**。
   - 判定**逐候选**生效：把 `.filter` 挂在 `.or_else` 链尾会让"精确命中是影子"直接短路掉
     后缀分支，等于把 qualified 命中一起否掉——这正是回归保护第 2 条要防的形态。
   - 采用某个候选时，把被跳过的**精确影子条目**的 `failed_model_calls` 叠回该候选：
     ✗ 是会话级 endpoint health，不能因为换了取数键就丢（退回全会话合计的路径本来就含它）。
   - 全会话 `model_calls == 0`（会话只失败过）时行为不变，零化条目照读（既有用例按新语义改写）。
2. **P2 归因键统一**（`xai-chat-state`）：
   - `src/actor/state.rs`：`ChatState.last_echoed_model: Option<(String, String)>`
     （配置 id → 上游回显 id，`// LOCAL:` 注释）。
   - `src/actor/mutations.rs`：`record_model_call_usage` 成功路径记该配对；
     `record_model_call_failure(None)` 的兜底键改为「**仍与当前 `sampling_config.model` 配对**的
     回显 id」（换过模型就不吃陈旧回显），从未回显过才退回配置 id。
   - `ChatState` 只有一处构造（`state.rs:237`），新字段不涉反序列化（会话账本本就不持久化）。
   - `sampler_turn.rs:1166` 调用点注释同步（原文写的是"按当前采样模型归因"，语义已变）。

### 验证（本机 Windows，`scripts-local/ctest.sh` 门控）

| 命令 | 结果 |
|---|---|
| `GATE_FORCE=1 ctest.sh -p xai-grok-shell --lib status_line` | **18 passed / 0 failed**（含 b68a036、c66d29e 两个历史修复用例） |
| `ctest.sh -p xai-chat-state --lib` | **399 passed / 0 failed**（含 P2 新增 3 例） |

- 新增用例（shell）：`status_usage_shadow_key_does_not_freeze_the_row`（**现场账本夹具**：
  `record_main_loop_call("glm-5-3-flash", …)` + `record_main_loop_failure("glm-5.3-flash")` ×2
  → 断言退回合计、tokens 段可见）、`status_usage_prefers_the_qualified_echo_over_a_shadow_config_key`
  （影子与 qualified 并存，账本里另放一个无关模型，防止"退回合计"蒙对；同时断言影子 ✗ 未丢）。
- 新增用例（chat-state）：回显优先 / 从未回显退回配置 / 换模型不吃陈旧回显（3 例）。
- 回归保护：`status_usage_scopes_to_the_current_model`（b68a036，混合 1 成功 + 1 失败仍按模型 scoped）、
  `status_usage_matches_gateway_qualified_echo_by_suffix`（c66d29e）、
  `status_usage_without_a_model_keeps_cross_model_totals`、
  `status_usage_unknown_current_model_keeps_cross_model_totals` 全绿；
  `status_usage_model_known_only_through_failures_reads_zeroed_calls` 按新语义改写（注释写明原因）。
- 顺带（与本 bug 无关）：`GATE_FORCE` 跑 `-p xai-grok-shell --lib subagent_usage` 暴露
  `subagent_usage_fold_tests` 整族挂——根因同族N/R 的 `create_test_actor` 夹具写死 `/tmp`
  （`support.rs:287` `AbsPathBuf` `NotAbsolute`），纯环境族；已按 win-skip 族R 格式补登记
  `session::acp_session::subagent_usage_fold_tests::`，复跑该过滤器 0 failed（全被跳过）。

### 未做完

- **活体 TUI 复现**：本机重编整客户端成本高，留待发版后用新客户端实跑"先 429/400 再成功"，
  同时回答「perf 段是否也冻」（见 §未决）。低成本备选：`xai-grok-pager-pty-harness` 的 mock
  端点（`ScriptedResponse::json(429, …)` → 200 SSE，SSE 里 model id 与配置拼写故意不同）。
- **P3 未做**：拼写容忍匹配 / 客户端下送 catalog key，待活体与账本复核后再判。

### 发版（2026-10-10）

分支 `fix/local-status-line-shadow-key`（代码提交 `6c912a8`）已合 main（merge `350874c`）并随
tag **`v1.0.45`** 发版：build run `38037822957`（Linux `cargo test -p xai-grok-shell` 全套 + Windows）
与 release run `38039766302`（Linux 原生 + Windows 交叉编译）双双 job success，release 页 4 资产，
产物实测 `grok2.exe --version` = `grok 1.0.45 (350874c36b81)`。

「未做完」两条不变：活体 TUI 复现（含 perf 段是否也冻）与 P3。
