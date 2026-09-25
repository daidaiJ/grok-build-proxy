# 第三方网关 Responses 事件名方言导致流式解析中止（兼容性问题）

> 记录日期 2026-09-22。触发场景：本机 `~/.grok/config.toml` 用 Command Code 网关
> （`https://api.commandcode.ai/provider/v1`）挂自定义模型，`api_backend = "responses"`，
> TUI 里发需要推理的请求即报错、整轮失败。
> 本文只做问题留痕与结论，**不代表已修**；代码修复见文末「修复选项」。

## 症状

```
Couldn't read the response: serialization error: unknown variant `response.reasoning.delta`,
expected one of `response.created`, `response.in_progress`, `response.completed`, ...
```

- `~/.grok/logs/unified.jsonl` 记 `shell.turn.inference_failed`，`ctx.kind = "serialization"`、
  `is_retryable = false`；随后 `turn.terminal_failure` → `agent response failed` → 本轮 `outcome: "error"`。
- 用户侧表现：整轮直接失败。**重发有时能侥幸通过**，见下方「触发条件」。

## 根因

报错**不是**本仓库的 bug，也不是上游返回了坏数据，而是**第三方网关发了非标准的
Responses 事件名**，本仓库依赖的 `async-openai` 类型是 serde 严格枚举，遇到未知
`type` 直接反序列化失败。

官方 Responses 规范里 reasoning 增量事件只有这两个名字：

| 规范事件名 | 用途 |
|---|---|
| `response.reasoning_text.delta` | reasoning 正文增量 |
| `response.reasoning_summary_text.delta` | reasoning 摘要增量 |

Command Code 网关实测发的是 **`response.reasoning.delta`**（两者都不是）。原始抓包：

```
event: response.created
data: {"type":"response.created", ...}

event: response.in_progress
data: {"type":"response.in_progress", ...}

event: response.output_item.added
data: {"type":"response.output_item.added","output_index":0,
       "item":{"type":"reasoning","id":"rs_...","summary":[]}}

event: response.reasoning.delta
data: {"type":"response.reasoning.delta","sequence_number":3,
       "item_id":"rs_...","output_index":0,"content_index":0,"delta":"We"}
```

复现命令（无需启动 TUI）：

```bash
curl -sS -N -X POST "https://api.commandcode.ai/provider/v1/responses" \
  -H "Authorization: Bearer $COMMANDCODE_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"deepseek/deepseek-v4.1-flash","input":"say hi","stream":true,"max_output_tokens":64}'
```

注意：`event:` 行与 `data.type` **两处都是** `response.reasoning.delta`，不存在哪个才是正式的歧义。

## 触发条件（为什么"重发有时能过"）

该事件只在**模型真的吐出 reasoning 内容**时出现，位置固定在
`response.output_item.added`（`item.type == "reasoning"`）之后：

- 短问答 / 不产生 reasoning 的回复 → 该事件不出现 → 侥幸通过；
- 需要推理的提示（尤其 `--effort high`）→ 必现。

所以重试只是碰运气绕开 reasoning 阶段，不是可靠规避。

## 本仓库内的落点（问题发生处）

| 位置 | 说明 |
|---|---|
| `crates/codegen/xai-grok-sampler/src/client.rs:91` | `deserialize_response_event()` —— 错误实际抛出点。首次 `from_str::<rs::ResponseStreamEvent>` 失败后，**兜底分支只处理 `/response/tools` 里的未知工具**，不认识的事件名无从兜底，最终走 `tracing::error!` + `Err(SamplingError::Serialization(first_err))` |
| `crates/codegen/xai-grok-sampler/src/stream/responses.rs` | 事件消费：只 match 已知变体（`ResponseReasoningTextDelta` / `ResponseReasoningSummaryTextDelta` 等），流在解析层就已中止，到不了这里 |
| `crates/codegen/xai-grok-sampling-types/src/error.rs:126` | `SERIALIZATION_DISPLAY_PREFIX`，即报错串的前缀来源 |
| `crates/codegen/xai-grok-sampler/src/actor/request_task.rs:808` | 刻意把响应解析失败归到 `Serialization` 而非可重试的 `EventStreamError`（原文注释：EventStreamError is retryable, and a response-parse failure is deterministic on retry） |
| `crates/codegen/xai-grok-sampler/tests/test_actor.rs:1219` | `messages_unparseable_event_is_fatal_without_retry` —— 该行为有测试锁定：不重试、只尝试一次 |

**枚举定义不在本仓库**：`rs::ResponseStreamEvent` 来自 workspace 依赖
`async-openai = { git = "https://github.com/our-forks/async-openai.git", rev = "95b52ebdedf42143083cf3d6f0e0be7c84e9c808" }`
（`Cargo.toml` 顶层 workspace 依赖 + `Cargo.lock:524`）。**这是自家 fork**，可改。

## 设计层面的含义

现有设计把「解析失败」一律当作**确定性失败、不可重试**处理（`request_task.rs:808` 的注释 +
`test_actor.rs:1219` 的测试）。这对**官方**端点是对的：官方事件名稳定，解析失败就是真出错。

但第三方网关的方言差异是**另一类失败**：它不是数据损坏，而是「对端用了别家方言」。
两者混在同一个 `Serialization` 里，导致：

1. 同一个方言问题在每次带 reasoning 的请求上稳定复现；
2. 不重试的策略让用户没有任何自愈机会；
3. 报错串（`unknown variant ... expected one of ...`）对用户不可读、不可行动。

若后续要在 `deserialize_response_event` 里做方言容错，建议**只做事件名归一化**，不要放宽
字段级校验（否则会把真正的解析错误吞掉，破坏上表那条不确定性边界）。

## 修复选项

### 选项 A：改用 chat_completions（不改代码，本机已采用）

同一个网关的 Chat Completions 端点正常工作，实测 HTTP 200、流式 + `tool_calls` 均完整
（`finish_reason: "tool_calls"`）：

```bash
curl -sS -N -X POST "https://api.commandcode.ai/provider/v1/chat/completions" \
  -H "Authorization: Bearer $COMMANDCODE_API_KEY" -H "Content-Type: application/json" \
  -d '{"model":"deepseek/deepseek-v4.1-flash","messages":[{"role":"user","content":"hi"}],
       "stream":true,"tools":[{"type":"function","function":{"name":"run_shell_command",
       "description":"Run a shell command","parameters":{"type":"object",
       "properties":{"command":{"type":"string"}},"required":["command"]}}}]}'
```

配置改法（`~/.grok/config.toml`）：

```toml
[model."deepseek/deepseek-v4.1-flash"]
api_backend = "chat_completions"   # 原为 "responses"
```

**代价**：`web_search` 工具要求 Responses API，改后该模型的 web search 不可用。

### 选项 B：在 async-openai fork 里加事件名别名（改代码）

给 `ResponseStreamEvent` 加 `#[serde(alias = "response.reasoning.delta")]` 之类，
映射到 `ResponseReasoningTextDelta`。归属 `our-forks/async-openai`，需连同
本仓库的 `rev` 一起 bump。收益是所有网关通吃，代价是要维护 fork 差异。

### 选项 C：在 `deserialize_response_event` 做流层归一化（改代码，本仓库内）

在 `serde_json::from_str` 之前对 `data` 做一次事件名重写（仅限已知方言表），
复用现有「sanitize 后重试」的兜底结构（`client.rs:91` 那个分支已经在做 tools 清洗）。
好处是不动 fork；风险是引入一层字符串改写，需配单测锁死只改事件名。

## 受影响范围（本机自定义模型）

`api_backend = "responses"` 且走 Command Code 网关的三个模型均受影响，已统一改为
`chat_completions`：

| 模型 | 网关 | 原 backend | 现 backend |
|---|---|---|---|
| `deepseek/deepseek-v4.1-flash` | api.commandcode.ai | responses | chat_completions |
| `deepseek/deepseek-v4-flash` | api.commandcode.ai | responses | chat_completions |
| `xiaomi/mimo-v2.6-flash` | api.commandcode.ai | responses | chat_completions |
| `gpt-5.6-luna` | token-unlimited.com | responses | responses（未动，另一网关，未验证是否同病） |

配置备份：`~/.grok/config.toml.bak-20260922-224106-prebackend`。

## 未验证项（后续若要收口）

- `gpt-5.6-luna`（token-unlimited.com）是否也发 `response.reasoning.delta` —— 未抓包。
- Command Code 网关的 `response.reasoning.delta` 是否还伴随其他非标事件名 —— 只抓了首个
  流的前几十行，未穷举。
- 该网关 Chat Completions 端点对 reasoning 内容的承载方式（实测 `message.reasoning` +
  `reasoning_details[]`）是否与 reasoning replay 缓存的多轮回放兼容 —— 未验证。
