# Agent 软件的记忆模块：工程落地、取舍与演进

> 调研快照 2026-09-25。取数方法与可信度分级见 [README.md](README.md#取数与可信度分级两份文档通用)；
> 社区侧原始引语见 [notes/community-sentiment.md](notes/community-sentiment.md)。
> 本文只描述业界事实与判断，**不含本仓库落地方案**。

## 0. 结论速览

**三条口碑共识**（Reddit / HN / 各家文档三方交叉一致）：

1. **明文可读可改**胜出。表现最好的形态是 markdown 文件（可 `cat`、可 diff、可进 git、可 review），
   而不是托管向量库。HN 上关于 agent 记忆最高分的帖子标题就是《Agent memory as a file format》。
2. **分层优先级**是共识：越具体的层越优先（Claude Code 六层、Cursor `Team > Project > User`、
   `AGENTS.md` 子目录覆盖父目录、Cursor glob 规则）。
3. **索引按需加载**优于全量注入：`MEMORY.md` 式索引 + 主题文件懒加载，比每次全量塞入省数量级的 token。

**唯一真正的分野 —— 失效与真实性**：

| 做法 | 代表 | 代价 |
|---|---|---|
| 引用校验 + 使用即续期 + 不用就删 | Copilot Memory | 实现复杂，必须做 citation 与分支校验 |
| 不过期，靠"用前回查" + 人工审查 | Claude Code | 质量依赖用户自律；社区反馈"几周就积累噪音" |
| 无失效机制、无时间线 | Windsurf / Cursor Memories | 陈旧记忆误导，社区主要吐槽点 |

**核心优化点**（§4）：压注入预算、提提取质量、保真相新鲜、长任务延续、权限与审计、远程调参。

**演进方向**（§5）：记忆 API 化、校验溯源内建、compaction 与 memory 合流、自动记忆层 schema 标准化、
评测化、主动遗忘、组织级知识、模型侧内化。

---

## 1. 先把"记忆"拆成三层

讨论 agent 记忆时最大的误区是把三层混为一谈：

| 层 | 内容 | 代表实现 | 谁写 | 生命周期 |
|---|---|---|---|---|
| **指令层** | 人写的规则与约定 | `CLAUDE.md`、`AGENTS.md`、`.cursor/rules`、`.windsurfrules`、`.clinerules` | 人（可提交 git、可团队共享） | 长期稳定，人工维护 |
| **自动记忆层** | 软件自己沉淀的知识 | Claude Code Auto Memory、Cursor Memories、Cascade Memories、Copilot Memory、Codex `memories/` | 模型 / 软件（人可审、可删） | 随使用增长，需要治理 |
| **会话内上下文管理** | compaction、工具结果落盘、子代理隔离 | 几乎所有 harness | 软件（确定性触发） | 单次会话 |

**关键事实：这三层正在合流。** Claude Code 压缩会话时保留并恢复工作集（compact 后重挂最近 5 个文件）；
OpenClaw 在压缩前专门做一次"静默回合"把状态刷进记忆文件。所以"记忆模块"的边界已不只是那个 memory 目录，
而是"上下文预算 + 持久化"的联合设计。

---

## 2. 七家落地对照

| 产品 | 自动记忆 | 存储与注入 | 作用域 / 共享 | 失效与校验 |
|---|---|---|---|---|
| **Claude Code** | 有（Auto Memory） | `~/.claude/projects/<项目>/memory/`；`MEMORY.md` 为索引，**≤200 行且 ≤25KB**，启动只注入前 200 行，主题文件按需读 | 个人、按项目隔离；指令层另有 6 级（组织策略 → 项目 `CLAUDE.md` → 项目 rules → 用户 `~/.claude/CLAUDE.md` → `CLAUDE.local.md` → Auto Memory），越具体越优先 | **不自动过期**；定位是"线索而非结论"，引用记忆中的文件 / 版本前必须回查当前状态 |
| **Cursor** | 有（Memories） | 对话中生成，落盘为 **User Rules**；规则四类：Always Apply / Apply Intelligently / 按 glob / 手动 `@` | 记忆存个人规则（跟人走，不跟仓库）；项目规则在 `.cursor/rules`（`.mdc`，进 git）；Team Rules 由 dashboard 下发且**优先级最高**（Team → Project → User，冲突时前者胜） | 手动 |
| **opencode** | **无原生**（设计上不做） | 只有 `AGENTS.md`（项目 + `~/.config/opencode/AGENTS.md` 全局）；`instructions` 字段可合并任意 md 甚至远程 URL（5s 超时）；兼容 `CLAUDE.md` 与 `~/.claude/skills/` 作回退 | 项目级（进 git）+ 个人全局 | 无 |
| **Codex CLI** | 有（`~/.codex/memories/`，异步摘要生成） | `AGENTS.md` 是静态层，Memories 是生成层；`/memories` 控制当前会话能否读写记忆 | 个人本地 | 无明确机制（C 级信息） |
| **GitHub Copilot** | 有（Copilot Memory，public preview） | repo-level facts + user-level preferences，**每条带 citation 指回代码**；使用时先对当前分支校验，只采用校验通过的 | repo 级：需写权限用户的操作才产生，全仓库可共享、不可跨仓库；user 级按 billing entity 归属，企业管理员可导出 / 删除 | **未被使用 28 天自动删除**，被成功校验 / 使用可重置计时 |
| **Windsurf Cascade** | 有（自动生成，仅本机） | 自动记忆本地留存；要持久共享需让 Cascade 写进 rules（`.windsurfrules` / global rules） | workspace / 个人 | 无 |
| **Cline** | 有（Memory Bank） | 不是内置引擎，是**方法论**：6 个 markdown 文件（projectbrief / productContext / techContext / systemPatterns / activeContext / progress）+ `update memory-bank` 命令，被社区移植到 `CLAUDE.md`、Copilot 等 | 项目级 | 靠模型自觉更新 |

### 2.1 各家要点（含机制细节）

**Claude Code**（本主题信息量最大的一家，B 级源码拆解）：

- 自动记忆的**闭合四类型**：`user` / `feedback` / `project` / `reference`；写入原则是
  **"只保存无法从当前项目状态推导的信息"** —— 代码模式、架构、文件结构、Git 历史一律不存。
  入库自检问题："删掉这条，Agent 的行为会有实质不同吗？"
- 提取走**后台 fork agent**：共享主对话的系统提示与工具列表（缓存感知），用 `canUseTool` 白名单
  限制只能写记忆目录；**主 Agent 与后台提取互斥**（主写入时后台跳过），避免重复提取。
- 索引双重容量保护：先按行截断（200 行）再按字节截断（25KB），行数限制保"快速浏览"，
  字节限制保上下文预算。
- compaction：触发点约为有效窗口减 13K buffer；摘要走**结构化 9 段**（主要请求 / 技术概念 / 文件 /
  错误与修复 / 问题解决 / 全部用户消息 / 待办 / 当前工作 / 下一步）；压缩后按 token 预算重挂最多 5 个
  最近读过的文件；压缩后主对话里工具结果超限会落盘、只留 2KB 预览（单工具 50K 字符、单消息 200K 字符上限）。
- 开关：`/memory`、`settings.json` 的 `autoMemoryEnabled`、环境变量 `CLAUDE_CODE_DISABLE_AUTO_MEMORY`。

**Cursor**：`Memories` 是对话中生成、落到**个人 User Rules** 上，因此**换机器 / 换队友都不同步**
（社区已知痛点）。规则体系是四类 + 团队级强制，并建议单条规则 <500 行、引用文件而非复制内容（防陈旧）。

**opencode**：明确走"最小内核 + 外挂"路线——不做自动记忆，`AGENTS.md` 里教模型"用 Read 工具按需加载
`@` 引用、不要预先全量加载"。跨会话持久化靠插件 / MCP 补。

**Copilot Memory**：唯一把"记忆会腐烂"当一等问题的产品。citation 校验解决"是否还成立"，
28 天 TTL 解决"没用的记忆占预算"，repo / user 分域 + billing entity 归属解决企业合规。
代价是保守：Copilot code review 刻意**只用 repo facts、不用 user preferences**。

**Windsurf / Cline**：前者是"自动生成 + 本地 + 无失效"，社区主诉是"记了但不用"；
后者是把记忆做成**流程方法论**而非产品能力，每次会话全量读文档，成本高但确定性好。

---

## 3. 七个工程取舍轴

1. **谁决定记什么** —— 人写（opencode 最彻底）/ 全自动提取 / 自动沉淀 + 人可编辑（Claude Code、Cursor）。
   全自动的代价是噪音随记忆量放大（社区反复验证："记忆越多，Agent 越容易被不重要的记忆干扰"）。
2. **怎么注入** —— 全量注入（Memory Bank 式，最贵最稳）/ 索引 + 按需（Claude Code）/ 完全交给模型
   （opencode：提示词里教它懒加载）。这是 token 预算与确定性的直接对赌。
3. **记忆单元形态** —— 一句话事实（消费级做法）/ 结构化 observation / narrative + 依据。
   纯短句会丢掉"为什么"，而工程场景里"为什么"才是决策依据（"选择 PostgreSQL 是因为关系模型"这类
   解释性记忆几乎不会过期，而"认证在 src/auth/handler.ts"会）。
4. **作用域与共享** —— 个人 / 仓库 / 团队三级；跨机器同步是公认痛点（Cursor 记忆落个人规则即典型）。
5. **失效与真实性**（最关键）—— Copilot：校验 + TTL；Claude Code：不过期 + 用前验证 + 人工审查；
   Windsurf：无时间线。反面案例：相对时间不转绝对日期（"下周二上线"），几天后即变误导。
6. **写入防冲突** —— Claude Code 的类型闭合 + 主后台互斥；Cursor 的优先级链（Team > Project > User）
   解决规则打架。
7. **成本结构** —— fork 共享 prompt cache 的后台提取、异步摘要（Codex）、索引双上限（Claude Code）、
   超大工具结果落盘只留预览、子代理上下文隔离（四家一致：Pi / OpenClaw / Claude Code / Letta）。

---

## 4. 当前核心优化点

1. **压注入预算**：索引化、懒加载、工具结果落盘、prompt cache 共享，配上下文占用透明化
   （Claude Code `/context` 显示每部分占多少）。
2. **提提取质量**：闭合类型 + 互斥写入 + "不可推导才记" + 入库测试（删了行为会变吗）。
3. **保真相新鲜**：citation 校验、使用即续期 TTL、用前回查当前状态（Copilot 是当前标杆）。
4. **长任务延续**：compaction 与 memory 合流 —— 结构化摘要模板 + 压缩前 flush + 压缩后恢复工作集。
5. **权限与审计**：个人 / 仓库 / 组织三级 + 企业强制与可导出（Copilot billing entity、
   Cursor Team Rules 的 "Enforce this rule"）。
6. **远程调参**：Claude Code 的超大文件闸门（256KB 字节闸 + 25K token 闸）走服务端 feature flag 调，
   不发新版。

---

## 5. 演进方向（附依据强度）

| # | 判断 | 依据 |
|---|---|---|
| 1 | **记忆从"文档约定"变成"平台 API"**，成为有权限、有审计、可托管的资源 | Anthropic 在 API 层提供 memory tool + context editing（官方口径：单独 context editing +29%，叠加 memory tool +39%，**厂商自测数字**）；Copilot Memory 是产品化同类物；MCP memory server 是第三条路 |
| 2 | **溯源与校验内建**（citation + 分支校验 + TTL） | Copilot 已产品化；社区正在用人力做同一件事（定期审计、可重跑校验） |
| 3 | **compaction 与 memory 合并为统一 context store** | 压缩前 flush、压缩后恢复工作集、9 段结构化摘要模板已成事实标准 |
| 4 | **自动记忆层的 schema 标准化** | 指令层有 `AGENTS.md` 在收敛，自动记忆各家格式互不兼容、跨工具迁移不可能 |
| 5 | **评测化**：评测"记忆诱导的错误率"而非"能否记住" | LongMemEval 被广泛引用但目前主要是卖记忆产品方在引用；社区已出现对多款记忆工具的实测 |
| 6 | **主动遗忘成为一等公民**（时间衰减 + TTL + 冲突消解） | Copilot 28 天 TTL 已是孤例样板；HN 讨论中"按 frontmatter 时间戳清陈旧"是同类诉求 |
| 7 | **从个人记忆走向组织知识** | Copilot repo facts 全员共享、code review 消费记忆，已跑通 |
| 8 | **模型侧内化**：记忆读写成为可训练技能，prompt 层记忆工程被吃掉一部分 | 需长期观察；"模型更强 → 少脚手架"是 Anthropic 公开建议的方向 |

---

## 6. 自研时的决策清单（六问）

1. 记忆单元是短句还是 narrative + 依据？
2. 注入是索引按需还是全量？
3. 作用域如何分个人 / 仓库 / 团队，跨机器同步走哪条路？
4. 过期策略选哪种：TTL / 用即续期 / 不过期 + 强制回查？
5. 写入是主 Agent 自写、后台异步提取还是纯人工？
6. 冲突消解靠优先级链还是类型互斥？

### 被推翻的旧结论

- （空。刷新时在此登记：旧判断 → 新证据 → 修正。）

---

## 附录 A：来源清单

**A 级（官方）**

- GitHub Copilot Memory：`https://docs.github.com/en/copilot/concepts/agents/copilot-memory`
- Cursor Rules（含 Team Rules、AGENTS.md 嵌套、优先级）：`https://cursor.com/docs/context/rules`
- opencode Rules（AGENTS.md、instructions、优先级、Claude Code 兼容）：`https://opencode.ai/docs/rules/`
- Anthropic《Effective context engineering for AI agents》《Effective harnesses for long-running agents》
- Martin Fowler《Context Engineering for Coding Agents》（编辑视角的指令层全景）

**B 级（源码拆解，含数字）**

- Claude Code 记忆系统拆解（六层、闭合四类型、200 行 / 25KB、fork 提取与互斥、无自动过期）
- Arize《Context management in agent harnesses》（Pi / OpenClaw / Claude Code / Letta 四方对照：
  文件读取硬闸、工具结果预算、compaction 触发与保留策略、子代理隔离）

**C 级（二手转述，需复核）**

- Codex CLI `~/.codex/memories/` 异步摘要 + `/memories`（mem0 的分析文与 learn.chatgpt.com 片段）
- Cursor `Memories` 落盘为 User Rules（社区实测 + 中文体验文）
- Windsurf Cascade Memories（官方页面抓取为空，机制由第三方文章与社区帖拼出）

**D 级（厂商营销，仅取事实不取数字）**

- 记忆产品的横向对比文（Mem0 / Letta / Zep / Cognee / Hindsight / EverMind 等）：其性能对比与
  "94.6% vs 49.0%" 一类数字不采信；只用于确认"独立 memory 框架"这一类产品的定位差异。

**E 级（社区）**：见 [notes/community-sentiment.md](notes/community-sentiment.md)。
