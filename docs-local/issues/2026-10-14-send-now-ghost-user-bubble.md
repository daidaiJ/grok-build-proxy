# Ctrl+Enter 立即入队后,对话流偶发把上一条输入重新画成新气泡

- **状态**: open(待带取证复现)
- **严重度**: P2(功能可用,显示错误)
- **发现方式**: 用户活体使用口述(2026-10-14),非测试发现
- **环境**: 用户实机部署 `grok 1.0.55 (4788f51c58c9)`;排查时工作区 HEAD
  `9e7130dc`(feat/local-ayu-theme,仅文档/测试新增,未改队列代码)

## 症状

回车回合运行中,用户新输入提示词按 Ctrl+Enter 立即入队(实为 send-now:
取消当前回合并把新提示词排到下一个)。大多数时候一切正常;偶发**对话流里
新提示词气泡下方又出现一条上一条输入的气泡**(用户确认:纯显示层问题,
「看大模型输出完全没有上下文错位」)。

用户确认重复出现在**对话流(scrollback)**,非队列面板/输入框。

## 已核实事实

1. **落盘无重复**(本机取证,会话 `01a13a63-8780-7763-9314-2c72d76d45ee`,
   2026-10-14):`updates.jsonl` 全部 10 条 `user_message_chunk` 每条恰好一次,
   `promptIndex` 0..9 连续无空洞。重复只存在于活动 scrollback 的乐观绘制层,
   会话重放/模型上下文不受影响。
2. **用户实际操作两种模式混用**(unified.jsonl 逐事件核对,pid 13628):
   - 模式 A:输入文本直接 Ctrl+Enter(非空 composer send-now)
     ——12:35:58 len21、12:39:24 len30、12:47:09 len18、13:46:23 len30;
   - 模式 B:先 Enter 本地排队(`prompt.enqueue` + `prompt.drain_blocked
     turn_running queue_depth=1`),再空 composer Ctrl+Enter 强发队列顶行
     ——12:35:24+12:36:31(len76)、13:06:09+13:06:11(len57)、
     13:37:17+13:37:19(len36)。
   全部 11 次 send-now 的 shell 决策均为 `cancels_turn:true、
   merged_as_interjection:false`,且无 `from_queue_row` 字段(即全部走
   session/prompt 直发路径,无一次队列行 interject)。
3. Ctrl+Enter 中途语义:`agent_view/prompt.rs:561-606`(InterjectPrompt 臂):
   非空 composer 且 `can_send_now()` → `Action::SendPromptNow`(取消并立即发);
   空 composer → `try_send_now_queued_from_prompt` 强发队列顶行
   (`agent_view/queue.rs:81-100` → `force_interject_queue_row:362-406`,
   本地行 `remove_local_queue_row` 弹出后转 `SendPromptNow`)。
4. `unified.jsonl` 不含 qtrace 事件(六个事件名全 0 命中):qtrace 是
   `tracing::debug!(target: "qtrace")`,`unified.jsonl` 的过滤器不收。
   `GROK_DEBUG_LOG=1` firehose 过滤器含 `xai_grok_pager=debug,xai_grok_shell=debug`
   (`telemetry/src/logs/debug_log.rs:60`),按会话落
   `~/.grok/debug/<session_id>.txt`——**这是拿到决定性现场的正道**。

## 乐观绘制链路现状(静态核对,均已有防护)

send-now 全链:dispatch 画气泡 + `send_now_painted_blocks[P]`/`send_now_echo_pending[P]`
(`dispatch/interject.rs:170-180` → `dispatch/queue.rs:779-822`)+ 乐观队列回显
(`dispatch/queue.rs:52-73`);shell 取消当前回合、入队、promote 后广播
`x.ai/queue/changed`(running_text = 新行文本,`shell acp_session_impl/
prompt_queue.rs:449-536`);pager 认领:FIFO 交接竞态 stash(`acp_handler/
queue.rs:380-430`)→ `apply_turn_start_shim` 按 prompt_id 认领已画气泡
(`dispatch/queue.rs:976-1038`);shell 用户回显经
`take_send_now_user_echo` 按吞掉(`acp_handler/mod.rs:327-339`)。

已核对在位的防护:
- 同 id 同文本重复画:`push_send_now_user_block` repeat 臂直接返回
  (`dispatch/queue.rs:789-802`);
- parked send-now 对 running 行不 fire:`resolve_send_now_awaiting_confirm`
  (`agent_view/queue.rs:411-431`)`running == awaiting` 分支;
- shell 对 running/stale/出队行纯 no-op + 重播(`shell prompt_queue.rs:881-889`
  `"queue send-now no-op (running id / stale / drained / not owner)"`);
- 广播后失确认行气泡退役:`handle_queue_changed` removed_painted
  (`acp_handler/queue.rs:233-253`)。

## 未证实假设(按嫌疑排序,均待 GROK_DEBUG_LOG=1 现场判定)

1. **H1 fire-时点气泡重画**:第二次 Ctrl+Enter 停在乐观回显上
   (`force_interject_queue_row` optimistic 分支 park,`agent_view/queue.rs:375-384`),
   确认广播到达时 fire → `arm_send_now_and_paint`(`acp_handler/queue.rs:281-289`
   → `dispatch/queue.rs:855-870` → `push_send_now_user_block`)。若 fire 与
   shell 处理之间行被 promote/版本失效,shell no-op 而 pager 已画
   → 幽灵气泡。repeat 臂只在 `painted[P]` 仍在时护住;`painted[P]` 被认领
   (shim)或退役后重画无防护。**未构造出完整触发序列**。
2. **H2 shim 文本漂移重画**:`apply_turn_start_shim` 认领时
   `text != ub.text && !edited` → 删已画块 + 落到按文本尾匹配/新画
   (`dispatch/queue.rs:985-1038`)。若 `running_text` 取到旧值——
   fallback 链 `changed.running_text → 本地镜像 running_entry.text`
   (`acp_handler/queue.rs:150-170`,镜像取 apply 前快照)——会把旧文本画出来。
3. **H3 回显吞失败**:`take_send_now_user_echo` 文本兜底匹配失败
   (`agent_view/queue.rs:239-261`)且 shim 已 `clear_user_echo_skip`
   (`dispatch/queue.rs:911-913`)→ 回显进 tracker 被画成第二条气泡。
   需要 echo 文本与 pending 文本不一致才触发。
4. **H4 interjection 广播兜底重画**:`handle_interjection` 尾部
   `push_block(interjection_prompt)`(`acp_handler/mod.rs:728-733`)对
   已被认领(send_now_painted_blocks 已 remove)的 send-now 行失去去重。

## 归属判定

**上游自带(2026-10-14 二次核对收口)**:prompt-queue / send-now / interject 整套机制为
上游代码;对照 `PATCHES.md` 各期(主题/目录/auth/用量/状态行/rewind/工具面/
LSP 等),且 `git log -- <file>` 实证全部相关文件(`dispatch/queue.rs`、
`acp_handler/queue.rs`、`agent_view/queue.rs`、`dispatch/interject.rs`、
shell `prompt_queue.rs` / `interjection.rs`、`acp/tracker.rs`)**只有
"Synced from monorepo" 提交、从未被本地补丁触碰**;最后上游变更落在
2026-09-19/09-23(v1.0.37 同步线)——该竞态自 v1.0.37(2026-09-28 发版)
起就存在,非近期本地回归。修后视上游态度决定是否上报。

## 发版纪律(用户指令,2026-10-14)

修复完成后**重指现有 `v1.0.55` tag 重发,不切新 tag**;推送前本机验证,
重指后 `build.yml` / `release.yml` 双 job success、4 资产非草稿才算完成,
并活体复验部署端 `grok2 --version`。沿 v1.0.39/41/51 重指先例;
注意删 tag 可能令 release 转 draft,重推后须确认已恢复发布。

## 复现与取证计划

0. **qtrace 采集前置知识(2026-10-14 实证)**:qtrace 是
   `tracing::debug!(target: "qtrace")`,EnvFilter 按显式 target 匹配——TUI 常驻
   过滤器虽有 `xai_grok_pager=trace`(tracing.rs:374),qtrace 事件照样被滤掉
   (unified.jsonl 六个事件名 0 命中即证);firehose(`GROK_DEBUG_LOG=1`)是
   RUST_LOG 免疫的固定指令表(debug_log.rs:60/345/353),也没有 qtrace 项,
   同样收不到。**能收到 qtrace 的只有 `GROK_LOG_FILE` 单文件模式**
   (`default_file_filter` 遵守 RUST_LOG,debug_log.rs:74-84/352)。
1. **活体取证(决定性,零改动)**:用现有 v1.0.55 客户端带环境变量启动:
   `GROK_LOG_FILE=D:\temp\grok-qtrace.txt RUST_LOG="info,qtrace=debug" grok2`
   ——单文件 debug 日志会按序记录 qtrace 全事件(`server_broadcast_queue` /
   `queue_changed_recv` / `adoption_stashed` / `turn_start_shim` / `local_drain`
   / `turn_end`,均带 48 字符文本预览,pager 与 shell 两侧都有),旧文本从哪个
   事件冒出来可直接判定 H1–H4。复现后把该文件交给排查会话。
2. PTY 自动复现:已写 `xai-grok-pager-pty-harness` 场景
   `scenarios/send_now_chain_ghost.rs` + `tests/send_now_chain_ghost.rs`
   (模式 A+B 混合链,断言每个 prompt 气泡恰好一次 + wire 恰好一次,未提交)。
   **本机 Windows 跑不了**:harness 起真机 TUI 停在空屏(既有
   `empty_enter_send_now` 同样失败,2026-10-14 实测),该测试面向 Linux CI;
   根因定位后可扩成回归用例。

## 结论

(待复现后回填)
