# grok2 启动耗时专题 wiki

> **状态：排查完成（2026-10-03，客户端 1.0.40），缓解方案待拍板。**
> 只收深挖节点与可复现流程，不复述可从代码推出的内容。姊妹篇：
> [`status-line-perf-wiki.md`](../status-line-perf-wiki.md)（请求级 TTFT/TPS）。
>
> **2026-11-12 复测（v1.0.73）：** 远端目录阻塞已不再是本机主因
> （`startup.fetch_models_blocking` 0ms）；当前主卡点是新会话创建
> （中位 4675ms）与首轮 MCP grace（2s），空闲退出 join 76–203ms。
> span/事件细化方案见 [`span-plan.md`](span-plan.md)。
>
> **2026-11-17（v1.0.80 候选）：** 测量面已补齐（`session.create.breakdown` /
> `session.load.breakdown` / `session.mcp_ready` / `turn.first_prompt` /
> `exit.teardown.*`，见 span-plan §10）。同批启动缓解：模型目录缓存 TTL 300s → 24h、
> 阻塞预取等待上限 5s → 1s（后台线程继续拉取并写缓存）、用户指南文档仅在内容变化时重写。
> 下一步用新事件定位会话创建与首轮 MCP 的具体慢段。

## 结论（TL;DR）

冷启动（距上次启动 >5 分钟）的阻塞耗时几乎全部来自**模型目录远端拉取**
`startup.fetch_models_blocking`（两个调用点合计 ≈2.7–6.8s，网络波动主导）；
模型目录磁盘缓存 TTL 只有 300 秒，所以"隔一阵子再开"必中。热缓存（TTL 内）
阻塞路径塌缩到 ≈60–100ms。MCP init（~1.6s）是异步任务、marketplace 同步与
sampler 预热是后台 prewarm，都不挡首帧。

| 启动路径 span（self time） | 冷缓存 | 热缓存 |
|---|---|---|
| `startup.managed_policy → fetch_models_blocking` | 2660–5006ms | 无此 span |
| `startup.worker_spawn → fetch_models_blocking` | 0–1838ms | 无此 span |
| `startup.app_init` | 35–39ms | 39ms |
| `startup.config_load` | ~3ms | ~ms 级 |
| `session.mcp_startup → mcp_init_task`（异步） | 1638–1648ms | 1604ms |

## 排查流程（可复现）

### 1. 采画像：`GROK_SPAN_PROFILE_OUT` + ptyctl 驱动真实 TUI

span 画像层已内建：`xai-grok-telemetry/src/spans/span_profile.rs`，环境变量
`GROK_SPAN_PROFILE_OUT` 指向**目录**（每进程写 `tui-<pid>-<ts>.folded`；TUI 的
label 是 `"tui"`，见 `xai-grok-pager/src/tracing.rs`；headless/agent 是
`"agent"`/`"cli"`）。画像只在**优雅退出**时落盘，杀进程 = 白跑。

```bash
# 1) 后台 keeper 养着 TUI（必须独立后台任务：前台链被杀会连带 ptyctl+TUI
#    整棵 job 树 → 非优雅退出 → 无画像；用 -e 把 env 注进 TUI 进程）
./target/debug/ptyctl.exe run -n <name> -p <port> -c <dir> \
  -e GROK_SPAN_PROFILE_OUT=D:/cargo-tmp/spanprof -- D:/tool-cli/grok2/grok2.exe
# 2) 前台探测（与 keeper 分离的独立命令）：
./target/debug/ptyctl.exe screen -p <port> | tail -5   # 确认 composer 就绪
./target/debug/ptyctl.exe send -p <port> "/quit" -e    # 优雅退出 → 画像落盘
# 3) 解读：<dir>/tui-*.folded，每行 `<span 树路径> <self-time 微秒>`（按路径折叠累计）
sort -k2 -rn tui-*.folded | awk '{printf "%9.1fms %s\n", $2/1000, $1}'
```

**已踩过的坑：**

- `ptyctl wait --stable-ms` 永远等不到：状态行时钟每秒刷新，屏幕永不稳定 →
  卡满超时。就绪判定用 `screen` 人工看，或 `wait --text "<会话页特征串>"`
  （如审批模式尾行）；跨工具调用的墙钟计时无效（LLM 往返延迟 > 启动耗时）。
- 杀 keeper 后台任务 = 杀整棵 job 树（TUI 一起死），画像丢失。流程必须是
  "keeper 后台养 + 前台独立命令探测/退出"。
- folded 值是**全进程按 span 路径累计**的 self-time：`Connection`、`mcp.serve`、
  `run_sync_loop`、`session` 这类常驻 span 的值 > 进程墙钟属正常（多实例×多线程
  累计），解读启动问题只看 `startup.` 前缀行。

### 2. 关键路径定位

只看 `startup.` 前缀的行；本次主因 `startup.managed_policy →
startup.fetch_models_blocking` 与 `startup.worker_spawn → …` 两行。

## 机制锚点（改哪里看哪里）

- `crates/codegen/xai-grok-shell/src/agent/remote_config/fetch.rs:33`
  `prefetch_models_blocking` → `fetch_models_uncommitted`：先
  `ModelsCacheManager::load_fresh(&scope)`（按 auth-method/origin 分域），
  新鲜缓存直接返回；过期/缺失才走阻塞 `source.fetch(auth)`。
  span 计时在 fetch.rs:117。
- `crates/codegen/xai-grok-cloud-config/src/remote_settings/cache.rs`
  `CACHE_TTL = 24h`（2026-11-17 前为 300s）——冷启动频率的决定参数。
- 总开关：`features.remote_fetch = false`（`util/config/resolve/features.rs:58`
  `resolve_remote_fetch_enabled`；键名常量 `REMOTE_FETCH_CONFIG_PATH`）。注意
  **managed 层优先于用户层**（防部署侧被用户复活），本机无 managed 配置时用户
  值生效；关闭后 `fetch_models_uncommitted` 直接 `Unavailable` 不发网络。
- 调用点：`agent/app.rs:384`（agent 应用初始化）、
  `agent/remote_config/endpoint.rs:46`；会话侧入口
  `xai-grok-pager/src/acp/spawn.rs:293` `ensure_managed_policy_present`
  （在 session spawn 关键路径上，先于 composer 就绪）。

## 缓解选项与状态（2026-11-17 更新）

1. **已落地（v1.0.80 候选）**：`CACHE_TTL` 放长到 24h；启动阻塞等待上限
   `MODELS_BOOT_WAIT = 1s`（`xai-grok-shell/src/agent/remote_config/prefetch.rs`），
   超时后先用手头目录起 TUI，后台线程继续拉取并提交缓存；用户指南文档内容未变时
   跳过重写（Windows AV 不再每次扫描）。
2. **零代码**：用户 config.toml 加 `features.remote_fetch = false`。BYOK 用户
   （模型全在 `[model.*]`，不需要 xAI 远端目录）安全；订阅用户会失去远端模型
   列表。2026-11-17 起该开关同时硬跳过 `x.ai/billing`（返回空 config，pager 清
   缓存并停轮询）与 `retry_subscription_check`（不再请求
   `/user?include=subscription`，巡检只用缓存 auth meta），不再只是抑制。
3. **已落地（2026-11-17，v1.0.81）**：shell 侧 LSP 改为懒构建。spawn 只装
   `LazyLspBackend` 句柄，server 探测/配置加载/manager 构建延后到首次文件变更或
   LSP 调用；`pre_spawn.lsp_build_ms` 从 ~100ms 降到接近 0。首帧路径新增
   `startup.first_frame`（render/ack）与 materialize/terminal/writer 计时，用于
   定位剩余约 100ms。
4. **已落地（2026-11-18，v1.0.82 候选）**：`WorkspaceHandle::build` 的诊断 LSP
   同样改为懒构建。此前 workspace 层在 session 创建期扫描 PATH 并拉起
   clangd/gopls/rust-analyzer（约 91ms，计入 `pre_spawn.other_ms`）；现在只在首次
   文件事件/诊断/`lsp` 调用时由工厂构建，`ensure_started_background` 仍异步。
   预期 `pre_spawn.other_ms` 111ms → ~20ms，session create 380–390ms → ~290–300ms。
   懒构建结果通过 `session.lsp.lazy_build` 统一事件可查（不再受
   `session_timing::take()` 影响）。
4. **归因澄清**：`managed_policy` 是团队/部署策略检查，个人账号是 no-op；历史上
   看到的 0.4–2.0s 实际是模型目录阻塞预取落在该 phase 窗口内，不是策略本身。
   消费方对 `ensure_managed_policy_present` 无需再做迁移。

## 复测清单

- [ ] 冷缓存：改 TTL 或清 `grok_home` 下模型缓存文件后，按 §流程采画像，确认
  `fetch_models_blocking` 出现且量级一致。
- [ ] 热缓存：TTL 内二次启动，确认两个 fetch span 消失、阻塞路径 ≤100ms。
- [ ] `features.remote_fetch = false`：确认 fetch span 消失、模型目录功能
  （订阅用户）受影响的边界如实记录。
