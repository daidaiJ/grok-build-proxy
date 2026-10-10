# 项目工作规则（LOCAL fork）

## 代码理解工具优先级（2026-10-03 定）

**理解代码优先用 cbm**（`codebase-memory-mcp cli`，项目名 `D-CODE-ai-grok-build-proxy`），
grep/Read 退居补充。进入本仓库没探索过的子系统（如 permission / workflow / sampler 内部）
时，先跑 `cbm cli get_architecture`（hotspots/clusters）或 `query_graph`（Cypher 查调用链、
复杂度），一次拿到结构地图再读代码，禁止上来就连环 grep。用法与陷阱见 `~/.zcode/skills/cbm/SKILL.md`。

## 待办事项（TODO）

> **docs-local 维护约定（2026-09-30 定）**：根层与专题目录只放**活跃**文档（有未完成
> 待办 / 被引用的规约与账本 / 持续追加的 wiki）。排查、事故、审计类一次性记录在收口
> 当天移入 [`docs-local/archive/`](docs-local/archive/README.md)（索引与收口原因见其
> README），跨文引用同步改路径。**归档索引只追加**：新条目一律追加在
> archive/README.md 表尾，禁止插入/重排/修改已有条目；归档文档正文同理不改，
> 补充以文末"后记"追加。规约类完工文档（如 ui-i18n-plan）不归档但须在头部
> 标明"已完工 + 保留角色"。更新本索引时先核对文件真实存在。

- **模型级代理启用配置（`[model.<id>] use_proxy`）** — 已合 main（`76891f3e`），随
  **`v1.0.43`** 发版；实现记录见 PATCHES.md 二十三期。`use_proxy = true` 让该
  模型的采样请求走进程级出口代理（`[network] proxy_url` / `GROK_PROXY`），**默认
  false 恒直连**——即使配了进程代理，未显式启用的模型也不走（语义与旧版「白名单
  host 默认过代理」不同，升级注意）。没配代理 / URL 非法：规则丢弃 + warn 一次 +
  直连；代理运行中故障走既有重试，不做请求级自动绕过。落点：`xai-grok-extra-ca`
  （`build_reqwest_client_no_proxy` + URL 校验）、`xai-grok-sampler`
  （`SamplerConfig.use_proxy` + 共享直连 client 双胞胎）、`xai-grok-sampling-types`
  （`SamplingConfig.use_proxy`，随会话持久化）、`xai-grok-shell`（`ModelEntryConfig` /
  `ConfigModelOverride` / `ModelInfo` 字段 + 装配点 config.rs / sampler_turn /
  subagent / spawn / model_switch / tools）。验证：`cargo check -p xai-grok-shell
  --all-targets` 0 error；sampler 291 / extra-ca 15 / sampling-types 303 lib 全绿；
  新增 shell `config::tests` 2 例绿（GATE_FORCE 过滤跑）。**活回合已验（2026-10-09，
  v1.0.44 客户端 headless）**：debug 日志 `egress proxy auto-detected from Windows
  system settings: http://127.0.0.1:7890`，muse-spark 请求经隧道到达 cline（对端
  业务响应），glm-5.3-flash 直连一轮「成功」对照组正常。muse-spark 本身 403
  region 受限是 cline 侧策略（代理出口/直连出口都被拒），需 clash 对 cline.bot
  走代理规则或换出口节点，与本项目无关。
- **欢迎页 XL 熊猫 logo 档（v1.0.39 比例修复）** — v1.0.38 发的 44×13 盲文格
  资产存在**采样宽高比 bug**：`panda_dither.py` 直接把源图 resize 到 `cols*2 ×
  rows*4` 点阵，未补偿终端字符格 1:2（格高≈2×格宽、盲文点物理近正方），方形源图
  被横向拉宽 ~1.35×（"卡比兽"）。分支 `fix/local-panda-logo-aspect` **最小修复**：
  原版 44×13 资产原样横向重采样到 33×13（LANCZOS + 0.5 阈值，构图/笔触/抖动风格
  不变），物理宽高比 33:26 ≈ 1.27 与源图内容 1.289 对齐；XL 档仍与 Full 共用
  logo13，无其他改动。中间教训（未采用）：终端里盲文稠密区必呈麻点（字体内建点缝），
  实心黑只能靠色块 `█`；13 行小画布上色块/灰阶混合会碎（8 变体证伪），已全部放弃，
  过程产物在 `panda-aspect-out/` 未入库。管线 aspect 规则已写进
  [`tools/panda_dither.py`](tools/panda_dither.py) docstring（cols*2/rows*4 == 源图
  宽高比，即 cols ≈ rows × aspect / 2）。背景：Windows 终端贴真图不可行（ConPTY
  剥 APC，`xai-grok-pager-render/src/terminal/image.rs` 的 `protocol_for_brand`
  硬编码 None），此为字符路线在 Win 下的质量上限。已验证：`ctest.sh -p
  xai-grok-pager --lib welcome` 223/223 全绿。**已按用户指示直接覆盖 `v1.0.39`
  旧 tag 发版**（2026-10-01，tag 重指 `c01f0db`，release run 36842496711 双 job
  success，4 资产同名覆盖、URL 不变；注意删 tag 曾把 release 转 draft，重传后
  已恢复发布）。历史备注：`v1.0.38` release 实为草稿状态留在 release 页（昨天
  的正式发布是 v1.0.39），tag 已恢复原指向 `5667ea0`。

- [`docs-local/port-roadmap.md`](docs-local/port-roadmap.md) — **移植路线图
  （四特性施工合 main，2026-10-03 随 v1.0.41 统一发版）**：三个移植预研专题
  （workflow-pi-port / kimicode-port / step-code-port）候选特性按「coding+agent
  价值 × 国模适配」定序。施工结果：① rewind 分支树 undo（T1/T2a/T3 落地：
  SwitchEdge 化 + 分支面 picker + redo 通道；T2b replay 统一双路径未做）
  ② headless `--non-interactive-denial continue`（勘误后实做面：step P1 静态
  分析上游已有）③ canonical context edit（journal 直记 replace_visible/
  hide_visible）④ 工具面 allowlist（deferred exposure T1；T2 延迟发现未做），
  另两随手件（流恢复注入 / 子 agent 结果信封）未动。**交接协议在案（MANDATORY）：
  每个任务完成或中断都要在 roadmap §6 交接台账追加条目；重大里程碑另进本文
  Handoff 摘要。**

- [`docs-local/fork-feature-inventory.md`](docs-local/fork-feature-inventory.md) —
  **fork 已有特性基线清单（移植预研对照表，持续回填）**：102 crate 按 11 功能域的
  职责地图 + LOCAL 自产特性表（来源分支/期号）+ 既有预研「已有/不做」判定汇总 +
  已知盲区（「未核实」清单）。**新预研专题（xx-port 类）开工前必须先对照本清单下
  「已有」判定，防止把 fork 已有设计当缺口带进来；预研完工后把新澄清的行回填进
  其 §3/§5。**教训背景：kimicode-port 曾误判 steer「无」、误把 sqlite-journal 当
  会话历史 journal。基线：2026-10-02 `main`。

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
  `build` 36402142384 双 job success、该用例转为 ok；详见同步文档"已收口"节
  ③ ~~34 处 i18n 丢失站点~~ 已收口（2026-10-02）：搬迁类在 `feat/local-sync-i18n-restore`
  新落点重放 `tr()`（五文件，登记 PATCHES.md），删除类核实站点已移除；五过滤 152 过 0 挂
  ④ ~~`upstream` 标记分支~~ 已建（2026-10-02，`f0e3be1`）并已并入 Handoff 摘要（见下），本条全部收口。

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
  **已收口（2026-10-02）**：T1/T2/T3 施工完成（分支 `feat/local-usage-quota-estimate`，
  实现记录见 PATCHES.md）；换周期（09-30 07:50）天然实验未采到——期间无客户端跑过
  会话，金额口径与 `/stats week` 联动**维持待验证**，等下次换周期恰有会话在跑再采。
  关键实测：`creditUsagePercent` 全整点、7%→76% 跳变非本机（本周账本 xAI 直连 0 调用）、
  `isUnifiedBillingUser=true`、`historyLen=0`。
  现场 QA（pct 已变仍报采样不足 = Δpct < 2 整点门，非无 xAI 消耗）见该文档末节，
  补 user-guide / 文案时复用。
- [`docs-local/usage/`](docs-local/usage/README.md) — **供应商套餐用量专题（已落地，
  随 `v1.0.46` 发版 2026-10-10；取数修复 + 单飞/缓存/显示分离重构随 `v1.0.47`
  2026-10-11，PATCHES 二十七期；活体核验待用户回填）**：用户已拍板——当前模型
  属于「推理同一把 SK 可查套餐用量」的供应商时 `/usage` 面板改显该供应商套餐量
  替代 SuperGrok 并标注来源、窗口粒度 5h>周>月、控制台账户/Cookie/AK-SK 类凭据
  一律不支持（火山 Ark / 百炼据此剔除）、缓存默认 5 分钟可配（`[provider_usage]
  cache_minutes`，单位分钟）。**首批五家全实现**：OpenCode Go + Command Code（点名）
  + 智谱 GLM / Kimi / MiniMax（拍板追加）；Cline / Token Unlimited 放弃。供应商判定
  = 模型 `base_url` host 匹配（无显式配置）。落点：新 crate
  `xai-grok-provider-usage`（纯函数适配器层，一家一文件 + 夹具单测）、shell
  `x.ai/providerUsage` 扩展（TTL 缓存 + GLM 裸key/Bearer 重试 + MiniMax 双路径）、
  pager `Effect::FetchProviderUsage` + usage_modal 供应商分流渲染。
  目录内：[`quota-endpoints-and-credentials.md`](docs-local/usage/quota-endpoints-and-credentials.md)
  （2026-09-27 调研：七家端点/凭据/百分比口径，含开源参照 MyTokenDashboard /
  cc-switch / OpenChamber / CodexBar 矩阵）、
  [`provider-quota-display-todo.md`](docs-local/usage/provider-quota-display-todo.md)
  （施工方案 + 实况：现状锚点 / 数据模型 / 分期进度 / 验收标准 / 剩余待定决策）。
- [`docs-local/status-line-perf-wiki.md`](docs-local/status-line-perf-wiki.md) — 状态行性能段
  （ttft/tps）专题 wiki：TTFT 三参考点对照、网关响应头行为分类（Command Code 早头 vs
  火山 Ark 晚头）、0ms 过滤×晚响应头的相互作用、unified.jsonl 取证 playbook、双 tps 口径。
  只收深挖节点，不复述可从代码/文档推出的内容。随修复 `feat/local-perf-ttft-request-anchor`
  （TTFT 参考点前移到请求发起）落地。
- [`docs-local/startup-perf/`](docs-local/startup-perf/README.md) — **grok2 启动耗时专题
  wiki（排查完成 2026-10-03，缓解待拍板）**：主因 = 模型目录阻塞拉取
  `startup.fetch_models_blocking`（缓存 TTL 仅 300s，冷启动 2.7–6.8s，热缓存
  ≈60–100ms；MCP init 异步不挡首帧）。含 `GROK_SPAN_PROFILE_OUT` + ptyctl 驱动
  真实 TUI 的可复现流程与已知坑（stable-ms 永不稳定、杀 keeper 连带 TUI 树丢画像）、
  机制锚点（fetch.rs / cache.rs / features.remote_fetch 开关，managed 层优先）。
  缓解两案：config 关 `features.remote_fetch`（BYOK 安全）或立分支改 TTL/异步化。
- [`docs-local/limit-inference.md`](docs-local/limit-inference.md) — **供应商 cache
  TTL / RPM / TPM 反推记录，T1/T2 已实现**（分支 `feat/local-limit-probe`）：sampler
  HTTP 层被动落盘每次请求的限流头/429 现场/前缀哈希/终端 cached_tokens 到
  `~/.grok/limit-probe/records.ndjson`，`scripts-local/limit-inference.py` 离线反推
  TTL 生存曲线与 429 滑窗限流夹逼。全被动零 token 成本；`GROK_LIMIT_PROBE=0` 关闭。
  T3 主动探测（idle sweep 补 >1h 长尾、多前缀容量测试）未做。
- [`docs-local/stats-modal-todo.md`](docs-local/stats-modal-todo.md) — `/stats` 加时间窗
  option（5h/day/week）+ 输出改成 Grok 自制窗口样式（对齐 agents / usage limit 面板）
  + TUI 残留英文说明 i18n 扫尾。分支 `feat/local-stats-modal-i18n`，基线 `86a6e1d`。
  **T1/T2/T3 全部施工完成（2026-10-02）**：T3 扫尾见 PATCHES.md 十六期（+157 组键、
  22 文件接线、豁免台账在案，扫描器 `scripts-local/i18n_sweep.py`；已 rebase 到
  10-02 main 重验证）。待办 = 活回合 `/lang zh` 抽查（发版后在新客户端看）。
  文档内含现状锚点、改动点、验收标准与纪律要求。
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
  选项与未验证项。**已修（2026-10-02，选项 C 流层方言归一化：`fix/local-responses-event-dialect`
  已合 main，登记 PATCHES.md，sampler 291/0）**。遗留：活体验证未做（需把受影响模型
  切回 `api_backend = "responses"` 实跑带推理请求；本机配置仍 chat_completions）；
  该网关其他非标事件名未穷举，归一化 miss 会打 warn 日志可发现。
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
  pi 特性移植预研（选型完成；P1 已落地，P2 T1 已落地、T2 未开工）**：对照 pi（earendil-works/pi，
  `0.84.0`–`v1.0.0`）近两月特性与 `xai-workflow` 现状，结论 = 编排原语不缺、真缺口三个
  （编排上下文膨胀 / 子 agent 工具面全量声明 / 成本）。两个选型特性已出设计原型：
  P1 [canonical-context-edit.md](docs-local/workflow-pi-port/canonical-context-edit.md)
  （journal 化 `context_edit` host call + prompt 构建层可见面，历史不动与重放确定性
  兼容）、P2 [deferred-tool-exposure.md](docs-local/workflow-pi-port/deferred-tool-exposure.md)
  （exposure 收敛三档 + `AgentOpts.tools/defer_tools` + `search_tools` 延迟声明，
  不碰重放语义）。**2026-10-02 已增补对照 pi `0.99.2`/`v1.0.0`（结论不变）**：P2 吸收
  pi 自身迭代的三条约束（描述静态化 #10212、`describeNamespace`/description 参与排名
  定型、resume 丢声明时序坑→新增验收 5）；codemode 瘦身/`generateImages` 不改变 D1
  判定。codemode 判定不移植（QuickJS 双运行时破坏 journal hash 确定性，决策 D1，
  只吸收按分支 KV / 大输出旁路两个思想）；cache warming 拟 P2' 独立专题。
  分支拟 P1 `feat/local-workflow-context-edit` / P2 `feat/local-deferred-tool-exposure`。
  基线：2026-09-30 `main` `95aad87`（pi 基线 `v1.0.0`，2026-10-01 发布）。
  **施工状态（2026-10-03）**：P1 已落地（`feat/local-workflow-context-edit`，
  replace_visible/hide_visible journal 直记，PATCHES 十九期）；P2 T1 工具面
  allowlist 已落地（`feat/local-deferred-tool-exposure`，PATCHES 二十期），
  T2 defer_tools + search_tools + 国模能力门未开工。
- [`docs-local/kimicode-port/`](docs-local/kimicode-port/README.md) — **kimicode
  （Kimi Code CLI）特性移植预研（选型完成；P1 rewind 分支树 undo 已落地）**：对照 MoonshotAI/kimi-code
  （main `21406fb`，2026-09-30）与 fork 现状。头号候选 = `/rewind` 从破坏性截断升级
  wire 分支树 undo；**2026-10-02 摸底已出设计原型
  [rewind-branch-undo.md](docs-local/kimicode-port/rewind-branch-undo.md)**：推翻"缺
  journal"前提——fork `updates.jsonl` 已是 append-only + `RewindMarker` 分支标记
  （sqlite-journal/session-events 均非会话历史，原判有误），D1 推荐 = RewindMarker
  SwitchEdge 化（非新建 journal），prompt 放回编辑器 fork 已有等价物，真差距 = 分支
  可往返 + 旧分支点可见 + 边界预计算；T1–T3 合计 5–7 人天，分支拟 `feat/local-rewind-branch-undo`。
  次选 = `select_tools` 延迟工具声明（并入 workflow-pi-port P2 对照定稿，公告流 +
  历史 schema 持续剥离两点待吸收）；再次 = 子 agent 结果信封（stop_reason→next_step
  映射 + resume 同会话）与委托图约束。hooks/插件市场/持久化/ACP 判定已有不移植
  （决策 D2–D5）。本地 clone `D:\CODE\ai\kimi-code`。基线：2026-10-01 `main` `2b8adae`。
  **施工状态（2026-10-03）**：P1 已按 rewind-branch-undo.md 落地 T1/T2a/T3
  （`feat/local-rewind-branch-undo`：SwitchEdge 化 + 分支面 picker + redo 通道；
  T2b replay 统一双路径未做，见 roadmap §6）；次选 select_tools 随
  workflow-pi-port P2 T2 另切片。
- [`docs-local/step-code-port/`](docs-local/step-code-port/survey.md) — **Step-Code
  （阶跃星辰 stepfun-ai/Step-Code）特性移植预研（调研完成；P1 勘误后实做
  headless continue 已落地）**：TS monorepo
  （MIT，573 星，本地 clone `D:\CODE\ai\Step-Code`，基线 `519e4de4` 2026-09-30）。
  头部候选：P1 bash 命令 AST 静态安全分析（三态判定 + `analysisIncomplete` 不冒充
  安全，fork 无命令内容静态分析，headless 安全基座）；P2 会话分支树 + 离开分支自动
  摘要（branch summarization + summary-overflow 降源，与 kimicode-port P1 同题互补，
  决策 D2 = 并入该专题对照定稿）；P3 不完整流恢复注入（projection-only 续作指令，
  ≤1 人天小件）；P4 子 agent env 防递归 + 输出驱动 idle watchdog（待核实 fork spawn
  链路，决策 D4）。fork 已有面（goal/plan/todo/MCP 导入/secret 脱敏/steer 机制）已
  在对照矩阵澄清防重复建设。分支拟 P1 `feat/local-shell-command-analysis`。
  **施工状态（2026-10-03）**：P1 经勘误收口——tree-sitter 静态分析上游
  2026-09-23 同步后已有（`xai-grok-workspace/src/permission/`），实做缺口只剩
  headless `--non-interactive-denial continue`（`feat/local-shell-command-analysis`，
  PATCHES 十八期）；P2 分支树随 kimicode P1 落地（branch summarization 未做）；
  P3/P4 未动。

- [`docs-local/issues/`](docs-local/issues/README.md) — **使用问题台账（一 issue 一文件）**。
  当前 open/reproduced 条：**无**。最近一条 **429/400 后状态行不再更新（P2）已修并随
  `v1.0.45` 发版（2026-10-10，merge `350874c`）**——根因 = 显示侧按「当前模型
  配置 id」精确取账本，而成功调用记在「上游回显 model id」、终止性失败（429/400）记在
  「配置 id」，两拼写不同（`glm-5.3-flash` vs 回显 `glm-5-3-flash`）时行冻死在只有 ✗ 的
  影子条目上（tokens/cache/think 段整段消失）。修复 P1（显示侧影子键守卫，逐候选判定 +
  叠回被跳过影子的 ✗）+ P2（失败归因改用最近回显 id），验证与遗留见
  [`issues/2026-10-11-status-line-stale-after-429-400.md`](docs-local/issues/2026-10-11-status-line-stale-after-429-400.md)
  文末「结论（二）」与 PATCHES.md 二十五期。

## 🔄 Handoff 摘要

### 供应商套餐用量上 /usage 面板 — v1.0.46 落地；取数修复+单飞重构随 v1.0.47（2026-10-11）

- **当前状态：** 五家供应商（OpenCode Go / Command Code / 智谱 GLM / Kimi / MiniMax）
  的套餐用量在 Usage limit 标签页替代 SuperGrok 块并标注来源；判定 = 模型 base_url
  host 匹配（无显式配置），缓存 `[provider_usage] cache_minutes` 默认 5 分钟。
  三层落点：新 crate `xai-grok-provider-usage`（纯函数适配器，一家一文件）、shell
  `x.ai/providerUsage` 扩展（TTL 缓存 + GLM 裸key/Bearer 重试 + MiniMax 双路径）、
  pager `Effect::FetchProviderUsage` + usage_modal 分流渲染。
  **v1.0.47（2026-10-11，merge `f91592d`，分支 fix/local-provider-usage-fetch）修复
  + 重构**：活体核验发现 Command Code 端点源站延迟尾部 >5s 击穿 5s 超时、body 读错
  被 `unwrap_or_default` 吞成 `invalid JSON: EOF`——修复 = 超时 15s + body 读错真实
  上报（err_chain 展开源链）+ 缓存/单飞改 **(provider, SK 指纹)** 键控（同 SK 跨模型
  共享缓存，同账号并发至多一个在途请求，用户拍板「锁带上 SK 才像话」）+ 触发端
  请求单发（在途期间重开面板不再发新请求）+ 回包无条件落 agent 级缓存（模态关闭
  不丢，按 model_id 绑定显示过滤）+ **stale-while-revalidate**（刷新中/失败显示旧
  数据 + 「查询于 HH:MM」，在途加「刷新中…」；用户拍板「TTL 到期先保证有显示」）。
  协议 A/B（httpx 冷连接 h1/h2 交错 5+5）：h1 mean 1749ms / h2 mean 1428ms、10/10
  成功、延迟由源站主导——协议不是耗时因素，保持 reqwest 默认。build run
  38078649203 / release run 38078649198 双 job success（约 27 分钟），4 资产非草稿；
  本机已部署，`grok2.exe --version` = `grok 1.0.47 (f91592d5cf5d)`，旧 v1.0.46 留
  `grok2.exe.v1.0.46.bak`。
- **验证：** provider-usage 54/54、pager usage_modal 38/38（新增 stale-while-revalidate
  用例）、GATE_FORCE shell provider_usage 5/5（新增单飞锁键共享 + 指纹确定性）、
  pager check --all-targets 0 error、i18n 扫描 0 挂。百炼单 key 查询 2026-10-10 复核
  官方 CLI 源码确认不支持（三域分离），不立项。
- **遗留：** ① 面板活体核验（用户人工继续：Command Code 模型开 /usage 对账探针
  `scripts-local/commandcode_usage.py`、刷新中旧数据显示、OpenCode Go 403 回退、
  缓存 debug 日志 `provider usage: cache hit`）② 解析形态偏差 → 修适配器走
  patch 发版，纯函数层改起来最快。
- **详情指针：** [`docs-local/usage/provider-quota-display-todo.md`](docs-local/usage/provider-quota-display-todo.md)
  （拍板 9–11 + 分期实况）；PATCHES.md 二十六 / 二十七期。

### 状态行 429/400 后冻结 — 已修并随 v1.0.45 发版（2026-10-10）

- **当前状态：** P1（显示侧影子键守卫 + 叠回被跳过影子的 ✗）+ P2（终止性失败归因改用
  「最近一次成功回显的 model id」）已合 main（merge `350874c`），随 tag **`v1.0.45`** 发版；
  build run `38037822957`（Linux 全量 + Windows）与 release run `38039766302`（Linux 原生 +
  Windows 交叉编译）均双 job success，release 页 4 资产，`grok2.exe --version` 实测
  `grok 1.0.45 (350874c36b81)`。
- **验证：** Linux CI 跑了 `cargo test -p xai-grok-shell`（Windows 本机跑不全的 shell 套件）；
  本机门控 `status_line` 18/0（含 `b68a036` / `c66d29e` 两个历史修复用例）、
  `xai-chat-state --lib` 399/0。
- **遗留：** ① 活体 TUI 复现（新客户端实跑"先 429/400 再成功"），并回答 perf 段是否也冻（未决项）；
  ② P3（拼写容忍匹配 / 客户端下送 catalog key）。
- **详情指针：** [`docs-local/issues/2026-10-11-status-line-stale-after-429-400.md`](docs-local/issues/2026-10-11-status-line-stale-after-429-400.md)
  文末「结论（二）」；PATCHES.md 二十五期。

### think-split-quoted-marker-fold — verified（preview.6 实测通过）

- **当前状态：** 修复已合并 main（`bc33d257`）、tag `v1.0.37-preview.6` 已发布；CI（release + build）已核对；
  **活回合验证已补完**：正向用例（行内代码引用 / 围栏）正文完整不折叠、真标记路径未误伤、落盘扫描 0 命中、
  回归 `ctest.sh -p xai-grok-sampler --lib` 272/272。客户端状态（2026-10-02 更新）：
  本机在用 `D:\tool-cli\grok2\grok2.exe` 已是 `1.0.40 (9117b5e6)`（v1.0.40 发版当班更新，含此修复与 /stats 窗口化 + i18n 扫尾），旧记录
  "`D:\tool\cli\grok2.exe` 仍是 preview.4 无此修复"已过时；路径与版本以
  `grok2 --version` 实测为准
- **关键证据：** 正文里被反引号引用的 `<think>` 曾被切分器当控制标记，把回答尾部改道 reasoning；
  判定签名＝正文 chunk 以反引号结尾 + 紧随 thought chunk 以反引号开头 + `<think>` 字面量两通道都缺
  （3 会话 6 处现场）；修复后 `ctest.sh -p xai-grok-sampler --lib` 272/272，mutation 下 5 例转红。
  ⚠️ CI 不跑 sampler 用例（只把它当依赖编译），见 handoff §8
- **验收标准：** 全部达标（release 4 资产 + `grok2.exe --version` = `1.0.37-preview.6`；build 双 job success；
  活回合正向 + 落盘扫描 + 回归三项证据见 handoff §5 / §13）
- **详情指针：** [`docs-local/archive/think-split-quoted-marker-fold.md`](docs-local/archive/think-split-quoted-marker-fold.md)
  （⚠️ 原 `.handoff/` 目录从未入库、已不存在，archive 内该文是唯一完整记录）

### upstream 标记分支 — 已建（下次上游同步的 diff 基准）

- **当前状态：** 本地分支 `upstream` 指向 `xai-org/grok-build` main `f0e3be1`（2026-09-23 快照），
  即上次同步（`v1.0.37`）的基线。
- **下次同步用法：** 同步前先把该分支快进到上游新 main（`git fetch upstream main:upstream`，
  免 checkout 的 fast-forward），再以
  `git diff upstream...main` 圈定本 fork 的本地改动面；冲突裁定与验证流程沿用
  [`docs-local/sync-upstream-2026-09-23.md`](docs-local/sync-upstream-2026-09-23.md) 的既定打法
  （52 冲突人工裁定、LOCAL 补丁按 PATCHES.md 重放、白名单 crate lib 测试门禁）。

### 遗留滚动推进第一批 + ④T2 开工 — v1.0.42（2026-10-04）

- **当前状态：** 随手件-流恢复注入（`feat/local-stream-recovery-injection`，
  PATCHES 二十一期：不完整流失败 → 下一轮用户 prompt 前置一次性恢复提示，
  sampling::error 29/29 + session-events 16/16）与 ④T2 设计定稿（三参照合并 +
  **用户拍板：声明模式可配置 direct/deferred，默认传统**）已合 main；**① T2b
  在 v1.0.42 Linux CI 红灯后暂退**（build run 37174064017：rewind_synthetic_turn
  族 6 用例挂——T2b"journal 即真相"锚点 vs 夹具空 journal 的语义冲突，代码三
  文件已还原，重做清单在 roadmap §6 台账，分支 feat/local-rewind-branch-undo
  保留全部 T2b 提交）。`v1.0.42` tag 重指至回退后 main（df7ef40）。
- **本地遗留：** ① T2b 夹具重做（6 夹具 journal+内存一致 + 两处计数语义推导 +
  WSL 验证）后重复合 main；④ T2 施工四切片（管道 / 能力门 / search_tools
  运行时+声明态+resume 原子性 / 面板 i18n，切片计划见 roadmap §6 开工条目）；
  ② headless 活跑 A/B；③ 活回合 token 曲线；随手件-子 agent 结果信封；
  活回合 TUI 抽查随新客户端。
- **详情指针：** [`docs-local/port-roadmap.md`](docs-local/port-roadmap.md) §6
  交接台账（2026-10-04 接手登记 + ①T2b 完成/回退两条目 + ④T2 开工条目）；
  PATCHES.md 二十一～二十二期。

### 移植路线图四特性统一发版 — v1.0.41（2026-10-03）

- **当前状态：** 任务①②③④ 已合 main 并随 `v1.0.41` 发版（tag 重指一次：Linux CI
  首跑暴露 ① redo 回放两处语义缺口——两用例名不含 "rewind" 此前从未被本机过滤
  覆盖，修复 `6e3c858`/`aae18d3` 后沿 v1.0.39 先例覆盖发版；release run
  37115871788 / build run 37115862185 双双 job success，4 资产，release 页已恢复
  发布）。客户端待更新到 1.0.41 做活回合抽查（/rewind 分支树 undo 等）。
- **本地遗留：** ① T2b（replay 重建统一双路径）；④ T2（defer_tools +
  search_tools + 国模能力门，开工前三参照合并定稿）；② headless 活跑 A/B；
  ③ 活回合 token 曲线；两随手件（流恢复注入 / 子 agent 结果信封）。
- **详情指针：** [`docs-local/port-roadmap.md`](docs-local/port-roadmap.md) §6
  交接台账（四任务交接条目 + CI 缺口修复 + 完成交接条目）；PATCHES.md rewind 节
  + 十八～二十期。

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
