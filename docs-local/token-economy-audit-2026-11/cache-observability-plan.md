# 缓存可观测性两件（命中率口径 + 断崖检测）— 施工计划

> 状态：待开工（2026-11-13 登记）。来源：kimi-code / zcode / codex 设计对照里
> 唯二负收益为零的候选，见
> [`cli-design-port-eval-2026-11-13.md`](cli-design-port-eval-2026-11-13.md) §2.1/§2.2。
> 基线：fork v1.0.74，本机 10 天 1910 次主回合、98.3% 命中、4.65M 未命中。

## 0. 为什么是这两件

稳态命中已到上限（98.3%），剩下 1.7% 未命中无法回答"是哪一类"：
首轮冷启动 597k（13%）之外，其余是新增内容还是中途断崖，账本里没有会话维度；
turn_summary/recap/compaction 这些 sidecar 根本不进账本。断崖一次约 $0.02（150k
上下文）+ 一次 TTFT 抖动，没有检测就只能等用户感知到卡顿或月底对账。

两件都不改请求字节、不改模型看到的内容，纯观测层。

## 1. P1：账本加 `sessionId` + `purpose`（命中率口径）

### 现状锚点

- `ModelCallSample` 字段：`tsUnixMs / modelId / promptTokens / completionTokens /
  cachedPromptTokens / cacheCreationTokens / reasoningTokens / ttftMs / tps /
  durationMs`（`xai-grok-tools/src/model_usage_ledger.rs`）。
- 只有主回合循环追加：`xai-grok-shell/src/session/acp_session_impl/turn.rs:3489`。
- sidecar（turn_summary / recap / compaction / title / btw）走
  `side_call` / 各自 `conversation_collect`，不写账本，也没有 purpose 标记。
- 会话级汇总另有 `sessions/<id>/usage.json`（有 sessionId、按 turn 聚合），
  但 `/stats` 的 5h/周/月视图读的是 `model-usage.jsonl`。

### 改动

1. `ModelCallSample` 增两个可选字段：`session_id: Option<String>`、
   `purpose: Option<String>`（`main` / `recap` / `turn_summary` / `compaction`
   / `title` / `btw` / `other`）。旧行缺字段按 `unknown` 处理，读取侧向后兼容。
2. 主回合追加点填 `session_id` + `purpose=main`。
3. sidecar 收口：在 `side_call` 与 turn_summary / compaction 的采样回包处补
   `append_sample`（与主回合同一函数，`purpose` 取调用方标记），避免每处各写一份。
4. pager `/stats`：现有聚合加维度——按会话、按 purpose 拆；保留全局视图为默认。

### 验收

- 同一会话的 `main` 与 `turn_summary`/`recap` 调用在 `/stats` 可分列；
- 任取一轮命中跌到 <50% 的回合，能定位到具体会话、轮次与 purpose；
- 旧账本文件（无新字段）读取不报错，聚合结果与升级前一致；
- `cargo check -p xai-grok-tools -p xai-grok-pager --all-targets` 0 error，
  ledger 单测覆盖"旧行兼容 + 新行解析"。

### 风险

无（只加字段与聚合维度）。注意 `/stats` 面板宽度与 i18n 文案。

## 2. P1 断崖检测 + P2 闲置过期提示

### 2.1 断崖检测（P1）

判定（纯函数，放 shell 侧，便于单测）：

- 输入：本会话上一轮与本轮的 `promptTokens` / `cachedPromptTokens`、上下文大小、
  `tools_hash` / `msgs_hash`（limit-probe 已有同款 FNV 前缀哈希）、模型 id、
  本轮是否发生 compaction / 模型切换 / plan 模式切换。
- 触发：上一轮命中率 ≥ 0.9 且本轮 `cached` 绝对值下降 ≥ 20k（或命中率下降
  ≥ 30 个百分点），且不属于已知原因（模型切换、compaction、首次请求）。
- 输出：unified 事件 `session.cache_break`，带 `cached_before/after`、
  `prompt`、`tools_hash`、`msgs_hash`、候选原因、上下文大小；`/stats` 汇总
  "近 N 天断崖次数/浪费 token"。

实现落点：

- 进程内每会话保留最近一轮的 `(prompt, cached, hashes)`（小 ring，不落盘）；
- 事件写 unified.jsonl（与 `model catalog: priced` 同族），不弹 UI；
- limit-probe 维持 opt-in，检测器不依赖它（哈希在 shell 侧自算或从 sampler
  回包附件带出）。

验收：

- 人为构造（换模型 / 改工具集 / 压缩）时，事件恰好触发一次并标出原因；
- 正常轮次与首轮不触发；
- 一次真实断崖能在 `unified.jsonl` + `/stats` 两处对上。

风险：误报。用"已知原因白名单 + 阈值"压住；首版只记事件，不做任何自动动作。

### 2.2 闲置过期提示（P2）

- 规则（对齐 kimi `evaluateCacheHint`）：空闲时间 > 该模型 cache TTL 且上下文
  ≥ `min_tokens_to_hint` 时提示；resume 后与提交前两个时机。
- TTL/阈值表按供应商：DS 隐式缓存给保守值（如 1h / 100k），昂贵模型单独配。
- 门控：预估差额 = 上下文 × (未命中单价 − 命中单价)；低于阈值（如 $0.05）不提示，
  DS 默认静默；用户可 "don't ask again"。
- 验收：提示只在跨过 TTL 且上下文达标时出现；拒绝/关闭后本会话不再弹；
  不影响发送路径（可取消并保留输入）。

## 3. 依赖与顺序

1. 先做 §1（账本字段）——§2.1 的会话维度与 `/stats` 汇总直接复用。
2. 再做 §2.1（检测器），首版只记事件。
3. §2.2 最后做，按单价门控，默认不打扰 DS。

## 4. 不做

- 不做自动重发/自动压缩等反应式动作（先把观测做准）；
- 不把 limit-probe 改成默认常开（磁盘与噪声成本）；
- 不在请求里加任何新字段（观测层零请求变更）。

## 5. 锚点

- `xai-grok-tools/src/model_usage_ledger.rs`（样本结构与聚合）
- `xai-grok-shell/src/session/acp_session_impl/turn.rs:3489`（唯一追加点）
- `xai-grok-shell/src/session/acp_session_impl/side_call.rs`、
  `turn_summary.rs`、`compaction.rs`（sidecar 回包处）
- `xai-grok-sampler/src/limit_probe.rs`（前缀哈希与被动记录，opt-in）
- kimi：`apps/kimi-code/src/tui/controllers/cache-hint-controller.ts`、
  `apps/kimi-code/src/tui/utils/cache-hint.ts`
- zcode：`docs/context-cache-hit-rate.md`、`core/src/compact/policy.ts`
