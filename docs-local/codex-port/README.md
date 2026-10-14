# Codex CLI（openai/codex）× 本仓库 特性移植预研

> **状态：调研完成，选型待拍板。**
> 调研快照：2026-10-14；codex main `78dc2503`（2026-10-14）；release 窗口
> `rust-v0.150.0`（2026-08-26）→ `rust-v0.162.1`（2026-10-09）stable 全量
> changelog 逐条过筛（alpha 版无 body，方向以 stable 为准）。
> 本地源码 `D:\CODE\ai\codex-src\codex-main`（main tarball 快照，非 git clone——
> 直连 GitHub 超时，经 clash 7897 代理下载 codeload tarball 16.7 MB）。
> fork 基线：main（2026-10-14，v1.0.57 之后）；fork 侧事实全部本机 grep/读码
> 复核（可信度 A），codex 侧 = release notes + 逐文件读码（A⁻），人天估算 = C。

## 对象是什么

Codex CLI = OpenAI 官方终端 coding agent，Rust workspace（`codex-rs/`，5274 个
.rs 文件），发版极频（8 月底以来 stable 0.150→0.162，几乎日更 alpha）。与 fork
同源生态位：ratatui 全屏 TUI + hooks + skills + MCP + 插件市场 + 沙箱。近期
演进的两大主线：① **Windows 一等公民化**（原生沙箱、daemon 共享、控制台窗口
抑制、PowerStore 路径修复）② **会话寿命与多客户端**（app-server-daemon 常驻、
thread recovery、worktree 托管、跨端 resume 冲突处理）。

## TL;DR — 值得移植的三件

1. **异步提问（async questions）**——agent 发出澄清问题后**继续干活**，用户
   答案异步回填（codex `tui/src/bottom_pane/async_questions/` 四件套 +
   `async_question_reply.rs` + app-server 协议 `ToolRequestUserInputParams`）。
   fork `ask_user_question` 是阻断回合的强同步门（`grok_build/ask_user_question/mod.rs:326`
   `run()` 内等答案，fallback 才 fire-and-forget），长任务中途问一句就整轮停摆，
   是体验差距最明显、工程量又可控的一项（工具协议 + pager 问题面板非阻断化 +
   回答回合路由）。
2. **Windows 原生沙箱**——codex `windows-sandbox-rs` + `windows-sandbox-service`
   约 **3.6 万行**：ACL 授权面（`acl.rs`/`allow.rs`/`deny_read_acl.rs` deny-read
   目录树）、elevated 服务进程、DPAPI、ConPTY 集成。fork `xai-grok-sandbox` 只有
   Landlock/Seatbelt（经 nono），Windows 完全裸奔（全 crate grep 无 Windows 实现）；
   本仓主力开发机就是 Windows。重件，可与下条合并立项；近期 release 里 0.158–0.162
   每版都有 Windows 沙箱修复，说明该路径已过孵化期。
3. **MCP 工具级 `output_token_limit`**——per-MCP-server-tool 配置输出 token 上限，
   截断在 conversation-history 层做、跨 resume 一致（`core/src/mcp_tool_call/
   conversation_history.rs`、config `edit/document_helpers.rs:214`）。fork 只有
   全局实验性工具输出压缩（六期），per-tool 配置是小而实用的增量。

## 真缺口候选（按 价值 × 国模/fork 适配 排序）

| # | 特性 | codex 锚点 | fork 现状 | 规模 |
|---|---|---|---|---|
| 1 | 异步提问（不阻断回合） | `tui/src/bottom_pane/async_questions/`、协议 `ToolRequestUserInputParams.json` | `ask_user_question` 阻断 `run()`；fallback fire-and-forget 有但非主路径 | 3–5 人天 |
| 2 | Windows 原生沙箱 | `windows-sandbox-rs`（ACL/deny-read/elevated service/DPAPI）3.6 万行 | 仅 Landlock/Seatbelt；Windows 无沙箱 | 重件 15+ 人天（可裁剪：先 deny-read ACL 面） |
| 3 | MCP 工具级 output_token_limit | `conversation_history.rs` + config edit | 全局输出压缩（六期），无 per-tool 上限 | 1–2 人天 |
| 4 | PostToolUse hook 增强：`should_block` + `additional_contexts` + `feedback_message` | `hooks/src/events/post_tool_use.rs:38-43` | fork hooks post_tool_use = 验证 gate，fail-open，无附加上下文注入/反馈通道 | 2–3 人天 |
| 5 | `/usage` 分析面板（活动图表 + 按模型/plugin/skill 分解） | `tui/src/analytics/`（约 40 文件） | fork /usage=额度、/stats=窗口账本，无活动分解 | 4–6 人天（可裁剪） |
| 6 | 审批 UX 组合：新输入到达时审批评审重试不 abort；审批证据跨 compaction 保留；未答问题回填 composer | 0.155/0.157/0.158 release notes | fork 待核（§盲区） | 2–4 人天 |
| 7 | rate-limit banner 带动作入口 | 0.152 notes | credit_bar 有警告线（`views/credit_bar.rs`）无跳转动作 | ≤1 人天 |
| 8 | `/mcp login <name>` 会话内手动重登 | 0.161 notes；`McpServerOauthLoginParams` 协议 | fork MCP OAuth 有 401 重试 + 凭据存储，无 slash 手动入口（`mcps.rs` grep 0 命中） | ≤1 人天 |
| 9 | daemon 常驻后台 server（自动更新、thread recovery、跨客户端共享） | `app-server-daemon/` 1.15 万行 | fork workspace-daemon 只服务 workspace RPC，无会话 daemon | 架构级大件，暂缓 |
| 10 | 沙箱网络策略代理（跨 redirect/WebSocket 执行网络限制） | `network-proxy/`（MITM 代理 + domain matcher + 凭据 broker） | fork 沙箱无网络层 | 重件，与 #2 同域 |
| 11 | compaction 图片计入预算并裁旧图 | 0.150.1 backport #41003 | 视觉桥未落地，暂缓 | 待视觉桥后 1 人天 |

## 判定「已有」清单（防重复建设，已逐条 grep 复核）

| codex 特性 | fork 锚点 |
|---|---|
| `/copy` 转录复制（0.154/0.158 Markdown 保真） | `pager/src/slash/commands/copy.rs:18` |
| `/fork` 会话分叉保草稿（0.157 快捷键 f） | `slash/commands/fork.rs:75`（分叉 + worktree 分叉均已接线） |
| vim 编辑模式 undo/redo/搜索（0.152/0.153/0.154） | `slash/commands/vim_mode.rs` |
| worktree 隔离会话 + 面板管理（0.154→0.162 默认开） | dashboard `dispatch_dashboard_toggle_worktree` / `dispatch_new_worktree_session`（`app/dispatch/router.rs:15-73`）+ `xai-fast-worktree` CoW clone（比 codex 普通 git worktree 更快） |
| 全屏 TUI / 转录搜索 / 鼠标选择复制（0.156/0.157） | ratatui 原生全屏 + `find.rs` / `jump.rs` |
| 主题扩容（0.156 六套） | 36 期已移植 11 套 opencode 主题（17 套总量） |
| mermaid 流式渲染（0.158/0.159 增强） | `xai-grok-mermaid` |
| steer 类 `instant_interrupt`（0.159） | `xai-interjection-core`（Ctrl-S 机制层） |
| 流失败保留已流答案/plans（0.156） | 二十一期流恢复注入（注入式，题面相邻覆盖） |
| 插件市场 CLI 管理（0.153） | `slash/commands/plugin.rs` + `xai-grok-plugin-marketplace` |
| 会话全文搜索（0.156 transcript search） | `xai-grok-session-search` FTS5 |
| 模型目录缓存与陈旧条目清理（0.160） | 三十二期裸 id/缓存修复（`~/.grok/cache/prices.cache`） |
| 队列消息重连恢复（0.160） | `xai-prompt-queue` |
| reasoning summary 状态行展示（0.155） | 状态行 item 扩展（二期/五期）；细节待活体核对 |
| skills/hooks 热升级（0.154） | `skills_watcher` 特性 |

## 判定「不移植」清单

| 特性 | 理由 |
|---|---|
| Guardian 审查层（ approval 独立安全评审服务） | OpenAI 账号绑定服务；其「审批证据保留 + 重试」思想并入候选 #6 |
| cloud-tasks / cloud 客户端 | OpenAI 云服务绑定 |
| Daybreak / Cyber 路由（0.161） | 产品侧模型路由，无对应面 |
| 全双工语音对话（0.155–0.161 持续加码） | fork voice = 听写 only（`xai-grok-voice/src/lib.rs:3`）；duplex 语音含 webrtc/opus 运行时打包，重且价值低 |
| code-mode（QuickJS 调工具，0.162 还在加 streaming helpers） | workflow-pi-port D1 已判死（双运行时破坏 journal 确定性） |
| Bedrock/多云模型目录、GPT-6 系列迁移提示 | fork BYOK 国模场景，目录走 model-catalog 双源 |
| agent-graph-store / agent-roles / message-board | Codex Desktop 多 agent 协同产品面，fork 无对应产品形态 |
| Touch ID / user-verification（0.155） | macOS 专属；Windows Hello 等价物可并入 #2 顺手项 |

## 与既有专题的关系

- **kimicode/step-code-port**：无重叠（本调研窗口内 codex 无分支树/命令分析新动作）。
- **workflow-pi-port**：code-mode 判定一致（D1 维持）；codex 0.162 给 code-mode
  加 streaming promise helpers + ranked tool search，与 P2 deferred-tool-exposure
  的「工具面收敛」同题不同法（codex 走 code-mode 运行时，fork 走 allowlist），不影响。
- **视觉桥（vision-bridge）**：候选 #11（compaction 图片预算）是其下游小件。
- **lsp-port**：codex 本窗口无 LSP 动作。

## 已知盲区（未核实 ≠ 没有，开工前先核）

- fork 审批流程对「新用户输入到达」的现有行为（候选 #6 前置核实）。
- 状态行是否已展示 reasoning summary 实时内容（仅确认了 item 扩展历史）。
- fork `ask_user_question` 的 `fallback_fire_and_forget`（`mod.rs:245`）实际触发
  条件——若 headless 已走该路径，候选 #1 的实做面收窄为 TUI 非阻断面板。

## 来源

- GitHub API releases 0.150.0–0.162.1 stable bodies（2026-10-14 拉取，/tmp 快照）
- 源码 `D:\CODE\ai\codex-src\codex-main`（main `78dc2503`，2026-10-14 tarball）
- fork 侧锚点：本机 grep（2026-10-14，v1.0.57 后工作区）
