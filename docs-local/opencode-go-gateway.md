# OpenCode Go 网关接入（grok 改版配置启用指南）

> **状态（2026-09-27）：支持已落地**（分支 `feat/local-usage-quota-estimate` 起的
> UA 覆盖改动；`${session_id}` 会话头模板此前已实现，见
> `sampler_turn.rs` 的 `expand_session_header_templates`）。
> 本文是配置启用方法与要求背景。

## 网关的强制要求

OpenCode Go（官方付费 `/zen/go` 与免费层同源）对进入网关的请求有**客户端标识强制**：

| 项 | 要求 | 说明 |
| --- | --- | --- |
| `User-Agent` | 必须标识实际应用 | 免费层校验严格（`opencode/<版本>` 且版本 ≥1.17）；付费层放宽为"非通用 SDK UA"即可 |
| 会话 ID 请求头 | 必须有（`x-opencode-session`） | 同一会话稳定取值，跨会话不同；网关用它做路由/缓存亲和 |
| `x-opencode-request` | 每请求唯一 | **网关自行生成**，客户端缺省即可 |
| `x-opencode-client` / `x-opencode-project` | 建议携带 | 静态标识 |
| 例外 | 部分官方适配过的端点豁免 | 以网关侧列表为准 |

## grok 改版的对应能力

1. **`User-Agent` 可覆盖**（本次新增）：sampler 先写默认 `grok-shell/<版本> (<os>; <arch>)`，
   再应用 `extra_headers`——配置里的 `User-Agent` 项**覆盖**默认值（回归测试
   `extra_headers_can_override_user_agent` 锁定顺序）。
2. **`${session_id}` 模板**（既有）：`extra_headers` 值中的 `${session_id}` 在每轮
   请求前展开为当前会话 ID——同一会话稳定、跨会话不同，正对 `x-opencode-session`
   的要求（`sampler_turn.rs:730`）。
3. **静态头**：`extra_headers` 原生支持任意静态头。

## 配置示例

```toml
[model."grok-code-free"]        # 模型 id 自取；base_url 指向网关
model = "grok-code"
base_url = "https://<opencode-go-网关地址>/openai/v1"   # 路由按网关文档：/openai、/anthropic、/codex
api_backend = "chat_completions"
api_key = "<网关 key>"

[model.grok-code-free.extra_headers]
# UA 覆盖（必配）：付费层标识实际应用即可，免费层需 opencode/<版本> 且 ≥1.17
"User-Agent" = "opencode/1.18.18 (windows amd64)"
# 会话 ID（必配）：${session_id} 每会话展开，同会话稳定
"x-opencode-session" = "${session_id}"
# 静态标识（建议）
"x-opencode-client" = "grok2"
"x-opencode-project" = "grok-build-proxy"
```

注意：

- `User-Agent` 之外的请求头如果只想要默认 grok UA，**不要**在 extra_headers 里写
  `User-Agent` 项（不写 = 用默认）。
- 全局 `[models].extra_headers` 会折叠进所有模型（per-model 同名键优先）——若只给
  网关模型打标，写在该模型的块里，别放全局。
- `x-opencode-request` 不用配：网关每请求自生成；真直连官方且被要求时再评估
  per-request 头注入（当前未做，属下一个特性）。
- Anthropic 路由（`/anthropic/v1/messages`）走 `api_backend = "messages"`，认证头
  由网关按 `x-api-key` 处理，无需额外配置。

## 评估结论（2026-09-27）

| 要求 | 结论 |
| --- | --- |
| UA 标识可覆盖 | ✅ 本次改动（默认保留 grok-shell，配置可覆盖） |
| 会话 ID 请求头 | ✅ 既有 `${session_id}` 模板 |
| 静态客户端头 | ✅ extra_headers 原生 |
| 每请求唯一头 | ➖ 网关自生成，客户端无需（直连官方如被强制再立项） |
