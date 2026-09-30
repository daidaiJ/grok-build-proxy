# P2 设计原型：延迟工具声明 + exposure 三档

> 状态：预研原型，未开工、未验证。语义来源：pi `0.99.0` tool_search 与
> tool exposure（docs/cli.md#tool-search、docs/extensions.md#tool-exposure，
> B 级转述）。fork 基线 `95aad87`。

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
  `description` 必填进 `search_tools` 的 namespace 描述（pi Unreleased 的
  `describeNamespace` 同款思路），验收 1 的 A/B 兜底。
- **xai-grok-agent 级联重编**：工具面裁剪动 agent builder/config，触及其
  依赖树；施工时先 `cargo check -p` 验证再进测试（AGENTS.md 调试时间成本原则）。
