# 项目工作规则（LOCAL fork）

## 🔄 Handoff 摘要

### harness-cost-p0-p1 — in-progress（长程观察）
- **当前状态：** P0 随 v1.0.83、P1（replay diet + relay + idle-wait）随 v1.0.84 已发版并部署（`grok 1.0.84 (b5cb7026d927)`）。
- **关键证据：** `docs-local/token-economy-port/replay-diet-2026-11-21.md`；release 44900168801 双绿，Windows zip sha256 `060e6691…` 已核对；PATCHES 七十九、八十期。
- **验收标准：** 长程会话 `shell.relay_compact.relay_summary_verbatim_missing` 与 `shell.idle_wait_compact.fired/skipped` 可观测；P1 按 §3.5 验收。
- **详情指针：** [`.handoff/harness-cost-p0-p1.md`](.handoff/harness-cost-p0-p1.md)

### 未验证事项
- [x] release 44900168801 双绿、Windows zip sha256 核对
- [x] 本机部署 v1.0.84 并保留 1.0.83 回滚
- [ ] 新会话账本 composition 字段落盘、`/stats` 行显示
- [ ] 长程压缩的 relay verbatim_missing / idle-wait net 日志

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
2. **编译最贵**。全量 pager lib test ≈ 6–11 分钟编译 + 4 分钟跑；共享 crate 会级联重编。
   语法/类型验证用 `cargo check -p <pkg>`；行为验证按模块过滤；一轮批量收集信息，
   禁止"改一行→全量→再改一行"。**编译/测试输出禁止接 `tail`/`head` 管道**：失败
   详情会被丢弃，只能整轮重跑；完整输出直落后台日志或 `> file 2>&1`，失败后只对
   已写完的日志 tail/grep。
3. **长任务放后台**。长编译/全量测试放后台，期间做不冲突的分析；禁止连续大超时阻塞。
   Windows 上 cargo 被 kill 可能留冻结进程：`tasklist | grep cargo/rustc`，必要时
   `taskkill //F //IM cargo.exe`。
4. **Windows 环境与测试门禁（MANDATORY）**。本机 Windows，上游基线 Linux CI。失败先判
   环境族：8.3 短名 `~`、路径分隔符、HOME/USERPROFILE、POSIX shell、终端字形、
   `File::open` 开目录、保留设备名 `nul/con/aux/com1…`（写入进黑洞仍报成功）。环境族
   按"屏蔽噪声"处理，真 bug 才修。目录身份用 creation_time（mtime 随子项变化；~15 秒内同名重建也会骗人）。
   - 本机任何 cargo test 一律走 `scripts-local/ctest.sh -p <pkg> --lib`（白名单 +
     `RUST_MIN_STACK=32MB` + `TMP=D:\cargo-tmp` + 自动拼 `docs-local/win-skip.txt`
     的 `--skip`）。禁止裸跑；一次性分析确需裸跑用 `GATE_FORCE=1`，事后恢复。
   - `win-skip.txt` 是已知必挂测试的唯一台账（每条带根因）。新环境族先登记再跑；
     处置：不兼容/不重要 → 门控，重要 → 评估修复；修复后撤销并记 PATCHES.md。
   - 分诊见 `docs-local/WIN-TEST-GATE.md`：`win_scan.py`（零编译）→ `win_sample.py`
     → 只对失败模块深排；孤例才打 `#[cfg(windows)] #[ignore]` 并登记 PATCHES.md。
   - **上游（Grok 同步）套件整包回归一律 WSL / Linux CI**，禁止本机逐个排查。
     LOCAL 改动验证 = `cargo check` + 本地自有 crate 的 lib 测试（xai-chat-state /
     sampler / sampling-types / compaction / pager 等）。
5. **临时目录不走 C 盘**。走 `ctest.sh` 自动注入；裸跑 cargo 手动带
   `TMP='D:\cargo-tmp' TEMP='D:\cargo-tmp'`。
6. **上游同步会覆盖同步文件**。对同步文件打的 LOCAL 补丁（cfg 门控等）必须在
   `docs-local/PATCHES.md` 有据可查，便于同步后重放。
7. **A/B 先算成本**。归因"我的改动还是既有问题"时，优先零编译证据（读码、既有 CI
   日志、grep）；确需运行时对照时用已构建产物，或并入修后唯一一轮验证。禁止为对照
   单独多烧一轮重编。多主题验证时小 crate 快验证放最前、长链大编译放最后；快问题
   别排在慢构建后面（cargo 构建锁全局互斥，探针会卡到超时）。
8. **lib test 过滤省跑不省编**。选重编级联最小的 crate；旁支 crate 新套件先跑基线。
   rustc 连续 ACCESS_VIOLATION（0xc0000005）≠ 代码错误，是增量缓存损坏：
   `rm -rf target/debug/incremental` + `CARGO_INCREMENTAL=0` 重试。
9. **大杂烩单测先拆分再调试**（LOCAL 标注），让失败自报用例名。Barrier/锁测试挂死
   往往是"被测流程在锚点前就失败"的放大，先查早期错误再怀疑并发。
10. **已验证的主题立刻提交**。失败重试与提交推送解耦，绿一块走一块。

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
- token 经济二轮 — P1 已随 v1.0.84 发版部署（replay diet + relay 默认开 + 空闲等待压缩 270s/ROI）；待长程观察 relay/idle-wait 日志；`token-economy-port/replay-diet-2026-11-21.md`。
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

- 本机 `grok 1.0.84 (b5cb7026d927)`，三处部署同 hash `467a685d…`；回滚 `grok2.exe.1.0.83-534af67a90eb.bak`。
- 发版详情与 sha256：`docs-local/PATCHES.md` 七十三~八十期。最近：v1.0.84 P1 长程压缩（replay diet + relay + idle-wait）、v1.0.83 harness 成本 P0 观测、v1.0.82 lazy LSP。

## 文档地图（docs-local）

- 经济性/缓存：`token-economy-audit-2026-11/`、`token-economy-port/`（路线 + 下钻 + TTL）、`cache-strategy-port/`、`status-line-perf-wiki.md`、`limit-inference.md`、`usage/`、`usage-quota-estimate-todo.md`。
- harness/模型：`harness-model-fit/`、`agent-cli-refresh-2026-11/`、`agent-cli-tracking/`、`model-limits-config.md`、`dsh-port/`。
- 移植/特性：`workflow-pi-port/`、`kimicode-port/`、`step-code-port/`、`opencode-port/`、`lsp-port/`、`vision-bridge/`、`subagent-model-group-todo.md`、`port-roadmap.md`、`fork-feature-inventory.md`。
- 上游/兼容：`sync-upstream-2026-09-29.md`、`sync-upstream-2026-09-23.md`、`upstream-responses-event-compat.md`、`PATCHES.md`。
- 运维/取证：`startup-perf/`、`code-index-setup.md`、`stats-modal-todo.md`、`webdav-sync-todo.md`、`quota-pct-jump-audit.md`、`compaction-failure-rca.md`、`inspect-config-keys-todo.md`、`issues/`、`archive/`。
