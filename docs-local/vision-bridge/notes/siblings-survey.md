# 兄弟 agent 视觉方案普查（父目录同类 agent，2026-10-14）

> 阅读日：2026-10-14。范围：父目录 `D:/CODE/ai/` 下同类 coding agent（qwen-code 已有专文
> [qwen-code.md](qwen-code.md)，本文覆盖其余 12 仓）。
> 方法：四路并行代码调研 + 主线对关键锚点抽查复核；结论均落到 file:line（相对各仓库根）。
> 动机锚定：glm-5.3 / glm-5.2 / deepseek v4.1 flash 等纯文本模型遇带图请求直接 400；
> 手动切模型会把 prompt cache 全部打穿（跨模型缓存不共享，缓存成本 gap 严重）。
> 视觉桥 = 旁路方案：**原图不进主模型请求**，由识图模型 / 视觉子代理在旁边转写，
> 主模型请求全程纯文本，缓存键不动，回合不因 400 炸掉。

## 结论矩阵

| 仓库 | 血统 | 档位 | 主模型不识图时的处置 |
|---|---|---|---|
| qwen-code | gemini-cli 系 | **full-bridge**（内联转写） | 见专文：转写替换 parts，原图剥除 |
| MiMo-Code | **opencode fork** | **full-bridge**（委托式视觉子代理） | system 注入「你看不见图」块，指引派识图子代理 |
| minimax-code | 自研（vendored pi） | **full-bridge**（降级占位 + 服务端识图工具） | 图降级为附件清单 reminder + `images_understand` 工具转写 |
| codex | OpenAI 官方 | gate-only（门控最完整） | attach 拒绝+警告+热键禁用；请求构建期剥除为占位文本 |
| kimi-code | Moonshot 自研 | gate-only（三层门 + 400 恢复） | 提交即拒 / 工具拒绝 / 引擎降级占位；400 可剥图重试 |
| Step-Code | pi 改编 | gate-only（API 层单点剥除） | transform 层换 `(image omitted…)` 占位 |
| opencode | sst | gate-only（带内 ERROR 文本） | 剥图 + 注入 ERROR 文本，靠模型口头转告用户（披露缺口） |
| goose | block | gate-only（格式层门） | 图块换占位文本；工具图搬合成 user 消息；anthropic 格式直传（门缺口） |
| crush | charmbracelet | gate-only（静默丢图） | 用户附件静默丢弃（无警告）；工具图换占位文本；历史按当前模型重过滤 |
| deepseek-harness-codearts | 自研插件 | gate-only（固定像素预算） | 投影为 `[image unavailable]` 占位符 + 每图固定像素/字节预算 |
| zcode | 自研 | gate-only（硬拒绝） | 请求前直接 throw，不剥除不占位 |
| openagents | Python 平台 | 无图像支持 | 图走「落盘 + Read 工具」间接路径，依赖宿主 agent 自身识图 |
| openrouter-cli | Go | 非 agent | 模型目录/价格查询 CLI，无消息构造 |
| pi-dynamic-workflow-milanglacier | pi fork | 无图像支持 | 仅编排层扩展，图像关键词全仓 0 命中 |

**总图景：13 仓只有 3 家有真桥，且三种形态互不相同；其余全是 gate-only 或无图像支持。**

---

## 一、三家真桥

### 1. MiMo-Code — 委托式视觉子代理（与用户设想最接近）

fork 自 opencode（`README.md:543`），但没有抄 opencode 的 gate-only，自己加了委托链路：

- **system 注入披露**：主模型无 image 能力时在 system 里注入 `<vision-capability>` 块——
  「You CANNOT see or interpret image content… dispatch a vision-capable subagent via the
  actor tool, passing the image file path」，附 top-3 可用 vision 模型引用与
  `actor models --vision` 指令（`packages/opencode/src/session/system.ts:139-170`）。
- **误判掩码**：模型 id 含 gpt/claude/gemini 时豁免注入（`system.ts:139-142`）——按 id 猜
  能力的补丁，本 fork 用能力位不需要。
- **缓存自觉**：代码注释明说 vision 块按 call 惰性解析，「若 provider 列表会话中变化，
  该块在回合间不同，会打断 cached system prefix」（`system.ts:147-149` NOTE）——
  印证本 fork D2 应把门控判定做成会话级稳定值。
- **工具门**：`tool/read.ts:338-368` 无 vision 时拒绝读图，输出 warning 并给出含
  `--model <visionRef>` 的具体 dispatch 命令；有 vision 时 shrink 后内联。
- **特点**：转写按需发生（子代理自己 Read 原图文件后文字汇报），主模型请求纯文本；
  依赖「有子代理基建 + 原图落盘」两个前提。

### 2. minimax-code — 降级占位 + 服务端识图工具

自研（`@mavis/*`），工具层建在 vendored pi 上：

- **能力双源**：目录模态 + 显式配置（`packages/local-runtime-v2/src/service/model-system/
  resolution/model-ref.ts:492-493`）。
- **降级路径**：模型不支持/超限时图不进请求，改发 `<system-reminder>` 附件清单 +
  工作流指引（`packages/agent-tools/src/shared/multimodal-attachments.ts:122-198`；
  `local-turn-input-preparation.ts:655-676`：「current model cannot read this image
  inline. Do not use the read tool to infer its visual contents. If images_understand is
  available, pass the exact path to it; otherwise ask the user to switch」）。
- **识图工具**：`images_understand` 批量 ≤10 图 → mcp-server 的 **Gemini backend** →
  文本结果（`packages/agent-tools/src/cloud/matrix-tools/tools/images-understand.ts:1-5`）；
  原生视觉模型上该工具被 suppress 并发诊断事件（`matrix-tools/index.ts:9-11`、
  `local-turn-tool-catalog.ts:209-217`）。
- **五级 cap**：mime 白名单 / 模态 / 单图字节 / 条数 / 请求体预算
  （`multimodal-attachments.ts:140-190`）；降级原因枚举 + 双向披露（给模型 `[Note:…]`
  + 用户可见）。
- **特点**：桥按需发生（模型决定要不要看图才调工具），最省转写成本；代价是依赖
  服务端识图基建（MiniMax 自家 mcp-server），BYOK 用户没有等价物。

### 3. qwen-code — 内联转写桥（本 fork plan.md 的同型，见专文）

`visionBridge` 服务在请求构建前把图转写成文字、替换 parts；本 fork plan.md T1/T2 即此形态。

---

## 二、gate-only 阵营（无桥，按门控质量分三档）

**第一档：门控 + 占位 + 用户可见（可作基线参考）**

- **codex**：能力位 `input_modalities`（元数据缺失时默认支持，`protocol/src/openai_models.rs:193-196`；
  目录读不到时 `unwrap_or(true)` 放行，`tui/src/chatwidget/settings.rs:380-388`）。
  attach 时拒绝+警告+粘贴热键动态禁用（`tui/src/chatwidget/interaction.rs:322-333`）；
  请求构建期 `strip_images_when_unsupported` 换占位文本
  （`core/src/context_manager/normalize.rs:344-380`）；`view_image` 工具能力门直接报错
  （`core/src/tools/handlers/view_image.rs:98-111`）；压缩期图像按 ≈1844 token/张计预算
  （`core/src/context_manager/history.rs:1082-1090`）。
- **kimi-code**：能力位 `image_in` 三源叠加（models.dev 映射 + 协议前缀注册表 +
  `KIMI_MODEL_CAPABILITIES`/config，`packages/kosong/src/capability.ts:10-52`）。
  三层门：TUI 提交即拒（`apps/kimi-code/src/tui/kimi-tui.ts:1584-1590`）、工具层拒绝并
  让 agent 转告用户（`human/media/tool.ts:52-56`）、引擎侧降级为媒体路径文本
  （`agent/media/mediaResolverService.ts:244-251`）。**独有 400 恢复**：识别上游图像
  格式 400 → 剥媒体重试（`kosong/src/errors.ts:252-319` `isImageFormatError`）；
  请求级媒体预算 20MB（`mediaResolverService.ts:48`）。

**第二档：剥除但披露弱（静默或只给模型看）**

- **opencode**：请求构建期 `unsupportedParts()` 单点收口（用户图与工具图同管线，
  `packages/opencode/src/provider/transform.ts:409-441`），图换成
  `ERROR: Cannot read … Inform the user.` 带内文本，靠模型口头转告；TUI 无贴图警告。
  扩展点：插件 hook `experimental.chat.messages.transform`（`packages/plugin/src/index.ts:282-290`）
  理论上可自建转写桥。用户图/工具图统一 `Image.normalize` 预算（≤2000px/≤5MB base64）。
- **Step-Code**：pi 单点 `transform-messages.ts` 请求组装层剥除
  （`packages/providers/src/api/transform-messages.ts:36-54`）；能力位来自服务端
  `enable_vision_input` 元数据（`packages/providers/src/step-provider/index.ts:116,134`）。
  工具图先放进消息、发送前剥，模型被动收到占位。
- **goose**：`supports_vision` 三级来源（config 显式 → 目录回填 → ollama 运行时；
  声明位优先级最高，`goose-providers/src/openai.rs:473-477`）；图块换
  `[image omitted: model does not support vision]`（`openai.rs:450-459`）；工具图搬进
  紧随的合成 user 消息绕 OpenAI 系「tool 消息禁图」（`openai.rs:289-301`）；
  **缺口**：anthropic 格式对 Image 块无条件直传（`anthropic.rs:478-480`），门只盖
  openai 格式层。
- **crush**：catwalk `SupportsImages` + config 覆盖；UI 层直接不注册贴图热键、
  请求层**静默丢弃**用户附件（`internal/agent/agent.go:1670-1681`）——披露最差样本。
  特色：session 层每次构建请求按**当前模型**重过滤历史（中途换文本模型历史图自动剥除，
  `filterFileParts:1742-1752`）；工具图同样有合成 user 消息重排（`agent.go:2270-2345`）。
- **deepseek-harness-codearts**：投影为 `[image unavailable]` 占位符 + 每图**固定**
  像素/字节预算（`src/image-budget.ts:1-60`，源自真实网关故障：单请求图片 token 总量
  100K 上限、实测 617 px/token）；二道门硬拒 `UNSUPPORTED_CONTENT`。

**第三档：硬拒绝**

- **zcode**：请求前 `validateRequestProperties` 直接 throw「Model does not support image
  input」（`apps/zcode-cli/packages/adapters/src/model/model.ts:166-193`），
  不剥除不占位，用户贴图直接失败。

---

## 三、对本 fork 的对照收获（对照 plan.md D1–D7）

1. **形态选择成立**：full-bridge 三种形态里，本 fork 主线（plan.md T1/T2 内联转写，
   复用 `image_describe`）与 qwen-code 同型，是唯一不需要子代理基建/服务端识图后端的
   形态；MiMo（委托子代理）与 minimax（识图工具）可作为后续演进记录在案——
   后者「按需转写」最省成本，但 BYOK 用户没有 Gemini backend 等价物。
2. **缓存一致性是共同痛点**：MiMo 注释自认 vision 注入块可能打断 cached system prefix。
   本 fork D2 的 `session_needs_image_bridge()` 应在会话内稳定（能力位解析一次），
   转写产物走旁路（描述 envelope），主请求文本序列不变——这正是用户要的
   「切模型不打缓存」的替代品。
3. **能力位来源三档**：目录元数据（models.dev：opencode/kimi/MiMo）、服务端上报
   （Step `enable_vision_input`）、显式配置+env（kimi/goose 三级）。fork D1 的显式 TOML
   优先 + 内建缺省表成立；可选增强 = 用 `xai-grok-model-catalog`（v1.0.49 管线）的目录
   模态数据做远端回填，缺省语义仍按 D1 表。
4. **缺省值方向业界分歧**：codex 缺数据默认支持（防挡用户）、opencode 默认 false
   （防 400）。fork D1「内建 grok 缺省 true / 用户新建缺省 false」居中且理由充分，
   维持。
5. **剥除收口点**：单点收口（opencode `transform.ts`、Step `transform-messages.ts`）
   是业界最佳实践，多点门控（kimi 三层）属防御纵深。fork T2 定的「tool_calls render
   层单点收口（`render_non_replaced_tool_body`）」与之一致，MCP/扩展自动覆盖。
6. **工具图重排合成 user 消息**（crush/goose）：绕「provider 拒绝 tool 消息带图」的
   兼容手法。桥开启后主路径无图用不上；但**识图主模型** native 路径遇 glm 网关这类
   严格实现时可作 fallback 参考。
7. **防注入是差异化项**：全 13 仓只有 qwen-code 有 untrusted/DATA 语义，其余全缺
   （opencode 明查零命中）。fork D5 保持。
8. **预算语义可补字节档**：kimi 20MB 请求级、deepseek-harness 固定像素/字节、minimax
   五级 cap。fork D6 的「用户 16 张 / 工具 4 张分计数 + 240s」之外，`image_normalize`
   已有尺寸压缩兜底，够用；若 T2 实测网关有整请求图片 token 上限（deepseek-harness
   案例 100K），再补固定像素预算。
9. **新收益——400 兜底恢复**：kimi 的 `isImageFormatError`（识别上游图像 400 → 剥媒体
   重试一次）是 plan.md 未覆盖的低成本保险：即使 D3 判定桥关闭（BYOK 未授权），带图
   请求打到严格网关仍可能 400，兜底可保回合不炸。建议 T2 顺手带上（sampler 错误分类
   + 剥图重发 + 披露）。
10. **披露保持高配**：crush 静默丢图、opencode 推给模型转告是反面教材；fork T3 的
    张数/模型/host 披露要求超过业界平均，保持。跨供应商烧额度问题（D3）全 13 仓
    只有 qwen-code 与本 fork 有正面设计。
