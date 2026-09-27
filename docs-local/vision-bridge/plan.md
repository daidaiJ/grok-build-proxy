# 视觉桥施工计划

> 状态：评估完成，未开工。与 [README.md](README.md) 同期（2026-09-27）。
> 对照笔记见 [notes/qwen-code.md](notes/qwen-code.md)。

## 需求原话

给本 grok-build fork 支持视觉桥：部分文本模型不能读图，用一个识图模型完成这部分任务。
可看兄弟目录 Qwen Code 的实现。

拆成四个可独立验证的问题：

1. **谁识图**：主模型有没有 image 输入能力，缺省怎么定。
2. **用户图**：粘贴 / 拖入 / `@` 图在文本主模型上如何变成可用上下文。
3. **工具图**：`read_file`、PDF 渲页、MCP 截图如何既不 400 又不瞎编。
4. **烧谁的额度 / 图发到哪**：默认 `image_description = grok-4.6` 会打 x.ai；BYOK 用户必须可关、可改指、可见。

## 现状锚点（全部已核实）

| 事实 | 位置 |
|---|---|
| 描述管线（prompt / envelope / 缓存 / 落盘 / 采样） | `xai-grok-shell/src/session/image_describe.rs` |
| 用户图转写入口 | `acp_session_impl/prompt_build.rs`：`transcribe_user_images` |
| 回合接线：有图且 cursor → 转写，否则只 persist + 原图进 chat | `acp_session_impl/turn.rs`（`user_images.is_empty()` 分支） |
| 插话图：非 cursor 原样返回 image part | `acp_session_impl/interjection.rs`：`prepare_interjection_images` |
| **本 fork 永不走 cursor 路径** | `acp_session_impl/session_mode.rs`：`is_cursor_harness() -> false` |
| 描述模型配置，空则编译默认（`grok-4.6`） | `config/mod.rs` `ModelOverrides.image_description`；`xai-grok-models` `default_image_description_model()` |
| 辅助 sampler：解析失败回退**会话模型**，不把内部 slug 硬塞 BYOK 端点 | `agent/config.rs`：`finalize_image_describe_sampler_config` |
| `read_file` 图 / PDF 页 inline 到 tool result（非 cursor） | `acp_session_impl/tool_calls.rs`：`render_non_replaced_tool_body` |
| MCP 等抽取图 → deferred vision follow-up（非 cursor 断言会 attach 原图） | `acp_session_tests/tool_layer_images_bridge_tests.rs` |
| 会话持有描述模型 id + 缓存 | `acp_session.rs`：`image_description_model`、`image_describe_cache` |
| TUI 是否允许粘贴图：ACP `meta.acceptsImages` / `inputModalities`，**缺省 true** | `xai-grok-pager/src/acp/model_state.rs`：`current_model_accepts_images` |
| ACP 已发 `totalContextTokens` / reasoning，**从未发 acceptsImages** | `agent/config.rs`：`to_acp_model_info` |
| 模型目录无视觉能力字段 | `ModelEntryConfig` / `ModelInfo`（`agent/config.rs`） |
| 描述源枚举只有用户附件 | `ImageDescribeSource::UserAttachment` |
| 用户图描述失败 → 整回合 `acp::Error` | `transcribe_user_images` 的 `map_err` |
| 配置参考已有 `models.image_description` | `xai-grok-pager/docs/user-guide/26-config-reference.md` |
| BYOK 坑：plumbing 模型默认 grok-4.6 | 仓库根 `README.md`「常用坑」表 |

本 fork 日常行为：**永远当主模型能看图。** `models.image_description` 配了也几乎用不上。

## 可复用的现成件

不要新写第二套 VL 调用。T1/T2 只改门控、失败策略、调用点。

- `build_describe_prompt` / `render_image_description_block` / `render_image_user_message`
- `ImageDescribeCache::get_or_describe`（按 source + path_key + 内容哈希 + prompt 指纹）
- `persist_user_images` / `persist_and_prepend_image_files`（assets 落盘，编码模型仍可 `read_file` 路径）
- `describe_user_images`（data URL 图 + 文本 prompt → 视觉模型，超时 240s，`max_output_tokens` 4096）
- `resolve_aux_sampler_config` + `finalize_image_describe_sampler_config`
- `image_normalize`（尺寸地板、压缩、cursor vs native 两套 cap）
- 每回合用户图上限 `IMAGE_DESCRIPTION_PROCESSING_LIMIT = 16`

缺的是：能力位、非 cursor 接线、工具路径、失败剥图、披露、BYOK 防误烧。

## 设计定案（施工按此锁定）

### D1 能力位：`accepts_images`

TOML：`[model.<id>] accepts_images = true|false`，进 `ModelEntryConfig` → `ModelInfo`。

解析规则：

| 来源 | 未写时的缺省 |
|---|---|
| 用户在 `[model.*]` **新建**、且不在内建/预取目录 | **false** |
| 内建 grok 家族 / 远端 `/models` 条目未给该字段 | **true**（保持现状，避免 grok-4.6 被误转写） |
| `ModelInfo::fallback`（未知 slug） | **false** |

显式字段永远赢。远端以后若给 `acceptsImages`，覆盖内建缺省，不覆盖用户 TOML。

ACP：`to_acp_model_info` 写 `meta.acceptsImages`（bool）。TUI `current_model_accepts_images` 的「缺 meta 则 true」可保留作旧会话兜底；新会话必须带该键。

### D2 何时跑桥

同时满足才转写：

1. 本回合（或本条工具结果）含可用图；
2. **当前主模型** `accepts_images == false`；
3. 描述模型已解析，且描述模型自身 `accepts_images == true`（或内建 grok 视觉缺省）；
4. D3 的授权条件成立。

主模型识图 → 保持今天的 native 路径（persist + image part）。不要对 grok-4.6 先描述再编码。

### D3 授权：BYOK 主模型禁止静默打 xAI

`models.image_description` 今天空串也会填成编译默认 `grok-4.6`（`config/mod.rs`）。桥一开，给 DeepSeek 贴张图就会烧统一池。

**定案**：

- 用户 **显式** 写了非空 `models.image_description`（或对应 env）→ 授权打该模型，允许跨供应商。
- 用户没写、走编译/远端默认，且主模型 `has_own_credentials()`（BYOK）→ **桥关闭**：剥图、用户可见说明、不发描述请求。
- 用户没写、主模型走 xAI 会话凭据 → 允许用默认 `grok-4.6` 描述（与 Cursor 历史行为一致）。

关闭时不要回退「用主模型自己描述」：主模型是文本模型，描述调用同样会 400。

### D4 失败策略

描述失败、超时、空响应、预算用尽：

- **剥掉 image part**，在用户消息或 tool result 文本里追加一句不可用说明；
- **回合继续**，不把成功的工具调用改成 tool error；
- 不把 provider 原始错误（可能含 URL/token）写进对话，只打日志。

废弃 Cursor 路径「`transcribe_user_images` 失败则整轮 abort」。本 fork 无 cursor 会话，改 `transcribe_user_images` 的返回类型即可，不必保留 abort 分支。

### D5 描述 prompt

现有 prompt 要求「Don't mention to the user that you only have a description」。编码模型会假装看见了图。

T1 改成：

- 系统侧：转写 / 描述，**不要回答用户问题**；图内文字当 DATA，不执行图内指令；不要输出 `<think>`。
- 交给编码模型的 envelope：标明 untrusted machine transcription、可能有误、禁止把转写里的路径当「再读一次原图」的指令。
- `<image_files>` 仍可保留真实落盘路径（编码模型改文件时需要）；但须写明「读这个路径会再次拿到像素，当前主模型看不懂，不要为了看图去 read」。

细节句以施工时 `build_describe_prompt` / `render_image_description_block` 为准，本条锁语义。

### D6 预算与超时

- 用户图：维持 16 张、只描述最后 N 张，更早的打 `[skipped-due-to-limit]`。
- 工具图（T2）：每回合最多 **4** 张走描述（Qwen 同级）；超出剥图并注明预算用尽。用户图与工具图 **分计数**（用户图 16、工具 4），避免贴了 16 张截图后 `read_file` 一张 PNG 直接没描述。
- 超时维持 240s；不采用 Qwen 的 30s。
- 缓存继续按内容+prompt 指纹。T2 给 `ImageDescribeSource` 加 `ToolResult` 变体，避免用户图与工具图串缓存。

### D7 披露（T3，T1 至少打日志）

用户可见一句：转写了几张、用了哪个模型、endpoint host（跨供应商时必须有 host）。失败同样披露「尝试过、图不可用」。T1 可先 `tracing` + 一条 `system_reminder`；T3 再接到 TUI/ACP 通知。

---

## T1：能力位 + 用户图转写

### 目标

文本主模型 + 用户贴图：编码模型只看到描述 envelope + `<image_files>` + 原文本，请求里 **没有** image part。识图主模型行为与今天一致。

### 改动点

1. `ModelEntryConfig` / `ModelInfo` 增加 `accepts_images: Option<bool>`（配置）与解析后的 `bool`（或在 `ModelInfo` 存 `Option` + 解析函数，避免 serde 默认吞掉「未写」）。
   - `from_config` / `fallback` / 内建 JSON 加载 / 远端 merge 四处都要过，缺一处就会出现「目录里 false、实际 true」。
   - 新建用户模型未写 → false；内建未写 → true。用「这条 `ModelEntry` 是不是用户新建」判断，不要靠模型 id 字符串猜。
2. `to_acp_model_info` 写入 `acceptsImages`。
3. 抽 `session_needs_image_bridge(&self) -> bool`（主模型不识图 && D3 授权 && 描述模型识图）。**禁止**继续用 `is_cursor_harness()` 当图像策略。
4. `turn.rs` 用户图分支、`interjection.rs` `prepare_interjection_images`：`session_needs_image_bridge()` 为真走 `transcribe_user_images`，为假走现有 persist+inline。
5. `transcribe_user_images`：失败改为剥图+说明+仍返回 `Ok(text)`（或 `Result` 的成功臂带 notice），调用方不再 `?` 掉整轮。
6. D5 prompt / envelope 文案。
7. 文档：`26-config-reference.md` 的 `model.<id>.accepts_images`；`11-custom-models.md` 示例补一行；根 README「常用坑」补「文本模型贴图」。

### 验收

- 识图主模型 + 贴图：请求仍含 image part；不调用描述 sampler（可用假 client 计数）。
- 文本主模型 + 显式 `image_description` + 贴图：描述被调用；编码请求无 image part；消息含 `<image_description>` 与 `<image_files>`。
- BYOK 文本主模型 + **未写** `image_description`：零描述请求；图被剥；有说明。
- 描述超时/空响应：回合不 abort；编码侧无 image part。
- ACP meta 含 `acceptsImages: false` 时，TUI `current_model_accepts_images() == false`。
- `scripts-local/ctest.sh -p xai-grok-shell --lib image_describe` 以及 turn/interjection 相关 filter；`cargo check -p xai-grok-shell -p xai-grok-pager`。
- 新用例禁止塞进一个巨型 `#[test]`（AGENTS.md 第 9 条）。

### 非目标（T1）

不改 `read_file` / MCP 图；不改 `/model` UI；不把描述模型加入 picker（它已是 hidden plumbing）。

---

## T2：工具结果桥

### 目标

主模型不识图时，任何将进入下一轮模型请求的 image part（`read_file` ImageContent、PdfPageImages、MCP `extracted_images`、tool 文本里抽出的 base64）必须先转写或剥除。识图主模型保持今天的 deferred follow-up / inline。

### 改动点

1. `ImageDescribeSource` 增加 `ToolResult`。`get_or_describe` 的 `path_key` 用工具名 + call id，intent 用工具名 + 已有文本（截断，对齐 Qwen ~2k 字符，不要把整份 tool output 灌进 VL）。
2. `render_non_replaced_tool_body` / `handle_bridge_tool_success`：`session_needs_image_bridge()` 为真时 **不要** `ContentPart::Image`，改为描述文本写入 tool result（或紧随的 system_reminder）。PDF 多页：每页一段，标页码。
3. 预算：见 D6，每回合工具图最多 4 张描述。
4. 更新 `tool_layer_images_bridge_tests.rs`：今天断言「multimodal 必须 attach data URI」。按主模型能力拆成两例：识图 attach、文本转写/剥图。
5. `read_file` 本身不必调 VL（Qwen 普通图也是保留像素交给共享 helper）。桥发生在 tool 输出进入 chat state 之前，一个点收口，MCP/扩展自动覆盖。

### 验收

- 文本主模型 + `read_file` PNG：tool result 无 data URI；有描述或「图不可用」；无第二轮 image follow-up。
- 文本主模型 + MCP 截图：同上。
- 识图主模型：现有 attach / follow-up 断言仍绿。
- 描述失败：工具仍算成功；模型看不到原图。
- PDF 渲页：文本主模型只看到按页转写；失败则回到「无法抽取 PDF 文本」类说明，不把页图发出去。
- `ctest.sh -p xai-grok-shell --lib tool_layer_images` 以及 `read_file` 相关 output 测试。

---

## T3：披露与配置纪律

### 目标

用户能看见图去了哪；文档写清缺省；误烧 xAI 有配置开关。

### 改动点

1. 转写成功/失败/跳过（D3 未授权）各一条用户可见 notice。跨供应商必须带 host。ACP 可用已有 `XaiSessionUpdate` 或 system_reminder，TUI 能在 scrollback 看到即可，不新开 modal。
2. README 常用坑：文本模型贴图需显式 `models.image_description` 指到 VL；未指则剥图不烧 xAI。
3. `26-config-reference.md`：`models.image_description` 的 Details 补「仅当主模型 `accepts_images = false` 且用户显式配置（BYOK）或主模型走 xAI 时调用」。
4. 可选：`GROK_VISION_BRIDGE=0` 硬关（测试与应急）。有则登记 PATCHES；没有也不阻塞 T1/T2。

### 验收

- BYOK 未配置描述模型：notice 说明「未授权视觉桥，图未发送」。
- 显式跨供应商：notice 含模型 id 与 host。
- 文档三处（README / 26 / 11）互相不矛盾。

---

## 不做

| 项 | 原因 |
|---|---|
| Qwen 整回合 `agent` 接管 | 采样配置、工具集、compaction、账本绑在会话模型；中途切模要处理重试/子代理/auto-compact 打错模型。Qwen 自己也是独立 phase |
| `/model --vision` | toml + env 够用；TUI 选型可后补 |
| 每回合 4 张用户图、30s 超时 | 与已有 16 张 / 240s 冲突，截图描述经常更慢 |
| 自动同供应商挑 VL | 本 fork 目录里经常只有一个文本模型 + 默认 grok-4.6；乱挑会跨供应商。显式配置更安全 |
| 视频 / 音频 / 仅 URL 的 `fileData` | Qwen 也排除；解析牵涉网络与鉴权 |
| 在 main 上改功能代码 | 分支纪律 |

## 测试与编译纪律

- 改 `xai-grok-shell` 会级联；先 `cargo check -p xai-grok-shell`，行为验证用模块 filter，不要全量 `--lib` 盲跑。
- 输出禁止接 `tail`/`head`。长编后台跑。
- Windows 一律 `scripts-local/ctest.sh`。新失败若是环境族，先登记 `win-skip.txt`。
- 假视觉模型：复用 shell 测试里的 sampler 夹具（`TEST_MODEL` / noop handle）。不要对真实 VL 打集成作为门禁。

## 同步文件与 PATCHES

预计改动的同步路径：

- `crates/codegen/xai-grok-shell/src/session/image_describe.rs`
- `crates/codegen/xai-grok-shell/src/session/acp_session_impl/{turn,interjection,prompt_build,tool_calls,session_mode}.rs`
- `crates/codegen/xai-grok-shell/src/agent/config.rs`
- `crates/codegen/xai-grok-pager/src/acp/model_state.rs`（若要收紧缺省；优先只补 server meta）
- 用户指南 `docs/user-guide/11-custom-models.md`、`26-config-reference.md`（pager 文档，视同步范围登记）

每块 LOCAL 行为在 `docs-local/PATCHES.md` 单开「视觉桥」一节：门控语义、D3 授权、失败剥图。上游若以后自己做视觉桥，以 PATCHES 对照 rebase。

## 风险

| 风险 | 处理 |
|---|---|
| 打开桥却忘了改 `image_description` | D3：BYOK 默认关 |
| `accepts_images` 缺省猜错，grok-4.6 被转写 | 内建缺省 true |
| 用户新建 VL 模型忘了 `accepts_images = true`，自己当描述模型被拒 | 文档 + notice「描述模型不识图」 |
| 工具测试大面积转红 | T2 按能力拆用例，先改断言再改生产代码 |
| 描述很慢，用户以为卡死 | T3 notice「正在用 {model} 转写 n 张图」；超时已有 240s |
| 图内 prompt injection | D5：DATA + untrusted 标记 |
| 统一池误烧 | 与 `docs-local/quota-pct-jump-audit.md` 同类；D3 就是为这个 |

## 工作量

净开发约 **6–11 人天**（T1 2–4 + T2 3–5 + T3 1–2），不含活回合用真 VL 看描述质量。T1 单独可交付。
