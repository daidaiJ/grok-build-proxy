# 模型选择器目录详情展示（模态 / 思考 / 上下文长度 / 美元价格）

> 状态：**T1 已落地**（2026-10-17，分支 `feat/local-model-picker-catalog-details`，
> PATCHES 四十六期）。T2 剩余项（窄终端主动折叠、stale-while-revalidate 标注）
> 未见明显需要，记录在文末「施工实况」。
> 需求原话：通过缓存的模型信息，在选模型时，右边显示模态和支持思考、上下文长度、
> 美元价格，这个记录到 docs local 后续再实现。

## 需求

`/model` 模型选择器（pager）里，选中/列出模型时在**右侧**显示该模型的目录
元信息，帮助用户在切换前判断成本与能力：

- **模态**：是否识图（`accepts_images`，视觉桥 D8 已进目录与 ACP meta）；
- **支持思考**：`reasoning` 位（目录已有）与 effort 菜单有无；
- **上下文长度**：`context_length` / 本地 `context_window`；
- **美元价格**：input / output per-1M（`pricing`，与 `/stats` 成本列同口径）。

## 数据源（全部已就位）

| 数据 | 来源 | 状态 |
| --- | --- | --- |
| 价格 / 上下文 / 思考位 / accepts_images | `xai-grok-model-catalog`（`CatalogMatch`，v1.0.49/51 建，D8 于视觉桥补 `accepts_images`） | 已有 |
| 进程级取数缓存 + 磁盘缓存 + 单飞 | shell `x.ai/modelCatalog` 扩展（TTL 缺省 1 天，`prices.cache`） | 已有 |
| pager 侧消费先例 | `/stats` 成本列的 model_catalog_store（进程级 store + 6h 负缓存 + 裸 id↔vendor/model 匹配） | 已有 |
| picker 渲染面 | pager `/model` picker（`views/` / dispatch，列表 + 右侧详情区） | 待摸锚点 |

## 设计草案（施工前再核实锚点）

1. **取数路径**：picker 打开时向 shell 发一次 `x.ai/modelCatalog`（批量带当前
   可见模型 id）；TTL 内免请求，stale-while-revalidate 沿用 usage_modal 先例
   （刷新中显示旧数据 + 「查询于 HH:MM」）。
2. **显示**：picker 右侧或每行尾部追加紧凑字段（窄终端折叠策略待定）；无目录
   命中显示 `–`（与 /stats 成本列口径一致，负缓存防重刷）。
3. **口径**：价格统一 per-1M USD；思考位 = `reasoning == Some(true)` 或本地
   `supports_reasoning_effort`；模态只标「识图/纯文本」两级，不展开输出模态。
4. **命中语义**：沿用 v1.0.51 匹配索引（裸 id↔`vendor/model` 段命中）；命中失败
   不阻塞选择。

## 待拍板点

1. 显示位置：列表行内追加 vs 选中项右侧详情栏（窄终端怎么办）。
2. 字段取舍：四项全显 vs 只显价格+上下文；模态/思考用图标还是文字。
3. 远端条目无价格时显示 `–`；本地 `context_window` 与目录 `context_length`
   不一致时以哪个为准（建议本地为准，目录仅兜底）。
4. subagent 会话 / headless 是否同样注入（建议 v1 只做 TUI picker）。

## 分期（拍板后再排）

- T1：数据面（picker 触发查询 + 进程 store 复用）+ 行内/详情渲染 + i18n。
- T2：窄终端折叠、stale 显示、负缓存口径对齐、文档（26-config-reference 不涉及；
  picker 使用说明补一段）。

估计净开发 1–2 人天（纯 pager 面 + 既有数据源，无 shell 改动）。

## 追加需求（2026-10-18 口述，未施工）：按目录思考位回填模型条目

> 需求原话：以后通过 /model 切模型时，根据模型是否支持思考，来看要不要给配置文件
> 里面补这个（`supports_reasoning_effort`）。动机即发票：手配的 glm-5.3-flash 条目
> 没开思考位，`/effort` 菜单空挂，而目录里 `z-ai/glm-5.3-flash` 明明 `reasoning: true`。

**设计草案**：目录命中（复用 v1.0.51 匹配索引）`reasoning == Some(true)` 且用户
config 条目未显式开 `supports_reasoning_effort` → toml_edit 定点写回 `true`。
写回通道复用 /auth T2/T3 的 `write_model_entries`（flock + 全批原子 + 注释保留）。

**拍板点**：
1. **触发点两案**：a) 挂 `/model` 切换生效时（本需求原话）；b) **挂启动预热**——
   视觉桥 D8 已有「启动预热 toml_edit 回填 accepts_images」同机制、同通道、同幂等
   语义，把 `supports_reasoning_effort` 并进同一次回填遍历，改动面最小且覆盖全部
   目录命中条目（不等用户切到才修）。建议 b，a 作为补充（切换时 store 必温、
   覆盖启动后新取数场景）。
2. **热生效**：写回后 ModelInfo 需重载才亮 `/effort`——同回合 hot-set（model_switch
   路径合并目录补丁）还是提示「重启生效」（embedded 无 watcher，v1.0.51 reload_models
   先例可沿）。
3. **不降级**：目录 `reasoning` 非 true 时不写也不删用户手写值；恒只升不降。
4. **档位列表不代写**：目录只有布尔 `reasoning`，没有档位值；GLM 系只吃
   low/high/max（medium 会被端点拒），回填擅自造 `reasoning_efforts` 有风险——
   只回填布尔位，档位仍由用户钉（必要时 /effort 菜单对未知档位 400 做容错提示）。
5. `accepts_images`（D8 已做）与思考位回填的幂等日志是否统一落 unified.jsonl
   （`model catalog: backfilled` 一类，可观测）。

估计净开发 ≈0.5 人天（b 案：D8 遍历加一键 + shell 测试；a 案另接 model_switch
触发点）。施工时先对照本档「数据源」表与 D8 落点。

## 施工实况（2026-10-17，T1 落地）

四个拍板点按用户本次口述 + 文档建议值落地：

1. **显示位置**：行右侧既有 `right_label` 列（原来显示 description，模型行常为空
   = 用户说的「右边空白区域」），选中项右侧详情栏方案未采用。
2. **字段**：四项全显，token 串「模态 上下文 思考 价格」单空格拼接
   （如 `文本 256k reasoning $3/$15` / `视觉 1M $0.15/$0.50`）；未知段逐段省略；
   思考位只标支持（与 effort 链式菜单的 reasoning 口径一致）。
3. **口径**：本地 `totalContextTokens` 恒在 → 上下文本地为准、目录仅兜底；
   模态本地 `acceptsImages`（视觉桥 D1 meta）优先；思考本地
   `supportsReasoningEffort` 优先、目录 `reasoning` 兜底；价格只来自目录，
   已知无命中（负缓存）显式 `–`、未取到先不显示（回包后下一帧自动补）。
4. **范围**：v1 只做会话内 TUI picker（两条打开路径：ModelPicker 快捷键 action
   与 `/model` 输入流，共用 AgentView ArgPicker 渲染）；effort 子菜单、theme
   等其他 ArgPicker 不受影响；welcome/dashboard 的 /model 分流路径未接。

落点（纯 pager 面，无 shell 改动）：

- `app/model_catalog_store.rs`：`picker_detail_label(meta, matched)` 组合器 +
  `fmt_tokens` / `fmt_price` + 9 个单测。
- `app/modals.rs`：ArgPicker 渲染对 `command=model|m && args_query 空` 的模型阶段
  按行查 store 组装 `right_label`（渲染时查进程级 store，回包落地后下一帧自动
  补显；全未知回退行 description）。
- `app/actions.rs` + `app/agent_view/input.rs` + `app/dispatch/router.rs`：
  新 `Action::FetchModelCatalogForPicker`——picker 打开时对当前 agent 全部模型 id
  触发 `Effect::FetchModelCatalog`（FetchSessionList 先例：modal 已开、Action 只补
  取数副作用；store 单飞 + 负缓存把门）。
- `slash/i18n.rs`：+`vision`/视觉、`text`/文本（`reasoning`/推理 复用既有键）。
- `docs/user-guide/04-slash-commands.md`：/model 节补一段右侧详情列说明。

T2 剩余（未做，当前未见需要）：窄终端主动折叠（既有 truncate 兜底，最坏情形
尾部价格段被截）、stale-while-revalidate「查询于 HH:MM」标注（usage_modal 先例）。
