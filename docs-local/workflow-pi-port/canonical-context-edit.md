# P1 设计原型：canonical context edit（不改写历史的可见上下文编辑）

> 状态：预研原型，未开工、未验证。语义来源：pi `0.87.0` `ContextEditEntry`
> （docs/session-format.md#contexteditentry）与 `context_with_system`
> （per-request 系统消息变换）。fork 基线 `95aad87`。

## 1. 问题

迭代式 workflow（评审→修改→再评审 N 轮）里，编排脚本把上一轮子 agent 的完整
`AgentResult.output` 拼进下一轮 `spawn_agent` 的 prompt，prompt 随轮数线性膨胀；
Rhai 脚本自己只会字符串切片，不会"摘要"（摘要本身要 spawn 一个 agent）。

fork 现有的两条路都不对：

- **不治理**：token 线性涨，长 workflow 跑不深；
- **compaction**（`xai-compaction-transcript`）：重写历史，与 journal 重放相斥——
  journal 记的是原始结果（`engine.rs host_call` 落盘 `seq/kind/hash/value`），
  重放必须命中同一 hash；历史一旦被改写，重放语义整体坍塌。

## 2. pi 的语义（B 级：官方文档转述）

- `ContextEditEntry`：会话文件里追加"编辑条目"，**历史条目物理不动**，模型每轮
  收到的是"全量历史 + 编辑条目"应用后的可见面。编辑是会话历史的合法组成部分，
  可随会话持久化、可按分支继承。
- `context_with_system`：扩展在**每次请求发出前**对 system 消息做变换，同样不落
  历史变更。

关键性质：编辑操作本身是一等公民事件，而不是对历史的原地 mutation。这正好和
本仓库 journal 的 append-only 模型同构。

## 3. fork 移植原型

### 3.1 语义定案（与 pi 对齐点）

**历史（journal）只增不改；可见面 = 原始结果 + 按 seq 顺序应用的编辑序列。**
编辑是 workflow 的 host call（journal 化、可重放），不是脚本内存操作。

### 3.2 落点一：workflow host fn

`engine.rs register_host_fns` 新增：

```rhai
// 把 seq 号为 id 的 spawn_agent 结果在"后续 prompt 可见面"中替换为 digest；
// 原始结果仍在 journal，审计/回放不丢。
replace_visible(id, digest);
// 删除可见面（比如中间产物的工具性输出）。
hide_visible(id);
```

实现走既有 `host_call` 通道（kind: `context_edit`，payload 含目标 seq + 编辑类型 +
digest 哈希），天然获得：journal 记录、重放命中、divergence 检测（脚本编辑序列变了
→ hash 不匹配 → 报错，与 `agent("EDITED prompt")` 行为一致，见 `engine.rs` 现有
divergence 测试）。**不需要新的确定性机制。**

### 3.3 落点二：prompt 构建层应用编辑

`xai-grok-shell/src/session/workflow/host_service.rs` 的 spawn 路径（现
`:490` capability_mode、`:504` agent_type→subagent_type 解析处）：

- `AgentOpts`（`xai-workflow/src/host.rs:5`）新增 `context_digests:
  Vec<(u64, String)>`——脚本不手填，由 host 在记录 `context_edit` 时自动累积，
  spawn 时随 opts 下发（进 payload、进 hash，编辑序列变化自然触发 divergence）；
- prompt 组装时，被 digest 覆盖的 seq 以占位形态呈现（如
  `[agent#3 result: <digest>]`），原始 output 不再进可见面。

### 3.4 落点三（独立小项，可拆）：`context_with_system` 对应物

sampler 请求构建层的 per-request system 变换钩子。与 workflow 解耦，主要服务
主会话；若 P1 范围想收敛，此项挪 P2' 一起评估。

### 3.5 明确不做

- 不动 `xai-chat-state` 的 actor 历史与 compaction——交互主会话的治理走既有
  compaction 通道，本特性只管 workflow 编排面（边界见 §5 待定决策 4）。
- 不做"脚本内自动摘要"（= spawn 摘要 agent + `replace_visible`，是脚本层
  惯用法，文档给 recipe，不加 API）。

## 4. 验收标准

1. 迭代 5 轮的评审 workflow（fan-out 评审 → 修改 → 再评审），第 N 轮子 agent
   prompt 中前 N-1 轮结果以 digest 占位出现，**prompt 总 token 不随轮数线性增长**
   （实测对比曲线留档本目录）。
2. journal 重放：中断后 resume，编辑序列与 spawn 序列重放一致，`agent()` 不重跑；
   脚本编辑序列被修改 → divergence 报错（复用既有 divergence 测试形态）。
3. 审计：journal 中原始 `spawn_agent` value 完整保留，digest 只影响可见面。
4. `xai-workflow` lib 测试全绿（`ctest.sh -p xai-workflow --lib`），
   级联重编面控制在 xai-workflow + xai-grok-shell 两个 crate。

## 5. 待定决策

1. digest 占位的呈现格式（纯文本摘要行 vs 结构化 ref）——影响下游模型引用旧结果
   的方式，等第一个真实 workflow 场景定。
2. `replace_visible` 是否允许对同一 seq 二次编辑（追加式 vs 首次生效）——倾向
   首次生效，重放实现最简单。
3. 编辑是否需要在 TUI workflows 面板可见（phases 展示旁加折叠）——体验项，不阻塞。
4. 交互主会话（非 workflow）是否后续跟进同语义——涉及 chat-state 大改，本专题
   边界外，只登记。

## 6. 风险

- **digest 质量不可控**：digest 由脚本/上游摘要 agent 给出，过损会导致后续轮
  决策退化——靠 recipe（摘要 agent + replace_visible）与验收 1 的曲线观测兜底。
- **hash 链变长**：`context_digests` 进 spawn payload 后，任何早期编辑变化都会
  diverge 后续全部 spawn——这是语义正确的（脚本变了就该 diverge），但要写进
  文档避免被当 bug 报。
