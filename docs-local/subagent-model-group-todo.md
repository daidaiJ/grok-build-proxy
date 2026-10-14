# Subagent 模型配置优化与模型组（降级链）设计

> 状态：**设计完成，待拍板开工**（2026-10-14）。六个拍板点见 §拍板点。
> 需求原话：内置 subagent 可配 model 与推理深度；同一 model id 来自多个供应商，要能精确指定
> 供应商条目并做降级链（A 家 4.1flash 用不了换 B 家 glm-5.3 flash）——「模型组」概念。
> 视觉桥（[vision-bridge/plan.md](vision-bridge/plan.md) D8/D9）的降级链**引用本设计的模型组
> 原语**：一个概念，两个消费者。视觉桥维持调研态，本专题先落地原语。

## 背景与动机

- 日常模型池是跨供应商的 flash 档（deepseek-v4.1-flash / glm-5.3-flash）。单家 429/5xx 时
  手动 `/model` 切换要付**缓存成本 gap**：不同模型、不同供应商之间缓存互不相通，整条前缀重烧。
- 降级链把切换自动化：平时钉死首选家（缓存稳定），失败才付一次 gap 落到下一家。
- subagent 是第一消费者：子代理跑廉价 flash + 低 effort 就够，主循环保持高级模型；
  组内降级语义与 subagent 现有限流参数（`subagent_rate_limit_max_attempts`）天然衔接。

## 现状锚点（2026-10-14 全部核实）

| 事实 | 位置 |
|---|---|
| `[model.<id>]` 条目即供应商 pin：`model`（路由 slug）+ `base_url` + `api_key`/`env_key` + `api_backend` + `use_proxy` + 重试/限流参数 | `xai-grok-shell/src/agent/config.rs:3888` `ModelEntryConfig` |
| 条目已有 effort 三件套 + variants（按 effort 换 model id 的后端） | 同上：`reasoning_effort` / `supports_reasoning_effort` / `reasoning_efforts` 菜单 / `reasoning_effort_server_default` / `variants` |
| 用户 config 实况：全局 `default_reasoning_effort = "medium"`；同一上游模型三家并存（`glm-5.3-flash`(方舟) / `z-ai/glm-5.3-flash`(Command Code) / `cline-pass/glm-5.3-flash`(cline)） | 用户 `~/.grok/config.toml`（2026-10-14 提取，密钥排除） |
| per-subagent 模型 pin 已存在：`[subagents.models].{agent_name}`，解析优先级 = config pin > `AgentDefinition.model` > 继承父会话；unknown pin warn 后 fall through | `subagent/mod.rs` `resolve_subagent_sampling_config`；字段注释 `config.rs:1500` |
| goal/persona 的 runtime model override 赢在用户 pin 之前 | 同文件 `resolve_effective_model_config` |
| role/persona 已带 `model` + `reasoning_effort` 字段 | `xai-grok-subagent-resolution/src/config.rs` `SubagentRole` / `SubagentPersona` |
| task tool 已有 per-spawn `model` / `reasoning_effort` runtime overrides | `xai-grok-tools/.../grok_build/task/mod.rs` |
| effort 应用与能力校验：`apply_supported_effort`（`EffortTarget::{NewSession, ModelSwitch, SummaryClient}`）+ `model_for_effort` variants 路由 | `mvp_agent/reasoning_effort.rs` |
| 子代理侧 effort：runtime override 解析失败忽略 + `model_supports_reasoning_effort` 门 | `subagent/handle_request.rs`（effort 分支） |
| task tool 父模型点模型权限开关 `TaskModelSelection::{Selectable, Inherited}` | `task/model_policy.rs` |
| 内部模型 pin 先例：`[models]` 节 `default_reasoning_effort` / `web_search` / `session_summary` / `image_description` / `prompt_suggestion`；internal models（含 subagents）豁免 `[models]` allowlist | `config.rs` `ModelOverrideConfig`（:1057 起、:1071 豁免注释） |
| 重试循环始终同条目内（`classify_error` → `RetryDecision`；per-entry `max_retries` / `rate_limit_retry_threshold` / `subagent_rate_limit_max_attempts`） | `xai-grok-sampler/src/actor/request_task.rs` |
| 全仓无跨模型 fallback 链 | grep 证伪（`fallback` 命中均为无关语义） |
| 内置 subagent 类型（`general-purpose` / `explore` 等）按名字寻址；`[subagents.models]` 的 key 即 agent name | `task/admission_tests.rs` / `backend_tests.rs` |
| 模型目录缓存四路倒排匹配可对上用户 config 的 model id（裸 id↔`vendor/model` 段命中） | `xai-grok-model-catalog`（v1.0.51 修复后语义） |

**缺口**：① per-agent effort 无 config 入口（现状只有 role/persona/spawn 三条路，`[subagents.models]`
只有 model id）；② 模型组/降级链完全没有；③ 供应商精确指定已天然成立——`[model.<id>]` 条目 id
就是 pin，组引用条目 id 即可，无需发明新命名空间。

## 设计定案（待拍板）

### G1 模型组原语：`[model_groups.<name>]`

```toml
# 有序降级链：条目引用 [model.<id>]（供应商 pin），顺序 = 优先级
[model_groups."cheap-flash"]
entries = [
  { model = "cline-pass/deepseek-v4.1-flash", effort = "high" },
  { model = "z-ai/glm-5.3-flash" },                      # effort 缺省 → 全局 default_reasoning_effort
  { model = "glm-5.3-flash" },                           # 方舟兜底
]
sticky = true             # 降级后粘住首个可用条目（缺省 true）
cooldown_minutes = 10     # 失败条目冷却期，期内跳过（0 = 不冷却）
```

语义：

- 条目引用 `[model.<id>]` 条目 id；组是**引用**不是复制——条目的能力位、上下文窗、限流参数
  仍以 `[model.<id>]` 为准。
- 每条目可选 `effort` 覆盖；应用走现有 `apply_supported_effort`（条目不支持 effort →
  warn + 忽略，沿用既有语义；`variants` 后端继续生效）。
- 启动校验：引用不存在的条目 → warn + 丢弃该条目（与 unknown pin warn-and-fall-through 同风格）；
  组空 → warn + 整组不可用，消费方 fall through 现有优先级。校验放 shell config 解析层，纯函数可单测。

### G2 降级语义

- **触发**：当前条目的同条目重试预算耗尽仍失败，且错误类别 ∈ {429 限流耗尽、5xx、连接失败/超时}。
  400/401/403 不触发——多为配置错或内容错，换供应商会掩盖 bug；图像 400 归视觉桥 D9 管辖。
- **时机**：仅**首字节前**失败可降级（与现有重试包络一致）；流中失败维持现状报错，不中途换模。
- **粘性**：降级后本会话后续回合直接用新条目（`sticky = true` 缺省），避免每回合重付 gap；
  `sticky = false` 每回合从头按序试。
- **冷却**：失败条目记会话级冷却（`cooldown_minutes`，缺省 10），期内组解析直接跳过该条目。
- **披露与记账**：降级发生 → transcript 一条用户可见 notice（「模型 X 不可用，已降级到 Y」）+
  unified.jsonl 事件；状态行 model id 按「最近一次成功回显」口径更新（v1.0.45 修复的同一键）；
  `/stats` 成本列按实际 model id 计价（目录匹配已有，无需改）。
- **重放/回卷确定性**：组解析是会话级运行时状态，不进 journal；回合记录存实际使用的 model id，
  rewind/replay 语义不受影响。

### G3 subagent 绑定（第一消费者）

```toml
[subagents.models]            # 现状保留：精确 pin 单条目
explore = "cline-pass/deepseek-v4.1-flash"

[subagents.model_groups]      # 新增：绑组（与上一行同 agent 互斥，都写 → warn + models 赢）
general-purpose = "cheap-flash"

[subagents.efforts]           # 新增：per-agent effort 覆盖（不绑组也能用）
explore = "low"
```

- 内置 subagent（`general-purpose` / `explore` 等）与自定义 role 同机制按 agent name 寻址，
  无需新机制；`[subagents.toggle]` 继续管开关。
- **model 优先级**（自高到低）：spawn runtime override（现状）> `[subagents.models]` 单条目 pin
  （现状）> `[subagents.model_groups]` 组绑定（新增，解析为「当前激活条目」）> `AgentDefinition.model`
  （现状）> 继承父会话（现状）。组绑定占据用户 pin 层；与单条目 pin 冲突时单条目赢（更精确）+ warn。
- **effort 优先级**（自高到低）：spawn runtime override（现状）> role/persona effort（现状）>
  `[subagents.efforts].{agent}`（新增）> `[models].default_reasoning_effort`（现状全局默认）。
- workflow 子 agent 与 spawn 链路是否同走 `resolve_subagent_sampling_config`：施工时核实，
  目标是不出现第二条解析路径。

### G4 主循环绑组（可选扩展，本专题不做）

`[session]` 或主模型选择允许引用模型组；手动 `/model` 仍最高优先。列为可选后续，要做另立项。

### G5 视觉桥消费（引用，不施工）

视觉桥的描述模型选择与 400 反应路径（vision-bridge plan.md D8/D9）引用模型组：视觉组条目须
`accepts_images == true`（能力位过滤，能力位来源含 D8 目录回填），降级语义同 G2。

## 拍板点（6）

1. **effort 优先级**：per-agent `[subagents.efforts]` 与组条目 `effort` 谁赢？建议 per-agent
   显式赢（意图更具体），组条目 effort 是组级缺省。
2. **降级触发类别**：是否含 401/403？建议不含（密钥错换供应商大概率也错，且会掩盖配置错误）。
3. **绑组配置形状**：独立 `[subagents.model_groups]` 表（建议，类型清晰、diff 干净）vs 复用
   `[subagents.models]` 值写 `group:<name>` 前缀。
4. **冷却生命周期**：会话级内存态（建议，v1）vs 跨会话持久。
5. **主循环绑组**：本专题不做（建议）；要做另立项。
6. **条目间 `context_window` 差异**：激活条目切换后 auto-compact 阈值与会话簿记按新条目重算——
   复用 model_switch 零回合重建还是轻量重算？施工核实后定，倾向复用。

## 分期

| 期 | 内容 | 粗估 |
|---|---|---|
| T1 | `[model_groups.*]` 解析 + 校验 + 组解析纯函数（shell config 层）+ 单测 | 1–1.5 天 |
| T2 | `[subagents.model_groups]` / `[subagents.efforts]` + `resolve_subagent_sampling_config` 组语义扩展 + handle_request effort 接线 | 1–2 天 |
| T3 | 降级链运行时：首字节前 failover（锚点：`request_task.rs` 重试循环 / 子会话 sampling 装配）+ sticky/cooldown + notice + unified.jsonl 事件 + 状态行联动 | 2–3 天 |
| T4 | i18n、user-guide `26-config-reference.md`、PATCHES 登记、回归（ctest 门控 shell 套件） | 0.5–1 天 |

## 验收标准

- **A（用户真实场景）**：组 `["cline-pass/deepseek-v4.1-flash", "z-ai/glm-5.3-flash", "glm-5.3-flash"]`，
  第一供应商断网/假 429 → subagent 自动落第二家；notice 可见；状态行与账本 model id 正确；
  第二回合直接用新条目（粘性），冷却过期后恢复首选。
- **B**：`[subagents.efforts] explore = "low"` → child `sampling.reasoning_effort = low`；
  组条目不支持 effort → warn + 忽略该覆盖，回合不失败。
- **C**：未知条目/坏组/空组 → warn + fall through 现有优先级；不 panic、不静默吞配置。
- **D**：`[subagents.models]` 与 `[subagents.model_groups]` 同 agent 并存 → warn + 单条目 pin 赢。
- **E**：i18n 0 挂；`ctest.sh -p xai-grok-shell --lib`（subagent/config 相关 filter）绿。

## 纪律

- 分支 `feat/local-subagent-model-group`；main 只收文档。**Ayu 主题分支
  （`feat/local-ayu-theme`，含 ghost-bubble 立案文档）按用户指示（2026-10-14）随本专题一起
  合并发版**，该分支工作区停车文件届时一并处置。
- 同步文件补丁登记 PATCHES.md；新环境族失败登记 `win-skip.txt`。
- 施工前对照 [fork-feature-inventory](fork-feature-inventory.md)「已有」判定（本设计已核：
  模型组无、subagent pin 有、effort 管线有——防重复建设已做）。

---

## 施工实况（2026-10-14，PATCHES 四十一期）

六拍板点按建议值落地：① per-agent `[subagents.efforts]` 赢过组条目 `effort`；② 401/403
不触发降级；③ 独立 `[subagents.model_groups]` 表；④ 冷却 = 进程级内存态（`OnceLock`
单例，跨会话共享，懒回收过期——比拍板建议的「会话级」更广，理由：冷却本意是让
「故障的供应商」在后续 spawn 中被跳过，跨会话才符合直觉）；⑤ 主循环绑组未做（G4 维持）；
⑥ 组条目沿用 `[model.<id>]` 自身的 context_window/簿记（无会话中途换条目，问题后移）。

**与设计的偏差（如实记录）**：

- **v1 降级语义 = spawn 级**：条目失败（任务尝试以 provider-unavailable 类错误终止）
  → 进程冷却 → 后续 spawn/子代理解析顺延下一可用条目。设计 G2 的「首字节前 failover +
  会话内粘性」需要会话中途换采样配置的簿记手术，留 v2。实际效果：失败任务的**下一次
  spawn** 自动落下一家；长任务进行中的失败仍整任务报错，由父模型自然重试触发降级。
- **降级触发点 = 子任务尝试终点**（`result.error` 保守签名匹配），sampler 层 429 重试
  预算耗尽后的错误串按 429/5xx/connection/timeout/quota 归类；400/401/403 永不触发。
- **披露面 v1 = unified.jsonl 事件**（`entry marked cooling` / `resolved past the
  first entry`）；transcript 用户可见 notice 留 v2。

**落地清单**：`[model_groups.<name>]` 配置（`ModelGroupConfig`/`ModelGroupEntry`，
cooldown 缺省 10 分钟）；`[subagents.model_groups]`/`[subagents.efforts]` 三张 skip 表 +
load 时 pin×组冲突 warn；解析优先级 spawn > pin > 组链 > AgentDefinition > 继承；组条目
effort 带能力门 + variants 路由；`ModelGroupBinding` 随解析返回、失败标记冷却。
