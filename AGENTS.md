# 项目工作规则（LOCAL fork）

> **本地构建/检查禁令（2026-11-28，用户拍板）：** 本机禁止运行
> `cargo build` / `cargo check` / `cargo test`、`scripts-local/ctest.sh` 或
> 等价编译命令，除非用户当次明确授权。构建会占用用户的存储与时间；
> 验证一律走 CI 或只读代码审查（`rg`/读文件/脚本分析）。

## 🔄 Handoff 摘要

### harness-cost-p0-p1 — in-progress（长程观察）
- **当前状态：** P0 v1.0.83、P1 v1.0.84、replay diet 开关 v1.0.85、冷窗口 microcompaction v1.0.86 已发版并部署（`grok 1.0.86 (60b98dc19d36)`）；运行中的旧 TUI 仍为 1.0.85，重启后生效。
- **关键证据：** `docs-local/token-economy-port/p1-observation-2026-11-26.md`（首阶段：长回合输入成本 -9~-27%，命中中位 99%）；`replay-diet-2026-11-21.md`；release 46120857404 双绿，v1.0.86 Windows zip sha256 `18589be8…` 已核对；PATCHES 七十九~八十二期。
- **验收标准：** 长程会话 `shell.relay_compact.relay_summary_verbatim_missing`、`shell.idle_wait_compact.fired/skipped` 与 `shell.idle_microcompact.fired/skipped` 可观测；P1 按 §3.5 验收；replay diet A/B 用 `[compaction] replay_diet` 开关。
- **详情指针：** [`.handoff/harness-cost-p0-p1.md`](.handoff/harness-cost-p0-p1.md)

### 未验证事项
- [x] release 46120857404 双绿、v1.0.86 Windows zip sha256 `18589be8…` 核对
- [x] 本机部署 v1.0.86 并保留 1.0.85 回滚（重启 TUI 后生效）
- [x] 账本 composition 字段落盘；`/stats` 行待抽样
- [ ] replay diet A/B：本机 config 已设 `replay_diet = false`（A 组），B 组改 `true` 后用新会话对照
- [ ] relay 已见 2 次（24 条缺 1）；idle-wait 0 fired 根因=outstanding_tasks 门控（15 次 skipped 全为此因）；冷窗口 diet 已部署
- [ ] 冷窗口 microcompact 活体验收（v1.0.86 已部署，重启后生效）：普通空闲 >330s 应出现 `shell.idle_microcompact.fired`；有排队用户输入时应先出现 `shell.idle_maintenance.deferred` 且不压缩；下一条 prompt 首个请求不应新增 cache 未命中

## 代码理解（2026-10-03）

优先 cbm（`codebase-memory-mcp cli`，项目 `D-CODE-ai-grok-build-proxy`）：进入陌生子系统先
`get_architecture` 或 `query_graph` 拿结构地图，再读代码；grep/Read 补充。
codegraph 全量索引在本机（HDD）试跑过慢、已清理；需要时按 crate 临时建
scoped 索引。用法与陷阱：`~/.zcode/skills/cbm/SKILL.md`、
`docs-local/code-index-setup.md`。

## 分支与推送（MANDATORY）

- 直接在 `main` 施工，不开新分支：功能/修复/重构都提交 main，CI 绿后按 `vX.Y.Z` 打 tag。
- 一次推送一批：攒改动 → 一次 push → 红就修完再推；禁止一个提交一次 build。
  docs-only 改动被 `build.yml` 的 `paths-ignore` 跳过，不触发 CI。
- main 红了当班修：拿失败日志先分族（下节），修完连同原因再推。
- 历史 `feat/local-*` 分支只作考古；不再新增，存量 PR 按原样验收。

## 调试时间成本（MANDATORY）

一切调试先算时间账，优先长期时效：

1. **先分族再动手**。失败常成族（几十个同根因），按报错分族、一族一策，禁止逐个盲修。
   先读码、grep、看既有 CI 日志；确需运行时证据时先向用户申请，不要自行开构建。
2. **一次批量收集信息**。禁止"改一行→全量→再改一行"；一轮改完再交 CI。
   构建/测试输出不接 `tail`/`head` 管道，完整落盘后再检索。
3. **临时目录不走 C 盘**。需要落盘时用 `D:\tmp` / `D:\cargo-tmp`，用完即删。
4. **上游同步会覆盖同步文件**。对同步文件打的 LOCAL 补丁（cfg 门控等）必须在
   `docs-local/PATCHES.md` 有据可查，便于同步后重放。
5. **A/B 先算成本**。优先零编译证据（读码、既有 CI 日志、grep）；确需运行时对照时
   用已构建产物或并入唯一一轮 CI，禁止为对照单独多烧一轮构建。
6. **已验证的主题立刻提交**。失败重试与提交推送解耦，绿一块走一块。
7. **清理**。用户要求或收口时，删除本轮 `target/debug` 对应产物与
   `D:\cargo-tmp`，保留 `target/release` 与用户备份。历史 Windows 测试门禁/
   `win-skip.txt` 规则见 `docs-local/WIN-TEST-GATE.md`，仅在用户授权构建时使用。

## 记录与归档

本文件只放仍生效的规则与一行式活跃待办；过程记录、发版流水、已完成事项进
`docs-local/` 专题或 `docs-local/archive/`。

- 新待办一行：`- 主题 — 状态；指针`；完成后当轮删除，详情归档。
- 一次性记录收口当天移入 archive，索引只追加。
- 引用文件/链接前先核对存在。
- 预算：本文件 ≤10 KB、待办 ≤50 条；超了做治理并留快照。
- 治理前快照：`docs-local/archive/agents-md-snapshot-2026-11-20.md`（本次）；
  历史：`agents-md-snapshot-2026-11-16.md`、`agents-md-todo-handoff-snapshot-2026-11-13.md`。

## 活跃待办（2026-11-20）

### 施工中
- 订阅到期体验 + 启动/退出 span — v1.0.80 已部署，待活体验收；PATCHES 76、`startup-perf/span-plan.md` §10。
- 启动 500ms 目标 — v1.0.82：首帧 193/203ms、session 304ms；余 `tool_registry` 130–151ms、`skills` 42–63ms；PATCHES 78、`startup-perf/README.md` §5。
- 客户端循环检测后续 — MiMo 跨 turn `ignored` 裁剪、请求级修复持久化等 dump；B/C 组未开工；PATCHES 75、`agent-cli-tracking/TODO.md` ①。
- token 经济二轮 — P1 首阶段台账（11-22–26）已出：长回合输入成本 -9~-27%、命中中位 99%；idle-wait 0 fired 已定位为 outstanding_tasks 门控，冷窗口 diet 已部署待验收；待 12-10 或 10 个长会话续观；`token-economy-port/p1-observation-2026-11-26.md`。
- 冷窗口 microcompaction — v1.0.86 已发版并部署（`923f09e3…`），重启 TUI 后生效；验收 `shell.idle_microcompact.fired/skipped`；PATCHES 八十二期。
- replay diet A/B — v1.0.85 配置开关已发；A=`[compaction] replay_diet=false`（本机已设），B=`true`；两组用新会话、看账本 token 加权命中与 `$/call`；PATCHES 八十一期。
- IP 端点自签证书放行 — `feat/local-insecure-tls-ip-hosts` 待合；`[network] insecure_tls_hosts`。

### 待活体验证
- 循环检测（`/loop-detect abort|record`、`/stats` 汇总行、`~/.grok/cache/loop-detect.jsonl`）。
- 视觉桥、空闲自动压缩、cache P1/P2 触发后首轮命中、`/usage` 额度估算口径。
- `/stats` 的 `g` 分组与断崖汇总行；`/compress-fast` 在 append 模型上的失效标记。
- LSP 四项（诊断/懒启动/rename/面板）、`[api_dump]` 落盘、09-29 同步抽查、`/usage` 套餐块。
- `/compact <instructions>`（v1.0.75 已部署）。

### 未开工 / 待拍板
- workflow-pi-port P2 T2；kimicode T2b；step-code P3/P4；dsh-port P1。
- zcode 3.17.2 P1；opencode diff；webdav sync；`/dump`；startup-perf 缓解；21 套主题；harness-model-fit；agent-cli-refresh P3。
- zcode v4.0.0 增补 — P1 推理修复持久化 + tool `is_error`，P2 压缩文件回补 / 停滞提醒 / MCP 状态待拍板；见 `docs-local/zcode-v4-port/README.md`。
- qwen 1.0 增补 — idle cost（P0 余项）、microcompaction 规格（并入 P1）、调度闸门（P2）；`agent-cli-tracking/notes/qwen-code-1.0-2026-11.md`。

### 长期挂起
- Ctrl+Enter 幽灵气球（P2）；quota 他机对账；limit-probe T3；dump 自动清理；model-picker T2。

## 当前版本

- 本机 `grok 1.0.86 (60b98dc19d36)`，三处部署同 hash `923f09e3…`；回滚 `grok2.exe.1.0.85-e8d7325a19ae.bak`（运行中旧进程 `grok2.exe.running-1.0.85`，重启 TUI 后生效）。
- 发版详情与 sha256：`docs-local/PATCHES.md` 七十三~八十二期。最近：v1.0.86 冷窗口 microcompaction、v1.0.85 replay diet 配置开关（`[compaction] replay_diet`）、v1.0.84 P1 长程压缩、v1.0.83 harness 成本 P0 观测。

## 文档地图（docs-local）

- 经济性/缓存：`token-economy-audit-2026-11/`、`token-economy-port/`（路线 + 下钻 + TTL）、`cache-strategy-port/`、`status-line-perf-wiki.md`、`limit-inference.md`、`usage/`、`usage-quota-estimate-todo.md`。
- harness/模型：`harness-model-fit/`、`agent-cli-refresh-2026-11/`、`agent-cli-tracking/`、`model-limits-config.md`、`dsh-port/`。
- 移植/特性：`workflow-pi-port/`、`kimicode-port/`、`step-code-port/`、`opencode-port/`、`lsp-port/`、`vision-bridge/`、`subagent-model-group-todo.md`、`port-roadmap.md`、`fork-feature-inventory.md`。
- 上游/兼容：`sync-upstream-2026-09-29.md`、`sync-upstream-2026-09-23.md`、`upstream-responses-event-compat.md`、`PATCHES.md`。
- 运维/取证：`startup-perf/`、`code-index-setup.md`、`stats-modal-todo.md`、`webdav-sync-todo.md`、`quota-pct-jump-audit.md`、`compaction-failure-rca.md`、`inspect-config-keys-todo.md`、`issues/`、`archive/`。
