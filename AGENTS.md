# 项目工作规则（LOCAL fork）

## 待办事项（TODO）

- [`docs-local/stats-modal-todo.md`](docs-local/stats-modal-todo.md) — `/stats` 加时间窗
  option（5h/day/week/month）+ 输出改成 Grok 自制窗口样式（对齐 agents / usage limit 面板）
  + TUI 残留英文说明 i18n 扫尾。分支 `feat/local-stats-modal-i18n`，基线 `86a6e1d`。
  三个子任务 T1/T2/T3 相互独立，可分批施工；文档内含现状锚点、改动点、验收标准与纪律要求。
- [`docs-local/webdav-sync-todo.md`](docs-local/webdav-sync-todo.md) — WebDAV 同步（用量 +
  主机无关配置，服务端按机器分子目录）+ `/sync` 选择恢复/同步。**评估完成、未开工**；分期
  T1 传输层+假服务夹具 → T2 白名单/合并内核+`grok2 sync` CLI → T3 `/sync` TUI → T4 加密与凭据。
  净开发 6–8.5 人天，含测试联调 10–12 人天。三个前提风险：明文密钥外泄（默认剥离）、
  TOML 合并冲突、上游同步维护面；文档内含数据分类白名单、身份/目录/协议定案、备选路线对比
  与待定决策。基线 `f8e1fea3`（调研快照 2026-09-21）。
- [`docs-local/upstream-responses-event-compat.md`](docs-local/upstream-responses-event-compat.md)
  — 第三方网关（Command Code）发非标 `response.reasoning.delta` 事件，撞上 async-openai
  的 serde 严格枚举 → 流式解析中止、整轮失败且不重试。含抓包、本仓库落点、三个修复
  选项与未验证项。**尚未修**，当前以配置侧绕过（该模型改用 `chat_completions`）。
- [`docs-local/model-limits-config.md`](docs-local/model-limits-config.md)
  — 自定义模型 `context_window` / `max_completion_tokens` 的兜底语义（三个兜底值分属
  不同代码路径，含锚点）与「不烧 token 拿真实上限」的查法（OpenRouter 目录 /
  方舟 `token_limits` / CC `/models`）。含文档约数 vs 元数据精确值的差异、新增模型检查清单。
- [`docs-local/agent-memory-design/`](docs-local/agent-memory-design/README.md) — **专题调研
  （调研完成，不含落地方案）**：业界 agent 软件（Claude Code / Cursor / opencode / Codex CLI /
  Copilot / Windsurf / Cline）的记忆模块工程落地与取舍、核心优化点与演进方向。目录内含索引
  `README.md`（用途 / 范围 / 可信度分级）、主报告 `memory-design-survey.md`、社区风评
  `notes/community-sentiment.md`（Reddit 归档 API + HN Algolia，含原始引语）。边界：只谈 agent
  软件内部的记忆模块（指令层 / 自动记忆层 / 会话内上下文管理），不做独立 memory 框架选型。

## 🔄 Handoff 摘要

### think-split-quoted-marker-fold — in-progress

- **当前状态：** 修复已合并 main（`bc33d257`）、tag `v1.0.37-preview.6` 已发布并核对通过；release 与 build 两个 workflow 均 success，剩 TUI 实测
- **关键证据：** 正文里被反引号引用的 `<think>` 曾被切分器当控制标记，把回答尾部改道 reasoning；
  判定签名＝正文 chunk 以反引号结尾 + 紧随 thought chunk 以反引号开头 + `<think>` 字面量两通道都缺
  （3 会话 6 处现场）；修复后 `ctest.sh -p xai-grok-sampler --lib` 272/272，mutation 下 5 例转红。
  ⚠️ CI 不跑 sampler 用例（只把它当依赖编译），见 handoff §8
- **验收标准：** CI 侧已达标（release 4 资产 + `grok2.exe --version` = `1.0.37-preview.6`；build 双 job success）；
  余下：TUI 正向用例正文完整不折叠 + 落盘扫描 0 命中 + 回归 272 全过（详见 handoff §5）
- **详情指针：** [`.handoff/think-split-quoted-marker-fold.md`](.handoff/think-split-quoted-marker-fold.md)

### 未验证事项

- [ ] 真实 TUI 会话的正向用例（本会话只跑单测 + 线级事件断言，未跑活回合）
- [ ] 真标记路径在活的上游流里的表现（仅单测覆盖）
- [ ] 已知未覆盖形态（`"<think>"` / `**<think>**`）的现场确认（预期仍折叠）
- [ ] sampler 用例在 Linux CI 上的表现（CI 当前不跑；可选改进见 handoff §8）

---

## 分支纪律（MANDATORY）

- **主分支只收文档，不收功能代码**：一切新功能/修复/重构必须在 feature 分支（如
  `feat/local-*`）上开发提交，验证通过后合入 main 并按 `vX.Y.Z` 打 tag 发版。
  main 上只允许直接提交文档类改动（AGENTS.md、docs-local/、README 等），禁止在
  main 上直接提交任何非文档变更。

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
