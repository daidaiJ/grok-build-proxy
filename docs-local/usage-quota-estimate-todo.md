# `/usage` 周额度 token 估算（TODO）

> **状态（2026-10-02 收口）：T1/T2/T3 施工完成并收口**（分支 `feat/local-usage-quota-estimate`，
> 实现记录见 `docs-local/PATCHES.md` 的「/usage 周额度估算」一节）。基线 `97252c5`，
> 数据源与锚点均已实测验证（本机 unified.jsonl 93 条计费采样 + model-usage.jsonl
> 1468 条调用）。金额口径（待定决策 1）与 `/stats week` 联动（待定决策 3）**维持待验证**——
> 原"等 2026-09-29 23:50 UTC 换周期实测拍板"的条件已过（见下），不再挂起本专题。
> 现场 QA（补用户文档素材）见文末 **§现场 QA**。
>
> **收口（2026-10-02）**：换周期实测**未采到**——unified.jsonl 最后一条 billing fetch
> 停在 2026-09-27T11:55Z（preview.11），换周期（09-29 23:50 UTC）前后无任何客户端
> 跑过会话，`billing-samples.jsonl` 也停在 09-27 19:55（39 条）。金额口径维持
> **辅助路径、待验证**：等下一次自然换周期时恰有会话在跑（或用户主动实跑一次）
> 再看 historyLen 是否 0→≥1；`/stats week` 联动（决策 3）同样搁置，等金额口径
> 有数据后再一并定。功能本体（token 反推下界）已实装，不再阻塞。

## 一句话

服务端只给周用量百分比（不给绝对 token 数），用「本机 xAI 直连 token 消耗 ÷ 同期
Δpct」反推周额度下界并展示在 Usage limit 面板；多设备场景下该值是**严格下界**，
各机取 max，金额口径（history.included_used）作为跨设备精确解的后备路线。

## 需求与约束

- `/usage` 的 Usage limit tab 现在只有两块：账号额度百分比条 + 本会话 token 小计。
  用户想知道「每周额度大概多少 token」。
- **多设备约束（用户明确要求）**：pct 是账号级、全产品面的；本机账本只是一台设备的
  份额。任何单机推断都必须按"下界"呈现，否则会系统性低估额度。

## 实测取证（2026-09-27，本机数据）

### 服务端实际下发什么

unified.jsonl 里 `billing: fetched credits config` 共 93 条（shell 每次 billing fetch
落一条，见锚点 §锚点-5）。结构化事实：

| 字段 | 实测值 | 含义 |
| --- | --- | --- |
| `creditUsagePercent` | **全整点**（1/2/5/6/7/26/76/77/78） | 服务端量化到整数，Δpct 最小粒度 1 个百分点 |
| `currentPeriod` | `WEEKLY`，start/end RFC3339（本窗 09-22 23:50 → 09-29 23:50 UTC） | 周期切片有锚点，采样与账本按 period.start 切 |
| `isUnifiedBillingUser` | `true` | 统一消费计费池：pct 覆盖 grok.com 网页/App/Build 全部面 |
| `historyLen` | `0` | 无往期汇总；`history[].included_used`（分）是否存在、当前周期是否入 history 均未验证 |
| `monthlyLimit` / `used` | 缺失 | 旧绝对值字段不给 → 反推是真需求，不是 UI 偷懒 |
| `subscriptionTier` | `SuperGrok` | 面板标题已用 |

pct 轨迹（相邻不同值）：旧周期 09-22 15:48 = 26% → 周期切换清零 → 09-23 12:31 = 1%
→ 09-24 15:05 = 7% → **09-25 11:14 = 76%** → 09-26 = 78%。

### 关键事实：7%→76% 的跳变不是本机干的

model-usage.jsonl（09-22 起）1468 条调用**全部**是第三方模型（deepseek-v4.1-flash 走
Command Code 网关、glm-5.3-flash、xiaomi/mimo），**xAI 直连调用为 0**。即：本机本周
对 pct 的贡献 = 0，69 个点的消耗来自账号其他面（grok.com 网页/App、其他设备、并行
agent 等）。推论：

1. 对该用户当前用法，纯本机反推**基本无效**（Δtokens_xai ≈ 0），UI 必须有诚实的
   "本机无 xAI 消耗，无法反推" 状态，不能输出 0 额度这种垃圾值；
2. pct 的账本里第三方模型 token 必须剔除（不剔除会把 deepseek 的 2 亿 token 算进
   分子，输出荒谬额度）；
3. 跨设备精确解只能走金额口径（§金额口径）或等 WebDAV 同步（另一 TODO）合并多机样本。

## 数学模型

### 下界定理

周期内服务端消耗单调不减 ⇒ pct 单调不减。取样本对 (t₀,p₀),(t₁,p₁)（p₁>p₀，同周期），
配对同窗口本机 xAI 直连 token 增量 ΔT：

    Q̂ = ΔT / (p₁ - p₀) × 100

若其他设备同窗消耗 ΔU_other ≥ 0，则 Q̂ = Q · ΔT/(ΔT+ΔU_other) ≤ Q，故 **Q̂ 是下界**。
取所有可配对样本对的 **max Q̂**（最紧下界；本机份额越高的窗口越紧）。

### 质量门（缺一不可，否则显示"无法反推"）

- 可配对样本对里 max Δpct ≥ 2 个整点（整点量化下 1 个点的噪声占比过大）；
- ΔT > 0（本机窗口内确有 xAI 直连消耗）；
- 样本 ≥ 3 条且跨 ≥ 2 个不同 pct 值；
- 样本与 ΔT 同属一个 `currentPeriod.start`（跨周期直接丢弃，pct 会清零）。

### 金额口径（跨设备正解，待验证）

`history[]` 有 `included_used`（Cent）。若换周期后当前/刚结束周期入 history，且周期
末端有 pct 采样 p_end（消耗单调 ⇒ 最后一条采样即期末值），则：

    allowance_cents ≈ included_used / p_end × 100      （仅当 pct == included_used/allowance 线性成立）

金额是服务端记账、天然跨设备。再除以本机观测的 $/Mtok（ledger 的 `cost_usd_ticks`，
服务端单次报价）折算 token 当量。**验证点：2026-09-29 23:50 UTC 换周期后看
historyLen 是否 0→≥1、included_used 是否非空**——这是一次免费的天然实验，T2 是否
包含金额口径在此之前不拍板。

### 本机燃速投影（副产品）

本周已耗 tokens_xai / Q̂ → 「按本机当前速率，期末约再贡献 N%」。多设备下同样偏保守，
标注口径即可。

## 分子口径（哪些 token 动 pct）

- 只统计 **xAI 直连模型**：config `[model.<id>]` 的 `base_url` 缺省或指向官方端点
  （默认 `https://api.x.ai/v1`）；任何自定义网关（api.commandcode.ai 等）剔除。
- ledger 的 `TokenUsage.prompt_tokens` 恒为全量（未缓存 + cache 读 + cache 写，
  `xai-grok-sampling-types/src/conversation.rs:730` 注释明示"do not subtract"），
  分子 = prompt + completion，与服务端"处理过的 token"口径对齐（cache 读是否 1:1
  计入 pct 正是反推要回答的问题之一，不要预先加权）。
- 失败/中断调用本地不记（`UsageLedger::record_main_loop_failure` 零 token），服务端
  照算 → 又一个下界偏低的来源，文档里写明即可，不做补偿。

## 现状锚点

1. `crates/codegen/xai-grok-shell/src/extensions/billing.rs:60` — `BillingConfig`
   （`credit_usage_percent: f64`、`current_period`、`history`）；:32 `UsagePeriod`；
   :44 `BillingPeriodUsage`（`included_used/on_demand_used/total_used` Cent）。
2. `crates/codegen/xai-grok-shell/src/extensions/billing.rs:149` —
   `billing_unified_log_ctx`：每次成功 fetch 写 unified.jsonl（历史轨迹就是从这来的）。
3. fetch 触发点：`Effect::FetchBilling`（`xai-grok-pager/src/app/event_loop.rs:2916`、
   `effects/mod.rs:4803` 执行；dispatch 测试证实 **turn 结束静默拉一次**）→
   T1 只要在 fetch 成功处追加落盘，无需新增轮询。
4. `crates/codegen/xai-grok-pager/src/app/effects/helpers.rs:1359` —
   `credit_balance_from_config`（pct→`CreditBalance` 的唯一汇合点，T3 展示层挂这里
   旁路）。
5. `crates/codegen/xai-grok-pager/src/views/usage_modal.rs:877` `usage_limit_lines` /
   :929 `allowance_lines` — Usage limit tab 渲染，T3 的插入点。
6. `/stats` 数据源：`grok_home/cache/model-usage.jsonl`，读写经
   `xai-grok-tools/src/model_usage_ledger.rs`（`load_samples/aggregate`，
   Window 常量 :15-17）；条目含 `tsUnixMs/modelId/promptTokens/completionTokens/
   cachedPromptTokens/cost?`——T2 的分子来源。
7. config 模型表：`[model.<id>]` 块带 `base_url`（`xai-grok-config/src/
   config_override.rs:103` 的 overlay 面板可见），官方默认 `https://api.x.ai/v1`。
8. i18n：`usage_modal.rs` 全部文案走 `tr()`，中英表在
   `xai-grok-pager/src/slash/i18n.rs:1146` 一带。

## 改动点

### T1：billing 采样落盘（独立可先行，~半天）

- shell 的 billing fetch 成功路径（锚点 2 旁）追加一行到
  `grok_home/cache/billing-samples.jsonl`：
  `{tsMs, pct, periodStart, periodEnd, periodType, tier, historyLen, latestHistory}`；
- 去重：同 pct 且距上一条 < 60s 跳过；轮转沿用 limit-probe 的 32 MiB 方案；
  `GROK_BILLING_SAMPLES=0` 关闭；
- 失败的 fetch 不落盘（避免把网络错误当 0% 采进去）。
- 验收：跑一天会话，samples 行数 ≈ billing unified 日志条数（去重后）；
  既有 crate lib 测试不回归。

### T2：反推内核（纯函数 + 单测，~1 天）

- 新模块放 `xai-grok-tools`（与 `model_usage_ledger` 同居，账本读取现成）：
  `quota_estimate::estimate(samples, ledger_rows, xai_model_ids) -> EstimateResult`；
- 输入过滤：modelId ∈ xAI 直连集合（T3 从 config 读 `[model.*].base_url` 求出）；
- 实现 §数学模型 全部规则（max-over-pairs、质量门、周期切片）+ 金额口径 feature
  gate（history 数据可用时启用）；
- 输出枚举而非 f64：`Tight{tokens, basis_pair_ts}` / `Loose{...}` / `NoData(reason)`，
  reason 区分"本机无 xAI 消耗"/"样本不足"/"跨周期"；
- 单测覆盖：单调性、跨周期清零、整点量化门、多设备稀释（构造 ΔU_other 验证 ≤ Q）、
  第三方模型剔除、NoData 各分支。
- 验收：`ctest.sh -p xai-grok-tools --lib` 全绿 + 用本机真实 93 条采样 + 账本跑
  冒烟（预期输出 NoData("本机无 xAI 消耗")——这本身就是验收）。

### T3：UI 接线（~半天）

- Usage limit tab 在额度条下方加「额度估算」块：
  下界值（`≥ ~X.X M tokens/周`）+ 口径徽标（"本机口径"）+ 样本对时间戳
  + 多设备提示文案（"其他设备的消耗未计入；多设备请以各机最大值为准"）；
  NoData 时显示原因而不是留白；
- `/stats week` 是否联动（另加一行换算率）→ 见待定决策 3；
- 文案进 i18n 中英两表；极简模式文本报表加同源一行。

## 验收标准（整体）

1. 单设备重度使用 xAI 模型的账号：跑 2-3 天后 Usage limit tab 出现非 NoData 的
   下界估算，且量级与 grok.com 用量页可对账（±50% 内）；
2. 本机（当前状态：xAI 直连 0 调用）显示 `NoData("本机无 xAI 消耗")`，不输出数字；
3. 周期切换边界：period.start 变化后旧样本不参与估算；
4. 关闭开关后零落盘、零 UI 变化；
5. `ctest.sh -p xai-grok-tools --lib` 与 `-p xai-grok-pager --lib`（涉改模块）全绿。

## 待定决策（拍板点）

1. **T2 是否包含金额口径**：等 2026-09-29 换周期 historyLen 是否 0→≥1 的实测结果；
   若 included_used 存在，金额口径升级为主路径，token 反推降级为辅助。
2. **UI 位置**：Usage limit tab 加块（推荐，与数据同屏）vs 独立第 4 tab（`/stats`
   已占 4 tab 先例）vs 两处都上。
3. **`/stats week` 是否联动**：加"每 1% ≈ N tokens"换算行对拍板（低成本）还是保持
   纯本机口径不动（口径纯粹性）。
4. **xAI 直连域集合**：只认 base_url 缺省 + `api.x.ai`，还是允许用户在 config 显式
   标记某网关模型"计入额度"（第三方网关转售 Grok 配额的场景，如 Command Code——
   若用户的 pct 实际是被这种网关消耗的，本机口径反而能对上账）。
5. **采样增强**：T1 只挂现有 fetch 点（turn 结束），是否再加会话启动强制拉一次
   billing（覆盖"打开就问"的场景）。

## 与既有 TODO 的关系

- `webdav-sync-todo.md`：多设备合并样本（各机 samples+ledger 同步后取全局 max）
  是 WebDAV T2 白名单的一个天然候选数据类；本文档不依赖它，但设计上把
  samples 文件做成"可同步的单机追加日志"（device 字段预留）。
- `limit-inference.md`：同一 ndjson 取证家族；若 xAI 响应头未来出现用量类头，
  limit-probe 的 `req` 行会先看到，届时每请求 pct 采样可替代 billing 轮询。

## 现场 QA（补文档素材）

> 只收用户可见症状 ↔ 内核原因，以及补 user-guide / 文案时容易写错的口径。
> 公式、质量门清单、分子口径见上文，这里不复述。

### Q1. 额度百分比已经变了，为什么还显示「计费采样不足」？

**短答（可直接搬）：** 额度条读的是**最新一次** billing 的 `creditUsagePercent`；
估算读的是 `~/.grok/cache/billing-samples.jsonl` 里的样本对。质量门要求配对
**Δpct ≥ 2 个整点**。1 个点的跳动够画条、不够当反推分母。

**文案陷阱：** `NoData(InsufficientSamples)` 的中文是
「计费采样不足，继续使用以积累。」（英键 `Not enough billing samples yet; keep using to accumulate.`）。
用户口语「数据太少估算不了」即此条。这个枚举把三种失败合成一句：

1. 本周期采样 < 3；
2. 不同 pct 值 < 2；
3. 有不同 pct，但最大 Δpct < 2（**本问命中**）。

与 `NoLocalUsage`（「窗口内本机无 xAI 直连消耗，无法反推。」）不是同一条路。
无采样文件时 `compute_quota_estimate` 返回 `None`，估算块**整段不渲染**——
那是第三种 UI，也不是这句文案。

**为何 1% 必须丢掉（可搬进用户文档）：** 服务端 pct 整点量化。显示 79→80 时，
真实增量落在 `(0, 2]`；分母接近 0 时 Q̂ 无上界。Δpct ≥ 2 才能保证分母至少约
1 个真点。

**现场（2026-09-27，功能上线当天、本机）：**

| 项 | 值 |
| --- | --- |
| `billing-samples.jsonl` | 6 条，同周期 `2026-09-22T23:50:39Z` |
| pct 轨迹 | 13:23 79% → 13:30 80%，其后同 80% 再采 3 次 |
| 最大 Δpct | **1.0** → 全部样本对被 `MIN_PAIR_DELTA_PCT=2.0` 丢弃 |
| 同窗账本 | `grok-4.6`（config 无 `base_url` → 当直连）≈ 1.86M token；第三方 0 |
| `historyLen` | 0 |

所以这次**不是**无 xAI 消耗。相对已落盘的 79%，再涨到 **81%** 就会出下界数字。

### Q2. 本周前面 0→79 的历史为什么没用上？

`billing-samples.jsonl` 从 T1 落地后才写，**不回填** `unified.jsonl` 里的 billing
轨迹。估算基线 = 升级后第一条采样（本机是 79%），不是周期起点。周期
`period.start` 一切换，旧样本作废，新周期重新积满 ≥3 条 + Δpct ≥ 2。

### Q3. 补用户文档时建议怎么写 / 文案待改

- 写清两套数据：额度条 = 实时 pct；估算 = 采样对 + 本机 xAI 直连 token。
- 写清门槛：「百分比至少再变动 2 个点」比「继续使用以积累」准确。
- 写清分子：只计缺省 `base_url` 或 `*.x.ai`；网关模型（Command Code / Ark 等）
  即使把 pct 推上去也不进 ΔT。
- **未改代码的文案债：** 把 InsufficientSamples 的三种失败拆开显示，至少把
  「Δpct < 2」说成「用量百分比至少再变 2 个点」，避免「百分比都动了还说采样不足」。
