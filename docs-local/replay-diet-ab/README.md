# Replay diet A/B（v1.0.85）

目标：用同一套台账对比两种回合内策略的成本与上下文稳定性。

| 组 | 机制 | 配置 |
|---|---|---|
| A | v1.0.83 行为：不做 in-turn replay diet | `replay_diet=false`（本机 config 已设） |
| B | v1.0.84+ 行为：长回合裁剪旧 reasoning / 工具结果 | `replay_diet=true`（启动器用 env 覆盖） |

v1.0.85 的优先级是 `GROK_REPLAY_DIET` > `[compaction] replay_diet` > 默认 true；
启动器只在本进程设置 env，不改全局 config。

## 启动

```bash
# A 组（旧机制）
bash docs-local/replay-diet-ab/ab_launch.sh A

# B 组（新机制）
bash docs-local/replay-diet-ab/ab_launch.sh B
```

启动器会生成 UUID、写入分组映射，然后以 `grok2 --session-id <uuid>` 开新会话：

- 映射：`~/.grok/ab/replay-diet/sessions.jsonl`
- 默认二进制：`/d/tool-cli/grok2/grok2.exe`，可用 `GROK2_BIN` 覆盖
- 可选标签：`AB_LABEL=... bash ... A`
- 额外参数原样传给 `grok2`；不要用 `--resume`，A/B 必须新会话
- 切换组不要在同一会话里做；策略在 spawn 时解析一次

## 采集协议

- 同一模型、同一工作区、同类任务；建议 A/B/A/B 交替，避开网关缓存 TTL 的时段偏置。
- 每组至少 5 个会话、50 次 main 调用；更稳的是每组 10 个长回合或 2–3 天。
- 长回合才会有 diet 触发；短问答两组几乎相同，样本没有区分度。
- 不要看状态行开头的 `cache`：它是会话累计值。用台账的逐调用行。

## 报告

```bash
python docs-local/replay-diet-ab/ab_report.py
```

可选价格（USD / 1M tokens，按你实际模型改）：

```bash
python docs-local/replay-diet-ab/ab_report.py \
  --input-rate 0.15 --cached-rate 0.03 --output-rate 0.60
```

输出包含：

- 成本侧：prompt tokens/call、token 加权命中、每调用估算成本、总成本
- 稳定性侧：命中中位/p10/min、`<90%` 调用占比、prompt p90/max、最大单次回落、
  `cache-breaks.jsonl` 的 `history_rewrite` / `unknown` 次数与未缓存 token
- B 组的 `shell.replay_diet` 次数、估算释放 token、stub 工具结果数、compaction 次数

`--from/--to` 可限制会话启动日期；`--map/--ledger/--breaks/--unified` 可覆盖路径。
少于 5 个会话/50 次调用时报告会给出样本不足警告；两组模型不同也会警告。

## 判读

- **成本**：B 的 `prompt tokens/call` 和估算 `cost/call` 应更低；若只省了 prompt 但
  output 或 compaction 成本上升，净收益可能为零。
- **稳定性**：B 的 `history_rewrite` 次数和 `hit p10` 通常更差；`max prompt drop`
  是 diet/压缩造成的上下文突变幅度。A 组应接近零 diet 事件和零 history rewrite。
- **决策**：以净 `cost/call` 为主，稳定性设下限（例如 p10 ≥90%、history_rewrite
  每 100 次调用 ≤ 1）；不满足就保留 A 或提高 B 的触发门槛。
- 已 diet 的会话不会恢复原文；A/B 结束后不要拿旧会话续测。

## 配置回退

不想用启动器时，直接改 `~/.grok/config.toml`：

```toml
[compaction]
replay_diet = false   # A
# replay_diet = true  # B
```

改完开新会话；env 仍会覆盖 config。备份在
`~/.grok/config.toml.bak-2026-11-27-replay-diet`。
