# Token 经济审计：deepseek flash 缓存、工具输出与输出侧（2026-11-12）

> 状态：分析完成，改造待拍板。范围：fork `D:\pro\grok-build-proxy` + 本机
> relay `deepseek-flash`（= DeepSeek V4.1 Flash，1M 上下文；catalog 价
> input $0.15/M、cache_read $0.003/M、output $0.6/M）。数据源：
> `~/.grok/cache/model-usage.jsonl`（11-02 → 11-12）、会话工件、relay 探针、
> 代码锚点。只读审计，本轮未改产品代码或用户配置。

## 0. 结论

- 稳态已接近上限：98.2% 的 prompt token 命中，中位 98.95%，p10 94.2%。
  单轮平均 139.6k prompt / 137.2k cached / 2.5k uncached。
- 剩余安全优化约 10–20%，集中在两处：新增工具输出（uncached 3.7M）和
  reasoning/输出（1.16M，其中 0.70M 是 reasoning）。
- 会话基座约 40k tokens，按 cached 计价，继续瘦身对账单影响很小；
  它主要影响首轮 prefill 和上下文占用。
- 唯一带乘数效应的杠杆是前缀字节稳定性；唯一免费可修的是
  `send_feedback` 描述里的会话草稿路径。

## 1. 实测 token 去向

统计口径：`model-usage.jsonl` 中 `modelId == "deepseek-flash"` 的记录，
2026-11-02 08:43 → 11-12 12:18，共 1502 轮。

| 指标 | 数值 |
|---|---|
| prompt tokens | 209.7M |
| cached prompt tokens | 206.0M（98.2%） |
| uncached prompt tokens | 3.7M（约 2.5k/轮） |
| completion tokens | 1.16M（约 772/轮） |
| reasoning tokens | 0.70M（占输出 60%） |
| 最大单轮 prompt | 345k（10 天内 0 轮越过 500k） |
| 命中率 <50% 的轮次 | 25 轮，全部落在会话起点、隔夜恢复或压缩后 |

成本拆分（按 catalog 价）：cached $0.62 + uncached $0.55 + output $0.70
≈ **$1.87 / 10 天**。未命中的 1.8% token 占了输入成本的一半。

会话基座（本会话工件实测）：

| 组成 | 大小 | 约 token |
|---|---|---|
| system prompt | 11.2 KB | 2.8k |
| tool definitions（23 个） | 53.6 KB | 13.4k |
| AGENTS/rules（首条 user） | 70.2 KB | 17.5k |
| skills 清单（system-reminder） | 26.6 KB | 6.6k |
| 合计 | 约 162 KB | 约 40k |

## 2. 缓存与断崖

**relay 探针**（自签证书放行，请求 37–2461 prompt tokens）：

- 尾部和中段 `system` 均返回 200，最新 system 指令生效（"secret word BANANA"）。
- 同一 1245-token 请求第二次 `cached_tokens=1152`（92.8%）。
- 在 system 首字节前加 10 字节（模拟日期/提示词变更）→ `cached_tokens=0`。
  断崖成本与后缀长度成正比。

**新会话首轮为什么只有 10,240 命中**：

- `send_feedback` 的工具描述嵌入本会话草稿路径
  （`crates/codegen/xai-grok-tools/src/implementations/grok_build/send_feedback.rs:137`），
  每个会话的 tools 段因此唯一。
- 对比两个会话的 `tool_definitions.json`：23 个工具里只有 `send_feedback`
  （会话路径）和 `spawn_subagent`（模型清单随配置）不同。
- 本会话首轮 55,603 prompt、10,240 cached；system+tools 完整约 16–18k，
  命中恰好停在路径所在位置之前。

**resume 不会重写 head**：顶层用户会话冷载保留 journal 里的旧 head
（`spawn.rs:1419` 的非 subagent 分支不覆写）；只有 subagent spawn、客户端
`systemPromptOverride`（`mvp_agent/mod.rs:1050`）和换模型
（`model_switch.rs:484`）会换字节。跨进程/跨版本恢复不构成缓存断崖。

**compaction 已做缓存对齐**：`features.compaction_verbatim_input` 默认 true
（`compaction.rs:1010` 读取该策略），summarizer 保留 tool I/O 以复用主请求前缀。本机
手工 compaction 工件：463 条消息、932 KB history + 39 KB tools，模型
`deepseek-flash`；大部分输入按 cached 计价。**不要**为了"省输入"剥
reasoning/tools，那会把整段 summarizer 输入从 cached 变 uncached。

## 3. 阻碍清单（按影响排序）

1. **前缀字节稳定性**（唯一乘数效应）：`send_feedback` 会话路径、
   `spawn_subagent` 模型清单随配置集合变化、换模型、改 skills/AGENTS。
2. **新增工具输出**：bash 默认上限 200k 字符（约 50k tokens/次，
   `agent/config.rs` 的 `default_output_byte_limit`），全量进入历史；
   `[tool_output_compression]` 默认关，且一期是 head/tail 有损压缩
   （唯一型 JSON 保留 92.7%、日志 78.8%、搜索结果尾部会丢）。
3. **输出侧**：config 设了 `default_reasoning_effort = "high"`，reasoning 占输出
   60%；subagent 未配 `[subagents.efforts]`。
4. **裁剪/压缩定位被缓存稀释**：prune 门 50% 硬编码
   （`request_builder.rs:217`）、默认 rewrite；1M 模型下 500k 才触发，
   10 天最大 345k，从未触发。压缩主要解决上下文与延迟，不是当前账单主项。
5. **观测缺口**：`model-usage.jsonl` 没有 session id/purpose，无法把辅助调用、
   压缩收益、断崖归因到会话；limit-probe 未开。
6. **辅助调用不是瓶颈**：recap 默认开（本机 10 次）、two-pass compaction
   （2 次）、title refresh 已关；金额可忽略，先不要动。

## 4. 安全改造（不伤稳定/体验）

| 优先级 | 改动 | 预期效果 | 风险 |
|---|---|---|---|
| P0 配置 | `[toolset.bash] output_byte_limit = 40000` | 封顶单次 bash 输出；中段截断+全量日志路径已有（`bash/mod.rs:364`） | 极低，信息可从日志取回 |
| P0 配置 | 模型加 `prune_mode = "append"` + `system_prompt_update = "in-history"` | 长会话越 500k 或提示词变更时避免整段断崖 | 低，relay probe 已过 |
| P0 代码 | `send_feedback` 描述去掉会话路径，路径放进工具输出 | 新会话首轮可复用完整 tools 前缀（+6k 命中） | 无 |
| P1 代码 | 无损压缩层：重复行折叠、JSON 精确去重、search 全量保留、往返自校验、CCR 可取回；跨轮 keep-earliest + 前缀单调 | 压缩新增工具字节且保语义、不破缓存 | 中，需要实现与回归 |
| P2 | prune 门做成可配（默认不变） | 为不同价格/窗口调参留口 | 无 |
| P2 | `model-usage` 增 sessionId/purpose | 把辅助调用、压缩收益、断崖归因到会话 | 无 |
| P2 | 按需配 `[subagents.efforts] explore = "low"` | 降 subagent reasoning，主线程质量不变 | 低，取决于 subagent 类型 |

```toml
[toolset.bash]
output_byte_limit = 40000

[model."relay/deepseek-flash"]
prune_mode = "append"
system_prompt_update = "in-history"
```

## 5. 不建议做

- **缩工具面 / deferred tool exposure**：缓存下收益极小，工具发现质量风险大。
- **全局降 reasoning effort**：省输出成本，赔任务质量；主线程维持 high。
- **激进裁剪**：省下 cached token 的钱，换来断崖和信息丢失。
- **为 compaction 剥 reasoning/tools**：破坏缓存对齐，成本反而上升。
- **为压缩重写历史**：前缀失效，社区实测净省接近 0。
- **LLM 逐条摘要工具结果**：额外调用与延迟，收益不稳。

## 6. 验收

- 同任务 A/B 5 个会话，比较 uncached prompt tokens、TTFT、compaction 次数、
  工具重跑/失败率。
- 压缩层要求 prefix 字节不变，且 `expand_output` 能取回原文。
- bash 上限改动后统计大输出命令的 `read_file` 回读频率，确认没有把
  常见工作流变成两跳。

## 7. 附：取证与锚点

```bash
# 按模型聚合 token / 成本
python - <<'EOF'
import json, os, collections
p = os.path.join(os.path.expanduser("~"), ".grok", "cache", "model-usage.jsonl")
agg = collections.defaultdict(lambda: [0, 0, 0, 0])
for line in open(p, encoding="utf-8"):
    r = json.loads(line)
    a = agg[r.get("modelId", "?")]
    a[0] += 1
    a[1] += r.get("promptTokens", 0)
    a[2] += r.get("cachedPromptTokens", 0)
    a[3] += r.get("completionTokens", 0)
for m, a in sorted(agg.items(), key=lambda x: -x[1][1]):
    print(m, "turns", a[0], "prompt", a[1], "cached", a[2], "out", a[3])
EOF
# 对比会话工具定义：~/.grok/sessions/<cwd>/<session>/tool_definitions.json
```

- `send_feedback.rs:137` — 会话路径进入描述。
- `request_builder.rs:145` — in-history 放置；`:217` — 50% prune 门。
- `spawn.rs:1419` — resume 保留 head；`model_switch.rs:484` — 换模型换 head。
- `compaction.rs:1010` — verbatim input 缓存对齐。
- `default_output_byte_limit` — `crates/codegen/xai-grok-agent/src/config.rs`。
- 压缩一期行为与二期无损层：
  [`docs-local/tool-output-compression-plan.md`](../tool-output-compression-plan.md)。
