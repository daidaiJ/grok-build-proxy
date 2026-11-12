# zcode v3.17.x → grok-build 移植专题

> **状态：调研入库，未开工（2026-11-07）**。回答的问题是：父目录 zcode 同步到 v3.17.2 后，
> 它的 CLI 功能设计与插件/技能有哪些值得移植到本 fork。本轮只入库文档，**无代码改动**。
> 上游：[zai-org/ZCode](https://github.com/zai-org/ZCode)，本地克隆 `D:/CODE/ai/zcode`，
> 基准 `a9fe401e`（v3.17.2，CLI 工作区 0.17.1，2026-11-03）。
>
> **事实源分工**（避免两处漂移）：逐条机制与 file:line 锚点在
> [`agent-cli-tracking/notes/zcode.md`](../agent-cli-tracking/notes/zcode.md)「2026-11-07 刷新」节；
> 引入定级、防误报与负面清单在 [`agent-cli-tracking/ADOPTION.md`](../agent-cli-tracking/ADOPTION.md) §9；
> **本文 = 决策摘要 + 待办台账**，只给结论、落点与触发信号，不重抄源码锚点。

## 1. 同步与去伪

`git -C D:/CODE/ai/zcode pull --ff-only origin main`：`3c5b2276`（2026-10-20，v3.16.2）→
`a9fe401e`（2026-11-03，v3.17.2），2 个快照提交（`a60ba6a7` 无版本标题 + `a9fe401e` v3.17.2），
新增 tag v3.17.1 / v3.17.2；根 `package.json` 3.17.2、CLI 工作区 0.17.1。diff
5,315 文件 / +226,212 / −613,001 行——**这是快照重排，不是特性量**。

去伪是本轮的方法收获：git 明确告警 `exhaustive rename detection was skipped`（文件数超
`renameLimit`），A/D 标记会把目录重排误判成新插件。用
`git ls-tree -r <old> | grep <path>` 与 `<new>` 交叉核对后才敢下结论：

- 插件包**没有**发生 `packages/` → `apps/zcode-cli/packages/` 迁移（那是更早版本的事）；
- 唯一真新增插件 = **`zcode-site-plugin`**（50 文件全 A，`3c5b2276` 无此路径）；
- superpowers 14 / zcode-guide 6 / bundled 1 技能数量零变化，两个 `skills-lock.json` blob hash 未变；
- 101 份新文档里只有 5 份是 CLI 设计文档，其余是桌面/远程/数据同步 issue，与终端 fork 无关。

## 2. 增量地图（按面）

| 面 | 3.17.x 带来什么 | fork 是否缺口 | 判定 |
|---|---|---|---|
| 协议 / 请求形态 | **provider-builtin-tool-compat**：GLM 服务端内置工具把多轮合成单条 assistant 下发成 markdown 伪装调用，按结构改写成原生 `mcp_tool_use`/`mcp_tool_result` 并删重，防模型「照着文字假装调用工具」 | 是（fork 的 `server_tool_use` 是 headless 里 Grok 原生 web_search 内联块解析，语义不同） | 🟡 条件性 P1 |
| 协议 / 请求形态 | **provider-tool-rounds**：服务端执行的内置工具按轮切分回放；`compactGroupKey` 取持久化 assistant 消息 id（tool call id 会复用）、有轮次不挂 tokens（服务端 usage 是多轮合计，会高估一倍） | 是 | 🟢 P1 参照 |
| 协议 / 请求形态 | **按协议固定请求形态**：仅 anthropic-messages 把 system-reminder 折进 tool result，OpenAI 兼容保持独立 user 消息 | 是（三协议转换已纯函数化、有 byte-equal 不变量测试，但 reminder 折叠未分协议定型） | 🟢 P1 参照 |
| 上下文 | **post-compact-file-restore**：压缩后按 `readFileState` 重读磁盘补回文件内容（≤5 文件 / 单文件 5k / 总 50k tokens） | 是 | 🟢 P1 参照 |
| 工具 | **ToolSearch 3.17 修正**：公告以 metadata 为唯一事实源渲染、模式切换只重渲染文案不替换历史文本、`hiddenNames` 区分「本轮隐藏」与「MCP 断连」、关键词明确不支持正则 | 是（workflow-pi-port P2 T2 **仍未开工**，原型四参照定稿于 3.16.2） | ⭐ P1 先做 |
| hooks / 停止 | **hook `onError`**：`PreToolUse` 的新配置字段 `allow|deny`（缺席 fail-open，仅 hook 自身故障生效，deny 等价 exit 2）；**interrupt-marker**：用户停止标记按请求/工具阶段区分 + 取消工具结果统一改写 + 冷恢复还原 | 部分（七事件 fail-open 已有，缺 per-hook 故障策略；无需加第八事件） | 🟢 P1 小件 |
| 工程治理 | **CONTRACT.md 契约四件套**（CONTRACT.md / contract.ts / module.ts / contract.example.ts / +contract.test.ts）+ `architecture-policy.yaml` 阈值（800 行/文件、300 行契约、12 公开方法、禁环禁深导入）+ 13 条规则 + baseline 只压存量 + pre-push 门禁 + `architecture:adopt` 脚手架 + 「先契约后代码」skill | 是（有 clippy/rustfmt/AGENTS.md 规约，无机器可执行的模块边界与 owner/依赖白名单） | ⭐ P1/P2 工程资产 |
| 插件 / 技能 | `zcode-site-plugin`：`site-deploy` 技能 + 7 个 `/site-*` + MCP `sites` + Sites 面板；官方 seed 表新增该项，`requiredSeedPaths` 差集拒装 + 逐文件 SHA-256 + marker + 原子替换 + 15s lock 有了完整实现 | 技能正文不适用；seed 完整性门禁是缺口 | ❌ / 🟢 P3 |
| 官方形态绑定 | session-features header、session-lifecycle V4 五态机、mcp-node-fallback、official-command-endpoint、local-ttft、input-prediction-sidecar、conversation-quote-context、node-repl-call-ledger、dynamic-workflow 因果绑定分析 | — | 🗂 留档 / ❌ |

## 3. 移植拍板清单

口径沿用 [ADOPTION.md](../agent-cli-tracking/ADOPTION.md) §0：**协议接口兼容性、token 经济学、
TUI 便利性**，不碰架构、不引入内置模型目录/厂商 preset。

| # | 项 | 落点 | 前置 / 触发信号 | 状态 |
|---|---|---|---|---|
| 3.1 | ToolSearch 3.17 修正并入 P2 T2 第五参照 | `docs-local/workflow-pi-port/deferred-tool-exposure.md` | 该专题「预研选型完成，未开工」，**现在补零成本** | ⏳ 未做 |
| 3.2 | provider-tool-rounds + 按协议折叠 reminder + 压缩后文件回补 | `xai-chat-state` 回放 / `xai-grok-sampler` usage 归属 / 请求构造边界 / `xai-grok-compaction` | 必须验与 §2.1 byte-equal 前缀不变量测试不冲突 | ⏳ 未做 |
| 3.3 | hook `onError` + interrupt-marker | `xai-grok-hooks` + shell stop/cancel 路径 | 都不碰架构，可独立提交 | ⏳ 未做 |
| 3.4 | CONTRACT.md 契约治理（**本轮唯一跨语言、长期复利的工程资产**） | 仓库工具链 + 新增 architecture-governance skill | 先定 adopt 范围：建议挑 `xai-grok-mcp` / `xai-grok-compaction` 两个高频改动 crate 试点 + baseline 压存量，**别一次性全仓铺开**；Rust 侧 trait/newtype 承载公开面、`pub(crate)` + workspace `Cargo.toml` 天然做入口边界 | ⏳ 未做 |
| 3.5 | provider-builtin-tool-compat | `xai-grok-sampler` 流 collect / 协议转换层 | **条件触发**：BYOK 网关真出现 markdown 伪装内置工具时，用 `[api_dump]`（`~/.grok/dump/`）现场取证再立项 | ⏳ 待取证 |
| 3.6 | workspace-mcp-trust 的 `allows` 准入重验 + `guard` 包 lease | `xai-grok-mcp` 准入 + `xai-grok-hooks` | 与 ADOPTION §7.1.10（workspace hook 信任摘要）合并成一个安全小件，先核对 `folder_trust` 覆盖面 | ⏳ 缓 |
| 3.7 | 官方插件 seed 完整性门禁 | `xai-grok-plugin-marketplace` bootstrap | 维持 §8.1.10 的 P3；实现参照已备，随插件市场轮 | ⏳ 缓 |
| 3.8 | superpowers 三件 skills-only A/B（TDD / systematic-debugging / verification-before-completion） | bundled skills | **§8.4 顺位第 2 条仍未做**；本轮核实 fork 里 `superpowers` 只出现在 pager 插件市场测试 fixture，技能正文零变化，随时可开工 | ⏳ 未做 |
| 3.9 | v3.18.0 的 Prompt Snapshot / 停滞提醒 / 修复持久化（成本·稳定性·体验轴） | `xai-chat-state` 冷载 + `xai-grok-sampler` 重试 + shell 后台任务 | 见 [`../agent-cli-refresh-2026-11/README.md`](../agent-cli-refresh-2026-11/README.md)；先 probe 冷恢复 head 重写是否真断缓存 | ⏳ 未做 |

**留档不立项**：session-features header 架构（能力位只增不减 + 冷恢复推导，词表绑官方统计）、
session-lifecycle 五态机/唯一写者/并发矩阵、`causality-order-bindings`（TS AST）、
`tools/provider-regression` 模型回归夹具、prompt-trajectory（fork 已有 unified.jsonl + limit-probe）。

## 4. 不移植（负面清单增补）

- **`zcode-site-plugin` 全家**：云运行时硬绑 Debian 12/x86-64、Node 24/Python 3.12、
  `0.0.0.0:3000`、90s 健康检查、包 ≤200MB，`src/server/api.ts` 直注 "zcode-server origin" +
  官方 JWT 头 + 配额/内容审核错误码（3611/3612/3608）。可借的只有「stage + `start.sh` +
  本地 check + 上传前显式确认」的交互契约。
- **mcp-node-fallback**（Electron Node shim）、**official-command-endpoint / local-ttft /
  input-prediction-sidecar / conversation-quote-context**（官方服务端 + 多端 V4）、
  **node-repl-call-ledger**（Worker broker）、**session-lifecycle V4**、桌面 Gen UI / office 查看面板 /
  CUA / Android·iOS 模拟器。
- 维持 ADOPTION §7.2 / §8.2 全部负面清单：dynamic-workflow 模型写 TS 工作流、
  model-option-map DSL、off-peak 票据、ReplyToChannel/Feishu、MCP Apps、restore-legacy-sessions。

## 5. 防误报基线（本轮实际核实，别再当新特性立项）

| 外部特性 | fork 现状 |
|---|---|
| superpowers 14 技能 | ❌ 未引入：`superpowers` / `test-driven-development` / `systematic-debugging` 只出现在 `xai-grok-pager/src/views/extensions_modal.rs` 与 `cta_e2e.rs` 的**插件市场测试 fixture**（模拟 obra/superpowers 市场源） |
| `server_tool_use` | ✅ 已有但语义不同：`xai-grok-pager/src/headless/reducer/messages/web_search.rs` 是 Grok 后端 web_search 内联块的 headless 输出解析，不是入站协议改写 |
| hooks 七事件 / fail-open | ✅ 已有（`xai-grok-hooks`）；「九事件/八事件」说法不成立 |
| 插件市场 / 安装账本 | ✅ 已有（`xai-grok-plugin-marketplace` + pager `/plugin`）；缺的是 `requiredSeedPaths` + SHA-256 门禁 |
| docx/pdf/pptx 技能、`/doctor`、`/mcps`、`create-skill` | ✅ 已有等价物；本轮这些类别只有 1–2 行修订或版本 bump |

## 6. 待办台账

- [ ] **3.1** 把 3.17 公告/投影修正补进 `deferred-tool-exposure.md` §2.3 之后（第五参照），随 P2 T2 开工
- [ ] **3.8** superpowers 三件 A/B（对比现有 `/code-review` / `/verify` 流程），再决定是否整包引入
- [ ] **3.9** v3.18.0 成本/稳定性/体验三件（Prompt Snapshot / 停滞提醒 / 修复持久化），按 `../agent-cli-refresh-2026-11/README.md` 的 P1 顺序
- [ ] **3.2** 协议/压缩轮三件对照，先跑前缀不变量测试
- [ ] **3.3** hooks 小件独立提交
- [ ] **3.4** CONTRACT.md 试点范围拍板（2 个 crate + baseline）
- [ ] **3.5** `[api_dump]` 抓到伪装内置工具现场再立项
- [ ] **3.6 / 3.7** 随 folder_trust 核对 / 插件市场轮

## 7. 刷新方法

```bash
git -C "D:/CODE/ai/zcode" fetch --all --tags
git -C "D:/CODE/ai/zcode" pull --ff-only origin main
git -C "D:/CODE/ai/zcode" log --oneline <old>..HEAD
# 判定「某资产是否新增」必须双版本树核对，别信 diff 的 A/D：
git -C "D:/CODE/ai/zcode" ls-tree -r <old> | grep <path>
git -C "D:/CODE/ai/zcode" ls-tree -r HEAD   | grep <path>
```

刷新后按 §2 各面补 `notes/zcode.md` 新节 → 在 `ADOPTION.md` 追加定级 → 回写本文 §3 台账；
版本号看根 `package.json`，CLI 工作区版本看 `apps/zcode-cli/package.json`。

## 8. 存疑

3.17.x 的 compat / rounds / session-features 证据主要是仓库内单测、脱敏 SSE fixture 与设计文档，
**未跑真实多网关 BYOK**；「服务端内置工具按轮回放」的收益只在官方 Coding Plan 链路上自证，
别家国模网关是否下发同形态需 `[api_dump]` 现场取证。
