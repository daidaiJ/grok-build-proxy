# 项目工作规则（LOCAL fork）

## 待办事项（TODO）

> **docs-local 维护约定（2026-09-30 定）**：根层与专题目录只放**活跃**文档（有未完成
> 待办 / 被引用的规约与账本 / 持续追加的 wiki）。排查、事故、审计类一次性记录在收口
> 当天移入 [`docs-local/archive/`](docs-local/archive/README.md)（索引与收口原因见其
> README），跨文引用同步改路径。**归档索引只追加**：新条目一律追加在
> archive/README.md 表尾，禁止插入/重排/修改已有条目；归档文档正文同理不改，
> 补充以文末"后记"追加。规约类完工文档（如 ui-i18n-plan）不归档但须在头部
> 标明"已完工 + 保留角色"。更新本索引时先核对文件真实存在。

- **欢迎页 XL 熊猫 logo 档** — 已随 **`v1.0.38`** 发版并部署（2026-09-30，分支
  `feat/local-panda-logo-tier`，CI 36672275454 绿后打 tag，release 36675360723 双 job
  success）。内容：新增
  `assets/logo/logo13.txt`（44×13 盲文格）
  + `LogoTier::Xl` 档（终高 ≥44 行显示，step_down 链 Xl→Full→Compact→Hidden，
  stacked 布局对档位自适应、hero box 仍固定 Full）。资产由图像半调管线
  [`tools/panda_dither.py`](tools/panda_dither.py) 生成（采样/LANCZOS/边缘钉实/
  纸白保护/Bayer 半调，用法见文件头 docstring）。背景：Windows 终端贴真图不可行
  （ConPTY 剥 APC，`xai-grok-pager-render/src/terminal/image.rs` 的
  `protocol_for_brand` 硬编码 None），此为字符路线在 Win 下的质量上限；Kitty/
  Ghostty/WezTerm（非 Win）若要贴真图是独立 feature。已验证：`cargo check -p
  xai-grok-pager` 通过、`ctest.sh -p xai-grok-pager --lib welcome` 全绿；中间产物
  （半调 PNG/对比图）在 `panda-dither-out/` 未入库。

- [`docs-local/sync-upstream-2026-09-23.md`](docs-local/sync-upstream-2026-09-23.md) —
  **上游 5 快照合并已完成**：分支 `sync/upstream-2026-09-23`（快照合并 `e43381a7` + 收尾
  修复 `c78b45bb`）已快进合入 `main`，随正式 tag **`v1.0.37`** 发布（2026-09-28）。
  上游 `xai-org/grok-build` main `f0e3be1`（09-23），本 fork 上一条同步线停在 `37949780`（09-09）；
  2003 文件 +205k/−73k，冲突 52 文件全部人工裁定（撤销本地 `respect_config_agent` 补丁，
  改用上游 `defer_builtin_agent_profile`）。已验证：`cargo check --workspace --all-targets`
  0 error/0 warning、白名单 6 个 crate lib 测试全绿（pager 10031 passed / 0 failed，
  新增 `win-skip.txt` 族Q 4 条环境族）。
  **待办**：① 活回合（TUI 实跑）验证（发 tag 时未做）② ~~Linux CI 一条新用例失败~~ 已收口：
  根因是 CI 无 ripgrep（该用例的 grep 根本没 spawn 成功，`exit -1` + 空 stdout 被记成
  `source_status = failed`），已在 `ci-setup` 增 `Install ripgrep (Linux)`；`1b9d5fc9` 复跑
  `build` 36402142384 双 job success、该用例转为 ok；详见同步文档"已收口"节 ③ 34 处 i18n 丢失站点按
  [`sync-2026-09-23-i18n-lost.txt`](docs-local/sync-2026-09-23-i18n-lost.txt) 补回
  ④ 下次同步前把本条并入 handoff 摘要，并把 `upstream` 分支移到 `f0e3be1`。

- [`docs-local/quota-pct-jump-audit.md`](docs-local/quota-pct-jump-audit.md) — 统一池
  周用量 7%→76% 跳变排查记录（2026-W39）：本机四步排查（pct 轨迹 / 账本聚合 /
  窗口活动三重核查 / config 审计）全部排除本机，消耗在账号其他统一池面；首嫌疑 =
  他机 xAI 直连 agentic 会话（燃烧速率 ≈日常 15–20 倍）。含他机对账 playbook、
  09-30 07:50 重置后的 Q 标定实验、unified.jsonl 活动文件取证坑。
  配套只读取证工具 `scripts-local/quota_audit.py`（**工作区未入库**——按分支纪律
  代码不走 main，入库存放分支待拍板）。他机对账结果待回填。
- [`docs-local/usage-quota-estimate-todo.md`](docs-local/usage-quota-estimate-todo.md) —
  `/usage` 周额度 token 估算：billing 采样落盘 → 本机 xAI 直连 tokens/Δpct 反推下界
  （pct 整点量化、多设备按"各机下界取 max"语义）→ Usage limit 面板展示。
  **T1/T2/T3 施工完成**（分支 `feat/local-usage-quota-estimate`，实现记录见
  PATCHES.md）；金额口径与 `/stats` 联动未做，等 09-30 07:50（北京）换周期
  historyLen 实测后拍板。关键实测：`creditUsagePercent` 全整点、7%→76% 跳变非本机
  （本周账本 xAI 直连 0 调用）、`isUnifiedBillingUser=true`、`historyLen=0`。
  现场 QA（pct 已变仍报采样不足 = Δpct < 2 整点门，非无 xAI 消耗）见该文档末节，
  补 user-guide / 文案时复用。
- [`docs-local/status-line-perf-wiki.md`](docs-local/status-line-perf-wiki.md) — 状态行性能段
  （ttft/tps）专题 wiki：TTFT 三参考点对照、网关响应头行为分类（Command Code 早头 vs
  火山 Ark 晚头）、0ms 过滤×晚响应头的相互作用、unified.jsonl 取证 playbook、双 tps 口径。
  只收深挖节点，不复述可从代码/文档推出的内容。随修复 `feat/local-perf-ttft-request-anchor`
  （TTFT 参考点前移到请求发起）落地。
- [`docs-local/limit-inference.md`](docs-local/limit-inference.md) — **供应商 cache
  TTL / RPM / TPM 反推记录，T1/T2 已实现**（分支 `feat/local-limit-probe`）：sampler
  HTTP 层被动落盘每次请求的限流头/429 现场/前缀哈希/终端 cached_tokens 到
  `~/.grok/limit-probe/records.ndjson`，`scripts-local/limit-inference.py` 离线反推
  TTL 生存曲线与 429 滑窗限流夹逼。全被动零 token 成本；`GROK_LIMIT_PROBE=0` 关闭。
  T3 主动探测（idle sweep 补 >1h 长尾、多前缀容量测试）未做。
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
- **`/dump` 现场取证转储**（⚠️ 2026-09-30 盘点：原 `docs-local/dump-evidence-todo.md`
  已不在工作区且从未入库，仅本条摘要幸存）— **设计完成、未开工**；分期 T1 派生层生成器
  （`INDEX.md` + `facts.json`）→ T2 出站请求体落盘
  → T3 `/dump` TUI → T4 打包/脱敏档位 → T5 回归夹具。实测依据：会话 `01a0d161` 原始
  25.5 MB / 168 文件（其中一个工具输出占 95%、`events.jsonl` 98.8% 行是同一种心跳）
  → 派生层 4.8 KB（1/5292）。关键缺口是 MiMo 取证三问要的**出站请求体从未落盘**；
  文档含三层结构、脱敏表、充分性/友好性两份验收清单与 5 条待定决策。基线：当时 `main`。
  重启该专题时先按本摘要重建文档再施工。
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
- [`docs-local/vision-bridge/`](docs-local/vision-bridge/README.md) — 视觉桥：纯文本主模型
  用识图模型转写用户图 / 工具图。**评估完成、未开工**；分期 T1 能力位 `accepts_images` +
  打开已有 `image_describe`（Cursor 管线在本 fork 写死关闭）→ T2 工具结果/`read_file`/PDF
  剥图或转写 → T3 披露与 BYOK 禁止静默打 xAI。整回合切视觉 agent 明确不做。对照兄弟目录
  Qwen Code。净开发约 6–11 人天。分支拟 `feat/local-vision-bridge`。基线：2026-09-27 `main`。
- [`docs-local/workflow-pi-port/`](docs-local/workflow-pi-port/README.md) — **workflow ×
  pi 特性移植预研（选型完成，下一步预研特性，未开工）**：对照 pi（earendil-works/pi，
  `0.84.0`–`0.99.1`）近两月特性与 `xai-workflow` 现状，结论 = 编排原语不缺、真缺口三个
  （编排上下文膨胀 / 子 agent 工具面全量声明 / 成本）。两个选型特性已出设计原型：
  P1 [canonical-context-edit.md](docs-local/workflow-pi-port/canonical-context-edit.md)
  （journal 化 `context_edit` host call + prompt 构建层可见面，历史不动与重放确定性
  兼容）、P2 [deferred-tool-exposure.md](docs-local/workflow-pi-port/deferred-tool-exposure.md)
  （exposure 收敛三档 + `AgentOpts.tools/defer_tools` + `search_tools` 延迟声明，
  不碰重放语义）。codemode 判定不移植（QuickJS 双运行时破坏 journal hash 确定性，
  决策 D1，只吸收按分支 KV / 大输出旁路两个思想）；cache warming 拟 P2' 独立专题。
  分支拟 P1 `feat/local-workflow-context-edit` / P2 `feat/local-deferred-tool-exposure`。
  基线：2026-09-30 `main` `95aad87`（pi 基线 `0.99.1`，2026-09-29）。

## 🔄 Handoff 摘要

### think-split-quoted-marker-fold — verified（preview.6 实测通过）

- **当前状态：** 修复已合并 main（`bc33d257`）、tag `v1.0.37-preview.6` 已发布；CI（release + build）已核对；
  **活回合验证已补完**：正向用例（行内代码引用 / 围栏）正文完整不折叠、真标记路径未误伤、落盘扫描 0 命中、
  回归 `ctest.sh -p xai-grok-sampler --lib` 272/272。⚠️ 用户当前在用的客户端 `D:\tool\cli\grok2.exe`
  仍是 `1.0.37-preview.4`（**无此修复**），日常会话照旧折叠，需换到 preview.6 产物才生效
- **关键证据：** 正文里被反引号引用的 `<think>` 曾被切分器当控制标记，把回答尾部改道 reasoning；
  判定签名＝正文 chunk 以反引号结尾 + 紧随 thought chunk 以反引号开头 + `<think>` 字面量两通道都缺
  （3 会话 6 处现场）；修复后 `ctest.sh -p xai-grok-sampler --lib` 272/272，mutation 下 5 例转红。
  ⚠️ CI 不跑 sampler 用例（只把它当依赖编译），见 handoff §8
- **验收标准：** 全部达标（release 4 资产 + `grok2.exe --version` = `1.0.37-preview.6`；build 双 job success；
  活回合正向 + 落盘扫描 + 回归三项证据见 handoff §5 / §13）
- **详情指针：** [`docs-local/archive/think-split-quoted-marker-fold.md`](docs-local/archive/think-split-quoted-marker-fold.md)
  （⚠️ 原 `.handoff/` 目录从未入库、已不存在，archive 内该文是唯一完整记录）

### 未验证事项

- [ ] sampler 用例在 Linux CI 上的表现（CI 当前不跑；本机无 WSL/Linux 环境，跑不了；
  是否把 `-p xai-grok-sampler` 加进 `build.yml` 的 linux job 仍未拍板，见 handoff §8）
- [x] 修复前对照（A/B）— 同提示词跑 `v1.0.37-preview.5` 产物：签名命中 1 处、466 字符（含 `## 四`/`## 五`）
  被改道 reasoning；preview.6 同提示词 0 命中
- [x] 真实 TUI 会话的正向用例 — preview.6 + `--minimal` 活回合：正文 398 字符含 `` `<think>` `` 与
  `## 四`/`## 五` 两节，思考通道 2800 字符纯规划，屏幕上有独立折叠思考块（“Thought for 3.9s”）
- [x] 真标记路径在活的上游流里的表现 — 让模型裸写 `<think>…</think>`：块进思考通道、正文留在正文通道
- [x] 已知未覆盖形态的现场确认 — `"<think>"` 与 `**<think>**` 当场复现折叠，与预期一致（登记为已知残留，不算回归）

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
