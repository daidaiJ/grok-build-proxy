# P2 设计原型：延迟工具声明 + exposure 三档

> 状态：预研原型，未开工、未验证。语义来源：pi `0.99.0` tool_search 与
> tool exposure（docs/cli.md#tool-search、docs/extensions.md#tool-exposure，
> B 级转述），**2026-10-02 增补 `0.99.2`/`v1.0.0` release notes 对本设计的
> 三条修正（§2.1）**。fork 基线 `95aad87`。

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

### 3.4 `search_tools` 延迟声明

- `defer_tools=true` 的子 agent 工具面 = 白名单（direct）+ 一个 `search_tools`
  工具；`search_tools(query)` 对 deferred 工具的**名称 + description** 做
  BM25（pi 同款排序，B 级），命中者声明给下一轮。
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
5. `ctest.sh -p xai-workflow --lib` + `ctest.sh -p xai-grok-agent --lib`（工具面
   裁剪在 agent 侧，注意该 crate 级联重编面，见 AGENTS.md 时间成本原则）。

## 5. 待定决策

1. BM25 是否首版就上——工具名精确匹配 + description 包含匹配可能已够用
   （fork 工具面远小于 pi 生态），首版可以降级为简单打分，留 trait 位。
2. MCP `toolExposure` 工具级覆盖与 server 级默认的优先级实现位置
   （config-types 合并时归一 vs mcp 侧读取时归一）——倾向后者，配置面保持哑。
3. `search_tools` 结果的缓存（同 query 重复调用）——pi 未提及，先不做。
4. 主会话是否跟进三档 exposure——见 §3.5，登记待实测。

## 6. 风险

- **deferred 发现率**：模型搜不到该搜的工具会导致任务退化——缓解：server
  `description` 必填进 `search_tools` 的 namespace 描述（pi `0.99.2` 已把
  `describeNamespace` + description 参与排名定型，原"Unreleased"状态已落
  地），验收 1 的 A/B 兜底。
- **恢复时序**：子 agent 会话恢复与 MCP 工具面重建若不同步，会出现 pi 1.0.0
  修过的同类丢声明 bug（§2.1 第 3 条）——验收标准需含"resume 后 deferred
  已声明工具仍可调用"用例。
- **xai-grok-agent 级联重编**：工具面裁剪动 agent builder/config，触及其
  依赖树；施工时先 `cargo check -p` 验证再进测试（AGENTS.md 调试时间成本原则）。
