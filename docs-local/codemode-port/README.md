# Code Mode 接入规划（2026-11-28）

> 结论：**现在不接入，先观察 2–3 个月；2027-02-15 做 gate 评审。**
> 不做只读/裁剪版 Code Mode。gate 通过后按本文的完整链路一次性接入；
> gate 不通过就继续等，不在等待期堆半成品。

## 1. 为什么现在不建议动手

### 1.1 psychoscript 的成熟度

`anomalyco/psychoscript` 是 Code Mode 的语言层，也是整个移植的关键依赖。截至 2026-11-28：

| 指标 | 数值 | 含义 |
|---|---|---|
| 仓库创建 | 2026-11-13 | 两周新 |
| Stars / forks | 457 / 11 | 关注度高，生产采用证据少 |
| Open issues | 21 | 仍在快速收敛 |
| Release | v0.1.0（11-19）→ v0.4.0（11-27），8 天 7 个版本 | 迭代极快 |
| 破坏性变更 | 0.2.0 根 API 改为 Promise；未发布 changelog 里 snapshot format 4 不兼容旧状态 | API/状态格式仍在动 |
| Rust crate | 本地 clone `e8380b6` 为 `0.1.3`；GitHub release 到 `v0.4.0` | Rust crate 与语言包版本号要分开跟踪 |

这说明它不是一个可以“钉一个版本、半年不动”的依赖。现在接，维护成本会集中在
追 API、追 snapshot 格式、追 oxc/ICU 依赖上，而不是 Code Mode 本身。

### 1.2 opencode 的背书与边界

- opencode `v2.4.2` 已经包含 `packages/codemode`，README 写得完整，
  但它是“opencode 自己的 code mode 实现”，不是广泛生产验证的通用组件。
- opencode 是主要背书方；社区口碑目前集中在“设计完整、解释器路线可信”，
  还缺少“多项目长期跑通”的证据。
- 这不否定方向。它说明：**方向对，时机早。**

### 1.3 为什么不做猴版

只读版、JSON `{code}` 版或“先跑通再说”的裁剪版，能验证协议，但达不到
Code Mode 的成本目标：

- 只读工具折不动 terminal-heavy 的主链路（本仓终端调用占比约 62%）。
- 没有写文件/终端，模型仍要逐步调用，省不下 model step。
- 没有 freeform wire，代码被 JSON 转义，token 和出错率都变差。
- 没有历史回放和嵌套权限，后面还要重做一遍。

所以本文的立场是：**要么按完整链路接，要么等；不交付一个达不到预期目标的中间态。**

## 2. 预期目标（验收口径）

Code Mode 是否值得接，用下面这组指标判断，而不是“功能能跑”。

| 维度 | 目标 | 测量方式 |
|---|---|---|
| 成本 | 长回合输入成本再降 15%–25% | 账本 token 加权 + `$/call` |
| 步数 | 终端密集型会话的 model step 降 15%–30% | 每用户 turn 的 model step 中位数 |
| 能力 | read / edit / write / terminal / MCP 全覆盖 | 嵌套调用目录与权限测试 |
| 安全 | 权限/hook/plan mode 不被绕过 | 嵌套调用逐项验收 |
| 回放 | custom call 与 tool result 类型严格配对 | Responses/Chat fixture 回放 |
| 缓存 | cache 命中率下降 ≤2% | 账本 cache_read 对照 |
| 构建 | Windows release <55 分钟；二进制 +≤15 MB | CI 计时与产物大小 |
| 稳定 | Code Mode 失败率 <5% | `shell.codemode.*` 日志 |

## 3. 选项与建议

### 选项 A：现在全量接（不推荐）

- 范围：freeform wire + 嵌套执行器 + psychoscript + 全工具。
- 估时：36–48 人日。
- 风险：psychoscript 0.x API 和 snapshot 格式继续变；每轮升级都要重跑
  Windows release 和回放测试。
- 适用：用户明确接受“追版本”的维护成本，且愿意把 psychoscript vendor 进来。

### 选项 B：现在做只读/裁剪版（明确不做）

- 能验证协议，但省不下主链路的 step 和 token。
- 后续 freeform、历史回放、权限还要重做。
- 结论：不做。

### 选项 C：等待 + gate（推荐）

- 2026-11-28 → 2027-02-15 观察 psychoscript / opencode / provider 支持。
- 等待期不写 Code Mode 代码，不做工具链升级；只做零成本的跟踪。
- 2027-02-15 按第 5 节 gate 评审：过则启动完整接入，不过则顺延一个季度。

### 选项 D：现在就做非 Code Mode 的成本优化

如果希望等待期也有收益，优先做已有的低成本项：

- 终端输出过滤/截断策略；
- 并行工具调用与 batch 合并；
- replay diet / compaction 的继续验收；
- 工具结果大小与重复调用治理。

这些不依赖 psychoscript，也不会在 Code Mode 接入后作废。

## 4. 等待期跟踪清单（2026-12 → 2027-02）

每月一次，只记录，不改代码：

1. **Rust crate 版本与破坏性变更**
   - `psychoscript` Rust crate 是否到 0.4/0.5/1.0；
   - 连续 4 周无 breaking change；
   - snapshot 格式是否稳定。
2. **opencode 集成状态**
   - Code Mode 是否仍默认/可选；
   - 是否有生产使用反馈、issue 收敛情况；
   - freeform wire 是否变化。
3. **社区采用**
   - 是否有 opencode 之外的 agent/工具采用 psychoscript；
   - issues 中 P0 稳定性/安全问题。
4. **provider 支持**
   - OpenAI Responses/Chat 的 custom tool 实测；
   - xAI Responses 的 custom tool 实测；
   - 主流 OpenAI-compatible 网关的拒绝/接受情况。
5. **构建成本**
   - Rust 1.99+ 工具链在本仓 CI 的耗时；
   - oxc/ICU 依赖树、Windows xwin 时间、二进制大小；
   - 只在 gate 前做一次 CI-only 的依赖 spike，不在等待期反复烧构建。

## 5. Gate 评审（2027-02-15）

全部满足才启动完整接入：

- [ ] Rust crate ≥0.4 或 1.0，连续 4 周无 breaking API/snapshot 变更。
- [ ] opencode 的 Code Mode 至少经历一个稳定 release 周期，无 P0 回退。
- [ ] 至少 2 个 opencode 之外的项目/产品在生产或公开场景使用 psychoscript。
- [ ] OpenAI 与 xAI 的 custom tool 至少各一家实测通过；其余走 JSON fallback。
- [ ] CI-only 依赖 spike：Linux/Windows 构建通过，Windows release <55 分钟，
      二进制增量 ≤15 MB；否则先解决依赖裁剪/vendor。
- [ ] Rust 1.100.x 工具链升级在本仓 CI 双绿。
- [ ] 本仓 freeform 回放/嵌套权限的 fixture 测试设计已评审通过。

任何一项不满足：顺延到 2027-05-15，继续跟踪；不启动代码。

## 6. Gate 通过后的接入方案

### 6.1 三段式架构

```text
model
  │  custom tool call (raw code) / fallback JSON {code}
  ▼
provider adapter
  │  normalize header + body -> valid JSON args
  │  keep raw/header for replay
  ▼
ToolCall (history) ──► ToolCallResponse (execution)
  │                         │
  │                         ▼
  │                  prepare_tool_call (permissions/hooks/locks)
  │                         │
  │                         ▼
  │                  NestedToolService -> dispatch
  ▼
AssistantItem.freeform_calls (replay side table)
```

### 6.2 接入顺序

| 阶段 | 内容 | 出口条件 |
|---|---|---|
| G1 工具链 | Rust 1.100.x；`rust-toolchain.toml` + CI action 同步 | CI 双绿，release 时间可接受 |
| G2 依赖 spike | `xai-grok-codemode` + `psychoscript`，feature 隔离 | Linux/Windows 构建、体积、依赖树达标 |
| G3 freeform 链路 | 工具声明、接收、归一化、历史回放、流式 | Responses/Chat fixture 全绿 |
| G4 嵌套执行 | `NestedToolService` + read/edit/write/terminal + 权限/hook/锁/取消 | 纵切可用，feature 默认关 |
| G5 狗粮与指标 | 真实会话、账本对照、权限/取消/回放 | 达到第 2 节指标 |
| G6 默认开启 | 支持的 provider 默认 on；其余 fallback | 无 P0/P1 回退 |

### 6.3 关键设计约束

- **freeform 是正式路径**：custom tool 走 raw text；JSON `{code}` 只做 provider fallback。
- **历史回放按当前声明决定 wire 类型**：custom call 配 `custom_tool_call_output`，
  function call 配 `function_call_output`；不能混。
- **raw 侧表**：`AssistantItem.freeform_calls` 按 `call_id` 关联；`ToolCall.arguments`
  始终是归一化后的合法 JSON。
- **嵌套执行走 `NestedToolService`**：它是 `Send + Sync`，不借用 `SessionActor`；
  `InnerDispatch`/`call_raw` 只用于只读 smoke，正式路径必须过权限/hook。
- **provider fallback**：不支持 custom tool 的 provider 用 JSON `{code}`，功能不减，
  只损失代码转义和缓存稳定性。
- **工具范围**：read/edit/write/terminal/web/MCP；排除 `execute` 自身、workflow、
  plan/ask 控制面工具。
- **限制**：timeout、max_tool_calls、max_output_bytes、max_heap_bytes、代码大小；
  取消时 abort 嵌套派发并释放文件锁。

### 6.4 工作量

| 阶段 | 估时 |
|---|---|
| G1 工具链 | 2–3 人日 |
| G2 依赖 spike | 2–3 人日 |
| G3 freeform 链路 | 8–10 人日 |
| G4 嵌套执行 | 12–16 人日 |
| G5 狗粮/指标 | 3–5 人日 |
| G6 默认开启/收尾 | 3–5 人日 |
| **合计** | **30–42 人日** |

这是按完整链路估的；不再给“只读 10–20 天”的缩水数字。

## 7. 风险与停止条件

| 风险 | 触发 | 应对 |
|---|---|---|
| psychoscript API 继续破坏 | 4 周内再次 breaking | 继续等；或 vendor + 钉 commit |
| ICU/idna 双份 | 依赖树出现 2.0+2.4 双版本、体积超预算 | vendor/patch；不行则 feature-gate |
| Windows release 超 55 分钟 | CI 计时 | 裁剪 oxc、feature-gate、单独产物；不默默放宽 timeout |
| provider 拒绝 custom tool | 400/协议错误 | JSON fallback；按 provider 灰名单 |
| 嵌套执行死锁 | 长终端/权限等待 | 服务层 spawn 派发；取消/锁顺序测试 |
| 缓存回退 | cache 命中 -2% | 检查 header/raw 字节稳定性；回退开关 |
| 模型代码质量差 | 失败率 >5% | 诊断回传 + 修复轮；prompt/目录调整 |

**停止条件**：gate 未过、Windows release 超红线、嵌套权限无法保证、
provider custom tool 无法稳定 fallback、或维护成本超过收益。任何一条成立，
不进入默认开启。

## 8. 决策记录

- **2026-11-28**：决定等待，2027-02-15 gate 评审。
- 不做只读/裁剪版；不把 JSON `{code}` 当正式路径。
- 等待期只跟踪，不写 Code Mode 代码、不升级工具链。
- 下次更新：2027-02-15（或 psychoscript 发生重大版本/安全事件时提前）。

## 9. 参考

- `docs-local/plugin-harness-survey-2026-11/README.md`：opencode Code Mode / psychoscript 调研。
- `docs-local/agentic-loop-token-cost-2026-11/README.md`：成本模型与 P4 评估。
- `docs-local/token-economy-port/harness-cost-roadmap-2026-11-20.md`：workload 占比与 P4。
- opencode clone `D:\CODE\ai\opencode`，`v2.4.2` 的 `packages/codemode`。
- psychoscript clone `D:\CODE\ai\psychoscript`，`e8380b6`。
