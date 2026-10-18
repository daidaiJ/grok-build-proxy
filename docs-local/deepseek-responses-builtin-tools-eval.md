# DeepSeek 官方 API 内置 websearch/webfetch × responses 协议 — 评估（调研完成，未立项）

> 2026-10-18 立题：`deepseek/deepseek-v4.1-flash-1017` 等 DeepSeek 官方 API 模型启用时，
> 用 `api_backend = "responses"` 能否自动使用官方最新提供的内置 websearch / webfetch 工具。
> 结论分两半：**协议与管线兼容（接近全通），但不自动——卡在 fork 自己的门控位与供应商档位**。
> 官方侧（Search/Fetch 指南计费节）明确把 dsh / Claude Code / Codex 宿主的 server 端
> 工具访问当作标准接入路径（当前免费，2026-11-01 起按次计费）——那些宿主「自动内置」
> 是因为**宿主已经内置了工具声明**；fork 扮演同一宿主角色，声明被自家门控拦下，
> 放行即等价。

## 结论

1. **web_search（responses 内置，服务端执行）**：fork 的 responses 管线**已经完整具备**
   下发、事件呈现、结果落地面（为 xAI 自家 backend search 所建，DeepSeek 用同一套
   OpenAI Responses 方言，事件名逐字命中）。但不自动生效，三道闸见下。
2. **webfetch**：DeepSeek **没有** responses 协议内的 fetch 工具类型。官方的「网页抓取」
   是独立 REST API（`POST /v1/tools/fetch`，与 Search API 并列，按次计费）——无论协议怎么
   接都不会「自动」出现，要用只能做客户端工具后端对接（fork `web_fetch` 工具换源）。
3. **用户当前模型是 flash 档**：官方口径 `deepseek-flash` 现由 DeepSeek-V4.1-Flash-1017
   服务，而 responses 内置 web_search **仅 `deepseek-v4-pro` 支持，deepseek-flash 下静默
   忽略**。即当前模型即使打开全部闸门也不会触发搜索；要试需换/加 v4-pro 条目。

## 三道闸（为什么现在「不自动」）

按请求组装顺序：

1. **agent 级 feature 已默认开**：`features.backend_tools`（env `GROK_BACKEND_SEARCH`，
   默认 `true`）——`crates/codegen/xai-grok-config-types/src/registry.rs:249-256`。
   非闸。
2. **per-model 能力位（主闸）**：`ModelInfo.supports_backend_search` 默认 `false`
   （`xai-grok-shell/src/agent/config.rs:4071`；内建表中仅 Grok 4.6 为 true，见
   `xai-grok-models/default_models.json:15`）；`backend_search_active()` =
   agent 开关 AND 该位（`acp_session_impl/sampler_turn.rs:417-438`）。BYOK DeepSeek
   模型恒 false → `hosted_tools` 恒空 → 工具声明根本不进请求。用户今天就能在
   `[model.<id>]` 写 `supports_backend_search = true` 手工打开
   （`ConfigModelOverride.supports_backend_search` 已支持，config.rs:4220）。
3. **组装 vendor 盲（开了会混入垃圾声明）**：builder 对 backend search 一律同时推
   `HostedTool::WebSearch` 与 `HostedTool::XSearch`（`xai-grok-agent/src/builder.rs:1357-1369`）。
   `x_search` 是 xAI 专有类型；DeepSeek 官方文档 tools 表称「其他类型忽略」，预期被
   静默吞掉（无害但脏，未实测）。**若立项自动化，这里必须按供应商分流**。

## 现成链路盘点（协议侧为什么「接近全通」）

| 环节 | 落点 | 状态 |
| --- | --- | --- |
| 工具声明 | `HostedTool::WebSearch` → `WebSearchOptions::to_tool_entry()` 产出 `{"type":"web_search"[,"filters":{…}]}`（默认无 filters），raw JSON 经 `splice_extra_tool_entries` 拼进请求体 | 与官方示例 `tools:[{type:"web_search"}]` 逐字一致（sampling-types `tool_overrides.rs:212-`、sampler `client.rs:206-224`） |
| 流事件 | `response.web_search_call.in_progress/searching/completed` → `ResponseWebSearchCall{InProgress,Searching,Completed}` → `SamplingEvent::BackendToolCallStarted/Completed` | 官方事件表与 async-openai 枚举同名；`stream/responses.rs:506-569` 全处理 |
| 结果 item | `ResponseOutputItemDone(WebSearchCall)` 携带 query+来源 URL | 同上 |
| 呈现/状态 | `BackendToolCallCompleted` → ACP tool call（`meta.backend=true`，status 按 payload，headless `web_search_tool_result_error` 分支） | `acp_session_impl/sampling_events.rs:539-` |
| 客户端工具让位 | hosted wire_name 撞名的 function 工具从请求剔除；压缩/快照同步剥 | `sampling-types/conversation/responses.rs:309-326`、`compaction.rs:1077-1083` |
| 无冲突杂项 | `no_inline_citations` include 仅对 trusted xAI route 注入（DeepSeek 不带）；DeepSeek 不支持的参数（include/store 等）静默忽略；无状态 API 与 fork 全量重放模型天然兼容 | `agent/config.rs:5388-5398` |

对 BYOK 用户这是**唯一可用**的联网搜索路径：客户端 `web_search` function 工具走 xAI
search API（`WebSearchConfig` 带会话凭据），DeepSeek-only 用户没有可用凭据。

## 官方文档要点（2026-10-18 抓取核实）

- **计费（官方 Search/Fetch 指南计费节原文）**：Search / Fetch 均按请求次数计费，每 1,000
  次成功请求 5 元；只有 HTTP 200 且产生结果才计费（失败/无结果/超时不计费）；账号级 QPS
  各 5。**关键句：「当前用户在 DeepSeek Harness/Claude Code/Codex 中使用 DeepSeek API 时，
  已经会通过 Server 端工具调用访问到 Search/Fetch 接口，这部分 Search/Fetch 接口调用目前
  未计费，并将于北京时间 2026 年 11 月 1 日 00:00 起按次收取调用费用。」**——官方把
  宿主侧 server 端工具访问视为既成标准接入路径，底层引擎与独立 REST API 同源、同价。
  https://api-docs.deepseek.com/zh-cn/guides/search
- Responses API：`base_url https://api.deepseek.com`，模型 `deepseek-flash` / `deepseek-v4-pro`。
  `tools` 部分支持：`function` 支持；**`web_search` 仅 `deepseek-v4-pro` 支持（服务端执行，
  结果入上下文继续生成），`deepseek-flash` 下忽略**；`custom` 仅 apply_patch；其余
  （file_search/code_interpreter/computer_use/mcp）忽略。事件表含
  `response.web_search_call.*`（注明仅 v4-pro）。上一次响应的 `web_search_call` item
  **可回传 input**（接口还原搜索结果拼进上下文）。
  https://api-docs.deepseek.com/zh-cn/guides/responses_api
- **Codex 官方接入指南同样口径**：「内置联网搜索仅支持 deepseek-v4-pro，一键脚本默认将其
  禁用」——官方适配宿主也没有对 flash 开内置搜索。
  https://api-docs.deepseek.com/zh-cn/quick_start/agent_integrations/codex
- Anthropic 兼容端点（`/anthropic`，dsh / Claude Code 走的通道）：输出 item 表
  `server_tool_use` / `web_search_tool_result` 标「支持」——dsh web profile 实测发
  `web_search_20250305` 服务端工具（GH discussion 408 抓到请求体）。
- 更新日志 2026-10-17：「开放自研搜索 API」+ V4.1-Flash-1017 快照（API 调用方式与价格
  不变，仍用 `deepseek-flash`）+ 针对 DeepSeek Harness/CodeBuddy/WorkBuddy 优化
  （升级 Harness ≥0.2.1-rc.1）。
- **v4-pro 的存续风险（2026-10-18 追查）**：官方曾于 2026-09-10 邮件通知 V4 Pro API
  服务 09-14 下线，09-11 因用户反弹**撤回决定**承诺继续服务（PANews/科创板日报、
  Reddit r/SillyTavernAI）；HF 权重也走过「下架→重新上架」一遭。今日价格页 v4-pro
  仍在售（V4-Pro-0813，并发档 500 vs flash 2500），但「唯一支持内置 web_search 的
  模型 = 经历过下线惊魂的旗舰档」这一事实，把**独立 Search/Fetch REST 集成**（与模型
  无关、flash 可用、计费透明）推向更稳的主路径；responses 内置 web_search 定位为
  v4-pro 专属增强。若官方后续把 web_search 下放到 flash（或 v4-pro 真退役），
  本评估的 D1/D2 方案不受影响（门控放行逻辑不变，只是受益面变化）。
- 待核清零：Search/Fetch 独立 REST 计费 = 上述 5 元/1000 次成功请求（官方原文，此前仅
  X 帖口径）。

## 实测验证（2026-10-18 10:41，官方 API 直打）

用本机官方 key 对 `https://api.deepseek.com/responses` 发真实请求
（`tools:[{"type":"web_search"}]` + 「联网搜索 2026-10-17 更新日志」prompt，
非流式，`max_output_tokens=1200`）：

| 模型 | output item 序列 | 判定 |
| --- | --- | --- |
| `deepseek-flash`（=本机 V4.1-Flash-1017 的 wire 名） | `reasoning, message`——模型回复「抱歉，这次我不能可靠地完成联网搜索…」 | **声明被收下但忽略，未执行** |
| `deepseek-v4-pro` | 19 个 item 含 **10 个 `web_search_call`**（`status=completed`，`action={type:"search",queries:[…]}`），message 按搜索结果总结更新日志 | **服务端真实执行** |

两个副产品：

- **item 形状**：`{type,id,status,action:{type,queries}}` 与 OpenAI Responses
  `web_search_call` 同构（无 results 数组——结果由服务端注入上下文），async-openai
  `OutputItem::WebSearchCall` 可直接反序列化，fork 管线兼容面再次确认。
- **成本**：v4-pro 单回合搜索把 input 顶到 **59,548 tokens**（本次高峰时段计费
  ≈0.54 元 input + 0.06 元 output，探针总耗 ≈0.6 元）——搜索结果整包注入上下文，
  pro 档单价下每搜索回合成本显著；flash（0.02–2 元/M input）+ 独立 Search/Fetch API
  的组合在成本结构上占优。

「web_search 仅 v4-pro 执行」自此为**文档 + 实测双源确认**，非文档孤证。

## 若立项「自动适配」（拍板点前置，净开发小）

- **D1 供应商判定**：hosted tools 组装按 `credentials.base_url` host 分流
  （`api.deepseek.com` → 仅 `web_search`；xAI/trusted → 现状双发）。host 匹配可直接复用
  四十九期 provider-usage 的判定先例（`xai-grok-provider-usage`，v1.0.62）。
- **D2 能力位语义**：`supports_backend_search` 现语义 = 「xAI backend search」。两案：
  a) 复用该位 + D1 分流（用户配 `supports_backend_search = true` 即启用，语义文档化）；
  b) 新增 `hosted_search_vendor: auto|xai|deepseek|none`（auto = host 判定，真「自动」）。
  倾向 a（改动面最小）起步、b 随需求。
- **改动面**：sampler / sampling-types / pager **零改动**（管线已通）；shell 只动
  `builder.rs` 组装段 + config docs + 测试。净开发 ≈0.5–1 人天 + 实测。
- **披露**：flash 档「声明被忽略」对用户不可见——宜在 /model picker 或文档标注档位要求，
  避免开了开关没效果来报障。

## 零代码试法（今天就能验）

1. 加 v4-pro 条目：`[model.ds-v4-pro]` `model = "deepseek-v4-pro"`、
   `base_url = "https://api.deepseek.com"`、`api_backend = "responses"`、
   `api_key`/`env_key` 同官方 key、`supports_backend_search = true`。
2. 该模型发起需联网的提问，看是否出现 backend web_search 调用块（ACP `meta.backend`）。
3. 预期：`x_search` 声明被 DeepSeek 静默忽略；若上游 400（与文档口径不符）再评估 D1 分流。

## 待实测 / 待核清单

- [ ] v4-pro + `supports_backend_search=true` 实测：搜索触发、`web_search_call` 呈现、
      `x_search` 是否真被忽略（400 或忽略）。
- [ ] `web_search_call` item 在 fork 历史重建中的去留（回传可还原 vs 丢弃，多轮上下文保真）。
- [ ] `web_fetch` 换源 DeepSeek Fetch API 的增值评估（fork 自带抓取已可用，DeepSeek 增值 =
      服务端渲染/反爬穿透），随需求另议。
- ~~Search/Fetch 独立计费~~已核：官方计费节 5 元/1000 次成功请求；宿主 server 端工具调用
  2026-11-01 起同价计费（见「官方文档要点」）。
