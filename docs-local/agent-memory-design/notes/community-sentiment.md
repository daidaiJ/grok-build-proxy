# 社区风评：Reddit 与 Hacker News

> 主报告见 [../memory-design-survey.md](../memory-design-survey.md)。本文只做一件事：
> 记录两个社区对 **agent 软件记忆模块**的真实评价（原话为主），并列出对主报告结论的修正。
> 快照 2026-09-25。

## 0. 取数方法与局限

- **HN**：官方 Algolia API。评论接口不返回投票数，只有主帖有 points / num_comments，
  因此 HN 部分用"主帖热度 + 引语"呈现，不做评论排序。
- **Reddit**：主站、`old.reddit.com`、redlib 镜像、`r.jina.ai` 代理在本机环境全部连不通
  （DNS 解析失败 / 连接超时 / 403），改走两个 Reddit 归档 API：**PullPush** 与 **Arctic-Shift**。
  归档相当于历史快照，**score 是归档时刻的值**，仅作相对比较；部分帖评论覆盖不全（如 r/Codeium 帖只取到 13 条）。
- 样本自选，不是统计抽样。以下引语保留原文，不修饰。

---

## 1. Reddit

### 1.1 r/ClaudeAI：Auto Memory 上线后（争议最集中，赞数即立场分布）

| 赞 | 立场 | 原话要点 |
|---|---|---|
| 47 | 反对隐式 | "我偏好显式（CLAUDE.md 优于自动维护的 MEMORY.md）。我不想让某句随口说的话变成法律。" |
| 39 | 支持 | 记忆 + rules 配合，从建分支到合并 PR 的整套流程"近乎完美地被遵循" |
| 22 | 实证收益 | zod v4 弃用 `z.string().uuid()`、shadcn Select 不接受空字符串——踩一次就长期记住，不用反复交代 |
| 15 | 强烈反对 | "极其糟糕。它会记住某个 skill 或命令，导致以后的 agent 随机调用你根本不想要的 skill……**无监督的随机记忆是糟糕的设计**"；且"我完全不知道这功能被加上了" |
| 12 | 运维现实 | 必须定期修剪 `MEMORY.md`：几周就积累噪音（如"用户偏好终端深色模式"这类一次性偏好），手动清理 10 分钟后质量明显回升 |
| 3 | 一致性风险 | 同仓库并行跑 5 个 agent 时，`CLAUDE.md` 是唯一能保证一致的东西；auto memory 会让各 agent 各学一套、逐渐发散 |
| 3 | 社死案例 | 屏幕共享给客户演示时，被 auto memory 引用了另一个项目目录的内容，"当场编理由圆过去，立刻关掉" |
| 3 | worktree 陷阱 | git worktree 下不生成 auto memory（另有专帖 `1r22ahd` 吐槽） |
| 3 | 机制对照 | Windsurf 老用户：**"memories 是智能的，可以有几千条、只在需要时加载；rules 文件是全量塞进上下文的"** |
| 3 | 特性本质 | "如果 Claude 做了某个动作，它会记住这个动作，下次自动做。有人爱它因为正循环复利，有人恨它因为负循环也复利。" |

补充两条技术性讨论：

- 有用户引用 arXiv 2507.11538《How Many Instructions Can LLMs Follow at Once?》，提出
  **"Claude Code 单轮大约能遵守 O(200) 条指令，system prompt 已占约 50 条，你实际剩 150 条预算"**，
  因此建议把细节写成"一行引用 + 按需打开文件"（该数字来自社区转述，未独立验证）。
- 记忆持久化位置：`~/.claude/projects/<path>/memory/MEMORY.md`，"所有 agent 共享"。

### 1.2 r/Codeium（Windsurf）

- 核心抱怨不是"记不住"，而是**记了不用**：高赞吐槽 "My windsurf has dementia. It forgets the rules
  a few lines in."（我的 Windsurf 得了痴呆，读几行就忘了规则）；以及"既然规则存在就应该被遵循"。
- 另有对 Memories 是否 enterprise-only 的混乱，官方（varunkmohan）出面认领问题称会改进。

### 1.3 r/CLine

- 社区把分工讲得比官方还清楚：**Cline Rules = 指令**（跨项目、可逐个开关），
  **Memory Bank = 项目知识**（每项目一套的结构化文档，"你在建什么、为什么"）。
- 有人猜测"rules 每条消息都发，memory 只在会话开头发"（未证实）。
- 一条与演进方向吻合的建议：**"我把记忆做成一个 Skill，需要时才调用，而不是每次 prompt 都带上，省 context。"**

### 1.4 r/codex

- 反复出现 "codex amnesia"；主流建议是**"用 MCP 做后端，跨客户端不锁定"**。
- 有人质疑"Codex 和 Claude Code 都有了，还做记忆项目干什么"。
- 对 Auto Memory 的实质批评：**只存摘要、不存原始历史，细节照样丢**；且"Claude 决定存什么"。

### 1.5 r/cursor（Memory Bank）

- Memory Bank "确实比裸用强"，但"过一阵 agent 就开始跳过 memory bank、懒得更新"；
  在 Cursor 里成本翻倍；也有不少人反馈体系复杂到"不知道发生了什么、为什么坏"。

---

## 2. Hacker News

### 2.1 主线：《Agent memory as a file format》（191 分 / 96 评论）

HN 上关于 agent 记忆最高分的讨论，作者主张 **markdown + 简单 embedding 的"记忆字段"格式**。
按投票，HN 的审美是**明文文件 + 语义检索 + 可导出可迁移**，而非托管记忆服务。代表性反应：

- "Agent memory 之于 computer memory，就像 MongoDB 之于关系数据库。眼看着事情转了一圈又回来。"
- "半个事实：这是一大堆文字在说'markdown + RAG'。"（对方案新颖性的质疑）
- "一线实践者的反驳：项目记忆通常小到能整个进上下文，grep 就够；**真正的难点是记忆怎么被创建、
  更新和治理**。"
- "我不觉得我需要更多 agent 记忆了……我喜欢它是可查看、可搜索的、而不是隐形的。"

### 2.2 最集中的批评：记忆投毒（poisoning）

- "只要一行被污染的文字，就会负面影响下游的一切……**任何可以从代码推导出来的信息都是噪音**，只会伤害 agent。"
- 引 Steve Yegge 的 "heresies" 概念：**已停止使用自管理记忆系统，因为异端条目更难诊断和清除**；
  改用 path-scoped rules + 临时目录。
- "Claude 喜欢把它'发现'的东西写进注释，之后就把注释当福音真理 —— **你得不断打理花园、拔草**。"
- "记忆容易被投毒。"（"Memory is prone to poisoning."）
- "它很有用，直到随时间累积到一定规模 —— 那时因为陈旧或跨语境误用而变得无用。"
- 对 RAG 式记忆的直接否定：**"这个方案的阿喀琉斯之踵就是 RAG……没有什么能胜过人工筛选的数据，
  记忆必须定期 review、压实、清理。"** 有人补低成本办法：用 frontmatter 的 created / updated
  按陈旧度排序清理（动机与 Copilot 的 28 天 TTL 完全一致）。
- 一位大厂内使用者："很多人过度依赖 Claude 往 MEMORY.md 写指令……**我好几次发现 agent 变'神经质'，
  就是 MEMORY.md 里写错了、或与 CLAUDE.md 直接矛盾**。改完立刻恢复。我现在有个 skill 定期审计
  MEMORY.md 和 CLAUDE.md。"（这条与 Copilot 的"引用校验"是同一问题的两种解法）

### 2.3 其它值得记的反应

- **反对把提示词当强约束**（Agent Skills 帖）："Snake oil……**老虎机随时能吐掉你在 AGENTS.md、
  memory.md 里明确写下的硬性要求**。这些 harness 假装 LLM 是严格遵守规则的，问题只是规则没写清楚 ——
  这是对 LLM 工作方式的根本误判。"（结论：硬约束只能靠沙箱 + 人工复核）
- **不把上下文窗口当记忆**："上下文窗口缺陷太大，我不会认为那是记忆。它更像关于当前情况的笔记，
  而不是'在记忆里'。"
- **官方 API 化的担忧**（Anthropic context editing / memory tool 帖）："本质是把常见模式形式化……
  **通往服务端托管消息历史的路，也就是更强的厂商锁定**。"另一条很准："CLAUDE.md 永远在你的活动上下文里，
  而新的 memory 本质上是本地 RAG。"
- **评测缺口**："这篇文章里全是断言，没有任何证据……**搭建正经 eval 的一部分，是先说清我们到底在优化什么**。"
- **一处社区实测**（Show HN 评论区）：对 6 个记忆工具在 19K 会话规模下做 cold start 召回，最好的也只有
  18/100 hit@1；同一套 BM25 在标准 500 会话集上约 85%。结论是检索式记忆在真实规模下很虚。
- **AGENTS.md 之争（2026-09）**：Claude Code 把"读 `AGENTS.md`"与遥测绑定，HN 反弹强烈
  （"别远程关掉我的 AGENTS.md"）。争论暴露两个工程问题：`CLAUDE.md` 与 `AGENTS.md` 并存时的
  **陈旧重复**，以及 `@` 引用可能触发的**递归循环**；有人提议给指令加命名空间
  （`Model.Claude.Opus.4.8:` / `Harness.ClaudeCode:`）。
- **token 预算实践**：有作者把全局 + 本地的 AGENTS.md / Skills 各控制在 5000 token 以内；
  另一极端是"两个很小的 AGENTS.md，初始上下文常年 4k 以内"。
- **一手替代方案**：ADR 式目录（`docs/decisions/`，含 "rejected solutions"、"acceptable risks"、
  稳定 ID 如 `R1`/`I3`）、交接文档 + 压缩校验（把"可重跑的验证"嵌进 markdown，逼下一轮会话先验证再交接）、
  自建 sqlite / Supabase 每周审计、多层 git 仓库 + wiki 链接。

---

## 3. 两社区的共识与分歧

**共识**（与主报告一致，但更悲观）：

1. 明文可读可改是唯一被同时认可的形态；托管 / 黑箱记忆在被质疑。
2. 自动记忆的核心风险是**投毒与陈旧**，不是容量 —— 负循环同样复利。
3. **必须有人工审计环节**：Reddit 是"每几天修剪 MEMORY.md"，HN 是"定期 review / 压实 / 清理"，
   并已出现工具化尝试。这正是 Copilot Memory 用机制替代人力在做的事。

**分歧**：

| 议题 | Reddit 侧 | HN 侧 |
|---|---|---|
| 显式 vs 隐式 | 要可 review 的规则文件，反对随口一句变成法律；但相当一部分人认为自动记忆净收益为正（前提是愿意审） | 更偏显式，且不少人**干脆不用**（"怕无关内容注入随机会话"） |
| 规则 vs 记忆谁更省 | rules 全量加载太贵，memories 按需加载更聪明 | 记忆文件最终会膨胀到需要索引与检索，于是又回到 RAG 的老问题 |
| 文件 vs 检索 | 关心"治理流程"：记忆怎么被创建、更新、清理 | 高赞主张 markdown + 简单 embedding；一线反驳"grep 足够，难点在治理" |

---

## 4. 对主报告结论的修正

- **强化**：主报告 §0 的"明文 + 分层 + 索引按需"三条共识被两社区证实。
- **新增**：当前产品化的自动记忆（Claude Code Auto Memory、Cursor Memories、Cascade Memories）
  在两个社区都处于"有人爱、有人恨、恨的理由高度一致"的状态；恨点集中在
  **无监督写入、不可审计、跨 worktree / 跨机器失效**。
- **不变**：唯一被普遍认为"设计上正确"的仍是 Copilot Memory 的引用校验 + TTL 路线，
  但它目前是 public preview，且刻意保守到 code review 不消费个人偏好。

---

## 附录：可复跑的取数路径

**HN（Algolia，稳定可用）**

```text
# 评论检索（注意用精准词，'memory bank' 会被硬件内存污染）
https://hn.algolia.com/api/v1/search?query=MEMORY.md&tags=comment&hitsPerPage=40
https://hn.algolia.com/api/v1/search?query=auto%20memory&tags=comment&hitsPerPage=40
https://hn.algolia.com/api/v1/search?query=AGENTS.md&tags=comment&hitsPerPage=40
https://hn.algolia.com/api/v1/search?query=context%20engineering&tags=comment&hitsPerPage=40
# 主帖列表（带 points / num_comments，可看热度）
https://hn.algolia.com/api/v1/search?query=agent%20memory&tags=story&hitsPerPage=30
# 单帖评论树（本文用的是 45479006 = Anthropic context management；49508317 = Agent memory as a file format）
https://hn.algolia.com/api/v1/items/<objectID>
```

**Reddit（归档 API；主站直连在本机不可用）**

```text
# PullPush：按帖取评论（偶发 429，需间隔）
https://api.pullpush.io/reddit/search/comment/?link_id=<post_id>&size=60
# Arctic-Shift：同一帖的等价接口（本文覆核了两者的结果一致性）
https://arctic-shift.photon-reddit.com/api/comments/search?link_id=<post_id>&limit=100
# 注意：/api/posts/search 直接调用返回 400，帖级搜索未跑通；submission 搜索只成功过零星几次
```

**本文涉及的帖 ID**

| 平台 | 帖 ID | 主题 |
|---|---|---|
| Reddit r/ClaudeAI | `1r6j36u` | "Claude Code's Auto Memory is so good"（争议主帖） |
| Reddit r/ClaudeAI | `1r22ahd` | auto memory 在 git worktree 下失效 |
| Reddit r/Codeium | `1i5vqz7` | "Rules are only good if followed, Memories…" |
| Reddit r/CLine | `1qsznk6` | Cline Rules vs Memory Bank |
| Reddit r/codex | `1rmhtmr` | "Memory for Codex" |
| Reddit r/cursor | `1jkfp7m` | "Memory Bank for Cursor" |
| HN | `45479006` | Managing context on the Claude Developer Platform |
| HN | `49508317` | Agent memory as a file format |

> 抓取缓存落在本机 `D:\Programs\websearch\fetchdata\`（websearch MCP 的输出目录），
> 文件名带时间戳，未纳入仓库。
