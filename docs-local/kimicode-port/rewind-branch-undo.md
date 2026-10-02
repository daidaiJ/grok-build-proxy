# P1 设计原型：`/rewind` 分支树 undo（RewindMarker 单向折叠 → 可往返分支）

> 状态：设计原型，未开工、未验证。语义来源：kimi-code `agent-core-v2/src/agent/undo/undoService.ts`
> + `src/wire/`（基线 `21406fb`，2026-09-30，见 [README](README.md) P1 节）。
> fork 摸底基线：main `2b8adae` 后的 2026-10-02 工作区（本文所有锚点当日 grep 复核）。
> 摸底结论先于本文：fork 的持久层**已经是 append-only + 分支标记**，kimi 的
> SwitchEdge 等价物（`RewindMarker`）已存在，缺的是"分支可往返"与"分支面可见"。

## 1. 现状：fork `/rewind` 全链路（锚点与语义）

### 1.1 链路总览

```
/rewind 或 /undo（alias）         pager/slash/commands/rewind.rs:8-16
  → Action::RewindShowPicker      app/dispatch/router.rs:1593
  → dispatch_rewind_show_picker   app/dispatch/rewind.rs:125（busy 时先 CancelOffer，:140-145）
  → Effect::FetchRewindPoints     app/dispatch/rewind.rs:155-158
  → x.ai/rewind/points            extensions/rewind.rs:86（SessionCommand::GetRewindPoints）
  → get_rewind_points             session/acp_session_impl/rewind.rs:25
  → picker / confirm              views/rewind.rs:59（RewindPhase）、:123（handle_rewind_key）
  → Effect::RewindExecute         app/dispatch/rewind.rs:312
  → x.ai/rewind/execute           extensions/rewind.rs:64（SessionCommand::Rewind，run_loop.rs:1295）
  → handle_rewind                 session/acp_session_impl/rewind.rs:132
  → dispatch_rewind_success       app/dispatch/rewind.rs:345
```

`RewindPointInfo`（prompt_index / created_at / num_file_snapshots / prompt_preview /
has_file_changes）定义在 `views/rewind.rs:13`，由 shell 的 `get_rewind_points`
（`acp_session_impl/rewind.rs:25-79`）生成：**对每个 prompt 0..N-1 都给一个点**
（"Every prompt is a checkpoint"，`:23`），preview 取自 chat-state 快照的
`prompt_texts`，文件快照数来自 `file_state_tracker.get_rewind_point_metas()`。
点列表数据源是**内存中的 chat-state snapshot**（`acp_session_impl/rewind.rs:30-34`），
不是 journal。

键盘/鼠标入口在 `app/agent_view/rewind.rs:71/:124`，slash 命令 `session_scoped`
（`slash/commands/rewind.rs:12`），键位定义为 scrollback 块级 contextual action
（`actions/defaults.rs:425`）。

### 1.2 语义：哪里破坏、哪里 append-only

`handle_rewind`（`acp_session_impl/rewind.rs:132`）按 mode 分半
（`RewindMode::All/ConversationOnly/FilesOnly`，`session/acp_types.rs:325-335`）：

**会话半边（破坏性发生在三层，但最底层是 append-only）：**

1. **内存态（chat-state actor）物理截断**：标准路径直接
   `conversation.truncate(keep_count)`（`:374-377`，切点由
   `conversation_truncate_for_prompt` 计算，`xai-grok-sampling-types/src/conversation.rs:1730`），
   然后 `replace_conversation` + 快照回写（prompt_index/prompt_texts/compaction
   marker 全部改写，`:381-394`）。actor 侧 `ReplaceConversation` 还会触发
   `persistence.replace_history`（`xai-chat-state/src/actor/mutations.rs:561`）。
2. **派生缓存 `chat_history.jsonl` 整体重写**：`replace_history` 走
   `PersistenceMsg::ReplaceChatHistory`（`session/persistence.rs:258,2181`）→
   `replace_chat_history`（`storage/mod.rs:1403`）。注意 `chat_history.jsonl` 本身
   就是**派生缓存**——"Rebuild the derived chat_history.jsonl cache from
   updates.jsonl, **the durable source of truth**"（`storage/mod.rs:254`），
   随时可从 updates.jsonl 重建。
3. **持久真相 `updates.jsonl` 只追加**：rewind 只往里追加一条 `RewindMarker`
   （`acp_session_impl/rewind.rs:414-418`，经 `persist_xai_update_only`
   `acp_session_impl/updates.rs:936`）。源码注释原话："Because `updates.jsonl` is
   append-only, **rewinding creates a timeline branch**. The marker tells the replay
   algorithm to discard accumulated state beyond `target_prompt_index`"
   （`extensions/notification.rs:733-739`）。重放侧 `filter_rewind_by`
   （`storage/mod.rs:1571-1596`）把 marker 折叠为 `result.truncate(target)`，
   旧分支的行**留在文件里但永远被折叠**——审计有据，恢复无门。

**文件半边**：file_state_tracker 按 `>= target` 截断或折叠合并
（`acp_session_impl/rewind.rs:455-467`，`truncate_from`/`merge_and_remove_from`），
磁盘 rewind-points 索引同步截断（`storage/mod.rs` 的
`truncate_rewind_points_from`/`merge_rewind_points_from` trait，`:1395-1403` 附近）。
文件内容回滚语义与 kimi 的 fileHistory 一样是另一套独立机制（kimi 自己的 `/undo`
也不回滚代码，README P1 节第 4 点），本设计不动它。

**副作用清理**：rewind 后清 turn summary/recap（`:420-433`）、重置 compaction
suppression（`:396-408`）、重开 title-refresh 水位（`:435-451`）、
后台任务列表按死分支过滤（`agent/mvp_agent/mod.rs:1268-1284` 用
`filter_rewind_lines`）。

### 1.3 compaction 与 rewind 的边界（已有精确计算，kimi ForkLineError 已有对应物）

- **是否跨 compaction**：`needs_compaction_replay` 查
  `last_compaction_prompt_index`（`acp_session_impl/rewind.rs:111-127`）——一旦
  发生过 compaction，`truncate_to_prompt_index` 按 User 计数的切法对**所有**
  post-compaction 目标都错（`:108-110` 注释），必须回放重建。
- **回放重建**：`replay_to_prompt`（`session/helpers/replay.rs:85`）流式扫
  updates.jsonl，遇 `CompactionCheckpoint` 装载 base（checkpoint 全文在
  `compaction_checkpoints/{checkpoint_id}.json`，结构定义
  `extensions/notification.rs:1404`，`replay.rs:246` `handle_checkpoint`：
  target 在 compaction 之前只取 `original_user_info`，之后才装载 base blob）；
  遇 `RewindMarker` 折叠（`replay.rs:346` `handle_rewind_marker`，含 base 栈
  回退与 lossy 截断注释）。
- **边界规则**：target 早于**最早存活的 checkpoint** 时回放是 lossy 的
  （base 全部弹出后按 raw items 截断，`replay.rs:372-374` 注释 "lossy"）；
  EOF 处 innermost base 不可读直接整体失败（`replay.rs:118`）；
  旧 checkpoint 文件被 30 天清理删除时不得阻塞基于较新 checkpoint 的 rewind
  （pty 回归测试 `xai-grok-pager-pty-harness/tests/pty_e2e/rewind_after_compaction_with_missing_checkpoint.rs:48`）。
- **缺口**：这套边界判定是**执行时**的——picker 的点列表（0..N-1 全量）不
  区分哪些目标在边界之外/会 lossy，用户选中后才可能失败或拿到被摘要折叠的
  重建结果。kimi 的 `ForkLineError('compaction_boundary')` 是**预计算**，
  不可撤的点在选择前就被排除/标注。

### 1.4 UI 行为：原 prompt 放回编辑器**已经存在**

- shell 在 rewind 时从快照取出目标轮原始 prompt 文本放进响应
  （`acp_session_impl/rewind.rs:286-298`，`prompt_text` 字段），
  同时存入 `rewind_pending_prompt`（`acp_session.rs:821`）。
- pager 成功处理里 `agent.prompt.set_text_discarding_images(prompt_text)`
  （`app/dispatch/rewind.rs:396-397`），否则恢复 stash 的草稿（`:398-400`）。
- 下一轮 prompt 时对比新旧文本，命中同一文本记 `regeneration`、改写记
  `edit_and_retry`（`acp_session_impl/turn.rs:596-608`）——kimi
  `restoreInputText` 的完整等价物已在，含"改了再发"的遥测区分。

### 1.5 相邻但不同体的两个 rewind 子系统（不要混淆）

- **subagent attempt store 的 rewind 记录**
  （`agent/subagent/attempt_store/rewind.rs:102` `RewindRecordV1`：Live/
  Superseded/Release/Checkpoint 定长二进制行）：子 agent 会话文件存储层的
  回收/回退账本，与主会话分支树无耦合，本设计不动。
- **`xai-sqlite-journal` 与 `xai-grok-session-events` 都不是会话历史 journal**：
  前者是 SQLite WAL/TRUNCATE 模式选择器（`xai-sqlite-journal/src/lib.rs:29`
  `JournalMode`，NFS 兼容），后者是 per-session 遥测 `events.jsonl`
  （`xai-grok-session-events/src/lib.rs:1-7`）。README P1 节"fork 落点"里点
  这两个 crate 名是**不准确的**，真正的落点是 `updates.jsonl` 持久层。

## 2. kimi 方案要点 → fork 映射

| kimi 机制 | fork 现状 | 差距 |
|---|---|---|
| `SwitchEdge` 追加进 wire.jsonl，声明式分支切换（`switchBranch({turns, fromTurnId})`） | `RewindMarker` 追加进 updates.jsonl（`acp_session_impl/rewind.rs:415`） | 已有等价物；但语义是**单向折叠**（filter/replay 只会向后截），无"切回旧分支" |
| 旧分支永久保留、可审计 | updates.jsonl 旧分支行物理保留；但 chat-state 内存、`chat_history.jsonl`、rewind-points 文件均物理截断 | 派生层截断可接受（可重建）；内存态截断使旧分支的 prompt_texts/prompt 索引丢失，picker 再也看不到旧分支的点 |
| `ForkLineError('compaction_boundary'/'insufficient')` 边界预计算 | `needs_compaction_replay`（`:111`）+ `replay_to_prompt`（`replay.rs:85`）执行时判定 | 判定存在但不在 picker 预计算；不可撤/会 lossy 的点照常列出 |
| undo 后原 prompt 放回编辑器（`tui/commands/undo.ts:193-208`） | `prompt_text` 放回 + `rewind_pending_prompt` regeneration/edit_and_retry 遥测 | **无差距**（1.4 节） |
| 撤销的撤销（redo）= 再追加一条 SwitchEdge 指回旧分支 | 无任何 redo 通道（grep 无 Redo 命令/动作） | 全部新增工作集中在这里 |
| 分支点列表来自 wire 树 | 点列表来自 chat-state snapshot（`:30-34`） | 需改为"journal 推导的分支面"，见 §3 |

## 3. D1 推荐结论与理由

**结论：走分支树路线，但不是"新建一套 journal"，而是把现有 `RewindMarker`
从单向折叠升级为可往返的分支选择记录（SwitchEdge 化），chat-state 内存态从
"截断后写入"改为"按活跃分支重建"。**

理由：

1. **持久层前提已经成立**。fork 的 updates.jsonl 是 append-only + 分支标记 +
   回放折叠，与 kimi wire.jsonl 的事件溯源模型同构；README 担心的
   "分支树 vs chat-state 快照"二选一，在摸底后变成"快照路线已寄生在
   append-only 真相之上"，升级成本远低于预想。
2. **快照路线到头了**。ChatStateSnapshot（`xai-chat-state/src/types.rs:28`）
   只覆盖当前时间线；要支持"撤回撤销/切回旧分支"，就得在快照之外再存
   abandoned 快照——等于在 journal 之外再造一份真相，与
   "updates.jsonl 是唯一真相"（`storage/mod.rs:254`）的既有架构对抗。
3. **重放确定性免费拿**。分支切换记录进 updates.jsonl 后，resume/回放/
   background-task 死分支过滤（`mvp_agent/mod.rs:1284`）自动获得一致的
   分支语义，只需让 `filter_rewind_by` 与 `handle_rewind_marker` 认识
   "指向前分支的 SwitchEdge"。
4. **用户体验差距其实只有一项**：prompt 放回（1.4）与文件不动语义都已达标，
   真正缺的是"后悔了能切回去"与"旧分支的点还能选"。

## 4. 改动点分层清单

### 动（按层）

**L1 shell 持久语义（核心）**

- `RewindMarker` 增量兼容扩展：追加字段标识"这次 rewind 切向哪个分支"
  （向前截断 = 现状；向后/指回 abandoned 分支 = 新 SwitchEdge 语义）。
  serde `default` 保持旧记录可读。
- `storage/mod.rs` 的 `filter_rewind_by`（`:1571`）与
  `session/helpers/replay.rs` 的 `handle_rewind_marker`（`:346`）：从
  "truncate 到 target"泛化为"把活跃指针切到 target 所在分支"——折叠语义
  只对**当前活跃分支之外**的行生效。
- `PromptExtractEvent::RewindTo`（`storage/mod.rs:1731`）同步泛化。

**L2 shell 会话执行（`acp_session_impl/rewind.rs`）**

- `handle_rewind`（`:132`）：标准路径不再"本地 truncate + 快照回写"，
  改为追加 SwitchEdge → 由 L1 的重建通道生成会话态（复用
  `replay_to_prompt`，跨 compaction 与普通路径统一——消除
  `needs_compaction_replay` 的双路径分叉）。
- `get_rewind_points`（`:25`）：点列表从"chat-state snapshot 的
  prompt_texts"改为"journal 分支面推导"（当前分支全量点 + 可选展示
  abandoned 分支），并预计算边界标注（§2 ForkLineError 行）。
- chat-state actor 不再需要 rewind 专用截断命令的"改写快照字段"用法
  （`:382-394`），快照回归纯派生。

**L3 pager UI（可选增量）**

- picker（`views/rewind.rs:59` RewindPhase）增加 abandoned 分支分组/标注；
  `/rewind` 的 `RewindResponse`（`acp_types.rs:355`）无 schema 破坏。
- 无需新 slash 命令：切回旧分支 = 对 abandoned 分支的点再执行一次 rewind
  （就是 redo），UX 零新概念。

### 不动

- 文件回滚半边（file_state_tracker 截断/合并，1.2 节）。
- prompt 放回链（1.4 节，已达标）与 `rewind_pending_prompt` 遥测。
- subagent attempt store rewind 记录（1.5 节）。
- `xai-sqlite-journal`、`xai-grok-session-events`（与本题无关，1.5 节）。
- compaction 本体与 checkpoint 存储格式。
- gateway-bridge 模式的会话半边（服务端持有会话；`extensions/rewind.rs:6`
  的 doc comment 提到 `handle_bridge`，但该文件内只有 `handle`，bridge 组合
  路径未核实——见 §7 风险 4）。

## 5. 验收标准

1. **非破坏可证**：rewind 两次后，session 目录 updates.jsonl 仍包含两次
   被弃分支的全部行（grep 断言）；`chat_history.jsonl` 重建（`rebuild_chat_history`
   `storage/mod.rs:268`）后与活跃分支一致。
2. **redo 闭环**：rewind → 对 abandoned 分支的点再次 rewind 成功切回，
   会话继续跑一轮成功；全程无新文件格式。
3. **旧版兼容**：只含旧格式 `RewindMarker` 的会话文件，新二进制回放结果与
   旧二进制逐条一致（夹具回放对比）。
4. **边界标注**：picker 对早于最早存活 checkpoint 的目标点标注不可撤
   （对应 pty 测试场景），执行时 `replay_to_prompt` 的失败/lossy 语义不回归。
5. **回归面**：`ctest.sh -p xai-grok-shell --lib`（acp_session_tests 的
   rewind_cross_compaction / rewind_synthetic_turn / load_user_prompts 族）+
   `-p xai-chat-state --lib` + `-p xai-grok-pager --lib`（views/dispatch rewind
   用例）全绿；`rewind_after_compaction_with_missing_checkpoint` pty 用例
   手动跑通一次。

## 6. 待定决策

1. SwitchEdge 记录形态：扩展现有 `RewindMarker`（加字段）vs 新增变体——倾向
   扩展（`skip_serializing_if` 保旧读新）。
2. abandoned 分支的点是否默认进 picker 列表（默认关 = 渐进增强，`/rewind`
   参数或设置项开）——倾向默认关。
3. L2 统一重建路径是否连 FilesOnly/All 的文件半边时序一起重排（现文件回滚
   先于会话截断，`:250-283`）——倾向本专题不动文件半边，仅会话半边统一。
4. bridge 模式（服务端会话）是否跟进 SwitchEdge——先登记，等 bridge 侧
   rewind 组合路径摸清（见 §7 风险 4）。

## 7. 风险

1. **`filter_rewind_by` 是多消费面共用件**：replay、prompt 提取
   （`load_user_prompts_from_updates`）、后台任务死分支过滤
   （`mvp_agent/mod.rs:1284`）都吃同一条折叠语义；泛化时任何一条消费面漏改
   都会出现"回放看 A 分支、任务列表看 B 分支"的分裂。缓解：以回放为准绳，
   其余消费面在 §5-1 的重建断言里各配一条。
2. **分支深度与 O(n) 回放成本**：SwitchEdge 双向往返会让 updates.jsonl 的
   有效折叠变浅（更多历史行处于"曾经活跃"），`replay_to_prompt` 全量流扫的
   成本随总行数而非活跃分支长度增长。缓解：checkpoint base 机制天然压制；
   必要时对 marker 记录行号做跳扫优化（现 `filter_rewind_lines` 已有
   "无 marker 不解析"的前置，`storage/mod.rs:1654` `filter_rewind_lines` 同思路）。
3. **chat-state 快照派生面收窄的连锁**：`prompt_texts` 从快照真相降级为
   派生缓存后，preview 生成（`acp_session_impl/rewind.rs:45-60`）与
   `extract_user_query` 依赖面需跟着走 journal 提取；遗漏会在长会话
   resume 后表现为 picker 预览丢失。
4. **bridge 模式盲区**：`extensions/rewind.rs:6` 注释与代码不符
   （无 `handle_bridge`），bridge 下会话 rewind 由谁组合未核实；施工前先
   用真实 bridge 会话复测 rewind 全链路再定 §6-4。
5. **上游同步面**：rewind 相关文件多属上游同步线（`Synced from monorepo`
   会覆盖），改动须在 `docs-local/PATCHES.md` 登记以便重放。

## 8. 分期粗估（C 级）

| 期 | 内容 | 规模 |
|---|---|---|
| T1 | L1 记录与回放语义（SwitchEdge 化 + 三消费面 + 旧格式兼容夹具） | 净开发 2–3 人天 |
| T2 | L2 handle_rewind 统一重建 + get_rewind_points 分支面 + 边界预计算 | 净开发 2–3 人天 |
| T3 | L3 picker 分支标注 + pty 回归 + PATCHES.md 登记 | 净开发 1 人天 |

依赖：无前置专题；与上游同步节律强相关（建议紧跟一次同步后开工，降低 §7-5
重放成本）。
