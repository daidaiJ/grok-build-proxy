# 压缩请求 400：并行 tool_call 半答未回填（2026-09-23）

> 症状：`/compact` 报 `Compaction failed - API error (status 400 Bad Request): invalid_request_error:
> An assistant message with 'tool_calls' must be followed by tool messages responding to each
> 'tool_call_id'. (insufficient tool messages following tool_calls message)`。
> 修复分支 `feat/local-compaction-toolpair-repair`；补丁登记见 `PATCHES.md`。

## 会话证据（决定性）

`~/.grok/sessions/D%3A%5CCODE%5Cai%5Cddmode/01a0c990-fc38-7491-9306-07ca6ef1e5f5/`
下的三份压缩请求工件（`compaction_requests/*.json`，schema_version 2）：

| 工件 | created_at (UTC) | 模型 | 输入条数 | 结果 |
|---|---|---|---|---|
| `42535529-…` | 2026-09-22T15:25:15Z | deepseek/deepseek-v4.1-flash | 488 | 成功 |
| `aaa5a961-…` | 2026-09-23T12:31:45Z | deepseek/deepseek-v4.1-flash | 439 | **400** |
| `c7cfc7de-…` | 2026-09-23T12:32:21Z | xiaomi/mimo-v2.6-flash | 439 | 成功 |

后两份输入逐字节同形（同一段历史、同一毫秒区间的重试），差别只有上游模型 —— 即
**故障不在本地的历史内容，而在上游严格程度**：同一份请求 DeepSeek 侧拒绝、MiMo 侧放行。

失败工件 `aaa5a961` 的历史尾部（`chat_history` 索引）：

- idx 436 `assistant`：`tool_calls = [call_00_R81IvQT8o0VpgOxhtxl01556 (get_command_or_subagent_output),
  call_01_eEW4XiQhz0IK8CiPIssm8288 (run_terminal_command)]`
- idx 437 `tool_result`：只有 `call_00` 的结果
- idx 438 `user`：压缩指令本体

即「并行两个工具调用，只有一个结果落地」——`call_01` 既无 `tool_result`，也无合成占位。

同一会话 `updates.jsonl` 的事件流印证成因：`call_01` 只有 `tool_call` 与
`tool_call_update(kind=execute)` 两条，**没有 `status=completed`，没有失败事件，也没有
合成结果**；`call_00` 在 12 秒后 completed，随后整轮就断了（下一个事件是 20 小时后的
`background_tasks: []`）。即回合在「第二个工具仍在飞」时中止，历史留下了半答的并行 run。

## 复现探针（对上游，最小 payload）

最小三例 POST 到 `https://api.commandcode.ai/provider/v1/chat/completions`
（模型 `deepseek/deepseek-v4.1-flash`，需带浏览器样式 UA，否则 Cloudflare 403 code 1010）：

| 用例 | messages | 结果 |
|---|---|---|
| A | user → assistant(2 calls) → tool(c1) → user | **HTTP 400**，报文与用户看到的一致 |
| B | user → assistant(2 calls) → tool(c1) → tool(c2) → user | HTTP 200 |
| C | user → assistant(1 call) → tool(c1) → user | HTTP 200（对照） |

结论：本仓库发出去的 assistant `tool_calls` 只要少一个 `tool` 回包，该网关（DeepSeek 上游）
必 400；补上回包即通过。

## 根因链（仓库内）

1. 正常模型请求：`ChatStateCommand::BuildConversationRequest` → `ensure_conversation_integrity()`
   → `repair_dangling_tool_calls`（`actor/mod.rs:338`、`actor/mutations.rs:85`），所以**普通回合
   不会挂**；落盘的 dangling 也会在 `ChatState::new` 加载时回填（`actor/state.rs:207`）。
2. 压缩请求：`run_compact_inner` 走 `GetConversation`（纯读 clone，按设计不在读处理器里修复，
   见 `actor/mutations.rs:83` 注释），随后只经
   `xai_chat_state::compaction_utils::prepare_conversation_for_verbatim_summarization`
   （`session/compaction.rs:941`，阶梯降级与 two-pass 亦同此函数）。
3. 该 prep 里的 `truncate_trailing_incomplete_tool_call` **只处理「末项就是带 tool_calls 的
   assistant」**；本例末项是 `tool_result` + `user`，函数不触发 → 半答 run 原样上车 → 400。

所以这是**压缩独有的缺陷**：状态里存在半答 run 时，普通回合自愈，压缩不会。

## 修复

`crates/codegen/xai-chat-state/src/compaction_utils.rs`
`prepare_conversation_for_verbatim_summarization`：在 `truncate_trailing_incomplete_tool_call`
之后补一次 `repair_dangling_tool_calls(…, HarnessHalted { class: "compaction" })`
（即按调用顺序为未答的 `tool_calls` 插入合成 `tool_result`，与正常请求路径的修复语义一致）。
新增两条单测：半答并行 run 回填（含真实结果保留、合成结果指名工具）、完整 run 不回填且幂等。

## 当前可用绕过（旧二进制）

`features.compaction_verbatim_input = false`（env `GROK_COMPACTION_VERBATIM_INPUT=0`）：
压缩走 lossy 路径，工具消息被整体剥掉、`tool_calls` 摊平成文本，不存在配对校验。
代价：摘要看不到工具 I/O 明细。

## 残留 / 未验证

- 反方向（`tool_result` 无对应 `tool_calls`）在压缩 prep 未处理；该形态由 `/repair`
  （`repair_history` → `strip_displaced_tool_results`）与会话加载覆盖，本次未在本机复现，
  如需一并兜底再动 prep。
- 未跑 xai-grok-shell 上游套件（纪律：上游套件在 Windows 不作为回归依据），验证 =
  `cargo check -p xai-chat-state --all-targets` +
  `scripts-local/ctest.sh -p xai-chat-state --lib compaction_utils`（153 passed）。
- 未用真实 439 条工件回放（约 28 万 input token 的实盘花费）；等价性由「最小复现探针 +
  真实工件形状核对 + 单测」三点支撑。
