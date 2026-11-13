# 引入 TODO：2026-09-28 补充挖掘批次

> 来源：2026-09-28 对 qwen-code / MiMo-Code（本地克隆 `D:/pro/agent-cli/`）的第二轮边角特性挖掘 + 对本仓库源码逐项核对。
> 只收录「实用且 grok-build（含本 fork）确认没有、且不在 ADOPTION.md 采纳/拒绝清单里」的项。
> 现状核对基线：本仓库 2026-09-28 工作区；qwen-code 基准 `85631a3d`、MiMo-Code 基准 `50cd7139`（见 notes/）。

## ① 客户端循环检测 + 循环历史恢复 ⭐ P0（本批最高优先级）

- **现状对照**：本仓库的循环防护是**服务端驱动**的——`doom_loop.rs` 只消费 xAI API 专属的 `response.doom_loop_check` SSE 事件（`crates/codegen/xai-grok-sampler/src/doom_loop.rs`，`abort_triggers_requires_confidence_and_honors_disarm` 用例可证）；`stream_classify.rs` 只做内容/错误分类，无循环语义。**BYOK 第三方模型（本 fork 的核心场景）跑长任务没有任何客户端循环兜底**——传输层有重试/断路器（`retry.rs`/`xai-circuit-breaker`），语义层是空白。ADOPTION.md 未覆盖此维度。
- **来源**：
  - qwen-code `packages/core/src/services/loopDetectionService.ts`（~1700 行）：文本 chunk 重复 + 工具签名重复双通道检测，纯客户端。
  - MiMo-Code `docs/compose/spec/loop-streak-recovery.md`：**thinking 主导**的循环检测（同一思考块跨连续 assistant 消息重放、工具参数仅微漂移——文本/工具签名检测都抓不到；其本地 DB 实测 thinking 连续重放最长 661 条）；关键设计：**检测一次、把循环梗段持久化为 parent user 上的 `ignored`+`synthetic` part，后续每轮请求持续裁剪重放**——逐请求重检测会破坏前缀稳定（其 journey log 明确踩过）；`max_span` 默认 64；绝不自造 Continue user。
- **落点**：sampler 层挂检测器（对照 `doom_loop.rs` 的 signal 管道）+ `xai-chat-state` 历史裁剪（对照 `replace_history` 语义，注意与 ADOPTION 1.3 协议边界合并、2.1-1 前缀不变量测试的交互——裁剪 span 必须进前缀不变量测试）。
- **验收锚点**：qwen 的 loop 阈值常量、MiMo 的 `loop-streak.test.ts`（24 用例）可作移植测试蓝本。

## ② 工具批次防洪水 🟢 P2（从 MiMo 原设计大幅精简）

- **定调（2026-09-28）**：MiMo `toolcall-flooding` 原设计的三件套（防洪水 / 失败级联 / FIFO 并行门）**设计过度，只引入防洪水单件**——失败级联和 FIFO 门改动执行层语义、测试面大，收益不成比例，不引入。
- **现状对照**：本仓库无批次调用数上限（`grep max_tool|tool_call_limit|batch.*limit|flood` 于 `xai-grok-agent`/`xai-chat-state` 零命中）。
- **来源**：MiMo-Code `docs/compose/spec/toolcall-flooding.md`（已交付）：单批工具调用超过上限（MiMo 取 17）→ 不执行、取消整批、落取消结果、注入 system reminder 重申「1-3 个为宜、至多 8 个」、继续采样；取消路径照常保留 usage/cache 记账。
- **落点**：sampler/agent 层在进入工具执行前加一个计数判断即可；开关放 `experimental` 配置节（参照 `thinking_tag_scrub` 形态）。

## ③ 未提交改动 turn-end 软提醒 🟢 P2

- **现状对照**：`porcelain` 只出现在 `xai-fast-worktree`（worktree 同步），对话流无任何「脏树提醒」。
- **来源**：MiMo-Code `docs/compose/spec/uncommitted-hint.md`（已交付，默认关）：用户来源的 main turn 完成后，若工作区 git 树脏，注入合成软提醒（porcelain 输出 + **gitignore 优先**引导）；不设 session-once 硬上限（后续脏轮次可再提醒）；hook/spawn 来源的 turn 永不注入（fail-closed 防自激循环）；干净树即停。
- **落点**：turn-end 注入路径（对照 `/remember`/memory 注入管线）；配置走 `experimental.uncommitted_hint`。

## 落地顺序

①（P0，对准 BYOK 稳定性主命题）→ ②③（P2 小件，随手带）。每项引入后按 ADOPTION.md 惯例在本文勾选并注明落点 PR/commit。
