# Harness × 国模适配专题——FrontierHarness 评估结论与 fork 吸收建议

> **状态：调研完成（2026-11-12），建议待拍板；零代码改动。**
> 数据：FrontierHarness v1.1 live（抓取 2026-11-12，约 65% cells 完成）+ v1.0 冻结结果。
> fork 基线：`main 7f1fdcd4`（v1.0.71 发版后，PATCHES 六十五期之后）。
> 关联： [`../cache-strategy-port/README.md`](../cache-strategy-port/README.md)、
> [`../dsh-port/README.md`](../dsh-port/README.md)、
> [`../codex-port/README.md`](../codex-port/README.md)、
> [`../fork-feature-inventory.md`](../fork-feature-inventory.md)。

## 用途与范围

- 回答两个问题：① Codex 与 Pi 在 GLM / DeepSeek / Kimi 上领先的共性是什么；
  ② 这些结论里哪些应吸收进本 fork。
- 只讨论 harness 接口层——工具面、上下文装配、协议路径；不评估模型能力。
  榜单是第三方评测，分数不是官方 benchmark score。
- 证据分级：fork 现状 = 本机读码/配置核对（A）；榜单数字 = 公开 API 聚合
  （A⁻，样本未跑满，排名会变）。

## 数据与口径

- Live：`GET https://frontierharness.org/api/live`，21 harness × 5 模型 × 62 任务
  × 4 次尝试。抓取时 `finished` 16,986 行 / 期望 26,040（约 65%），整体未完成。
  行字段由 `finished_fields` 给出（`task, harness, model, outcome, input_tokens,
  cached_tokens, output_tokens, cost_usd, real_seconds, ...`）。
- 口径：pass% = 该 harness×model 下**已完成的 scored attempts** 的通过率
  （`outcome in {pass, fail, timeout}`，timeout 记失败，infra 剔除）。每格
  n = 103–205 / 248，未跑满。
- v1.0 冻结参考：30 任务 × 1 次，Kimi K3。Codex 66.7% / Claude Code 63.3% /
  Pi 60.0% / Kimi Code 56.7% / OpenCode 50.0%。
- 模型：GLM-5.3、GLM-5.3-Flash、DeepSeek-V4.1-Flash-1017、Kimi-K3
  （四个国模）；Muse-Spark-Open（Meta，对照组）。
- 复现注意：`/api/live` 是紧凑数组，必须先按 `finished_fields` 建索引；pass 率
  受任务完成顺序影响，别把中间态当终值。

## 观测（live v1.1，2026-11-12 抓取）

### 通过率与名次

| 模型 | Codex | Pi | 同厂 / 重壳对照 |
|---|---|---|---|
| GLM-5.3 | **70.9% (#1)** | 69.4% (#3) | KimiCode 69.7% (#2) · OpenCode 59.9% (#14) |
| GLM-5.3-Flash | **75.6% (#1)** | 70.6% (#2) | KimiCode 68.8% (#4) · OpenCode 62.4% (#15) |
| DeepSeek-V4.1-Flash | 81.6% (#2) | **81.8% (#1)** | KimiCode 73.5% (#9) · OpenCode 71.9% (#15) |
| Kimi-K3 | **83.9% (#1)** | 69.6% (#4) | KimiCode 64.5% (#12) · OpenCode 64.0% (#13) |
| Muse-Spark-Open（Meta 对照） | **77.6% (#1)** | 57.3% (#8) | Claude Code 57.2% (#9) |

Codex 在 5 个模型里 4 个第一，DeepSeek 与 Pi 差 0.2 点、同属第一档。
Pi 在 3 个国模族（GLM、GLM-Flash、DeepSeek）进前二/前三，Kimi 上掉到第四；
Meta 对照只有第八。

### 成本与上下文

| 模型 | 每 trial 输入 token（Codex / Pi / KimiCode） | 每 pass 成本（Codex / Pi / KimiCode） |
|---|---|---|
| GLM-5.3 | 1.51M / 0.78M / 2.11M | $0.55 / $0.49 / $1.10 |
| GLM-5.3-Flash | 2.00M / 0.57M / 3.43M | $0.15 / $0.07 / $0.30 |
| DeepSeek-V4.1-Flash | 2.16M / 0.60M / 3.68M | $0.10 / $0.05 / $0.18 |
| Kimi-K3 | 0.66M / 0.58M / 1.20M | $0.71 / $0.90 / $1.45 |

样本未跑满，成本列会随剩余任务变化；方向性结论（头部两家比同厂/重壳省一半以上
输入 token）与 v1.0 冻结结果一致。

### v1.0 冻结结果补充

- 同一批 30 任务下，Codex 平均 62.4 轮、Pi 18.7 轮——两家回合数差 3 倍，
  所以共性不在"步数多/少"。
- 最难任务（`python-statemachine-state-data-scoping`）：Pi 90 轮 / $2.50 /
  98.2% 缓存；Codex 187 轮 / $5.97 / 99.1%；Claude Code 381 轮 / $64.36 /
  15.7% 缓存。三家都 pass，成本差 26 倍。
- 全库 token 加权缓存命中：除 Claude Code（25.0%）外，其余 91.6–99.1%。
  到 live v1.1，各家都到 93–97%，**缓存命中已不是区分项**。

## 共性分析

两家领先的共性可以压成一句：**把 harness 做薄——固定的小动作面、只追加的投影式
历史、单线循环、原生 OpenAI 协议；编排权交回模型。** 国模只被考"读代码→改文件→
跑命令"，长程元指令、动态工具发现、厂商私有缓存语义这些弱项没被触发。

1. **固定且小的动作面。**
   Pi 默认 4 个工具（`read, bash, edit, write`，源码 `selectedTools ?? [...]`）；
   Codex 核心是 shell + apply_patch + 计划，另加按开关的 view_image / web_search。
   schema 静态、无动态发现、无 MCP 工具墙。弱一点的工具选择能力不用每回合重新
   解析一套工具集。
2. **只追加 + 投影式历史。**
   Pi 的 session 是 JSONL 树，请求由 active branch 投影，压缩只插入一条 summary
   条目、原始记录不动；Codex rollout 同构。前缀稳定带来 94–96% 隐式缓存命中，
   也让模型看到的对话与自己写下的一致。
3. **单线循环。**
   一个 assistant turn → 工具 → 结果 → 下一 turn；没有自动 subagent、计划树、
   运行时重排。失败停在一步内、可恢复，不给模型制造元指令负担。
4. **原生 OpenAI 系协议。**
   Codex 走 Responses，Pi 走 Responses/chat；国模的主训练面就是 OpenAI 兼容
   接口。Anthropic Messages 是网关翻译层（本次 Kimi Code 被配置成
   `type = "anthropic"`，Claude Code 同样走 Messages），翻译会改变 cache、
   thinking 与 tool-call 的形状。

## 反证与边界

- **更薄不等于更强**：mini-SWE-agent 只有一个 bash 工具，仍然不是第一，
  说明"薄"是必要条件，不是充分条件；提示词与工具细节仍有分量。
- **不是国模专属**：Codex 在 Meta 的 Muse-Spark-Open 上同样第一（77.6%），
  这是 harness×model 适配，不是"为国模定制"。
- **prompt/工具预设本身有分量**：DSH 四个 preset 是同一二进制，只换提示词与
  工具配置，Kimi K3 上差 6.6 分（53.3–63.2）。
- **榜单不能拆因**：作者自述成本差异属于 harness-model 组合，无法分离
  cache / 工具循环 / 提示词各自的贡献；本专题的因果判断是从"共性 + 反证"
  推出来的，不是消融实验。

## fork 现状盘点

| 维度 | fork 现状 | 锚点 | 与榜单头部的差距 |
|---|---|---|---|
| 默认工具面 | `grok-build-plan` 20 个；`grok-build` 19 个 | `xai-grok-agent/src/config.rs:256,388` | Codex ~5、Pi 4 |
| 已有 lean 候选 | `grok-build-concise` 14 个；`codex` preset 10 个 | `config.rs:290,340` | concise 会换 prompt 且 `agents_md: false` |
| MCP meta 工具 | `search_tool`/`use_tool` 无条件在 core 列表 | `config.rs:256` | 无 MCP 的会话白带 2 个工具 |
| 缓存模式默认值 | `system_prompt_update=Off`、`prune_mode=Rewrite` | `xai-grok-sampling-types/src/types.rs:1356-1357` | 头部两家都是只追加 |
| 追加式裁剪 | 已实现；`messages` 后端回退到原地改写 | `xai-chat-state/src/actor/request_builder.rs:122` | 你的模型 4/5 是 chat_completions，可用 |
| in-history 系统提示 | 已实现；只对 chat/responses probe 通过 | `request_builder.rs:145`、`types.rs:1270-1277` | 需按网关重验 |
| 缓存观测 | 状态行 `cache %` / `miss N` 段 | `xai-grok-pager/src/views/status_line/segments.rs:180` | 已有，默认是否挂出待核 |
| 前缀回归护栏 | 无 | — | 未来改动可能悄悄改写历史 |
| 本地 harness A/B | 无（limit-probe 只测 TTL/限流） | `../limit-inference.md` | 无法回答"哪个 knob 有用" |
| per-model harness 绑定 | 支持 `agent_type` | `xai-grok-shell/src/agent/config.rs:3661` | 落地 lean 配置的现成通道 |

## 吸收建议

### P0-1：打开已实现的缓存模式（配置，约 30 分钟）

`system_prompt_update` / `prune_mode` 两个能力位默认全关（`types.rs:1356-1357`），
当前 `~/.grok/config.toml` 也未设置。对走 `chat_completions` / `responses` 的国模
逐个加：

```toml
[model."<your-model-id>"]
prune_mode = "append"
system_prompt_update = "in_history"
```

- 适用面：`chat_completions` / `responses`；`messages` 后端自动回退
  （`request_builder.rs:122`、`:145`）。
- **先 probe 再开**：in-history 的 probe 矩阵只覆盖过 DS 官方三 backend 与
  Command Code；append 插入的 marker 是 System item，relay 网关是否接受中部
  system 未验证。探针不过就只留 `prune_mode`。
- 验收：长会话过 50% 窗口后 `cache %` 不再单调跌到 0；同任务输入 token 下降。

### P0-2：用现成 preset 做一次快速对照（配置，约 15 分钟 + 机器时间）

给一个国模临时设 `agent_type = "codex"`（`agent/config.rs:3661`），同批任务对比
默认 20 工具。注意它会**同时**把 prompt 换成 Codex 模板，结果是"工具面 + prompt"
合包；要隔离工具面，走 P1-3。

### P1-3：新增 `grok-build-lean` 预设（0.5–1 人天）

- 工具集：`bash`、`read_file`、`search_replace`、`list_dir`、`grep`、`todo_write`
  + 后台命令三件套（`get_command_or_subagent_output` / `wait_commands_or_subagents`
  / `kill_command_or_subagent`）≈ 9 个；`scheduler × 3`、`monitor`、`goal`、
  `workflow` 从国模默认里拿掉（`xai-grok-agent/src/config.rs:256`）。
- 保留原 prompt 与 AGENTS.md：不要直接复用 `grok-build-concise`——它的定义里
  `agents_md: false`，会丢掉项目规则（`xai-grok-agent/src/config.rs:1460`）。
- `search_tool` / `use_tool` 改成"有 MCP server 才进工具面"（现在无条件在 core
  列表里）。
- 绑定：`[model.X] agent_type = "grok-build-lean"`；子代理可继续用
  general-purpose / explore / plan。
- 验收：A/B 中 pass 不低于默认 profile 3 个百分点，输入 token 降 20% 以上。

### P1-4：前缀护栏 + 本地 A/B（1–2 人天）

- 观测已有：状态行 `cache %` / `miss N`、limit-probe 的 prefix hash +
  cached_tokens。
- 加一条开发期断言/日志：同一会话前缀变化但没有追加事件 → warn；把该场景
  加进 headless 冒烟，防止未来的通知/记忆提醒/图片剥离又改写历史。
- A/B：10 个任务 × 2–3 个 profile × 3 次，从 `unified.jsonl` 出 pass / 成本 /
  cache / 轮次。先跑默认 vs lean，再跑 lean vs lean+in-history+append。

### P2（暂缓，触发再做）

- **messages 后端的追加式裁剪**：marker 改走 user/tool 角色，只有确认要走
  Anthropic 兼容端点时才值得做（2–3 人天）。当前榜单反而显示 Messages 路径
  对国模不占优。
- **主回合 `prompt_cache_key`**：现在主回合恒为 `None`，只有 Responses 映射
  会把它发上线（`xai-grok-sampling-types/src/types.rs:1169`）。对支持该 key 的
  网关是低成本实验，对 DeepSeek 系无效（0.5 人天）。

## 不建议吸收

- **默认开自动 subagent / 编排**：榜单底部就是重编排 harness；fork 的 subagent
  保持 opt-in。
- **默认开 deferred tool search（P2 T2）**：动态工具发现多一层失败面，现有数据
  不支持对国模默认开。
- **追"同厂原生"**：Kimi Code 在自家 K3 上只排第 12，原生不等于适配。
- **整套搬 Codex prompt**：工具面可以学；提示词是按 GPT 调的，必须与工具面
  分开 A/B。

## 验收与未决

- [ ] P0-1 的 relay 网关 probe：中部 System 是否被读作生效？marker 是否被接受？
- [ ] 长会话实测：过 50% 窗口后 `cache %` 是否稳住、token 是否下降。
- [ ] lean profile 的本地 A/B：pass 不降、token 降 20%+。
- [ ] 榜单跑满后复核排名（当前 65% cells，Kimi 的 Pi 名次可能上移）。
- [ ] 前缀护栏用例进 headless 冒烟。

## 更新记录

- 2026-11-12：初版。抓取 live v1.1 数据、读 fork 代码与配置，形成 P0/P1/P2 建议。
