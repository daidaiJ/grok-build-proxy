# 压缩失败根因：DeepSeek v4.1 Flash 的两起独立故障（2026-09-23 / 2026-10-19）

> **状态**：两起均已修复并随版本发出；本文是合并后的根因记录与遗留清单。
> **整理日期**：2026-10-22。事件 A 发生在 09-23，事件 B 在 10-19。
> **关联**：PATCHES.md「压缩请求半答并行 tool call 回填」节（事件 A）与五十四期
> （事件 B 的重试对齐）；事件 A 的原始排查记录在
> [`archive/compaction-dangling-toolcall-400.md`](archive/compaction-dangling-toolcall-400.md)。

用户最初的描述是「压缩时报 400，说内容不完全/不足」。实际是两件事：09-23 是真 400，
10-19 是传输中断。两者根因不同、修法不同，只有共同点是对严格上游（DeepSeek）暴露。

## 结论速览

| | 事件 A | 事件 B |
|---|---|---|
| 时间 | 2026-09-23T12:31:45Z | 2026-10-19T13:03:12Z |
| 模型 / 网关 | `deepseek/deepseek-v4.1-flash` via Command Code | 同左 |
| 用户可见错误 | `API error (status 400 Bad Request): … insufficient tool messages following tool_calls message` | `compact failed: reqwest error stream: Transport error: error decoding response body` |
| 根因层 | 请求内容：半答的并行 tool_calls 未回填 | 传输 + 恢复策略：网关链路抖动，压缩重试阶梯太弱 |
| 修复 | `b40cb95`（`v1.0.37-preview.5`） | `fc0e899`（`v1.0.66`） |
| 状态 | 已修，未复发 | 已修，未做活体复验 |

## 事件 A：2026-09-23 的 400 —— 半答并行工具调用未回填

### 现场

会话 `01a0c990-fc38-7491-9306-07ca6ef1e5f5`（工作区 `ddmode`）的压缩工件
`compaction_requests/aaa5a961-….json`：manual 触发、439 条输入、26 个工具定义、
`attempts = 1`、`outcome = deterministic`。历史尾部：

| idx | 项 | 内容 |
|---|---|---|
| 436 | `assistant` | `tool_calls = [call_00_R81IvQT8o0VpgOxhtxl01556, call_01_eEW4XiQhz0IK8CiPIssm8288]` |
| 437 | `tool_result` | 只有 `call_00` 的结果 |
| 438 | `user` | 压缩指令本体 |

即「并行两个工具调用，只有一个结果落地」。同一会话的 `updates.jsonl` 印证成因：
`call_01` 只有 `tool_call` 与 `tool_call_update(kind=execute)`，没有 completed、没有
失败事件、也没有合成结果——回合在第二个工具仍在飞时被中止，留下半答 run。

### 根因链

1. 正常模型请求走 `ChatStateCommand::BuildConversationRequest` →
   `ensure_conversation_integrity()`（`xai-chat-state/src/actor/mod.rs:339` →
   `actor/mutations.rs:86`），会为未答的调用插入合成 `ToolResult`，所以**普通回合不挂**。
2. 压缩走 `GetConversation` 纯读路径。该路径按设计不在读处理器里修复——并发工具执行
   会被误判为 dangling（`actor/mutations.rs:85` 注释）。
3. 压缩前的历史整理 `prepare_conversation_for_verbatim_summarization`
   只调用 `truncate_trailing_incomplete_tool_call`（`compaction_utils.rs:123`），
   而它只处理「末项就是带 tool_calls 的 assistant」。本例末项是
   `tool_result` + `user`，函数不触发 → 半答 run 原样上车。

### 为什么只有 DeepSeek 报错

同一份 439 条输入在 12:32:21Z 换 `xiaomi/mimo-v2.6-flash` 重试即成功。最小复现探针
（直接 POST Command Code 网关，需浏览器样式 UA，否则 Cloudflare 403 code 1010）：

| 用例 | messages | 结果 |
|---|---|---|
| A | user → assistant(2 calls) → tool(c1) → user | **HTTP 400** |
| B | user → assistant(2 calls) → tool(c1) → tool(c2) → user | HTTP 200 |
| C | user → assistant(1 call) → tool(c1) → user | HTTP 200 |

结论：这是上游严格度差异，不是本地历史内容损坏；补齐回包即通过。

### 修复

`b40cb95`（merge `b717099`，tag `v1.0.37-preview.5`）在 prep 末尾补一次
`repair_dangling_tool_calls(…, HarnessHalted { class: "compaction" })`
（`compaction_utils.rs:151`），与普通请求路径的修复语义一致。
单测：`verbatim_backfills_unanswered_call_in_partial_parallel_run` 与
`verbatim_backfill_skips_complete_runs_and_is_idempotent`
（`compaction_utils_tests.rs:3224` / `:3268`）。

### 残留

- 反方向（`tool_result` 无对应 `tool_calls`）在压缩 prep 未处理；该形态由 `/repair`
  与会话加载覆盖，未在本机复现。
- 诊断文案有误导：400 被归类为 `Deterministic` 后映射到
  `CompactionSampleError::Build`（`full_replace_compaction.rs:207`），Display 前缀是
  「Compaction sampler build failed: 」（`xai-grok-compaction/src/sampler.rs:67`）——
  工件里确实长这样，看起来像客户端配置失败，实际是上游拒绝了请求体。属 P3 文案问题，未改。

## 事件 B：2026-10-19 的传输中断 —— 压缩重试阶梯弱于主回合

### 现场

会话 `01a1542e-8202-7532-af0e-b7e1884f63a0`（本仓库工作区）的压缩工件
`compaction_requests/542b0946-….json`：manual 触发、176 条输入、
`attempts = 3`，三次全部 `transient`，错误字符串完全相同：

```
compact failed: reqwest error stream: Transport error: error decoding response body
```

### 先排除 400 复发

对这份 176 条请求按 chat_completions 的 wire 规则做配对校验：64 个 `tool_calls`、
0 处 dangling，尾部是 `assistant / tool_result / assistant / tool_result / user`。
事件 A 的形态没有复发。

### 传输现场

该会话 `updates.jsonl` 的 `retry_state` 事件（`type = retrying`）在
2026-10-19T12:58:36Z–18:49:00Z 之间共 **68 条**：67 条传输类 + 1 条 429 配额。
传输类拆分为：

| 原因 | 次数 |
|---|---|
| `http2 error: keep-alive timed out` | 26 |
| `reqwest error stream: Transport error: error decoding response body` | 20 |
| `client error (Connect): operation timed out` | 14 |
| `connection error: 远程主机强迫关闭了一个现有的连接。 (os error 10054)` | 4 |
| `connection error: connection reset` | 2 |
| `connection closed before message completed` | 1 |

即 Command Code 那条链路整晚在抖。此前口头汇报记作「68 次传输重试」，精确拆分是
67 次传输 + 1 次 429，此处以本条为准。

### 根因：抖动被压缩自己的重试策略放大成失败

修复前（该会话跑的是 `v1.0.63` 客户端）压缩与主回合的恢复能力对比：

| | 主回合 | 压缩（修复前） |
|---|---|---|
| 重试次数 | 15（`sampler/src/retry.rs:9`） | 3（`compaction.rs` 旧值） |
| 退避 | 指数退避 + 抖动，上限 30s（`retry.rs:11,42`） | 固定 3s |
| 客户端 | 首个可重试 `Http` 错误 → `RetryWithClientRebuild`，换 HTTP/1.1 客户端（`retry.rs:157-163` 判定，`actor/request_task.rs:474-481` 重建） | `prepare_chat_completion(false)` 建一次，三次尝试复用同一连接池 |
| 同晚表现 | 13:05:50 起 attempt 1/2 重试，13:12:00 恢复出内容 | 三次同样失败 |

所以直接原因是网关链路抖动（HTTP/2 keep-alive 超时与流解码失败），放大成用户可见
故障的是压缩路径缺少「换客户端」这一步：HTTP/2 连接池中毒后，同池重试只会重复同一
失败。这也是事件 A 排查时已经定位、当时未施工的那条遗留。

### 修复

`fc0e899`（merge `6054081`，tag `v1.0.66`）：

- `session/helpers/session_compact.rs:167` 新增 `is_transport_failure`：`Transient`
  且 detail 含 `reqwest error stream: `（`SamplingError::EventStreamError` 的 Display，
  `sampling-types/src/error.rs:165`）或 `request error: `（`Http`，
  `error.rs:146`）。
- `session/helpers/full_replace_compaction.rs:122` 新增 `rebuild_client_for_retry`：
  以 `force_http1 = true` 重建 `OaiCompatClient`；`sample_compaction` 在下一次尝试前
  消费 `last_failure_transport` 并重建（`:156-162`）。
- `session/compaction.rs:995` 重试 3 → 5，间隔仍为固定 3s。
- 单测：`transport_failures_are_distinguished_from_other_transient_ones`
  （`session_compact_classify_tests.rs`）。

## 取证方法与坑

- **压缩工件**：`~/.grok/sessions/<workspace>/<session>/compaction_requests/<id>.json`
  记录 `model / trigger / chat_history / tools / summary / error / attempts /
  attempt_details`，两起事件的现场都来自这里，不需要重放上游。
- **`unified.jsonl` 有覆盖起点**：本机该文件首行是 `2026-10-19T13:55:17Z`，早于它的
  日志已不在文件里。事件 B 的 13:03 失败窗口只能靠会话 `updates.jsonl` 的
  `retry_state` 事件统计，别把「unified 里搜不到」当成「没发生」。
- **事件 A 的等价性验证**用「最小复现探针 + 真实工件形状核对 + 单测」三点支撑，
  没有回放 439 条真实工件（约 28 万 input token 的实盘花费）。
- **从 v1.0.66 起**：非 429 的采样失败（含压缩）会落 `~/.grok/dump/<date>/`，一失败
  一 JSON（出站请求 + 响应或 SSE 前缀）+ `index.ndjson` 一行。下次同类问题可直接看
  原始请求，不必再从会话工件反推。

## 遗留

1. **重试参数仍未对齐主回合**：压缩是 5 次 × 固定 3s，主回合是 15 次 × 指数退避
   + 抖动。若要把两者统一，改动集中在 `compaction.rs` 的 `max_retries` /
   `retry_delay_secs` 与 `full_replace_compaction.rs` 的客户端重建点，可复用
   `RetryPolicy`，但共享引擎 `sample_summary_with_retries` 的退避是固定 sleep
   （`xai-grok-compaction/src/code_compaction/sample.rs:189`），需要一起改。
2. **two-pass 路径没有重试**：`two_pass_sample`（`compaction.rs:138`）每次调用新建
   客户端并直接调一次 `generate_session_compact`，不进重试循环；失败会回退到单遍
   压缩（单遍有重试），所以用户侧表现为退化而非失败，但该路径本身仍是单次尝试。
3. **事件 A 的反方向形态**（孤儿 `ToolResult`）在压缩 prep 未兜底，依赖 `/repair`
   与会话加载。
4. **400 的诊断文案**仍会显示成「Compaction sampler build failed」。
