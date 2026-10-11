# Agent CLI 特性追踪：八个终端编码 agent 对比研究

> 目的：为 grok-build-proxy（xAI grok CLI 本地 fork）的 agent/TUI 迭代追踪业界设计。
> 关注面（2026-09-19 定调）：**协议接口兼容性、token 经济学、TUI 易用性/便利性、稳定性小 trick**；
> **不做**内置模型目录/厂商 preset。引入评估见 [ADOPTION.md](ADOPTION.md)。

## 分析对象

| 项目 | 仓库 | 本地克隆 | 基准 commit | 血缘 |
|---|---|---|---|---|
| opencode | [anomalyco/opencode](https://github.com/anomalyco/opencode)（原 sst/opencode） | `D:/CODE/ai/opencode`（dev） | `5f9d9187` 2026-09-18 | 自研 TS/Effect，client/server |
| MiMo-Code | [XiaomiMiMo/MiMo-Code](https://github.com/XiaomiMiMo/MiMo-Code) | `D:/CODE/ai/MiMo-Code` | `50cd7139` 2026-09-19 | opencode 深度 fork（小米） |
| qwen-code | [QwenLM/qwen-code](https://github.com/QwenLM/qwen-code) | `D:/CODE/ai/qwen-code` | `85631a3d` 2026-09-14 | gemini-cli 深度 fork（Qwen 官方） |
| minimax-code | [MiniMax-AI/minimax-code](https://github.com/MiniMax-AI/minimax-code) | `D:/CODE/ai/minimax-code` | `30dd6f27` 2026-09-19 | 自研产品层 + vendored pi 生态 |
| kimi-code | [MoonshotAI/kimi-code](https://github.com/MoonshotAI/kimi-code) | `D:/CODE/ai/kimi-code` | `02d829e1` 2026-09-19 | 自研（pi TUI + agent-core-v2/kosong） |
| crush | [charmbracelet/crush](https://github.com/charmbracelet/crush) | `D:/CODE/ai/crush` | `5e3fe4b` 2026-10-03 | Charm 官方自研（Go，fantasy/catwalk） |
| goose | [aaif-goose/goose](https://github.com/aaif-goose/goose)（原 block/goose） | `D:/CODE/ai/goose` | `3bd8520` 2026-10-09 | Block 出品自研（Rust，AAIF 基金会治理） |
| zcode | ZCode（Z.ai/智谱官方） | `D:/CODE/ai/zcode` | `29628c9` 2026-09-24（v3.14.3 快照） | Z.ai 官方自研（TS，Claude Code 同构概念体系） |

> 口碑注记（2026-10-11，用户口径）：opencode 与 MiMo-Code 社区口碑下降，其设计仅作机制参照，不再作为对齐对象。

范围约定：只覆盖 TUI 层与 agent 通用优化；桌面/Web/控制台/IDE 集成不在追踪范围。

## 对比摘要

### 一句话画像

- **opencode**：自研 TS/Effect（client/server + SolidJS TUI）— 前缀缓存策略层教科书。
- **MiMo-Code**：opencode 深度 fork（小米）— 记忆系统 + 自我改进闭环。
- **qwen-code**：gemini-cli 深度 fork（Qwen 官方）— 国产模型适配与显式缓存最全。
- **minimax-code**：自研产品层 + vendored pi 生态 — 流重试与工程化纪律标杆。
- **kimi-code**：自研（pi TUI + agent-core-v2/kosong）— 方言自学习与 token 经济学。

### 七维对比表

| 维度 | opencode | MiMo-Code | qwen-code | minimax-code | kimi-code |
|---|---|---|---|---|---|
| TUI 渲染 | SolidJS@opentui，SSE 16ms 批渲染 | 自有 SolidJS 一套 | Ink+React19 按行高增量提交 | pi-tui fork 布局引擎 | pi-tui 双屏+kitty/IME 处理 |
| 稳定性 | 指数退避 retry.ts | honest failure 终态 | 分类重试，429/配额分治 | **提交边界**流重试 | 退避+配额/限流分型 |
| 上下文压缩 | 阈值+2000 字符截断+锚定增量摘要 | 903 行改造，tail 40k 预算 | 三级阈值梯+微压缩 | 0.9 阈值+SkipReason 细化 | 85% 触发+**交接文档**摘要 |
| Subagent | 前台/后台，task_id 续接 | Cascade resume，best-of-N+judge | Agent Teams+mailbox 协作 | explore/worker/verifier 三角色 | 三来源 spawn，fork 上下文注入 |
| 会话记忆 | JSON 文件存储+snapshot 回滚 | **SQLite FTS5 记忆树**+Dream/Distill | JSONL 租约写+Auto-Memory | session-system 分层+repair | transcript+minidb WAL，无 auto-memory |
| 前缀缓存 | **三断点 cache_control** 策略层 | **byte-equal 不变量** | DashScope 显式断点+cache key | 四桶 token+cacheReadRatio | cache_control+**断裂检测器** |
| 国产适配 | DeepSeek/Kimi 特判 | 每模型族 system prompt | preset 体系+OAuth+reasoning_content 全链路 | models.dev 快照+BYOK | **ReasoningKeyDialect 方言自学习** |

### 分维度最佳

- TUI 渲染 → **qwen-code**：逐帧合并+行高感知增量提交最精细。
- 稳定性 → **minimax-code**：提交边界让重试对用户零感知不重复。
- 上下文压缩 → **kimi-code**：交接文档式摘要保真意图与未决问题。
- Subagent → **qwen-code**：Agent Teams 多代理协作真正可编排。
- 会话记忆 → **MiMo-Code**：FTS5 索引+兼容 Claude Code 记忆格式。
- 前缀缓存 → **MiMo-Code**：byte-equal 不变量把命中变结构性保证。
- 国产适配 → **qwen-code**：preset 目录+OAuth+方言 converter 覆盖最全。

### 总评

要抄架构学 **opencode**（缓存策略层/压缩记账最可移植），要接国产模型学 **qwen-code**（方言与 preset 体系最全），要做 token 经济学学 **kimi-code**（断裂检测+spill 落盘闭环），要打磨 TUI 学 **qwen-code**（Ink 流式渲染体验最佳）；工程纪律另可借鉴 minimax-code 的 BASELINE 供应链管理。

**对本仓库（grok-build）的落地结论不在本文展开，见 [ADOPTION.md](ADOPTION.md)**：P0 = reasoning 方言自学习 + 防 400 套装 + 前缀缓存四件套 + 提交边界重试 + 交接文档摘要 + 工具输出 spill-to-disk；负面清单（Teams/best-of-N/换渲染栈/内置 preset）亦在该文 §5。

## 文档导览

- [ADOPTION.md](ADOPTION.md) — 引入评估（协议兼容性 / token 经济学 / TUI 易用性 / 稳定性 trick，含落点 crate 与优先级）
- [notes/opencode.md](notes/opencode.md) · [notes/mimo-code.md](notes/mimo-code.md) · [notes/qwen-code.md](notes/qwen-code.md) · [notes/minimax-code.md](notes/minimax-code.md) · [notes/kimi-code.md](notes/kimi-code.md) · [notes/crush.md](notes/crush.md) · [notes/goose.md](notes/goose.md) · [notes/zcode.md](notes/zcode.md) — 分仓库机制笔记（含仓库相对路径 file:line 引用）

## 2026-10-11 增补批次（crush / goose / zcode）

三份新笔记的机制对照摘要；引入评估与优先级在 [ADOPTION.md](ADOPTION.md) §7。

### 一句话画像

- **crush**：Charm 出品 Go 自研（client/server + SSE）— LSP 当工具面（8 工具）+ 多客户端共享 workspace + Channels（MCP 反向推送触发回合）。
- **goose**：Block 出品 Rust 自研（AAIF 基金会治理）— 状态机 op 管线 + 安全 inspector 三件套（注入扫描/egress 外泄检测/adversary LLM）+ ACP 三栖（server/client/P2P roaming）。
- **zcode**：Z.ai 官方 TS 自研（Claude Code 同构概念）— dynamic-workflow（模型写 TS + 编译器静态分析 + vm 沙箱 + journal 回放）+ microcompact 工具名白名单 + rewind 四态策略。

### 快速对照（与首批五家互补的维度）

| 维度 | crush | goose | zcode |
|---|---|---|---|
| 执行模型 | coordinator + sessionAgent | 状态机 op 管线（ops_* 可重入） | turn-machine + contracts 端口化 |
| 持久化 | SQLite（sqlc+goose） | SQLite（sqlx WAL，7 类 SessionType） | SQLite（版本化迁移） |
| 压缩 | 双阈值（大窗剩 20k / 小窗剩 20%） | 225k 上限 + 工具对批量摘要 + 大输出落盘 | 双预算 + microcompact 工具名白名单 |
| 权限 | 四元组 PermissionKey + persistent grant | 四档模式 + SmartApprove（LLM 判只读） | build/edit/plan/yolo + alwaysAsk 不可绕过 |
| 扩展 | MCP 三传输 + Hooks（仅 PreToolUse）+ skills 兼容扫描 | Open Plugins hooks + skills + recipe + 恶意软件检查 | 插件商店（CDN sha256）+ hooks 七事件 + workspace hook 信任摘要 |
| 独有 | Channels / crushrc=Bash 配置 / 多客户端 workspace | ACP 反向 provider / handoff memo / sigstore 自更新 / P2P roaming | dynamic-workflow / amend-workflow / formal-proof / 跨生态会话导入 |

## 追踪更新方法

五个克隆都是 `--depth 1` 浅克隆，刷新到最新：

```bash
for r in opencode MiMo-Code qwen-code minimax-code kimi-code; do
  git -C "D:/CODE/ai/$r" fetch --depth 1 origin
  git -C "D:/CODE/ai/$r" fetch --depth 1 origin '+HEAD:refs/remotes/origin/HEAD' 2>/dev/null
done
# 然后逐仓 git -C <path> checkout -B <默认分支> origin/<默认分支>（opencode 默认分支是 dev）
```

更新后重跑"侦察 + 深挖"流程即可刷新本文各节；每次刷新在对象表更新基准 commit。
