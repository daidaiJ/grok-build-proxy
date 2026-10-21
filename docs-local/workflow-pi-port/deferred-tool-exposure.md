# P2 设计原型：延迟工具声明 + exposure 三档

> 状态：预研原型 + **三参照合并定稿（2026-10-04，§2.2）** + **zcode 3.16.x 第四参照
> （2026-10-21，§2.3）**。语义来源：pi `0.99.0` tool_search 与 tool exposure
> （docs/cli.md#tool-search、docs/extensions.md#tool-exposure，B 级转述），**2026-10-02 增补
> `0.99.2`/`v1.0.0` release notes 对本设计的三条修正（§2.1）**，**2026-10-04 并入 kimi
> `select_tools` 三件套与 step 侧无参照判定（§2.2，kimicode-port 决策 D2 落实）**，
> **2026-10-21 并入 zcode 3.16.2 的完整实现对照（§2.3，A 级源码）**。fork 基线 `95aad87`。

## 1. 问题

workflow 的 `parallel()` 一次 fan-out 可达数百 agent（`MAX_PARALLEL = 1024`），
每个子 agent 默认带全量工具面：内置工具 + MCP 工具（fork 的 MCP 全量声明，
`tool_name.rs` 只做 admission 不做面裁剪）。工具面 token 成本 = agent 数 ×
工具面大小，且大工具面推高误用率。

fork 现有的 `ToolSearch→search_tool` 映射（`xai-grok-agent/src/builder.rs:3016`
测试）是 allowlist 场景：配置里点名 `ToolSearch` 时映射到 skill/memory 搜索入口。
它不是"未声明工具按需发现"——缺的是 pi 的**延迟声明**维度。

## 2. pi 的语义（B 级）

- **exposure 五档**：`direct`（声明给模型）/ `model-only` / `codemode` /
  `deferred`（不声明，模型可搜索发现）/ `hidden`（模型不可见）。
  MCP server 级 `exposure` + 工具级 `toolExposure` 覆盖。
- **tool_search**：对未声明工具按 BM25 排名；模型调 `search_tools(query)` 后，
  命中工具**声明给下一轮模型调用**；声明记录进会话（按分支持久，resume/分支
  不丢不串）。
- `describeNamespace(name)`：返回 namespace 描述 + 工具名清单，配合 server
  `description` 字段让模型知道往哪搜。
- `codemode.inlineBudget`：声明清单有 token 预算，超了说明"清单不完整"。

### 2.1 pi `0.99.2`/`v1.0.0` 的迭代修正（2026-10-02 增补，B 级）

pi 自己在两版里对本机制做了三处实质迭代，均转化为本设计 §3/§6 的约束：

1. **描述静态化（0.99.2，#10212）**：默认 exposure 的 MCP server 不再进
   `codemode`/`tool_search` 的工具描述清单——描述不再随 MCP server 连接、
   断开、换工具而变化（那是 prompt cache 的隐形杀手）；易变信息移到独立的
   `mcp_servers` system prompt 段（一行式摘要，每轮 prompt 开始时更新，
   **变更才追加进会话**）。配套：第一个 prompt 不再等待无 `direct` 工具的
   server（后台连接，脚本点名/search/`tool_search` 运行时才等待）。
2. **发现辅助定型（0.99.2）**：server `description` 字段参与 tool search
   排名；`describeNamespace(name)` 随版发布（返回 namespace 指令 + 工具名
   清单），namespace 名 `-`/`_` 等价、两种拼法都收（1.0.0 进一步把 MCP
   工具名里的 `-` 归一为 `_`，仅差 `-`/`_` 的冲突工具加 hash 后缀）。
3. **resume 恢复坑（1.0.0 修复项，反面教材）**：`tool_search` 已加载的
   deferred 工具在 resume/`/reload` 时被整批丢弃——会话先恢复工具面、MCP
   server 后重连，时序倒挂导致声明态丢失。**fork 侧对应物 = §3.4 的"声明
   记录在子 agent 会话内"**：施工时必须保证恢复顺序是"子 agent 会话恢复 +
   工具面重建"原子完成，不能出现"声明态在、工具面没挂上"的中间态。
4. **声明面瘦身（1.0.0）**：codemode 描述整体瘦身 ~40%（一行式工具声明 +
   指向 docs 的指针，模型要用时才读全文）——佐证 §5 待定决策 1 的降级路线
   （先简单打分、声明面从简），声明面 token 本身就是 deferred 方案的收益项，
   别让 `search_tools` 自己长成新的大面。

### 2.2 三参照合并定稿（2026-10-04，kimicode-port 决策 D2 落实）

同一道题的三个参照：pi（§2/§2.1，搜索式发现）+ kimi（`toolSelect.ts`，
公告式发现）+ step（**无工具面裁剪参照**，其 branch summarization 已属
kimicode-port ①；不带来新约束）。合并裁定：

1. **首版发现机制 = kimi 公告流 + 按名展开，不上 BM25**（roadmap §3 国模
   约束拍板）：deferred 工具只以名字清单出现在公告里；模型调
   `select_tools(names)`（fork 侧命名沿用 `search_tools`，§3.4）后完整
   schema 下一轮进声明面。按名认领比自主搜索对国模可靠。pi 的
   `describeNamespace`/description 参与排名三条约束**不丢弃**，降格为
   公告内容质量约束：公告必须携带 server `description` 一行式摘要，让
   模型"知道往哪认领"（§6 风险缓解的替代形态）。BM25 留 trait 位，
   实测按名认领不够再升（原待定决策 1 就此关闭）。
2. **kimi capability 门吸收（国模适配硬约束）**：只有模型能力位
   `dynamically_loaded_tools === true` 的模型走 deferred 档；其余模型
   **回落 direct 全量声明**——"按能力裁剪请求，而不是发出去等上游报错"
   （kimi `toolSelectService.ts:87-88` 的语义）。落点：defer 档生效前查
   模型能力位，不满足则静默降级 direct 并打一条 info 日志。
3. **kimi 历史剥离（`stripDynamicToolContext`）的 fork 等价物**：kimi 的
   动态 schema 在历史消息里流转，需要持续剥离省 token；fork 的 schema
   走**请求级 `tools` 参数**，历史里只可能有轻量公告消息（纯名字清单），
   天然无 schema 膨胀——不需要移植剥离函数，等价保障 = **公告消息保持
   轻量且变更才追加**（与 §3.4 静态描述约束同一形态）。声明态本身持久
   在子 agent 会话（§3.4），历史剥离问题在 fork 侧消解。
4. **公告流的具体形态（合并 kimi 增量流 + pi 静态化）**：声明态变化时
   追加一条 `<system-reminder>`（fork 既有注入形态），内容 = 本轮新增
   工具名清单（`tools_added` 语义）；首版不做 `tools_removed`（无移除
   入口，defer 声明态只增不减，降复杂度；pi 同样无按会话移除）。

### 2.3 zcode 3.16.x 第四参照（2026-10-21，A 级源码）

> 来源：zcode 3.16.2（`3c5b2276`），核心在 `apps/zcode-cli/packages/core/src/tool/search/`、
> `contracts/src/tools/{tool-search,deferred-tool-call}.ts`、`core/src/tool/deferred-call.ts`，
> 设计文档 `docs/design/v2/tool/19-tool-search.md`。完整变更清单与内置技能/插件判定见
> `docs-local/agent-cli-tracking/notes/zcode.md`「2026-10-21 刷新」节。

zcode 在 3.16.x 把「延迟工具声明」做成了完整产品面，验证了本原型的三条核心判断，也补了
fork 设计里没展开的几处：

1. **模式分层**：CLI `--toolsearch-mode auto|native|local|off`，缺省 off；Native 仅
   Anthropic Messages（beta `advanced-tool-use-2025-11-20` + `defer_loading` +
   `tool_reference`），Local 与 provider 无关（常驻工具 + `ToolSearch` +
   `DeferredToolCall` 包装），Off 全量。启动定型、不写 session、子 Agent 继承。
   → 与本原型 §3.1 三档 exposure 同构；fork 的 `supports_dynamic_tools` 能力门对应其
   「Local 要求模型支持普通工具调用」。
2. **deferred 分类元数据**：常驻名单（Agent/AskUserQuestion/Bash/Edit/Glob/Grep/Read/
   ReportFindings/Skill/ToolSearch/Write）+ 其余工具 `shouldDefer: true`；MCP 默认延迟，
   server 级 `alwaysLoad`（含 `_meta["anthropic/alwaysLoad"]`）与每工具 `searchHint`。
   → 本原型 §3.1/§3.4 的 `exposure` 三档 + server description 摘要可以照此落成
   `should_defer` / `always_load` / `search_hint` 三个 metadata 位。
3. **发现与公告**：`ToolSearch` 支持 `select:A,B`、关键词、`+required`、MCP 短名，默认
   5 条；公告只追加 added/removed/readded 名称增量，带 30 名阈值与 MCP server 分组，
   compact 后按保留历史重建。→ 本原型 §2.2 选「公告流 + 按名认领」被 zcode 的
   `select:` 精确选择印证；差别是 zcode 同时保留关键词打分，fork 首版仍按名匹配，
   把关键词留 trait 位。
4. **包装执行**：Local 模式用 `DeferredToolCall {name,input}` 承载调用，调度前解包为目标
   调用并复用同一 callID 的准入/审批/Hook/超时/取消/结果链；内部历史存目标名 +
   `modelCallWrapper` 标记，请求出口恢复包装。→ 本原型 §3.4 选择「下一轮把 schema 直接
   加入声明面」，不需要包装工具；但若实测某些国模对「动态 tools 数组」不友好，
   zcode 的包装是可退守的第二形态（模型只认一个稳定控制工具）。
5. **compact 后提升**：两次 compact 之间 tools 不随发现增长；成功 compact 后把已发现
   deferred 工具提升为普通 tools，避免 resume 后重搜。→ 本原型 §3.4 的「声明态随子 agent
   会话持久」需要补这一条：fork 子 agent 若触发压缩，发现集合也要从 compact 检查点恢复，
   不能只靠历史里的公告。
6. **持久化/冷恢复**：发现集合只从真实成功的 `tool_reference` 结果 + 有效 compact 检查点
   推导，不存 schema 快照；公告正文与 metadata 随隐藏 synthetic 历史持久化。
   → 对应本原型 §3.4 的 resume 验收 5；zcode 明确「公告在请求投影之后生成会累积」的坑，
   fork 侧公告必须在提交历史后、请求快照前生成。

**与 fork 原型的差异（不改本原型判定）**：

- zcode 面向主会话 + 所有 provider；本原型只管 workflow 子 agent 面（§3.5），主会话维持全量。
- zcode 缺省 off、按 CLI/App 显式开；本原型缺省 direct，`defer_tools=true` 才裁面。
- zcode Native 依赖 Anthropic 原生引用；fork 的 BYOK 国模主路径对应 Local，但本原型走
  「下一轮 schema 注入」，不引入包装工具。
- zcode 的常驻名单含 `ToolSearch` 自身与 `DeferredToolCall`；fork 首版控制面只有
  `search_tools`，需在工具面裁剪时保证它不被裁掉。

## 3. fork 移植原型


### 3.1 exposure 收敛为三档（决策 D2）

fork 没有 codemode 档的需求（README 决策 D1），五档收敛为三档：

| 档 | 语义 |
|---|---|
| `direct` | 声明给模型（现状行为） |
| `deferred` | 不声明；模型可经 `search_tools` 发现并声明 |
| `hidden` | 不声明、不可发现（保留连通性，如后台 server） |

落点：MCP server 配置加 `exposure` / 工具级 `toolExposure` 覆盖
（`xai-grok-config-types` → `xai-grok-mcp` admission 后过滤）；`/mcp`
（`slash/commands/mcps.rs`）展示与改档。

### 3.2 `AgentOpts` 扩展（workflow 面的入口）

`xai-workflow/src/host.rs:5` `AgentOpts` 新增：

```rust
pub tools: Option<Vec<String>>,      // 显式子集（direct 白名单）
pub defer_tools: bool,               // 未列入白名单的工具走 deferred
```

进 serde payload → 进 `request_hash` → 白名单变化自然 divergence（与
`capability_mode`/`effort` 同模式；`effort` 有过 legacy-hash 兼容先例，
`engine.rs replay_spawn_agent`，新字段直接进 hash 即可，无需兼容层）。

### 3.3 host_service 传递通道

spawn 路径（`xai-grok-shell/src/session/workflow/host_service.rs`，现 `:490`
capability_mode 解析处同层）把 `tools`/`defer_tools` 解析为子 agent 的
工具面裁剪参数。`capability_mode`（read-only 等权限档）先做交集——exposure
只裁"声明面"，不放大权限，两通道正交。

### 3.4 `search_tools` 延迟声明（定稿 2026-10-04：按名认领 + 公告流）

- `defer_tools=true` 的子 agent 工具面 = 白名单（direct）+ 一个 `search_tools`
  工具；deferred 工具以**名字清单公告**进会话（§2.2 第 1/4 条），server
  `description` 摘要随公告携带（pi discover 辅助约束的降格形态）。
- `search_tools(query)` 按**名称精确/前缀 + description 包含**匹配打分
  （不上 BM25，§2.2 第 1 条），命中者声明给下一轮。
- 声明记录在子 agent 自己的会话内（fork 子 agent 已有会话承载，声明变化随
  会话走，天然满足 pi 的"按分支持久"）；workflow journal 不感知——延迟声明
  是子 agent 会话内部状态，不产生 result-bearing host call，**不碰重放语义**。
- **静态描述约束（吸收 0.99.2 #10212）**：`search_tools` 自身的工具描述与
  deferred 工具的声明清单在 prompt 构建时一次性定型，不随 MCP 连接状态
  动态变化；server 层的易变信息（连接态、指令）若需要进 prompt，走"变更才
  追加"的独立段落，不回流到声明清单。resume 时声明态与工具面同批重建
  （§2.1 第 3 条的坑位规避）。

### 3.5 明确不做

- 不做 pi 的 `codemode` 档与 `namespace` 脚本 API（依赖 D1，不做）。
- 不做主会话的 exposure 管理（本专题只管 workflow 子 agent 面；主会话 MCP
  面维持全量现状，若实测主会话也吃紧再单独立项）。
- 不做 `inlineBudget` 精细预算——fork 先用"白名单 + defer"二分，预算超限
  场景等实测数据再定。

## 4. 验收标准

1. fan-out 20 个子 agent、MCP 面挂 3 个 server（≥30 工具）的对照实验：
   defer 档子 agent 首 prompt 工具面 token 相对 direct 档下降可量化（曲线留档），
   且任务完成率不劣化（同一 workflow A/B）。
2. `search_tools` 命中 → 下一轮可调用命中工具；未命中返回空集不报错。
3. `AgentOpts.tools`/`defer_tools` 改动触发 divergence（复用 `engine.rs`
   `agent_current_hash_mismatch_still_diverges` 测试形态）。
4. `capability_mode=read-only` × `defer_tools=true` 交集正确：deferred 命中也
   不会解锁写工具。
5. resume 用例：子 agent 中断后恢复，此前经 `search_tools` 声明的工具**仍可
   直接调用**（声明态随会话恢复，不重搜；对照 pi 1.0.0 修复的丢失 bug）。
6. **能力门（§2.2 第 2 条）**：无 `dynamically_loaded_tools` 能力位的模型在
   `defer_tools=true` 下回落 direct 全量声明，日志可见降级；deferred 工具不进
   请求面。
7. `ctest.sh -p xai-workflow --lib` + `ctest.sh -p xai-grok-agent --lib`（工具面
   裁剪在 agent 侧，注意该 crate 级联重编面，见 AGENTS.md 时间成本原则）。
8. **compact 后提升（zcode 3.16 补充）**：子 agent 触发压缩后，此前经
   `search_tools` 声明的工具仍可直接调用；发现集合从有效 compact 检查点恢复，
   不重搜、不因保留尾部丢失。
9. **公告幂等（zcode 3.16 补充）**：公告只在声明态真实变化时追加一次；请求投影
   不得反向写入公告历史（zcode 修过的「投影后生成 → 反复累积」坑）。
10. **MCP 元数据（zcode 3.16 补充）**：server 级 `alwaysLoad` 与每工具
    `searchHint` 传递到请求面；`alwaysLoad` 只影响声明曝光，不放大权限。

## 5. 待定决策（定稿后剩余）

1. ~~BM25 是否首版就上~~ **已关闭（2026-10-04，§2.2 第 1 条）**：首版按名
   匹配 + description 包含，BM25 留 trait 位。
2. MCP `toolExposure` 工具级覆盖与 server 级默认的优先级实现位置
   （config-types 合并时归一 vs mcp 侧读取时归一）——倾向后者，配置面保持哑。
3. `search_tools` 结果的缓存（同 query 重复调用）——pi 未提及，先不做。
4. 主会话是否跟进三档 exposure——见 §3.5，登记待实测。
5. ~~模型能力位的读取通道~~ **载体已核实（2026-10-04）**：fork 的模型能力位
   走 `SamplerConfig` per-model 布尔（先例 `supports_backend_search`，
   `config.rs:111`）。deferred 门新增同模式标志（拟
   `supports_dynamic_tools`，默认 false = 回落 direct），BYOK/国模在模型
   配置里显式打开；kimi 的 `dynamically_loaded_tools` 是服务端模型元数据，
   fork 无对应源，不改成运行时拉取。

## 6. 风险

- **deferred 发现率**：模型搜不到该搜的工具会导致任务退化——缓解：server
  `description` 必填进 `search_tools` 的 namespace 描述（pi `0.99.2` 已把
  `describeNamespace` + description 参与排名定型，原"Unreleased"状态已落
  地），验收 1 的 A/B 兜底。
- **恢复时序**：子 agent 会话恢复与 MCP 工具面重建若不同步，会出现 pi 1.0.0
  修过的同类丢声明 bug（§2.1 第 3 条）——验收标准需含"resume 后 deferred
  已声明工具仍可调用"用例。
- **公告与 compact 的账本一致性**：zcode 修过两类坑——公告在请求投影之后生成会
  反复累积进主历史；compact 只保留尾部时全量 metadata 与增量正文不一致。fork 侧
  必须让公告在提交历史后、请求快照前生成，并把发现集合写进 compact 检查点
  （验收 8/9）。
- **包装工具是第二形态，不是首版**：zcode Local 用 `DeferredToolCall` 包装调用，
  适合「模型只认稳定控制工具」的场景；本原型首版走下一轮 schema 注入，避免多一层
  历史/权限/媒体投影。若实测国模对动态 tools 数组不友好，再按 zcode 形态退守。
- **能力门不能照搬**：zcode Native 只对 Anthropic + Coding Plan 默认开，Local 依赖
  「模型支持普通工具调用」；fork 的 `supports_dynamic_tools` 仍是显式能力位，默认
  false 回落 direct，不能因为 zcode 能跑就默认打开。
- **xai-grok-agent 级联重编**：工具面裁剪动 agent builder/config，触及其
  依赖树；施工时先 `cargo check -p` 验证再进测试（AGENTS.md 调试时间成本原则）。
