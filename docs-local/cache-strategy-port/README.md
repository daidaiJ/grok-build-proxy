# 缓存策略优化移植（prompt-cache 友好化）— 调研汇总

> **状态：已拍板开工（2026-10-24）。** 回答三个问题：① 为什么 zcode 开源版
> 对 DeepSeek v4.1 的缓存命中率比 opencode v2 高；② grok2 客户端"上下文占用突然
> 减少"（100–300k 区间）是什么机制、对缓存意味着什么；③ dsh（DeepSeek 官方
> harness）的缓存适配源码验证结论。三块合流出一个移植方向判断。
> 基线：zcode `main 3c5b2276`（2026-10-20 sync，v3.16.2 系）、opencode dev
> `22e771f`（v2.0.x，2026-10-17）、deepseek-ai/deepseek-harness `main 22ddee37`
> （2026-10-23，本地 clone `D:\CODE\ai\deepseek-harness-upstream`）。本 fork 基线
> `main fd3e867`。

## 用途与范围

- 对象：harness↔模型接口层的 prompt cache（DeepSeek 前缀 KV cache 为主口径）
  友好化机制对照与移植选型。UI/统计口径只在与命中率观测相关的范围内涉及。
- 前置纪律对照（fork-feature-inventory）：in-history / addition-only /
  reasoning 回传 fork 均无（dsh-port README 已判定）；**工具结果裁剪为 fork
  独有**（zcode/opencode 均无任何历史中途改写机制）——这是本次新澄清的行。
- 关联文档：[`../dsh-port/README.md`](../dsh-port/README.md)（dsh 适配预研与
  P1 设计原型 [`../dsh-port/context-engineering-design.md`](../dsh-port/context-engineering-design.md)）、
  `../prompt-cache-notes.md`（Ark 缓存键实测）、`../limit-inference.md`
  （limit-probe 验收设施）。

## TL;DR

1. **zcode 的命中优势是结构性的**：稳定/动态段显式分层 + 动态内容一次性冻结进
   append-only 历史 + 工具说明不镜像进 system prompt + 主会话与全部子代理共享
   同一段跨会话字节一致的稳定头 + sidecar（title 等）串行化。opencode v2 在
   "会话内前缀稳定"上同样达标（其 InstructionState 增量机制甚至与 dsh 的
   in-history 哲学一致），差异集中在**跨请求前缀共享面**（头部内嵌工具派生内容、
   子代理 system 各自独立、title 并发小前缀请求）。
2. **grok2 的"上下文占用突降"是设计行为且是缓存断崖**：工具结果自动裁剪
   （>50% 窗口，静默）+ auto-compact（85%，有流程）。裁剪原地改写请求里的旧
   tool result → 第一个被改写点之后整段前缀作废且永不恢复——fork 的 DS 会话
   一旦过窗口一半，命中率断崖跌落到会话结束。
3. **dsh 缓存设计全部源码验证**（B 级转述升级 A 级）：in-history 系统提示、
   addition-only 工具块、thinking 签名回传、确定性图片（Files API 复用）、
   归因头（稳定匿名 user-id + session-id）；整套协议绑定在
   **Anthropic 兼容端点** `api.deepseek.com/anthropic`，第三方 CLI 走的
   `/chat/completions` 拿不到这套能力。
4. **移植方向**：fork 已有 dsh-port P1（in-history 能力位）设计在案；本次新增
   两个候选——**追加式裁剪改写**（把 50% 线的缓存断崖变成追加失效标记）与
   **sidecar 串行化/共享头**（低成本对齐 zcode）。详见 §4。

## 1. 三方机制对照矩阵（源码直读，锚点为 file:line）

| 机制 | zcode | opencode v2 | grok2（fork） | 缓存影响 |
|---|---|---|---|---|
| system prompt 分层 | stable/dynamic 两档 + meta_user 附件，动态段拆为后面的独立 system 消息（`apps/zcode-cli/packages/core/src/context/builder.ts:240-286`，排序 `:319-334`） | system = [agent 提示（内嵌工具名条件指引）, initial（日期+env+instructions）]（`packages/core/src/session/model-request.ts:110-117`） | 单条 system 固定 messages[0]，任何变更整段前缀作废（`dsh-port` §2 现状锚点） | 动态内容变化时的失效半径 |
| 日期/env 注入点 | meta_user 附件，一次性追加进 append-only 历史（`sections/current-date.ts:18`、`runtime/methods/context-history-entries.ts:10-15`）；git 状态为会话起始快照（`sections/env-info.ts:23`） | instructions 内建项，变化渲染成历史中 delta 消息（`instructions/builtins.ts:29-30` `changed`）——in-history 哲学 | system 内；变更即改写头部 | 稳态均稳定；变更时 zcode/opencode 只失效追加段 |
| 工具说明 ↔ system | **不镜像**（`builder.ts:48-49` 注释明示），工具变动只动 tools 数组 | system 首段内嵌 shell/write/edit 条件指引（`session/system-prompt.ts` `render()`）+ CodeModeInstructions 工具目录排在 instructions 首位（`context.ts:150-151`） | 无镜像（`xai-grok-pager` 侧同上游） | tools 数组变动两边都全废；zcode 保证无二次文本破坏 |
| 工具序确定性 | 注册序（`tool/registry.ts` Map 插入序） | 定义序（`Object.fromEntries` Map 保序） | 注册序 | 均确定 |
| 历史中途改写 | **无** | **无**（compaction 除外） | **有**：工具结果裁剪（§2）+ compact + integrity repair | fork 独有的断崖源 |
| 子代理前缀共享 | 主+子代理共享同一段稳定头（E2E 用例 I21 固定"ZCode prefix / agent-specific / common Notes / env-model context"四段，`docs/conversation-session-case-catalog.md`） | 每个 agent 独立 `agent.system`（`model-request.ts:110-113`），不共享 | 子代理独立前缀 | 聚合命中率：zcode 把多请求集中到同一缓存前缀 |
| sidecar 时序 | title 在主回复完成后串行（case V01） | title 与主回合**并发** fiber（`session/runner/llm.ts:54-55,177-180`），`model.small` 同 provider 回退同一模型 → 并发小前缀请求 | compact/preflight 混在回合内；title 独立 | opencode 增加缓存压力与口径噪声 |
| DeepSeek 特判 | 无（不发 cache 字段，纯靠前缀稳定；仅 anthropic 适配层透传 cache_control） | `requireReasoning` 恒真 → assistant 全量回传 `reasoning_content`（`ai/src/protocols/openai-chat.ts:566-569,452-501`）；`prompt_cache_key` 对 deepseek 不发（`providers/deepseek.ts` 无 `supportsPromptCacheKey`） | `prompt_cache_hit_tokens` 取 max 进 Usage（`xai-grok-sampling-types`）；无回传 | 确定性序列化，非命中差异主因 |
| fork/嵌套会话 cache key | fork 共享稳定前缀（"fixed stable fork prefix" 验收用例） | TODO：嵌套 fork 不复用根会话 cache key（`model-request.ts:294`） | — | 对 DeepSeek 无影响（不发 key） |
| 命中率测量口径 | main-turn 聚合、token 加权、排除 sidecar（`docs/context-cache-hit-rate.md`） | 无专门面 | limit-probe（被动落盘 prefix hash + cached_tokens，`../limit-inference.md`） | 跨 CLI 对比需统一到网关/探针口径 |

**差距归因（按证据强度）**：单会话稳态下三家都应接近 DeepSeek 高命中
（本仓实测 98.7%，`prompt-cache-notes.md`）；能造成"高不少"的是——
① 聚合口径下的**跨请求前缀共享面**（zcode 全部会话+子代理共享稳定头，头部与
工具集解耦；opencode 头部随会话/代理/工具变化）；② opencode 的并发 sidecar
小前缀请求；③ 测量口径差（zcode UI 数字天然偏高，须以网关/探针原始
cached_tokens 复核）。

## 2. grok2 现场观察：上下文占用突降 = 缓存断崖（本次新发现）

用户观察：会话在 100–300k 区间显示占用突然减少。机制已锚定：

- **工具结果裁剪（静默）**：`xai-chat-state/src/actor/request_builder.rs:131`
  `should_prune` = `total_tokens > context_window / 2` → 每轮请求构建时
  `prune_conversation`（`:137`）把 3 轮外旧 tool result 软裁（>4000 字符留头尾
  各 1500）或硬清空（≥10 轮旧 → `[Tool result omitted — too old]`；默认参数
  `xai-chat-state/src/types.rs:83-95`）。另有保留会话路径：每条 user 消息入账后
  `prune_retained_conversation` 硬清理会话本体（`actor/mutations.rs:338`，
  `updates.jsonl` 不动）。显示骤降链路：跨 50% 线 → 下轮请求实发裁剪后小请求 →
  provider 回报 `prompt_tokens` 变小 → `record_token_usage`
  （`mutations.rs:436-441`）一步下调且不回升。无任何界面提示。
- **auto-compact（可见）**：`check_auto_compact_needed_inner`
  （`xai-grok-shell/src/session/compaction.rs:2182`）85% 阈值（默认，
  `util/config/resolve/compaction.rs:3`），触发后历史改写为摘要 + reseed
  （`mutations.rs:548-583`）。
- 100–300k 落点 = 各模型 `context_window` 的 50%/85% 线（256k→131k、400k→200k、
  512k→256k…）。
- **缓存语义**：请求层裁剪原地改写请求里的旧 tool result 字节 → 从第一个被改写
  点起整段前缀作废，之后每轮都断——即 fork 的 DS 会话过窗口 50% 后命中率断崖、
  到会话结束不恢复。这与 §1 两家"无历史中途改写"形成 fork 独有的第三变量。
- 现有可调项：`[compaction.pruning]`（enabled/keep_last_n_turns/soft_trim_*/
  hard_clear_age_turns，`agent/config.rs:582,2015`）、
  `GROK_AUTO_COMPACT_THRESHOLD_PERCENT`、per-model
  `auto_compact_threshold_percent`。关 pruning 保缓存 = 提前撞 85% compact。
- **待取证**（C 级，观察自用户现场，未跑探针）：limit-probe 复核
  "占用骤降那轮 cached_tokens 断崖"，同时区分裁剪与 compact 两臂。

## 3. dsh 缓存设计源码验证（A 级，基线 `main 22ddee37`）

本地 `deepseek-harness-codearts` 为 CodeArts 登录插件（非 harness 本体）；官方
仓库已浅克隆 `D:\CODE\ai\deepseek-harness-upstream` 供后续核对。验证结论
（`packages/llm/llm-deepseek/`）：

| 机制 | 锚点 | 状态 |
|---|---|---|
| `systemPromptUpdate: 'in-history'`：系统快照追加在最近 user/tool-result 轮之后，端点把任意位置最新 system 读作完整生效提示 | `src/models.ts:14`；`src/serialize.ts:83-130`（`flushSystemUpdates`） | ✅ 与 dsh-port 文档一致 |
| `toolUpdate: 'addition-only'`：`tool_addition`/`tool_removal` 引用块 + beta header | `src/models.ts:15`；`src/serialize.ts:95-96`；`src/adapter.ts:132-145`（`anthropic-beta: mid-conversation-tool-changes-2026-07-01`） | ✅ |
| thinking 签名回传：reasoning 块以 `thinking` + 原生 `signature` 回放，跨模型不可移植 | `src/serialize.ts:34-38`；`src/replay.ts` | ✅ 新增细节 |
| 确定性图片：Files API 上传复用 + `file_id` 引用 + `anthropic-beta: files-api-2025-04-14`；表示漂移/路径改写/路由变化从首受影响 token 失效 | `src/files-api.ts:151`；README "KV Cache effect" | ✅ |
| **归因头（新发现，第三方 CLI 均未发）**：模型请求带 `x-deepseek-harness-user-id`（稳定匿名 id）+ `x-deepseek-harness-session-id` | `src/adapter.ts:146-147` | ✅ 新增行 |
| **端点绑定（结构性事实）**：整套缓存友好协议族（in-history / tool_addition / thinking signature / Files）都在 Anthropic 兼容端点 `https://api.deepseek.com/anthropic` 的 Messages API 上；`/chat/completions` 无此方言 | README 默认 baseURL | ✅ 强化 §3.2 probe 门判断 |
| cache 指标映射：`cache_read_input_tokens` 进 usage | `src/translate.ts:36,159` | ✅ |

对 fork 的落点不变：dsh-port P1（能力位 + in-history 放置 + 三 backend probe +
limit-probe A/B）依然成立，官方 main 至 10-23 机制无变化。**注意**：in-history /
addition-only 的端点语义只对 Messages（Anthropic 兼容）端点有官方背书，fork 的
deepseek 条目实际走 chat_completions/responses——probe 门（dsh-port §3.2）
仍是 P1 前置。

## 4. 移植候选与分期建议（供拍板）

| 顺位 | 候选 | 内容 | 依赖/关联 | 规模（C 级粗估） |
|---|---|---|---|---|
| P1 | dsh in-history 能力位（已有设计） | `system_prompt_update = "in-history"` per-model 能力位 + 三 backend probe + limit-probe A/B | dsh-port P1 设计原文，零新增设计 | 净开发 3–5 人天 |
| P2 | **追加式裁剪改写**（本次新增） | 对 `prompt_cache_update` 类能力模型（或全局开关），工具结果裁剪从"原地替换"改为"追加失效标记块"（dsh addition-only 同构思想）：请求里旧 tool result 保留原字节，其失效声明作为后续追加块发送；50% 线之后缓存可续命 | 与 P1 共用放置/确定性设计（§3.3 派生式 vs journal 直记）；交互面：`[compaction.pruning]` 语义分叉 | +2–3 人天（叠在 P1 上） |
| P3 | sidecar 串行化 + 共享头对齐 | title 等 sidecar 改主回复后串行（对齐 zcode V01 语义）；子代理稳定头共享面评估 | 纯 shell/pager 小件，收益集中在多子代理/多会话聚合场景 | ≤1 人天 |
| P3' | 归因头实验 | `x-deepseek-harness-user-id`/`-session-id` 对齐 dsh 发送（作用未证实，纯跟随官方） | 待 probe：header 是否影响缓存路由/限额 | ≤0.5 人天 |
| P4 | **工具集会话内冻结 + 稳定序**（2026-10-24 追加，pi #10720 思想客户端化，见 §7.3） | 会话生命周期内工具数组字节冻结：初始集固定、新工具只追加尾部、过滤项（plan mode 剥 cursor 工具 `sampler_turn.rs:531` / Messages 后端剥 use_tool 文件表单 `sampler_turn.rs:539`）不再剥数组；MCP 晚连接追加不重排 | pi #10720 机制不可移植（DS 无动态工具加载），纯客户端拼装；7.2 实测已给出跨会话 `tools_hash` 漂移现场 | 2–3 人天 + probe 前置（DS 缓存前缀是否含 tools 数组） |

**建议验收**（全部复用 limit-probe，零新增设施）：同一会话 A/B（机制 on/off），
指标 = 触发点后首轮 `cached_tokens/prompt_tokens`、TTFT、`/stats` 成本；场景选
触发密集型（50% 线跨越 + 模型切换/工具集变化）。

## 5. 待拍板决策

> **拍板结果（2026-10-24，按交接文档建议值执行）：D-a 做（P2 随 P1 同批）；
> D-b 复用 `[compaction.pruning]` enabled + per-model 能力位门控（对齐 use_proxy
> 先例）；D-c P3/P3' 随批顺带、归因头先 probe；D-d 顺位进 roadmap §6。**
> probe 门已过：四象限两连发实验全部通过（DeepSeek 官方三 backend + Command
> Code 网关主通道），结果与放行矩阵回填在
> [`../dsh-port/context-engineering-design.md`](../dsh-port/context-engineering-design.md)
> §3.2——chat_completions / responses 放行，messages backend 暂不放行
> （fork 折叠转换层 + 端点降级双原因）。

| # | 决策 | 建议 |
|---|---|---|
| D-a | P2 追加式裁剪做不做（当前断崖只影响 >50% 窗口的长会话） | 用户 DS 长会话是主场景，做；随 dsh-port P1 同批 |
| D-b | P2 与 `[compaction.pruning]` 现有配置的关系（新开关 vs 复用 enabled） | 复用 enabled + 新增 per-model 能力位门控（对齐 use_proxy 先例） |
| D-c | P3/P3' 是否立项 | 低成本随手件，随批次顺带；归因头先 probe |
| D-d | 施工顺位 | 进 port-roadmap §6 与现有候选一起拍板 |

## 6. 来源

- zcode：`D:\CODE\ai\zcode`（`main 3c5b2276`）——`apps/zcode-cli/packages/core`
  context builder / message-history / tool registry；`docs/context-cache-hit-rate.md`、
  `docs/conversation-session-case-catalog.md`（I21/V01）。
- opencode：`D:\CODE\ai\opencode`（dev `22e771f`）——`packages/core/src/session/*`、
  `packages/ai/src/protocols/openai-chat.ts`、`packages/ai/src/providers/deepseek.ts`、
  `packages/ai/src/cache-policy.ts`（对内联标记协议生效，DeepSeek 走隐式前缀缓存）。
- dsh：`deepseek-ai/deepseek-harness`（`main 22ddee37`，clone 于
  `D:\CODE\ai\deepseek-harness-upstream`）——`packages/llm/llm-deepseek/`。
- pi：`earendil-works/pi`（tag `v1.3.0` `71023ae`，2026-10-15，clone 于 `D:\CODE\ai\pi`）
  ——§7.1 三特性源码核对；官方 changelog pi.dev/changelog/releases/1.3.0。
- fork：`xai-chat-state`（pruning/mutations/request_builder）、`xai-grok-shell`
  （compaction/config resolve）、`docs-local/prompt-cache-notes.md`、
  `docs-local/limit-inference.md`。
- 用户现场观察（2026-10-24）：上下文占用 100–300k 区间突降；未跑探针取证。

## 7. pi 1.3.0 三特性适配评估 + Command Code 网关实测（2026-10-24 追加）

> 背景：pi 1.3.0 发布三件缓存/上下文优化（切 effort 不破缓存 / 切模型剔除旧
> thinking / Kimi 中途加工具不破缓存）。逐行核对源码后评估对 DS 的适配性，
> 并用 Command Code 网关实测取证。本节为独立调研追加，不改变 §4 既有拍板。

### 7.1 三特性判定（源码直读，clone `D:\CODE\ai\pi` tag `v1.3.0`）

| pi 特性 | 机制（源码锚点） | 对 DS/fork 判定 |
|---|---|---|
| 切 effort 不破缓存（#10853） | `reasoning.effort` 固定，改档走历史内 `configuration_update` 输入项追加回放：每条记录 `providerThinkingLevel` 的旧回复前插当时档位、尾部补当前档位，历史字节永不变（`packages/ai/src/api/openai-responses-shared.ts:226`；仅 openai-responses + API key，ChatGPT 登录态排除） | **不适用，收益 0**。DS 前缀缓存键只含 messages 前缀，请求级 `reasoning_effort` 不进缓存键——问题本身不存在；且 DS chat_completions 无消息流内配置通道。该特性只对 fork 的 responses 后端模型（gpt-6-luna 等）有理论价值，与 DS 无关 |
| 切模型剔除旧模型 thinking（#10995） | 所有 API 共用预处理 `transformMessages`：非同模型（provider+api+model 三元组）的 thinking 块直接丢弃（`packages/ai/src/api/transform-messages.ts:128-140`）；签名/加密推理例外保留，跨模型 text 签名与 toolCall `thoughtSignature` 一并剥 | **适配、建议做，但收益是行为质量 + token 体积，非命中率**——切模型必换缓存池，切换点注定冷启动。fork 现状：`ConversationItem::Reasoning` 兄弟项出站时**无条件**折叠进 `reasoning_content`，不看产出模型（`xai-grok-sampling-types/src/conversation/chat_completions.rs:192`）；DS 官方对输入 messages 携带 `reasoning_content` 有拒收/不计入口径（reasoner 系 400）。落点：`conversation_to_chat_messages` 按产出模型剥离异源 Reasoning |
| 中途加工具不破缓存（#10720） | kimi-coding 切 Chat Completions（官方 Kimi Code CLI 同款端点）+ 端点动态工具加载：顶层 `tools` 冻结为初始集，新增工具以带 `tools` 字段的 system 消息锚在历史原位（`packages/ai/src/api/openai-completions.ts:1250-1259`）；只支持追加，删除/同名重定义回退全量（`transcript.ts` `hasNonAdditiveToolChanges`） | **思想适用、机制不可移植**——依赖端点接受工具型 system 消息，DS 无此协议面。客户端自实现 = 工具集会话内冻结 + 只追加，新增 §4 P4（设计见 §7.3）；思想与 dsh addition-only 块同构 |

### 7.2 Command Code 网关实测（A 级，2026-10-24，curl 直测 + v1.0.68 headless）

- **协议覆盖**：`api.commandcode.ai/provider/v1` 同时提供 chat/completions 与
  responses 两协议，DS 模型（deepseek-v4.1-flash）两边都通。fork 侧三条 CC 条目
  配置显式 `chat_completions`，代码缺省也是 `ChatCompletions`
  （`xai-grok-sampling-types/src/types.rs:1155`）——"协议转换"在配置层已是事实。
- **responses 端点（DS 模型）可用**：非流式返回标准 `response` 对象（`reasoning`
  summary 项 + `message` 项；非标附加顶层字段 `thinking:{type:"enabled"}`，无害）；
  流式事件族全标准（`response.created/in_progress/output_item.*`/`response.completed`
  + `function_call` / `function_call_arguments.delta` 全家），**本次未出现
  2026-10-02 记录的非标 `response.reasoning.delta`**（当时疑似 glm 路径，DS 路径干净）；
  工具调用可用；usage 带 `input_tokens_details.cached_tokens` + `reasoning_tokens`。
- **responses 前缀缓存实证**：同大 prompt（instructions ≈1161 token）连发两次：
  第一次 cached 0 → 第二次 **cached 1152**（18×64 块，尾部 9 token 未入块）。
  CC responses 后端对 DS 有真实前缀缓存且如实上报。
- **chat_completions 缓存实证**：v1.0.68 headless 轮次 `cached_prompt_tokens=10240`
  （跨会话稳定命中系统提示段；CC 网关透传，fork 的 DeepSeek 平铺字段解析
  `prompt_cache_hit_tokens` max-in 正常吃到）。
- **现场证据：工具集跨会话不稳定（P4 直接实证）**——同模型同工作区两次独立
  headless 会话 `tools_hash` 不同（`06860b92…` vs `899016cc…`），prompt_tokens 差 2：
  headless 工作区加载 3 个 MCP，约 40k token 的工具块每次会话字节都在变，跨会话
  DS 前缀缓存在这块全灭，两次稳定命中的只有系统提示段（10240）。

### 7.3 P4 设计要点：工具集会话内冻结 + 稳定序

- **冻结范围**：会话生命周期内出站 `tools` 数组字节不变——初始集在首回合定版，
  之后新工具（MCP 晚连接、deferred exposure T2 的 `search_tools` 加载）只**追加**
  到尾部，禁止删除/重排/同位重定义（与 dsh addition-only 同一约束）。
- **过滤项改造**：plan mode 剥 cursor 工具（`sampler_turn.rs:531`）与 Messages
  后端剥 use_tool 文件表单（`sampler_turn.rs:539`）这类随请求翻转的过滤，改为
  不动数组、在工具 description 内标注（或预先在会话建立时定版）。
- **probe 前置**：DS 缓存前缀是否包含 `tools` 数组（官方文档无明确口径）。
  limit-probe A/B：稳定工具集 vs 中途换工具集，看 `cached_tokens` 跳变与
  `tools_hash` 的对应关系。7.2 已证数据面可用（CC 两协议都回传 cached_tokens）。
- **验收**：复用 limit-probe——跨会话 `tools_hash` 恒等 + tools 块段
  `cached_tokens` 从 0 变正。

### 7.4 limit-probe 设施两 bug（本次实测发现，建议随 P1 批次顺手修）

- **首跑静默失效**：`xai-grok-sampler/src/limit_probe.rs` `append_line` 只
  `create(true)` 建文件不建父目录，`~/.grok/limit-probe/` 不存在时每次写入
  静默失败、永不落盘（本次手动 `mkdir` 后恢复）。修法一行 `create_dir_all`。
- **门与文档不符**：`ENABLED` 实为显式 opt-in（env 未设置即关闭，
  `limit_probe.rs:36`）；`limit-inference.md` 与 AGENTS.md 均写"默认开、
  `GROK_LIMIT_PROBE=0` 关闭"。验收跑 probe 须显式 `GROK_LIMIT_PROBE=1`；
  两处文档按实现改口径（或代码改回默认开，二者取一）。
