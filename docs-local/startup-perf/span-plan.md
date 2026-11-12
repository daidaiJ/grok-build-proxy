# 启动 span 细化专题（方案）

> **状态：方案（2026-11-12），未开工。** 基线 `grok2 1.0.73 (a4f73dc8)`。
> 目标：把「进程启动 → TUI 首帧 → 会话就绪 → 首条 prompt → 首 token」和
> 「quit → 进程退出」的每一段补成可回归的 span/事件；先消盲区，再谈优化。
> 现场数据与复现流程见 [`README.md`](README.md)，本文件只定义测量面与施工切片。

## 1. 为什么现在补

2026-11-12 复测（`~/.grok/logs/unified.jsonl`，v1.0.72/1.0.73，18 次会话创建）：

| 指标 | 实测 | 备注 |
|---|---|---|
| `startup interactive`（TUI 可交互首帧） | 772–1673ms，中位约 880ms | 启动体验的第一段 |
| `session.create.done`（新会话就绪） | 3016–6867ms，中位 4675ms | 当前最大卡点 |
| 首条 prompt 的 MCP grace | 最多 2s | `MCP_STARTUP_GRACE` |
| 空闲退出 `worker_join` | 76–203ms | 正常 |
| 刚启动/运行中退出 | 2148ms + `Finishing session…` | 2s 级尾巴 |

现有埋点只能回答「总共多慢」，回答不了「慢在哪一段」：

- `startup timing` 只在 `startup::is_active()` 期间镜像到 `unified.jsonl`；裸启动时
  会话创建发生在 `startup interactive` 之后，`session.spawn*` 计时全部不出现在
  unified，只在 `GROK_INSTRUMENTATION=log` 的 `instrumentation.log` 里。
- 11-09 唯一一次完整样本显示 `session.spawn_and_register` 4264ms，其中
  `agent_build` 2278ms（`skills_discovery` 1955ms），**进入 spawn 前的 1757ms
  没有名字**。
- MCP 只有 `session.mcp_init` 总时长；5 个 server 谁慢、慢在 connect 还是
  list_tools，没有字段。
- 退出只有 `session_end.worker_join` 总时长；flush、upload drain、worker runtime
  shutdown、terminal restore 分不清。

## 2. 现有埋点地图

| 层 | 名称 | 落点 | 门槛/生命周期 | 现有字段 | 局限 |
|---|---|---|---|---|---|
| 启动阶段 | `startup phase` / `startup timing` / `startup complete` / `startup interactive` | `xai-grok-telemetry/src/spans/startup.rs` → `unified.jsonl` | `startup::is_active()`；首帧后 `DONE` | `phase`、`name`、`elapsed_ms`、`startup_total_ms` | 首帧后不再镜像；无会话内部拆解 |
| 连接 | `connect finished` | shell `agent` / pager | 每次连接 | `elapsed_ms`、`phases`、`connect_target` | 只到 ACP initialize |
| 会话阶段 | `session.setup.phase` | `agent/mvp_agent/session_setup.rs` | 每次 `session/new` / `load` | `phase`（进入时刻） | 无耗时；只能靠相邻时间戳做差 |
| 通用计时 | `instrumentation_timer!` | `xai-grok-telemetry/src/spans/instrumentation.rs` | `GROK_INSTRUMENTATION=log` | `name`、`elapsed_us`、`fields` | 深挖通道；默认不进 unified |
| Span 画像 | `GROK_SPAN_PROFILE_OUT` | `xai-grok-telemetry/src/spans/span_profile.rs` | 优雅退出 | folded self-time | 杀进程丢失；常驻 span 累计值需过滤 |
| 退出 | `session_end.worker_join` | `xai-grok-pager/src/acp/spawn.rs` | 每次优雅退出 | `elapsed_ms`、`outcome`、`notice_shown` | 无内部拆分 |
| 终端 | `teardown.kitty_pop_fence` | `pager/src/app/teardown_fence.rs` | 仅 kitty 键盘协议路径 | `decision`、`reader_join` | 不覆盖普通退出 |
| 首轮 | `turn.tool_prep` span | `session/acp_session_impl/sampler_turn.rs` | 每轮 | `mcp_wait_ms`、`total_prep_ms` | tracing span，不进 unified |
| 请求性能 | 状态行 TTFT/TPS | `docs-local/status-line-perf-wiki.md` | 每请求 | 请求级 | 不含启动与会话创建 |

## 3. 盲区清单

### P0：不补就无法做启动优化

1. **会话创建关键路径没有统一事件。** `session.create.done` 只有 RPC 总时长；
   `session.spawn*`、`agent_build`、`skills_discovery` 在首帧后不进 unified。
2. **pre-spawn 1757ms 无埋点。** `session.spawn_and_register` 开始到
   `session.spawn_actor_call` 之间只有 `session_env` 一个计时；候选工作：
   `build_lsp_backend`（唯一 `await`，`agent_ops.rs:4676`）、trust、
   plugin registry、agent definition/profile、envrc、hunk tracker、
   workspace ops、model/effort 解析。
3. **skills discovery 只有一个总数。** 需要拆 walk（目录遍历）与 parse
   （frontmatter/YAML/markdown），并按来源计数：user / repo / bundled /
   plugin / server；同时记录 disabled 数量。`[skills] disabled` 只影响提示词和
   调用，发现阶段仍会解析。
4. **MCP 初始化没有 per-server 拆分。** 需要 `connect_ms` /
   `initialize_ms` / `list_tools_ms` / `tool_count` / `outcome`，以及
   `session.mcp_ready`（总时长、成功/失败数、是否吃满 grace）。
5. **首条 prompt 的等待链不可见。** enqueue → SessionCreated → MCP grace →
   tool prep → 请求发起，目前只有 `turn.tool_prep` span 的 `mcp_wait_ms`。
6. **退出内部不可见。** `flush_all_sessions`、`drain_pending_uploads`、
   `shutdown_worker_runtime`、`restore_terminal`、`unified_log::flush_blocking`
   各自耗时没有字段；2s 尾巴无法归因到 `MCP_STARTUP_GRACE` 还是
   `WORKER_RUNTIME_SHUTDOWN_GRACE`。

### P1：下一步

7. **resume / `session/load` 只有总时长。** 需要 read updates、replay、
   context build、compaction checkpoint、spawn 拆分；大 `updates.jsonl`
   的历史回放是独立风险。
8. **plugin / hooks 发现重复且无计时。** 一次进程内多次 refresh，需要
   来源计数与单次耗时，避免把插件发现算进会话创建。
9. **模型目录拉取的“跳过/快速失败”原因不在字段里。** 当前 BYOK 配置下
   `fetch_models_blocking` 0ms，但日志只有 `model catalog refresh failed`；
   需要 `skipped_reason` / `attempt` / `cache_state`，并限制重试噪声。
10. **进程起点依赖 launcher。** `spawn_to_first_frame_ms` 只在设置
    `GROK_SPAWN_TIMESTAMP_MS` 时存在；benchmark 与用户现场需要同一口径。

## 4. 目标时间线

统一定义 t0–t11，所有事件都带 `session_id`（有会话后）和 `first_session`：

| 点 | 含义 | 事件/span | 关键字段 |
|---|---|---|---|
| t0 | 进程 spawn | `startup.spawn`（依赖 launcher 时间戳） | `spawn_at_ms` |
| t1 | 配置/managed policy/bootstrap | 现有 `startup phase` | `phase`、`elapsed_ms` |
| t2 | ACP connect | 现有 `connect finished` | `elapsed_ms`、`phases` |
| t3 | 首帧绘制 | `record_first_draw` | `time_to_first_frame_ms` |
| t4 | 可交互 | `startup interactive` | `interactive_ms`、`spawn_to_first_frame_ms` |
| t5 | 新会话就绪 | `session.create.done` + 新增 `session.create.breakdown` | RPC 总时长 + 子阶段 |
| t6 | MCP 就绪 | 新增 `session.mcp_ready` | `total_ms`、`ok`、`failed`、`grace_used_ms` |
| t7 | 首条 prompt 发起 | 新增 `turn.first_prompt` | `queue_ms`、`mcp_wait_ms`、`prep_ms`、`to_request_ms` |
| t8 | 首 token | 状态行 TTFT 已有；补 `first_token_ms` 到 `turn.first_prompt` | `ttft_ms` |
| t9 | quit 信号 | `exit.teardown.start` | `busy`、`reason` |
| t10 | 会话 flush / drain / join | 新增 `exit.teardown` 拆分 | `flush_ms`、`drain_ms`、`runtime_ms`、`join_ms` |
| t11 | 进程退出 | `exit.teardown.done` | `total_ms`、`notice_shown` |

`startup complete` 保持“启动账本结项”的现有语义（通常早于
`startup interactive` 首帧事件）；会话创建、MCP、首 prompt 用独立的
`session.startup.*` 事件族，不再依赖 `startup::is_active()`。这样既有消费者
不受影响，新事件也不会被首帧截断。

## 5. 建议的事件与字段

### 5.1 `session.create.breakdown`（shell 侧，info）

由 `new_session` / `load_session` 结束时发一次；`cwd` 只保留类别
（project / home / non-git），不写完整路径。字段是**嵌套**关系：顶层四桶
相加等于 `total_ms`，子桶只在各自父项内拆分，不重复计入总账。

```json
{
  "session_id": "…",
  "first_session": true,
  "kind": "new",
  "total_ms": 4675,
  "persistence_ms": 404,
  "spawn_and_register_ms": 4264,
  "git_discovery_ms": 3,
  "finalize_ms": 4,
  "spawn_and_register": {
    "pre_spawn_ms": 1757,
    "spawn_actor_call_ms": 2507,
    "pre_spawn": {
      "lsp_build_ms": 0,
      "trust_ms": 0,
      "envrc_ms": 45,
      "agent_define_ms": 0,
      "hunk_ms": 0,
      "plugin_registry_ms": 0,
      "other_ms": 1712
    },
    "spawn": {
      "agent_build_ms": 2278,
      "tool_setup_ms": 0,
      "history_persist_ms": 0,
      "hooks_discovery_ms": 0,
      "workflow_restore_ms": 0,
      "config_load_ms": 0,
      "git_scan_ms": 0,
      "other_ms": 229,
      "agent_build": {
        "skills_ms": 1955,
        "skills_walk_ms": 600,
        "skills_parse_ms": 1355,
        "skills_count": 140,
        "skills_disabled_count": 11,
        "skills_sources": {"user": 64, "repo": 0, "bundled": 22, "plugin": 1, "server": 0},
        "tool_registry_ms": 180,
        "agents_md_ms": 30,
        "subagent_discovery_ms": 14,
        "gitignore_ms": 11,
        "other_ms": 88
      }
    }
  }
}
```

顶层四桶 `persistence_ms + spawn_and_register_ms + git_discovery_ms +
finalize_ms` 相加等于 `total_ms`。`spawn_and_register_ms` 再拆
`pre_spawn_ms` + `spawn_actor_call_ms`；`pre_spawn.other_ms` = `pre_spawn_ms`
− 已列子项之和，用来兜住未命名工作。`spawn.other_ms` = `spawn_actor_call_ms`
− `agent_build_ms` − 已列 spawn 子项。`agent_build.other_ms` = `agent_build_ms`
− `skills_ms` − 已列 agent build 子项。`skills_walk_ms` / `skills_parse_ms`
是 `skills_ms` 的内部拆分，来源计数放进 `skills_sources`（键为
`user/repo/bundled/plugin/server`）。

### 5.2 `session.mcp_init` / `session.mcp_ready`（shell 侧，info）

每次 MCP init pass 发一条 `session.mcp_init`，结束时发 `session.mcp_ready`：

```json
{
  "session_id": "…",
  "total_ms": 2100,
  "server_count": 5,
  "ok": 4,
  "failed": 1,
  "grace_used_ms": 2000,
  "servers": [
    {"name": "codegraph", "transport": "stdio", "connect_ms": 900,
     "initialize_ms": 700, "list_tools_ms": 120, "tool_count": 1, "outcome": "ok"},
    {"name": "context7", "transport": "http", "connect_ms": 1000,
     "initialize_ms": 0, "list_tools_ms": 0, "tool_count": 0, "outcome": "timeout"}
  ]
}
```

约束：`servers` 最多 16 条；超过时只保留慢/失败的 16 条并加
`truncated: true`。`name` 是用户可见的 server 名，可接受；不写 URL/命令。

### 5.3 `turn.first_prompt`（shell 或 pager，info）

每个会话只发首条 prompt 一次：

```json
{
  "session_id": "…",
  "queued": true,
  "queue_ms": 4200,
  "session_ready_ms": 3100,
  "mcp_wait_ms": 2000,
  "prep_ms": 35,
  "to_request_ms": 6235,
  "ttft_ms": 850,
  "from_launch_ms": 7235,
  "mcp_ready": false
}
```

`queue_ms` = enqueue → prompt 开始处理；`session_ready_ms` = enqueue →
SessionCreated（若未排队则为 0）；`mcp_wait_ms` 直接取
`turn.tool_prep` 的字段；`to_request_ms` = enqueue → 请求发起，等于
`queue_ms + mcp_wait_ms + prep_ms`；`from_launch_ms` = enqueue 距进程起点
+ `to_request_ms`，依赖 t0。

### 5.4 `exit.teardown`（pager 侧，info）

```json
{
  "session_id": "…",
  "busy": true,
  "reason": "quit",
  "flush_ms": 80,
  "drain_ms": 0,
  "runtime_ms": 2000,
  "join_ms": 2148,
  "terminal_restore_ms": 86,
  "notice_shown": true,
  "outcome": "joined"
}
```

`runtime_ms` 为 `shutdown_worker_runtime` 的实测；`join_ms` 为
`session_end.worker_join` 的 `elapsed_ms`，包含 `flush_ms` + `drain_ms` +
`runtime_ms`；`terminal_restore_ms` 在 join 之前。`exit.teardown.done.total_ms`
才是 quit → 进程退出的总账。保留现有 `session_end.worker_join` 作为兼容总账。

### 5.5 命名与兼容

- 现有 `startup phase`、`startup timing`、`startup complete`、
  `startup interactive`、`session.create.done`、`session_end.worker_join`
  全部保留名称与已有字段，只增不改。
- 新事件统一 `session.startup.*` / `turn.first_prompt` / `exit.teardown`；
  字段用 `_ms` 结尾，布尔用 `ok/failed/busy/queued`。
- 探索性计时继续走 `instrumentation_timer!`（`GROK_INSTRUMENTATION=log`）；
  unified 只落里程碑与有界字段，避免每文件/每 server 展开。

## 6. 施工切片

### P0：测量面补齐（约 1–2 天）

1. **会话创建拆解。** 在 `session_setup.rs` 的 `SpawnSessionActor` 前包住
   `pre_spawn`，在 `agent_rebuild.rs` / `builder.rs` 增加 `skills_walk` /
   `skills_parse` 与来源计数，在 `session_setup` 末尾汇总并发出
   `session.create.breakdown`。
2. **pre-spawn 子计时。** 至少覆盖 `lsp_build`、`trust`、`plugin_registry`、
   `envrc`、`agent_define`、`hunk`、`workspace_ops`；`other_ms` 兜底。
   `build_lsp_backend` 的 `await` 在 `agent_ops.rs:4676`，重点确认它是否
   吃掉了那 1757ms。
3. **MCP per-server。** 在 `mcp_init.rs` 的 per-server 结果处记录
   `connect/initialize/list_tools`；在 `mcp_snapshot.rs` 的 grace 等待处记录
   `grace_used_ms`，结束时发 `session.mcp_ready`。
4. **首 prompt。** 在 `prompt.enqueue` 记录时间戳，在 `SessionCreated` 与
   `turn.tool_prep` 结束时补齐 `turn.first_prompt`。
5. **退出拆分。** 在 `AgentShutdownGuard` / `shutdown_worker_runtime` /
   `restore_terminal` 记录四段耗时，发 `exit.teardown`；保留现有
   `session_end.worker_join`。
6. **启动事件不再被首帧截断。** 新增 `startup::record_session_timing`
   （不受 `is_active` 限制，带 `first_session`），或允许 `instrumentation_timer!`
   在首帧后继续镜像白名单名称；二选一在实现时定。

**验收：** 裸启动一次 + 恢复一次 + 会话刚建好立即 `/quit` 一次；unified 中
同时出现 `startup interactive`、`session.create.breakdown`、
`session.mcp_ready`、`turn.first_prompt`、`exit.teardown`；字段可加总到
对应总时长（误差 < 10ms）；开销 < 5ms；现有 `startup`/`session_end` 消费者
测试不红。

### P1：resume 与外围（约 1 天）

7. `session/load` 拆 `read_updates` / `replay` / `context_build` /
   `compaction_check` / `spawn`，并记录 history 条目数、字节数。
8. plugin / hooks 发现记录来源计数与单次耗时；同一进程内重复 refresh
   在字段里标 `cached`。
9. 模型目录拉取补 `skipped_reason` / `attempt` / `cache_state`，重试只在
   状态变化时发一条。

### P2：回归与报告

10. `scripts-local/startup_report.py`：从 `unified.jsonl` 聚合
    `session.create.breakdown` / `turn.first_prompt` / `exit.teardown` 的
    p50/p90，按版本、冷/热、是否 resume 分组；不新增服务。
11. 回归门槛（建议值，先观察一轮再定）：
    - `startup interactive` p90 < 2s
    - `session.create.done` p50 < 3s（当前 4.7s，先以 4s 为过渡线）
    - `turn.first_prompt.mcp_wait_ms` p50 < 500ms
    - `exit.teardown.join_ms` p90 < 500ms
    超过只告警，不阻断发版。

## 7. 验证流程

```bash
# 1) 完整计时（所有 instrumentation_timer! 都落盘，不受首帧截断）
GROK_INSTRUMENTATION=log grok2

# 2) 只看启动首帧（benchmark / 快速回归）
GROK_SPAWN_TIMESTAMP_MS=$(date +%s%3N) GROK_EXIT_AFTER_FIRST_RENDER=1 grok2

# 3) span 画像（优雅退出才落盘）
GROK_SPAN_PROFILE_OUT=D:/cargo-tmp/spanprof grok2
```

现场跑四组，每组至少 3 次：

1. 裸启动，等首帧后立即输入一条短 prompt，等回复后 `/quit`。
2. 裸启动，会话就绪前就输入 prompt（复现首轮排队）。
3. `grok2 -c` 恢复一个大会话。
4. 会话刚建好立即 `/quit`（复现 2s 尾巴）。

对比时同时看 `~/.grok/logs/instrumentation.log`（全量计时）与
`unified.jsonl`（里程碑）。冷/热分开统计：冷启动的 skills 遍历与 Windows
Defender 扫描会把 first-run 拉高，混在一起会误判。

`ptyctl` 的已知坑沿用 README：`--stable-ms` 等不到稳定屏、杀 keeper 会丢画像；
就绪判定用 `screen` 人工看或 `wait --text`。

## 8. 风险与约束

- **Schema 兼容：** `StartupCompleted` / `StartupInteractive` 是产品事件，
  新字段一律 `Option` + `skip_serializing_if`；不改既有字段语义。
- **日志体积：** unified 只在会话创建/加载、MCP init、首 prompt、退出各发
  一条；per-skill/per-server 明细只在 `instrumentation.log` 或截断后的
  `servers` 数组里。
- **PII：** 不写完整 cwd、URL、命令；skill 只写数量与来源，必要时写名字。
- **计时嵌套：** `timer_parents` 按 task/thread 维护栈，跨 `spawn_blocking`
  不继承；跨线程的段用显式 `Instant` 差值，别依赖 span 父子。
- **平台差异：** Windows 冷启动受文件缓存/AV 影响，Linux CI 的绝对值不可
  直接对比；报告里标平台与冷/热。
- **开销：** 每个 timer 是 `Instant` + 一个 span，微秒级；不要在每个
  skill 文件上发 unified 事件。

## 9. 待定问题

1. `startup complete` 的终点保持“首帧/首个可交互”，还是扩到“首个会话就绪”？
   建议保持现状，另立 `session.startup.*`。
2. `session.create.breakdown` 每个会话都发，还是只发首会话 + 超过阈值的会话？
   建议首会话必发，后续只发 `total_ms > 1000` 的。
3. MCP per-server 明细进 unified 还是只进 instrumentation？
   建议 unified 只放 `ok/failed/total` 与最慢 server，明细进 instrumentation。
4. 退出拆分的 `busy` 判定用 `AgentActivity::is_busy` 还是 turn 状态？
   建议用 `is_busy`，与 leader 自动更新口径一致。
5. 是否把 `session.create.done` 的计时起点前移到 MCP 配置发现之前？
   现在 `discover_mcp_servers` 在计时外，用户感知的总时长会略大于字段值。

## 10. 代码锚点

- 启动阶段与事件名：`crates/codegen/xai-grok-telemetry/src/spans/startup.rs:25-33`
- 阶段预算（`SessionCreate` 3s）：`crates/codegen/xai-grok-telemetry/src/spans/startup.rs:52-60`
- 首帧/可交互：`crates/codegen/xai-grok-telemetry/src/spans/startup.rs:480-530`
- 首帧后停止镜像：`crates/codegen/xai-grok-telemetry/src/spans/startup.rs:566`
- 计时器落盘与 unified 镜像：`crates/codegen/xai-grok-telemetry/src/spans/instrumentation.rs:620-660`
- 会话阶段入口：`crates/codegen/xai-grok-shell/src/agent/mvp_agent/session_setup.rs:653`（spawn）、`:825`（git discovery）
- 会话创建总计时：`crates/codegen/xai-grok-shell/src/agent/mvp_agent/agent_ops.rs:4269`
- pre-spawn 唯一 `await`（LSP）：`crates/codegen/xai-grok-shell/src/agent/mvp_agent/agent_ops.rs:4676`
- spawn / agent build 计时：`crates/codegen/xai-grok-shell/src/agent/mvp_agent/agent_ops.rs:4802`、`:4905`
- agent build 入口：`crates/codegen/xai-grok-agent/src/builder.rs:696`
- skills discovery：`crates/codegen/xai-grok-agent/src/builder.rs:709`、`crates/codegen/xai-grok-agent/src/prompt/skills.rs:82`
- skills 解析：`crates/codegen/xai-grok-tools/src/implementations/skills/discovery.rs:670`
- MCP grace：`crates/codegen/xai-grok-shell/src/session/acp_session_impl/mcp_snapshot.rs:15`
- 首轮等待：`crates/codegen/xai-grok-shell/src/session/acp_session_impl/sampler_turn.rs:399-412`、`prompt_build.rs:629`
- 退出 join：`crates/codegen/xai-grok-pager/src/acp/spawn.rs:100-215`
- worker runtime grace：`crates/codegen/xai-grok-pager/src/acp/spawn.rs:36`
- 会话 flush 上限：`crates/codegen/xai-grok-shell/src/agent/activity.rs:36`
- 退出 watchdog：`crates/codegen/xai-grok-pager/src/app/exit_timeout.rs`
- 终端恢复：`crates/codegen/xai-grok-pager/src/app/terminal_restore.rs:27`
- 进程组回收：`crates/codegen/xai-tty-utils/src/lib.rs:81`、`process_scope.rs:230`
