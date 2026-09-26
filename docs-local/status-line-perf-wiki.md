# 状态行性能段（ttft/tps）专题 wiki —— 深挖节点与事实链

> 定位：只收**从代码或既有文档读不出来、必须跨层取证/实测才能得出**的关键节点。
> 入门与配置见 `xai-grok-pager/docs/user-guide/25-status-line.md`、`29-local-enhancements.md`；
> 改动台账见 `PATCHES.md`（4e27104 → 8e049b6 → 本期 request-anchor）。本文不重复三者。

## 1. TTFT 有三个参考点，互不相等

| 参考点 | 定义 | 起点 | 消费方 |
|---|---|---|---|
| 用户感知 TTFT | 请求发出 → 首个可见 token | `inference_start`（shell turn.rs model_timer） | pager `waiting_model` 阶段时长 |
| metrics 层 ttft | 请求发起 → 首个输出 chunk | `run_one_attempt` 顶部（每次物理尝试） | signals → 状态行 perf、`/stats`、turn delta、feedback |
| span 层 `http.*.ttft` | 响应头 → 首个内容 chunk | `HoldUntilContentStream.returned_at`（响应头后） | 仅 tracing/profiler，`response_headers_ms` 紧邻可补差 |

- 修复（2026-09-26 request-anchor）前，metrics 层与 span 层**同界**（`stream_start` 取于
  L2 流首次 poll，而 poll 前 `conversation_stream().await` 已等完响应头）——这是 Ark 全零
  的机制根源。修复后 metrics 层前移到请求发起，span 层**有意不动**（配 `response_headers_ms`
  可自行拼装），两者从此不等，勿混用。
- TTLB/ITL 维持 stream_start 参考（投递窗口口径连续），因此 ttft 可能大于 ttlb
  （晚响应头网关 + 快流）——两者无不变量约束，不是 bug。

## 2. 网关响应头行为分类（决定 ttft 可测性）

| 网关 | 响应头时机 | 头后段 ttft | 状态行表现 |
|---|---|---|---|
| Command Code（api.commandcode.ai） | 早发（先头后 token） | ≈ 真实首 token 延迟（400–600ms） | 正常显示 |
| 火山 Ark（ark.cn-beijing.volces.com coding） | 首 token 就绪才发，headers+首 SSE 同 flush | 恒 <1ms | 修复前全零隐藏 |

事实链（`01a0ddae` loop 31，preview.6，glm-5.3-flash）：`inference_start` 37.223 →
`waiting_model` phase 4731ms → thinking 42.010 → done 50.611（model_elapsed 13386）。
用户感知 TTFT ≈ 4.7s 全部落在旧参考点之前，实测 ttft=Some(0)。
**推论**：凡"headers 前置"的代理/网关，头后段测量即真实值；凡"首 token 前置 flush"的
直连推理端，头后段测量必塌缩为 0。同一二进制、同一天内切网关即可复现/消失——
**这不是版本回归，是网关差异**（model id 修复 7804d5e / limit-probe bd1f7a4 与症状
同时段落地纯属时间相关；且两提交分别只动 display_name 覆盖与旁路日志）。

## 3. 0ms 过滤 × 晚响应头的相互作用（踩坑核心）

- 过滤器（8e049b6）存在的理由：0ms 样本会稀释会话均值，且旧门槛 `avg>0` 在全零会话
  永假 → 整段永不显示。保留至今，作真伪影守卫。
- 晚响应头网关上的 0ms **不是"无数据"**，而是"真实窗口被参考点吃掉"——过滤只是把
  症状（perf 段隐藏）暴露出来。
- 历史解读修正：8e049b6 时"82% 采样 None/0 = 整体吐包"对 Ark 不准确。**判别法**：
  ttft=0 且 thinking 阶段持续数秒 → 晚响应头（流是活的）；ttft=0 且无任何 phase 推进
  → 真缓冲/整体吐包。两者处置完全不同。

## 4. unified.jsonl 取证 playbook

字段与关系（均为同一会话 sid 下按 ts 排序）：
- `shell.turn.inference_start` / `inference_done`：后者 `model_elapsed_ms` = 全窗口
  （submit→完成，含 drain barrier），`ttft_ms` = metrics 层原值（未过滤，可为 0），
  `tokens_per_sec` = output/model_elapsed（见 §6）。
- `turn.phase_transition`（pager 侧）：`waiting_model→thinking` 的 `phase_elapsed_ms`
  ≈ 用户感知 TTFT；`thinking→responding` 的 ≈ reasoning 流总时长。
- `itl_p50_ms=0`：亚毫秒级 chunk 突发（快模型宏分块），**正常**，勿当故障信号追。
- 时间线还原公式：真实 TTFT ≈ waiting_model 时长；投递时长 ≈ model_elapsed − 真实 TTFT
  （粗略，含 drain 开销）。

分桶纪律：**按 会话×模型 分桶，不按版本分桶**。同一版本内换网关即反转症状；
反之先核对日志 `ver` 字段与安装二进制 `--version`，"刚提交 X 后 Y 坏了"的归因
必须先排除"二进制根本没包含 X"（本次事故：症状归到 preview.7/8 的提交，实际运行
preview.6，且 Ark 全零早于提交两天——preview.4 时代 09-23 已 83 连零）。

## 5. perf 段门控链速查（跨 crate 联动）

`build_turn_perf`（shell status_line.rs）任一环 None ⇒ perf=None ⇒ segments.rs 整段省略：
1. `signals.snapshot()` None（无快照）；
2. `get_last_turn_usage()` None（本会话尚无成功调用）；
3. `signals.last_time_to_first_token_ms` None（**0ms 过滤**，§3）——此环消失则
   tps 一起消失（tps = output/(last_api−ttft)，8e049b6 的窗口自洽约束）。
另：`api_calls/tokens/cache/think` 段走 usage ledger（受 provider-qualified key
scoping 影响，b68a036 引入、c66d29e 修复），与 perf 段**无共享数据源**——两段故障
表现同为"行变短"，需按 §4 分桶后区分。

双 tps 口径并存（数字必然不同，勿互相对质）：日志 `tokens_per_sec` 全窗口 vs 状态行
tps 流式窗口；差值 = ttft/模型窗口比。

## 6. 修复后语义速记（request-anchor，2026-09-26）

- ttft = 最近一个**物理请求**发起 → 首 token；retry 每次尝试独立取锚，失败尝试的样本
  随其丢弃，不计入。
- 锚点含请求序列化与 bearer 准备（实测量级 1–5ms，对 300ms–5s 的值可忽略）；不含
  sampling gate/permit 等待（在 submit 之前，与 model_timer 同界排除）。
- tps 分母 `last_turn_api_duration_ms` = model_elapsed（submit→完成），窗口 =
  model_elapsed − ttft ≈ 流式窗口；两者同族时钟，配对成立。
- 涉及文件（排查此类问题先 diff 这条链）：sampler 三流 + `metrics.rs` +
  `actor/request_task.rs` + `client.rs conversation_collect*` → shell `signals.rs` +
  `sampler_turn.rs(record_response_token_usage)` → `acp_session_impl/status_line.rs` →
  pager `segments.rs`。
