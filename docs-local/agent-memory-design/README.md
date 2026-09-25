# Agent 软件的记忆模块设计（专题调研）

> 目的：跟踪业界 agent 软件（Claude Code、Cursor、opencode、Codex CLI、GitHub Copilot、
> Windsurf Cascade、Cline）的**记忆模块**在工程上如何落地、有哪些取舍、当前优化点与演进方向；
> 为 grok-build-proxy（xAI grok CLI 本地 fork）的记忆 / 上下文类迭代提供参照。
> 状态：**调研快照 2026-09-25**，纯文档，无代码改动。

## 本目录用途

- 收纳"agent 软件的记忆模块"这一主题的全部调研产出：主报告 + 社区风评笔记。
- **范围边界**：只讨论 **agent 软件系统内部**的记忆模块，即三层——指令层（人写的规则文件）、
  自动记忆层（软件自己沉淀）、会话内上下文管理（compaction / 工具结果落盘 / 子代理隔离）。
  **不**做独立 memory 框架（Mem0、Zep、Cognee、Supermemory 等）的选型对比；Letta 仅作为
  "记忆即运行时"的对照样本出现。
- **产出定位**：服务本仓库记忆 / 上下文相关设计的决策与"抄作业对象"筛选。**落地方案不在本目录**，
  需要引入评估时另立文档（同类先例：`../agent-cli-tracking/ADOPTION.md`）。
- **时效**：2026 年 agent 记忆仍是活跃战区，产品文档与实现月月变。引用处尽量落到机制与锚点，
  刷新按下面"维护方式"重跑。

## 文档导览

- [memory-design-survey.md](memory-design-survey.md) — 主报告：三层拆解、七家落地对照、
  七个取舍轴、核心优化点、演进方向、自研决策清单、来源与可信度分级。
- [notes/community-sentiment.md](notes/community-sentiment.md) — 社区风评：Reddit（PullPush /
  Arctic-Shift 归档）与 Hacker News（Algolia API）的原始引语、两社区共识与分歧，
  以及对主报告结论的修正点。

## 一句话结论

口碑好的记忆设计 = **明文可读可改（文件而非黑箱向量库）+ 分层优先级（越具体越优先）+
索引按需加载（不全量注入）**。真正的分水岭只有一条：**记忆过期后怎么不骗人** ——
目前只有 GitHub Copilot Memory 把"引用校验 + 使用即续期 + 不用就删"做成了产品机制，
其余各家基本靠用户手工维护。详见主报告 §0。

## 取数与可信度分级（两份文档通用）

| 级别 | 含义 | 例 |
|---|---|---|
| A 官方文档 | 产品方文档 / 官方工程博客 | GitHub Copilot Memory 文档、Cursor Rules 文档、opencode AGENTS.md 文档、Anthropic 上下文工程与 harness 博客 |
| B 源码拆解 | 第三方对实现的系统拆解（带数字与锚点，但非官方） | Claude Code 六层记忆结构、`MEMORY.md` 200 行 / 25KB 双上限、fork 后台提取与互斥写、compaction 9 段摘要 |
| C 二手转述 | 单篇二次分析 | Codex CLI `~/.codex/memories/` 异步摘要；Cursor Memories 落盘为 User Rules |
| D 厂商营销 | 卖记忆产品方的对比文，性能数字不可采信 | "Hindsight 94.6% vs Mem0 49.0%（LongMemEval）"一类口径 |
| E 社区 | Reddit / HN 用户实测与吐槽（样本自选，非统计） | 见 [notes/community-sentiment.md](notes/community-sentiment.md) |

## 维护方式

1. **刷新入口**：主报告 §2 的七家对照表逐行核对产品文档；§5 的演进判断逐条看是否已被实现或证伪。
2. **社区风评刷新**：`notes/community-sentiment.md` 附录给了可复跑的 API URL 形式。
   Reddit 主站在部分网络环境下无法直连（DNS/超时），走 PullPush / Arctic-Shift 归档 API 取评论。
3. 每次刷新更新本文件"调研快照"日期，并在主报告 §6"被推翻的旧结论"里记一笔。

## 已知缺口（未做 / 待补）

- 本仓库（grok-build fork）自身记忆模块现状**未核实**，本目录不含落地结论。
- Codex CLI Memories 未见官方文档，机制为 C 级；Cursor Memories 官方文档页当前重定向到 Rules 页。
- GitHub Copilot Memory 为 public preview，机制会变；企业 / 多租户记忆（TencentDB Agent Memory 等）未纳入。
- 记忆评测基准（LongMemEval 等）未做独立交叉验证，仅记录厂商口径与一处社区实测（见风评笔记 §2）。
