# 第三方模型的 prompt cache 观察（2026-09-21）

> 起因：`/stats` 里 `glm-5.3-flash`（火山 Ark）与 `step-3.5-flash`（StepFun）的
> 「cache hit」都是 0.0%，怀疑是本地统计或用量字段解析的问题。
> 结论：**统计链路没错**，两次 0 各有原因，都是"请求里没有可复用的前缀"。
> 复现脚本 `scripts-local/prompt-cache-probe.py`。

## 结论速览

| model | 端点 | 账本调用数 | 命中 | 原因 |
| --- | --- | --- | --- | --- |
| `glm-5.3-flash` | 火山 Ark `api/coding/v3` | 4 | 0 / 52620 | 4 次是 4 个独立的一次性会话，工具集/MCP 状态各不同，前缀头部即分叉 |
| `step-3.5-flash` | StepFun `step_plan/v1` | 1 | 0 / 13398 | 唯一一次调用就是冷启动，没有前文可命中 |

对照：`step-3.7-flash`（StepFun，同一会话 17 次调用）69.0%，会话内首轮同样 0、之后逐轮命中；
deepseek 主会话 98.7%。

## 字段与解析（不是本地 bug）

命中数字取自 `usage.prompt_tokens_details.cached_tokens`，正是
`xai-grok-sampling-types::types::Usage` 解析的字段（`TokenUsage::from` 里与
DeepSeek 风格的 `prompt_cache_hit_tokens` 取 max）。用 `~/.grok/config.toml` 里的
key 直接打同样请求，同一段提示词连发两次：

```
glm-5.3-flash   call1 cached=0   call2 cached=2176/2177    # 命中 99.9%
step-3.5-flash  call1 cached=0   call2 cached=1984/2104    # 命中 94%
```

## 两个 0 的具体原因

1. **单次调用 = 冷启动**。`step-3.5-flash` 在账本里只有一条样本，会话记录
   `sessions/D%3A%5Ctool%5Ccli/01a0be5c-…` 是 `modelCalls: 1` 的单发测试
   （会话摘要 "Reply with exactly OK"）。同理 `step-3.7-flash` 首轮
   （18:28，cached 0）与 `gpt-5.6-sol` 唯一一次都是 0。
   → 处置：`/stats` 对 `calls == 1` 的行把命中率显示为 `n/a`
   （`model_usage_ledger::fmt_hit_rate`，卡片/复制文本/极简文本三处同口径）。

2. **跨会话前缀被工具集打断**。`glm-5.3-flash` 的 4 次调用是 4 个不同会话
   （`…3002` / `…5920` / `…1fa9` / `…b8cd`），每个 `modelCalls: 1`，都是同一句
   "Reply with exactly: OK" 的试模型。四个会话的 `system_prompt.txt` **字节相同**
   （sha256 一致），但请求头部的工具块不同：`…3002` 的请求带着
   `MCP server connected: - ssh-mcp (4 tools)`，10 秒后的 `…5920` 还是
   `MCP servers currently connecting: codegraph, context7, ssh-mcp, websearch`
   ——MCP server 异步连接，工具定义随后到齐，两次请求的工具集不同。

## 决定性实验：tools 块算进 Ark 的缓存前缀

同一段 messages，只改 tools 数组：

```
tools 不变，重发      → cached=2432/2463   命中
tools 多一个工具      → cached=0           整段前缀作废
tools 改回原样        → cached=2432        旧条目仍在
```

所以 Ark 的缓存键包含 tools 块：**工具集一变，整段提示词（含 ~13k 的 system
prompt）都不可复用**。这解释了为什么"同会话内稳定工具集 → 逐轮命中"（deepseek
98.7%、step-3.7 69%），而"每个新会话第一轮 → 0"。

补充：Ark 的命中按细粒度块上报（探测到 1408/2432/2176 这类非整千值），
尾部内容变化不影响前缀命中（同前缀 + 不同 user 消息 → cached=1408/1472）。

## 可选优化（未做，先记录）

- **工具集在启动时固定**：MCP 工具后到就不要再往请求里加（或让 MCP 工具集所有
  会话一致），跨会话才有机会复用那 ~13k 前缀。当前行为下，任何新会话的第一轮
  （以及工具到齐那一轮）都会整段回退到全价。
- `/stats` 已有 `calls` 计数；如需进一步弱化冷启动噪声，可考虑对 `calls` 很小的
  行加"样本不足"标记，暂不做。

## 复现方式

```bash
python scripts-local/prompt-cache-probe.py                      # 默认 ark-glm-5.3-flash
python scripts-local/prompt-cache-probe.py --model step-3.5-flash
```

脚本只读 `~/.grok/config.toml` 里的 `base_url` / `api_key`（不落盘、不打印 key），
按场景依次发请求并打印每轮的 `usage`：`repeat`（同请求重发，看是否能命中）、
`tail`（前缀相同、尾部不同）、`tools`（messages 相同、工具数组多一个）。
每个场景 2–4 次调用、每次 ~2k prompt token。
