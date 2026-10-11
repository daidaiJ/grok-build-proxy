# /auth 供应商配置面板 + /stats API 成本列（设计完成；T0/T1/特性B/T2/T3 已落地，T4/T5 未开工）

> **施工状态（2026-10-11 T2 当班）**：T0（/login→/grok 硬改名 + 零配置首启 BYOK 引导）
> 已随 **`v1.0.48`** 发版；**T1（目录取数模块）+ 特性 B（/stats 成本列）已落地**
> （分支 `feat/local-model-catalog`，新 crate `xai-grok-model-catalog` + shell 扩展
> `x.ai/modelCatalog` + pager 成本列，实现记录见 PATCHES.md 二十九期 + roadmap §6
> 当日条目）；**T2（/auth 面板）+ T3（目录回填 + config 写回）已落地**
> （分支 `feat/local-auth-panel`：`views/auth_modal` 分步向导 + `/auth` 命令 +
> `x.ai/providerModels` 扩展 + `write_model_entries` toml_edit 写回 + welcome 门
> `a` 菜单直进面板，实现记录见 PATCHES.md 三十期）；T4（i18n 扫尾——面板字符串
> 随 T2 已带 zh 翻译，扫描 0 挂）与 T5（特性 C 能力门控）未开工。七轮拍板见
> 「拍板增补」节。
>
> 来源：用户拍板 2026-10-11（agent-cli-tracking 二轮调研衍生需求）。
> 参照实现：用户自有 **modelq** 仓库（`D:/CODE/ai/openrouter-cli`，Go，二进制 mqx）——OpenRouter +
> models.dev 取数、双目录解析、美元计价全流程现成，落地点直接对照。
> 关联：[`model-limits-config.md`](model-limits-config.md)（context_window / max_completion_tokens
> 兜底语义）、[`usage/`](usage/README.md)（用量面板）、`agent-cli-tracking/ADOPTION.md` §7（调研来源）。

## 现状锚点（2026-10-11 核实）

- 模型条目 `ModelEntryConfig`（shell `agent/config.rs:3840`）：`model`（路由 slug）/ `base_url` /
  `api_key`（明文）/ `env_key`（env 名，字符串或数组）/ `api_backend` / `max_completion_tokens` /
  `temperature` / `top_p` 等；`ModelInfo.context_window` 存在（`config.rs:3439` 使用）。
- 协议枚举 `ApiBackend`（sampling-types `types.rs:1094-1102`）：`ChatCompletions`（默认）/
  `Responses` / `Messages`——三值即 /auth 面板协议步选项。
- **目录回填已有 managed 侧先例**：prefetch 模型合并（`config.rs:3420-3455`）——prefetch 条目缺
  `context_window` / `api_backend` / `agent_type` 时从硬编码默认条目继承；/auth 的 BYOK 回填镜像
  这条路（目录值仅在与默认不同时写入，用户拍板）。
- `/stats` 模型行 token 拆分四列（`stats_modal.rs:455-465`）：input(`prompt_tokens`) / output
  (`completion_tokens`) / cache read(`cached_read_tokens`) / **cache write(`cache_creation_tokens`)**；
  后者来自账本 `usage.cache_creation_prompt_tokens`（chat-state `usage.rs:80`）——仅 Anthropic 风格
  usage 上报该字段，用户的 OpenAI 兼容网关恒 0，故「缓存写」列恒空（用户痛点本源）。
- 账本已按 per-model 聚合 input/output/cached_read/cache_creation（`usage.rs:77-84`）——成本可在
  渲染层计算，**账本零改动**。
- 全仓无 models.dev / OpenRouter 目录引用（grep 0 命中）——取数模块为绿地。

## 参照实现：modelq 取数 / 解析 / 计价流程

- **OpenRouter 目录**：`GET https://openrouter.ai/api/v1/models`，免鉴权（`internal/api/client.go:15-40`，
  UA "modelq"，60s 超时）；`Pricing` 为 per-token USD 十进制字符串：`prompt` / `completion` /
  `input_cache_read`（`:128-152`），展示按百万换算（`InputPerM` 等）。
- **models.dev 目录**：`GET https://models.dev/api.json`（`internal/modelsdev/modelsdev.go:207`
  download），本地缓存 TTL（`:263-291`，写失败只损失一次重下载）；结构 provider→model，含
  `Limit`（token 上限；**input 可低于 context**，`:113-114`）与 `Cost`（vendor 参考价 per 1M，
  支持分段价 `CostTier`，`:120-134`）。
- **双目录解析**：`modelsDevRef` 置信匹配（`internal/cmd/helpers.go:32-35`）——exact → canonical →
  vendor 级；仅 fuzzy 子串命中视为噪声、不富化。
- **汇率（可选）**：USD→CNY 四源兜底（frankfurter / fawazahmed0 cdn / pages.dev / open.er-api.com，
  `internal/fx/`）——fork 首版只需 USD，汇率路线留档备用。

## 特性 A：/auth 交互面板

交互流（协议 → baseUrl → SK → 自动检索模型 → 目录回填 → 写回）：

1. **协议选择**：ChatCompletions（默认）/ Responses / Messages，写 `api_backend`。
2. **baseUrl** 输入（示例提示 `https://api.x.ai/v1` 形态）。
3. **SK 掩码输入**；存储拍板点：a) 直接写 `api_key`（现状即明文，面板不恶化但需文档标注）；
   b) 引导 `env_key`（写 env 变量名）。默认建议 b、提供 a 选项（见待拍板 1）。
4. **自动检索模型**：`GET {base_url}/models`（OpenAI 兼容网关普遍支持；失败回退手填 slug）。
5. **目录回填**：OpenRouter 目录按 slug 精确匹配（modelq 置信序），命中且**与默认值不同才写**
   `context_window` / `max_completion_tokens`（用户拍板「和默认配置值不一样」）；差异逐项展示确认。
   语义镜像 managed prefetch 合并先例（`config.rs:3420-3455`）。
6. **写回**：生成 `[model.<id>]` 条目写回用户 config。config 回写面现状无先例（`/lang` 等均为
   会话态）——需新增 TOML 回写，策略拍板点：整文件重序列化 vs 定点插入（注释保留）。

安全注意：SK 输入掩码；不进 transcript / 日志 / debug（`xai-grok-secrets` 只盖出站遥测面，
面板本地回显需自查）。

## 拍板增补（2026-10-11 四轮）：首启默认 BYOK 引导 + `/login` → `/grok`

- **首启默认路径改向（已拍板）**：零配置（无 BYOK 条目、无 env key、无缓存 token）时，
  默认提供 BYOK 的 `/auth` 交互引导继续使用，**推 Grok 认证的现状废止**。
  - 根因锚点：`build_auth_methods` unpinned 顺序（shell `agent/auth_method.rs:85-87`）——零配置时
    仅通告 `grok.com` 一个方法，pager 又以 `auth_methods.first()` 取启动元数据 ⇒ 首启必落 Grok OAuth。
  - 改法：零配置分支的默认推荐位给 BYOK 面板（进 /auth 交互），`grok.com` OAuth 降级为面板内
    可选项（「使用 Grok 订阅登录」入口触发既有 login 流）。
  - ACP 兼容注意：auth method 列表仍按协议通告 `grok.com` 等方法（编辑器客户端依赖 method id），
    变的只是自家人（pager）的默认首屏与推荐位，`default_auth_method_id` 语义同步审一遍。
- **`/login` → `/grok` 改名（已拍板）**：原「Grok 订阅登录/认证」命令更名，与 BYOK 的 `/auth`
  划清语义（/auth = 供应商/模型配置，/grok = xAI 账号登录）。
  - 波及面：`slash/commands/login.rs:8`（meta）、`slash/registry.rs:995`（内置清单）、
    `slash/mod.rs:1888,1924` 与 `:2747`（测试）、`slash/acp_command.rs:289`（ACP 命令列表）、
    i18n 描述（`i18n.rs:201`）与 logout 文案（`:204`）。
  - 兼容（已拍板，2026-10-11 五轮）：**不保留别名**——`login` 硬切换为 `grok`，引用点一次性改净
    （含 ACP 命令列表与全部测试断言）。

**分期更新**：新增 **T0 小件（可先行独立发版）** = `/login`→`/grok` 改名 + 零配置首启默认路径
改向（BYOK 引导）；T2 /auth 面板落地后，T0 的引导位从「占位提示」升级为「直接进面板」。

**拍板增补（2026-10-11 七轮，T2 开工前）**：

- **SK 存储默认（拍板 1 已定）**：**明文 `api_key` 直写默认**——面板第 3 步直接收 SK
  （本地掩码回显），写 `api_key` 字段；不引导 `env_key`（该字段仍被解析，手配不受影响）。
- **配置条目 id（用户补充拍板）**：`[model.<id>]` 的 `<id>` 由用户给缩写，唯一即可——
  面板确认步可编辑，默认从模型 slug 派生短名，校验与既有 `[model.*]` 条目不冲突。
- **config 回写机制**：**toml_edit DocumentMut 定点编辑** `[model.<id>]` 键 + 既有
  `lock_config_writes` flock 守卫（与 settings 写互斥），注释/未知段/格式保留；
  不可解析文件拒绝覆盖（同 pager `config_toml_edit.rs` 语义；shell `plugin/mod.rs`
  已有 toml_edit 定点编辑先例）。

**拍板增补（2026-10-11 六轮，T0 施工当日追加）**：

- **价格缓存与数据面（用户拍板）**：「模型价格很少变动，一天甚至多天更新一次价格缓存就行，
  只留官方价格」——价格目录 TTL 从 modelq 短缓存放宽到 **1 天起步**（多天亦可，可配）；
  价格面只保留**官方价格**（供应商/官方渠道公布价），不做第三方聚合价、不做均价/估价。
  影响 T1：原参照 modelq 的双源合并 + 置信匹配收敛为「官方价单一口径」（OpenRouter
  pricing 对 OpenRouter 路由模型即官方牌价；models.dev `Cost` 为 vendor 官方参考价），
  具体双源取舍在 T1 开工时按此口径定稿。
- **/auth 面板交互形态（用户拍板）**：面板交互**参照本仓其他 slash 拉起的配置面板先例**
  设计（/model 选择器、settings modal 等，T2 开工时对照实现选型）；「自动检索模型」一步的
  模型列表改为**用户可操作**——关键词过滤 + 勾选（多选确认后进入目录回填/写回），
  替代原设计第 4/5 步的单线条「检索→逐项差异确认」流程（回填「与默认不同才写」的拍板保留，
  只是确认面从逐项展示改为勾选后批量回填展示）。

**分期**：
- T1 目录取数模块（新 crate 或 shell 模块：OpenRouter + models.dev 双源、TTL 缓存、置信匹配；
  单测夹具直接搬 modelq 同款 JSON 响应）。✅ 已落地（2026-10-11，PATCHES 二十九期）
- T2 /auth 面板（pager 弹窗交互 + 输入校验 + /models 拉取）。✅ 已落地（2026-10-11，PATCHES 三十期）
- T3 目录回填确认面 + config 写回。✅ 已落地（2026-10-11，随 T2 同批；toml_edit 定点写回）
- T4 i18n + 无配置启动引导（若拍板做 onboarding，见待拍板 3）。
  面板与 welcome 引导字符串已带 zh 翻译（随 T2），扫描 0 挂；剩余 i18n 扫尾待做。
- T5 模型能力门控（特性 C：纯文本模型遇图 mask/阻断；依赖 T1 增模态字段扩展）。

**T2/T3 已落地补记（2026-10-11，分支 `feat/local-auth-panel`）**：

- **落地面**：`views/auth_modal`（分步向导：协议→baseUrl→SK 掩码输入→模型列表
  过滤+空格多选+手填回退→确认（id 可编辑+冲突标红）→写入→完成）；`/auth` 命令；
  shell 扩展 `x.ai/providerModels`（GET {base_url}/models，OpenAI/Anthropic/裸数组
  三形态解析，15s 超时）；`write_model_entries`（toml_edit 定点编辑 `[model.<id>]` +
  `lock_config_writes` flock 守卫，注释/未知段保留，全批原子、id 冲突整批拒绝）；
  welcome 门 `a` 菜单项直进面板（AppView 级第二宿主 `welcome_auth_panel`，BYOK
  用户被困门后是主场景）；写回后 `[model.*]` 走既有 `ConfigUpdate::ModelsChanged`
  热重载，`/model` 即选即用。
- **回填语义**：目录值与新建模型兜底不同才写——`context_window` 兜底 200_000
  （`agent/config.rs:3704`，相同值省略）；`max_completion_tokens` 缺省 = 不发送，
  目录有值即写；目录未命中 = 不写值（能力未知不门控）。
- **SK 纪律**：`SecretKey` newtype 手写 Debug 打码（Effect/draft 均携带明文，
  任何 `{:?}` 落盘不出现密钥）；扩展层只进 Authorization 头，不进日志/错误消息。
- **验证**：pager 门控 lib auth_modal 13 / welcome 226 / slash 568（含 /auth 注册 +
  shell 保留字同步用例）/ dispatch::tests 1732 全绿；shell GATE_FORCE 过滤
  model_entry_writes 7 / provider_models 6 / slash_commands 105 全绿；
  两 crate `cargo check --all-targets` 0 error 0 warning；i18n 扫描 0 挂。
  活回合验证待新客户端实跑（发版后）。

**T1 已落地补记（2026-10-11）**：模态字段（`architecture.input_modalities` /
`modalities.input`）尚未入 `CatalogEntry`——特性 C 开工时随 T5 的 T1 扩展一并补上
（解析 + 夹具 + `CatalogMatch` 透传）。

## 特性 B：/stats「缓存写」→「API 成本」

- **方案**：模型行 token 拆分中恒空的「缓存写」列替换为「成本」列：
  `cost = prompt×price_in + completion×price_out + cached_read×price_cache_read`
  （+ `cache_creation×price_cache_write`，仅 Messages 网关有值）；模型行内显示 + 面板合计行；
  5h/day/week 三时间窗联动既有过滤。
- **价格源**：特性 A 的 T1 目录模块（OpenRouter pricing 按 slug 匹配，models.dev Cost 兜底）；
  无价模型显示 `–` 并从合计剔除、标注 unknown 条数。
- **缓存策略**：目录 TTL 缓存（modelq 先例）；离线 / 未取到价时成本列空，其余列不受影响。
- **落点**：`stats_modal.rs` 渲染 + 成本小函数（渲染层，账本零改动）+ 目录模块。
- **i18n**：`("cache write", "缓存写")`（`i18n.rs:298`）替换为 cost 标签；引用点
  `stats.rs:124`、`stats_modal.rs:459/566` 与测试断言（`stats.rs:314`、`stats_modal.rs:821`）
  同步改。
- **不在本次范围**：status line 加成本位（待拍板 6 留档）；CNY 换算（modelq fx 路线留档）。

## 特性 C：模型能力门控——纯文本模型遇图 mask 替换或阻断（2026-10-11 用户追加）

- **需求原话**：按 OpenRouter 拿到模态能力的模型，如果是纯文本遇到视觉的情况，
  应触发 mask 替换机制避免请求 400 报错，或者直接阻断。
- **动机**：目录取数（特性 A 的 T1）天然携带模态能力；BYOK 用户配了纯文本模型后
  贴图/工具带图会直接吃上游 400（当前无任何前置判定，报错形态是裸的
  upstream error），体验差且难自查。
- **数据源（T1 需小扩展）**：`CatalogEntry` 增模态字段——OpenRouter 取
  `architecture.input_modalities`（含 `"image"` 即识图；`architecture.modality`
  形如 `text+image->text` 可作展示），models.dev 取 `modalities.input`
  （modelq 两源结构均已有该字段，解析层各加一段）；目录未命中/字段缺失 =
  能力未知，不门控（保持现状放行）。
- **门控行为（拍板点 7，建议双层）**：
  - **用户显式图**（粘贴/拖入）→ 建议**阻断**：发送前报清晰错误（指明当前模型
    不支持视觉输入，指路 /auth 换模型或换识图模型），静默替换会让回合答非所问；
  - **附带图**（read_file 图/PDF 渲页、MCP 工具结果 inline 图）→ 建议 **mask
    替换**：图块替换为文本占位符（如 `[图片已省略：当前模型不支持视觉输入]`）+
    一行披露，回合继续不 400。
  - 与视觉桥的关系：mask/阻断是**无识图桥时的兜底**；`feat/local-vision-bridge`
    （T1 能力位 `accepts_images` + 打开 `image_describe` 转写）落地后，同一能力位
    优先走转写，本特性退为「未配桥 + 未配识图模型」的保底层。能力位数据源统一
    用目录模态（本特性 T1 扩展产出），避免两套判定。
- **落点**：`xai-grok-model-catalog`（`CatalogEntry` 增 `input_modalities`/识图
  bool，解析 + 夹具各一小段）；能力判定查询走 shell 扩展回包（`CatalogMatch`
  增字段透传）；门控执行点与视觉桥 plan.md 的现状锚点同位（prompt 构建层用户图
  入口 + 工具结果剥图口），实现期先读视觉桥 plan.md 的锚点清单。
- **分期**：**T5（独立小件，不阻塞 T2–T4）**；T1 扩展可先行（纯函数层 + 夹具，
  半天级）。T3 面板回填时顺带在模型列表展示「支持视觉」标记（目录字段现成）。

## 验收标准

- A：全新配置（无 `[model.*]`）走 /auth 五步产出一条可用条目，采样请求成功；目录值与默认
  不同才写入；SK 不出现在 transcript / 日志 / debug 落盘。
- B：/stats 三窗成本列有值且与手算一致（夹具对账）；无价模型显示 `–`；「缓存写」移除后
  i18n 扫描 0 挂；既有 stats 测试同步更新全绿。

## 待拍板决策

1. **SK 存储默认**：`env_key` 引导（建议默认）vs 明文 `api_key`（提供选项）。
2. **config 回写策略**：整文件重序列化 vs 定点插入（注释/未知段保留）。
3. **首启 onboarding**：~~是否自动引导~~ **已拍板（四轮）**——零配置首启默认 BYOK `/auth` 交互，
   Grok 认证降级为面板内选项；原 `/login` 改名 `/grok`。
4. **目录源优先级**：OpenRouter 主、models.dev 兜底（modelq 双源合并的 fork 版取舍；或仅接一源）。
5. **成本货币**：仅 USD（已拍板口径「美元成本」）；CNY 换算后续按需。
6. **status line 成本位**：本次不做，留档。
7. **纯文本模型遇图门控行为**（特性 C / T5，2026-10-11 用户提出，双层建议待确认）：
   用户显式图 = 阻断、附带图 = mask 替换 + 披露；mask 占位符文案与披露形态、
   能力未知（目录未命中/无模态字段）时是否放行（建议放行）待拍板。
