# 项目工作规则（LOCAL fork）

## 代码理解工具优先级（2026-10-03 定）

**理解代码优先用 cbm**（`codebase-memory-mcp cli`，项目名 `D-CODE-ai-grok-build-proxy`），
grep/Read 退居补充。进入本仓库没探索过的子系统（如 permission / workflow / sampler 内部）
时，先跑 `cbm cli get_architecture`（hotspots/clusters）或 `query_graph`（Cypher 查调用链、
复杂度），一次拿到结构地图再读代码，禁止上来就连环 grep。用法与陷阱见 `~/.zcode/skills/cbm/SKILL.md`。

## 分支纪律（MANDATORY，2026-11-13 改口径）

- **直接在 `main` 上施工，不再开新分支**：功能/修复/重构一律提交到 main，靠
  GitHub Actions 的 build 结果验收，通过后按 `vX.Y.Z` 打 tag 发版。
- **一次推送一批**：main 的 push 会触发 `build.yml`（docs-only 改动被 `paths-ignore`
  跳过、不跑 CI），因此仍按「攒一批改动 → 一次推送 → 红就修完再推」执行，
  禁止一个提交一次 build。
- **main 红了当班修**：没有 PR 门禁兜底，CI 失败就是 main 失败；拿到失败日志先分族
  （见下节时间成本原则），修完连同原因一起再推。
- 历史 `feat/local-*` 分支保留作考古对照，不再新增；现存未合的 PR 按原样验收合并。

## 调试的时间成本原则（MANDATORY）

一切调试先算时间账，优先长期时效，不为单点问题反复烧整轮编译：

1. **先分族，再动手**：本仓库测试失败往往成族出现（几十个同根因）。先取一份失败清单按报错分族
   （如 UnsafeDirectory/字形降级/POSIX 专属/路径分隔符），一族一策，禁止逐个盲修。
2. **编译是最贵的操作**：全量 `cargo test -p xai-grok-pager --lib` ≈ 编译 6–11 分钟 + 跑 4 分钟；
   改共享 crate（xai-grok-config / xai-grok-agent / …）会级联重编整棵依赖树。因此：
   - 语法/类型验证用 `cargo check -p <pkg>`（快一个量级）；
   - 行为验证按模块过滤：`cargo test -p xai-grok-pager --lib <module::>`；
   - 一次改动批量收集信息后只跑一轮，禁止"改一行→全量→再改一行"的循环。
   - **编译/测试输出禁止接 `tail`/`head` 截断管道**（乐观跑法）：一旦失败，断言详情
     已被管道丢弃，只能整轮重跑，时间成本翻倍。完整输出让它直落后台日志（不接
     管道）或 `> 文件 2>&1`；失败后只对**已写完的日志转储文件**做 tail/grep 取详情。
3. **不阻塞死等长任务**：长编译/全量测试放后台跑，期间做不冲突的分析/阅读，完成看通知；
   禁止连续 TaskOutput 大超时阻塞。Windows 上 cargo 被 kill 可能留下冻结进程，用
   `tasklist | grep cargo/rustc` 检查，必要时 `taskkill //F //IM cargo.exe`。
4. **本机是 Windows 开发机，上游基线是 Linux CI**：失败先判断是不是环境族
   （8.3 短名含 `~`、路径分隔符 `\` vs `/`、HOME 被 USERPROFILE 覆盖、POSIX shell 缺失、
   终端字形探测、`File::open` 开目录需 `custom_flags(FILE_FLAG_BACKUP_SEMANTICS)`、
   夹具文件名避开保留设备名 `nul/con/aux/com1…`——写入进黑洞还显示"成功"），是环境族的按
   "屏蔽噪声（cfg 门控/播种全局）"处理，真产品 bug 才修。目录身份用 creation_time（mtime
   随子项增删变化），但注意 ~15 秒内同名重建的隧道化会让 creation_time 也骗人。
   **Windows 测试门禁（MANDATORY，2026-09-19 定稿）——本机任何 cargo test 一律经门控，
   禁止裸跑**：
   - 唯一入口：`scripts-local/ctest.sh -p <pkg> --lib`（L0 白名单拦截非白名单 crate +
     L1 自动注入 `RUST_MIN_STACK=32MB`/`TMP=D:\cargo-tmp` + L2 自动拼
     `docs-local/win-skip.txt` 的 `--skip`）。裸 `cargo test` 会把 win-skip 已定案的
     已知必挂测试（flock/信号/POSIX 路径形态等族）重新放进输出污染结果，
     视为违反本规则；一次性分析确需裸跑时用 `GATE_FORCE=1` 显式放行并事后恢复。
   - `win-skip.txt` 是已知必挂测试的唯一台账（每条带根因注释）：新确认的环境族失败
     先登记再继续跑，不逐个排查；修复某族后从清单撤销并登记 PATCHES.md。
     处置标准：确认 win 不兼容或不重要的 → 门控；重要且非兼容性问题的 → 评估修复。
   - 分诊流水线与工具见 docs-local/WIN-TEST-GATE.md：静态扫描 `win_scan.py`（零编译）
     → 抽样 `win_sample.py` → 只对失败模块深排；溢出族已由 L1 消解，孤例才打
     `#[cfg(windows)] #[ignore]` LOCAL 补丁并登记 PATCHES.md。
   - 上游套件整包回归一律 WSL / Linux CI（L4），禁止在本机逐个排查（见第 11 条）。
11. **上游（Grok 同步）测试套件在 Windows 上不可作为回归依据**：约九成用例依赖 Linux 行为
   （POSIX 路径/权限/线程栈/终端探测），本机跑它是无底洞。LOCAL 改动的验证 = 
   `cargo check`（编译正确性）+ **本地自有 crate 的 lib 测试**（xai-chat-state/sampler/
   sampling-types/compaction/pager 等本地主题）；上游 shell 套件只做编译级验证，
   套件级回归留给 Linux CI 或专用会话批量处理，禁止在本机全量跑上游套件排查。
5. **临时目录不走 C 盘**：走 `ctest.sh` 时自动注入；裸跑 cargo 命令才需手动带
   `TMP='D:\cargo-tmp' TEMP='D:\cargo-tmp'` 前缀（目录已存在）。
6. **上游同步会覆盖同步文件**：`Synced from monorepo` 提交会冲掉同步文件里的本地改动；
   对同步文件打的 LOCAL 补丁（cfg 门控等）要在 docs-local/PATCHES.md 有据可查，便于同步后重放。
7. **最便宜的决定性实验先行**：归因"是我的改动还是既有问题"用 A/B——`git stash push -- <file>`
   跑原始版对比，比理论推演快且可靠（本轮一击排除 FileIdentity 嫌疑）。快问题（哪个用例挂）
   别排在慢构建后面：cargo 构建锁全局互斥，探针会被卡到超时。多主题验证时把小 crate 的
   快验证放最前，长链大编译放最后。
   **A/B 适用边界（2026-10-03 定）**：本仓库是 Rust——编译时间成本高，且任何改动
   都触发 cache miss 级联重编，"改动前先跑一遍原始版对照"是 Go 等快编译语言的习惯
   打法，在这里不是首选。基线结论优先零编译获取：读代码手推、既有 CI/测试日志、
   grep 现场；确需运行时对照时用已构建产物，或并入修后唯一一轮验证，禁止为"对照"
   单独多烧一整轮重编（v1.0.41 修前 A/B + 修后验证两轮近全量 shell 测试编译即反例）。
8. **lib test 过滤省跑不省编**：`cargo test --lib <filter>` 也要整编测试二进制，为验证一行
   改动选"重编级联最小"的 crate，别只看过滤名。旁支 crate 自己的 `--lib`（如
   xai-grok-pager-render）在只测 pager 时从没编过——新套件先跑基线再谈回归。
   rustc 连续在不同 crate 上 ACCESS_VIOLATION（0xc0000005）≠ 代码错误，是增量缓存损坏：
   `rm -rf target/debug/incremental` + `CARGO_INCREMENTAL=0` 重试，禁止盲目原样重跑。
9. **大杂烩单测先拆分再调试**：单个 `#[test]` 打包多场景时失败无法定位，被迫探针重跑整轮；
   继承的先拆（LOCAL 标注），让失败自报用例名。挂起 ≠ 并发 bug：Barrier/锁同步测试挂死
   往往是"被测流程在锚点前就失败"的放大，先查早期错误再怀疑并发。
10. **已验证的主题立刻提交**，不把全部改动押在一盏统一绿灯上；失败重试（如 rustc 崩溃）
    与提交推送解耦，绿一块走一块。

## 记录与归档约定（2026-11-13 治理）

本文件只放**仍在生效的规则**与**一行式活跃待办**。过程记录、发版流水、已完成
事项一律进 `docs-local/` 专题或 `docs-local/archive/`，不再在 AGENTS.md 累积。

- 新待办一行：`- 主题 — 当前状态；指针`；完成后当轮删除，详情归档。
- 一次性记录（排查/事故/审计）收口当天移入 archive，索引只追加。
- 引用文件/链接前先核对真实存在。
- 本文件超过约 15 KB 或待办超过 50 条时做一次治理（归档 + 合并）。
- 治理前快照：`docs-local/archive/agents-md-todo-handoff-snapshot-2026-11-13.md`
  （2026-09~11 的全部待办与交接原文）。

## 活跃待办（2026-11-13）

### 施工中 / 待合
- token 经济二轮 — A 组已随 v1.0.77 发版，剩余 B 组（推理降级 A/B、调用合并 A/B、`search_tool` schema 上限）未开工；交接卡 `.handoff/token-economy-next-round.md`。
- 压缩指令（`/compact <instructions>`）— v1.0.75 已发版并部署，待活体验收；见 `docs-local/compact-instructions-handoff.md`。
- IP 端点自签证书放行 — 分支 `feat/local-insecure-tls-ip-hosts` 待合；`[network] insecure_tls_hosts`。
- grok-doctor 技能 — P1；`docs-local/agent-cli-tracking/skill-plugin-port-eval-2026-11-13.md`。

### 待活体验证（随新客户端）
- 视觉桥（T1–T3+D8/D9）、空闲自动压缩（`shell.idle_compact.*` 事件 + 下一轮 TTFT）、cache P1/P2（触发点后首轮命中）、`/usage` 额度估算口径。
- `/stats` 的 `g` 分组与缓存断崖汇总行；`/compress-fast` 在 `prune_mode = append` 模型上清掉旧结果并写入失效标记。
- LSP 四项（诊断/懒启动/rename/面板）、`[api_dump]` 落盘、09-29 上游同步抽查、`/usage` 套餐块。

### 未开工 / 待拍板
- workflow-pi-port P2 T2（deferred tools）；kimicode T2b（replay 双路径）；step-code P3/P4；dsh-port P1。
- zcode 3.17.2 P1（provider-tool-rounds / foldReminders / post-compact 回补 / hooks `onError` / superpowers 三件 A/B）；opencode diff 面板；webdav sync；`/dump`；startup-perf 缓解；其余 21 套主题；harness-model-fit 建议；agent-cli-refresh P3（prompt snapshot）。
- qwen 1.0 增补 — `/context detail`+idle cost（P0）、microcompaction 规格（并入 P1）、调度闸门（P2）；`docs-local/agent-cli-tracking/notes/qwen-code-1.0-2026-11.md`。

### 长期挂起
- Ctrl+Enter 幽灵气球（P2，`docs-local/issues/`）；quota 他机对账；limit-probe T3；dump 自动清理；model-picker T2。

## 🔄 Handoff 摘要

### token-economy-next-round — A 组已发版

- **当前状态：** 压缩基线归因完成（报告 `docs-local/token-economy-port/compaction-baseline-attribution-2026-11-14.md`）；A 组与工具隐藏随 v1.0.77 发布并部署，工具声明 11,481 → 约 6,110 tokens。
- **剩余：** B 组——subagent 推理降级 A/B、调用合并 A/B、`search_tool` MCP schema 字节上限。
- **验收：** 账本出现 `purpose=compaction`；relay 会话 0 次 turn_summary；title 刷新输入约 10k；隐藏工具不在 `tool_definitions.json`。
- **详情指针：** [`.handoff/token-economy-next-round.md`](.handoff/token-economy-next-round.md)

## 最新交接（2026-11）

- **2026-11-14 v1.0.77 发版并部署**：token 经济 A 组（压缩用量入账、非 xAI 跳过
  turn_summary、title 刷新 10k 截断）+ 零调用工具静态隐藏（`scheduler_*` 保留）；
  main build `43563259558` 双绿，release `43567249488` 双绿、4 资产，Windows zip
  sha256 `9d67c8f369117dd6578089f1ff54db079eee3f523fd955e7e6f4065eae4e3843`；本机
  `grok 1.0.77 (7b7925b8cf86)`，回滚 `grok2.exe.1.0.76-02c22394fdfe.bak`，
  `~/.grok/bin` 已刷新（`*.bak-1.0.76-fork`）。Linux 单测失败与修复见 PATCHES 七十三期。
- **2026-11-13 token 经济波次完成**：#12/#13/#14 已合（`797b9365`/`6741d1a6`/`b84bb71c`）；
  #16（断崖 + `/stats` 分组）与 #17（`/compress-fast`）折叠进 main
  （`9d8f3b53`/`6fd59b0a`），Linux E0308 修复 `02c22394`；main build `43432927949`
  Linux/Windows 双绿；`v1.0.76` release run `43438765045` 双绿、4 资产，Windows zip
  sha256 `c60809e5a98b58930d157fa3250c86921709b207ab5f00d6fa30baf5d8d097ec`；本机部署
  `D:\tool-cli\grok2\grok2.exe` = `grok 1.0.76 (02c22394fdfe)`，回滚
  `grok2.exe.1.0.75-828d5f5e0486.bak`，`~/.grok/bin` 已刷新。交接卡已归档
  `.handoff/archive/token-economy-wave-2026-11.md`。
- **2026-11-13 分支口径变更**：后续直接在 `main` 施工，不再开 feature 分支（见「分支纪律」）。
- **v1.0.75（11-13 发版）**：`/compact <instructions>` 手动压缩保留重点。PR #11
  merge `828d5f5e`；release run `43373939909` success，4 资产，Windows zip sha256
  `2ea66d92…`；本机部署 `D:\tool-cli\grok2\grok2.exe` = `grok 1.0.75 (828d5f5e0486)`，
  回滚点 `grok2.exe.1.0.74-aae2d5692699.bak`。活体验收待新会话。
- **v1.0.74（11-12 发版）**：`send_feedback` 描述去掉会话路径；活体确认工具定义跨会话一致，首轮命中待新会话复测。
- **2026-11-13**：token 经济配置已应用——4 个 1M 模型 `prune_mode=append` + `system_prompt_update=in_history`，窗口 300K + 80% 压缩（实测本会话 84 分钟 ≈ $1.0，同负载 1M 窗口 ≈ $2.1）；修正 `in-history` 非法拼写（此前能力位未生效）。变更说明在 `D:\pro\agent-teams-docs\grok-token-economy-config-changes.md`。
- **v1.0.73**：release-warm 缓存预热、gash 后端、`inspect [privacy]` 修复。
- 更早发版与交接原文见 `docs-local/archive/agents-md-todo-handoff-snapshot-2026-11-13.md`。

## 文档地图（docs-local）

- 经济性/缓存：`token-economy-audit-2026-11/`、`cache-strategy-port/`、`status-line-perf-wiki.md`、`limit-inference.md`、`usage/`、`usage-quota-estimate-todo.md`。
- harness/模型：`harness-model-fit/`、`agent-cli-refresh-2026-11/`、`agent-cli-tracking/`、`model-limits-config.md`、`dsh-port/`。
- 移植/特性：`workflow-pi-port/`、`kimicode-port/`、`step-code-port/`、`opencode-port/`、`lsp-port/`、`vision-bridge/`、`subagent-model-group-todo.md`、`port-roadmap.md`、`fork-feature-inventory.md`。
- 上游/兼容：`sync-upstream-2026-09-29.md`、`sync-upstream-2026-09-23.md`、`upstream-responses-event-compat.md`、`PATCHES.md`。
- 运维/取证：`startup-perf/`、`stats-modal-todo.md`、`webdav-sync-todo.md`、`quota-pct-jump-audit.md`、`compaction-failure-rca.md`、`inspect-config-keys-todo.md`、`issues/`、`archive/`。
