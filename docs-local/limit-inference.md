# 供应商 cache TTL / RPM / TPM 反推记录（limit-probe）

> **状态：T1/T2 已实现**（被动记录 + 离线分析），T3 主动探测未做。
> 分支 `feat/local-limit-probe`。由上一轮调研（cache 有效期 / 最大条数 / RPM/TPM
> 三指标可观测性评估）选定"容易且有感知"的部分落地：**全被动、零额外请求、零 token 成本**。
> 缓存最大条数（容量）需要主动多前缀探测，不在本期范围。

## 一句话

在 `xai-grok-sampler` 的 HTTP 层把每次模型请求的**现场**（状态码、`*ratelimit*`
响应头、`Retry-After`、前缀哈希）和**终端用量**（`cached_tokens` 等）逐行追加到
本地 ndjson，长期积累后用离线脚本反推出每个供应商的 cache 有效期与限流阈值。

## 数据面

落盘文件：`%USERPROFILE%/.grok/limit-probe/records.ndjson`（32 MiB 自动轮转到
`.1`；`GROK_LIMIT_PROBE_FILE` 可覆盖路径；`GROK_LIMIT_PROBE=0` 关闭）。

三种行（`kind` 字段区分）：

| kind | 时机 | 关键字段 |
| --- | --- | --- |
| `req` | 每次物理 HTTP 响应（3 后端 × 流式/非流式 6 个发送点，含重试） | `status`、`rl`（所有名字含 `ratelimit` 的响应头）、`retry_after`、`attempt`、`tools_hash`/`msgs_hash`/`msgs_count` |
| `err` | 非 2xx 响应体 | 500 字符 preview（区分限流 429 与配额耗尽等错误族） |
| `usage` | chat 后端终端 usage（非流式解析处 + 流式 `include_usage` 尾包；responses 流式 `ResponseCompleted/Incomplete`） | `prompt_tokens`、`cached_prompt_tokens`（OpenAI details 与 DeepSeek 扁平字段取 max，同 `TokenUsage::from` 口径）、`completion_tokens`、前缀哈希 |

前缀哈希：64-bit FNV-1a（长度前缀链），对**序列化后的 tools 块**与**messages 数组**
分别计算。稳定性保证到"同一份日志内可 join"（跨二进制版本不保证）——这正是离线
分析需要的口径。`x-grok-*` 元数据字段 `#[serde(skip)]`，不参与哈希。

代码落点：

- `crates/codegen/xai-grok-sampler/src/limit_probe.rs` — 模块本体 + 5 个单测
- `crates/codegen/xai-grok-sampler/src/client.rs` — 6 个发送点接线；
  `extract_retry_after` 改 `pub(crate)` 供复用；`handle_response` 增 `probe` 参数
- 注意：`CallNote` 构造时会对全量 messages 再做一次 serde 序列化（~1MB 上下文
  约 5–10ms，相对模型调用延迟可忽略）；介意时 `GROK_LIMIT_PROBE=0`。

## 分析面

```bash
python scripts-local/limit-inference.py                 # 默认读上面的 ndjson
python scripts-local/limit-inference.py --min-n 1       # 样本少时放宽门槛
python scripts-local/limit-inference.py --window 60     # 429 滑窗宽度（秒）
```

### Cache TTL 反推（生存曲线）

对同一 `(session_id, model)` 内的相邻两次调用：tools 块哈希不变、消息数不减
（排除 MCP 工具集漂移和 compaction 的污染，即 `prompt-cache-notes.md` 里两个
"假 miss"根因），以**间隔时长**分桶（15s→24h 共 13 桶）统计 `P(cached>0)`。
断崖点 = 有效 TTL；断崖是否陡峭区分定时过期 vs LRU 驱逐。限制：只能观测真实
流量里出现过的间隔（agent 会话间隔多落在 1–30 分钟，>1h 长尾无样本）。

### RPM / TPM 反推

三层，从免费精确到被动估算：

1. **限流头直读**（出现即精确）：所有 `*ratelimit*` 头去重打印（OpenAI
   `x-ratelimit-limit-*`、Anthropic `anthropic-ratelimit-*-limit` 等）；
2. **429 滑窗夹逼**：每次 429 前.window 秒（默认 60s）内"已接受的请求数 ≈ RPM
   下界、已接受的 prompt+completion token 数 ≈ TPM 下界"；429 事件越多 min 越
   收敛到真实阈值。遵守 `model-limits-config.md` 的纪律：**不主动发超限请求**；
3. `Retry-After` 分布（中位数/最大值）。

## 验收记录（2026-09-26）

- `cargo check -p xai-grok-sampler` 通过；
- `ctest.sh -p xai-grok-sampler --lib` 277/277（含新增 5 例：哈希确定性/长度前缀
  防碰撞、chat 与 JSON body 的 identity 提取、ratelimit 头过滤、truncate）；
- 分析脚本合成数据冒烟：TTL 断崖、限流头直读、429 滑窗三类输出符合预期。

## 已知边界

- messages 后端只记 `req`/`err`，不记 `usage`（Anthropic 流式 usage 分散在
  `message_start`/`message_delta`，如需要后续在 `stream/messages.rs` 聚合点补）；
- responses 非流式不记 `usage`（罕见路径）；
- 流式 usage 若供应商逐包重复，离线按 `req_id` 去重保留最后一条；
- 同 key 多进程并发写文件：`WRITE_LOCK` 只保护本进程，跨进程追加在 ndjson 行级
  原子性以内（单行 <4KB，Windows append 语义下实践安全），分析端有坏行告警。

## 后续可选（未做）

- T3 主动探测：`prompt-cache-probe.py` 加 idle sweep（nonce 前缀 → verify →
  睡 t → probe）与"命中是否刷新 TTL"实验，补 >1h 长尾；多前缀容量测试测缓存
  最大条数（花钱，须单独拍板）。
- `/stats` 卡片展示 TTL 估计与限流头直读值。
