# 配置缺口盘点：env-only 与硬编码（2026-11-27）

范围：当前 checkout 的 `crates/codegen`、`crates/common`。只列**没有配置文件入口**、
且用户日常/调优有价值的项；排除 test/CI/debug 开关和已有 config 路径的项。
`GROK_REPLAY_DIET*` 的总开关已在本期补 `[compaction] replay_diet`；阈值仍是 env-only。

优先级按“影响面 × 不可替代性”排序。

| 优先级 | 设置 | Env / 常量 | 默认 | 证据 | 价值 | 建议键 |
|---|---|---|---|---|---|---|
| P0 | 采样 HTTP 传输 | `GROK_CONNECT_TIMEOUT_SECS`、`GROK_POOL_IDLE_TIMEOUT_SECS`、`GROK_POOL_MAX_IDLE`；H2 keepalive 15s/5s 写死 | 10s / 90s / 2 / 15s / 5s | `crates/codegen/xai-grok-sampler/src/shared_http.rs:89,151,155,163` | 所有模型流量共用；连接超时和池空闲直接影响代理/网关断线恢复 | `[network] connect_timeout_secs / pool_idle_timeout_secs / pool_max_idle_per_host / http2_keepalive_secs` |
| P0 | trace 上传队列上限 | 8 GiB、2h、8 并发写死；仅 auth probe 300s 有 env | 8 GiB / 2h / 8 | `crates/codegen/xai-file-utils/src/queue.rs:130,149,179,187,534` | 控制 turn 结束时的磁盘/上传压力；队列失控会占盘 | `[harness] upload_queue_max_bytes / max_age_secs / max_concurrent` |
| P1 | Hook 事件默认超时 | Tool 5s、Prompt 30s、Stop/PostTool 600s、SessionEnd 1500ms（上限 60s）；仅 `GROK_SESSION_END_HOOKS_TIMEOUT_MS` | 见左 | `crates/codegen/xai-grok-hooks/src/config.rs:121,126,131,137` | SessionEnd 的 1.5s 会静默截断长退出 hook；per-handler timeout 可配，事件默认值不可配 | `[hooks] session_end_budget_ms`（及 per-gate 默认） |
| P1 | Task 阻塞等待上限 | `GROK_MAX_WAIT_BLOCK_MS` | 3,600,000 ms（1h） | `crates/common/xai-tool-types/src/task.rs:772` | 会写进工具描述给模型；代理链路更短时需要同步收紧 | `[toolset] max_wait_block_ms` |
| P1 | tokio worker 线程 | `GROK_WORKER_THREADS` | 核数，上限 8 | `crates/codegen/xai-grok-pager-bin/src/main.rs:1792,1795` | 多核 Windows 的启动/线程预算调优 | `[cli] worker_threads` |
| P1 | fsnotify 策略/预算 | `GROK_FSNOTIFY_PER_DIR`、`GROK_FSNOTIFY_MAX_WATCHES` | Linux per-dir，否则 fanout；49,152 watches | `crates/codegen/xai-fsnotify/src/handle.rs:55,60,67` | Windows 固定走 fanout；大仓库 watch churn 的主要杠杆 | `[fsnotify] per_dir / max_watches` |
| P2 | 订阅状态轮询 | `GROK_SUBSCRIPTION_WATCH_INTERVAL_SECS`；LOCAL restricted 300s 写死 | 60s / 300s | `crates/codegen/xai-grok-pager/src/app/subscription.rs:18,25,65` | 额度/订阅面板刷新节奏 | `[usage] subscription_watch_interval_secs` |
| P2 | API dump 目录 | `GROK_API_DUMP_DIR` | `$GROK_HOME/dump/<date>/` | `crates/codegen/xai-grok-sampler/src/api_dump.rs:61` | `api_dump.*` 上限可配，目录不可配 | `[api_dump] dir` |
| P2 | AI 建议模型/开关 | `GROK_SUGGESTIONS_AI_MODEL`、`GROK_SUGGESTIONS_AI` | 无 | `crates/codegen/xai-grok-pager/src/views/suggestion_controller/mod.rs:305-312` | prompt suggestion 的模型 pin 只在 env | `[prompt_suggestions] ai_enabled / ai_model` |
| P2 | 启动自动打开 Dashboard | `GROK_OPEN_DASHBOARD_AT_STARTUP` | off | `crates/codegen/xai-grok-pager/src/app/event_loop.rs:1816` | `dashboard.enabled` 已有，自动打开没有 | `[dashboard] open_at_startup` |
| P2 | 隐私横幅重显 | `GROK_PRIVACY_BANNER_RESHOW_DAYS` | 远端设置，未设 | `crates/codegen/xai-grok-pager/src/app/event_loop.rs:1185` | 本地配置文件无法固定 | `[privacy] banner_reshow_days` |
| P2 | UI 生命周期超时 | `GROK_EXIT_TIMEOUT_SECS`、`GROK_PROMPT_ACK_TIMEOUT_SECS`、connect UI timeout | 各模块默认 | `exit_timeout.rs:9`、`prompt_ack.rs:14`、`connect_timeout.rs:13` | 启动/退出卡顿的可观测调优 | `[ui] exit_timeout_secs / prompt_ack_timeout_secs` |
| P2 | 上传熔断器 | `CB_WINDOW_SECS`、`CB_MIN_SAMPLES`、`CB_ERROR_RATE_THRESHOLD`、`CB_OPEN_DURATION_SECS`、`CB_FAILURE_CODES`、`CB_ENABLED` | 60s / 10 / 0.5 / 10s / [401] / on | `crates/common/xai-circuit-breaker/src/config.rs:69,84-95` | 连续 auth 失败时是否暂停上传；命名空间不透明 | `[harness] upload_circuit_breaker.*` |
| P2 | computer-hub 全局并发 | `XAI_TOOL_SERVER_GLOBAL_MAX_INFLIGHT`、`COMPUTER_HUB_URL` | 1024；SDK 默认 URL | `crates/common/xai-computer-hub-sdk/src/admission.rs:32,75`；`discovery.rs:28` | 所有 computer-hub 连接的进程级上限 | `[tools] global_max_inflight` |
| P2 | SQLite journal mode | `GROK_SQLITE_JOURNAL_MODE` | 库默认 | `crates/codegen/xai-sqlite-journal/src/lib.rs:44` | Windows 上 session/memory DB 的性能杠杆 | `[cli] sqlite_journal_mode` |

## 最值得先做的 5 项

1. **采样 HTTP 传输**（`shared_http.rs`）：本 fork 的核心是代理，连接超时/池/H2 keepalive 却全在 env/常量。
2. **上传队列上限**（`queue.rs`）：8 GiB / 2h / 8 并发是纯常量，容易造成磁盘和重试压力。
3. **Hook 事件默认超时**（`xai-grok-hooks/config.rs`）：SessionEnd 1.5s 的静默截断最容易踩。
4. **Task 阻塞等待上限**（`task.rs`）：数值已写进工具描述，配置化能保持客户端/服务端一致。
5. **fsnotify 策略/预算**（`handle.rs`）：Windows 只有 fanout 分支，大仓库 watch 稳定性收益直接。

## 备注

- “已有 config 路径”按本 checkout 的 `config-reference` 与 `xai-grok-config-types/src/registry.rs` 交叉核对；落地前仍需按各项的 precedence 再确认一次。
- 本表只做盘点，未改动上述任何设置。
