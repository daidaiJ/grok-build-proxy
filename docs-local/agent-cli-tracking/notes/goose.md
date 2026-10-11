# goose 研究笔记

> 仓库: https://github.com/aaif-goose/goose（block/goose，已移交 Linux Foundation 旗下 AAIF 治理）· 本地克隆: `D:/CODE/ai/goose`
> 语言栈: Rust（Cargo workspace 13 crate）· 血缘: Block（Square）出品，"not just for code" 通用 agent，无 fork 痕迹
> 分析基准 commit: `3bd8520`（2026-10-09，浅克隆单快照，PR #12789）· 分析日期: 2026-10-11
> 范围: 特性设计面（不逐文件流水账）；行号均实际读过源码

## 定位与血缘

Block 出品的通用开源 AI agent，形态 = 桌面应用（Electron）+ CLI + API + SDK 四合一，Rust 核心。已移交 AAIF（Agentic AI Foundation）基金会治理。扩展生态基于 MCP 开放标准；LLM 接入面极宽（40+ provider），特色是可把其他 coding agent（Claude Code/Codex 等）经 ACP 当模型用。旧数据目录策略仍保留 `top_level_domain: "Block"` 痕迹（`crates/goose-mcp/src/lib.rs:12`）。

## 架构速览

- **Cargo workspace 13 crate**：`goose`（核心 340+ rs）、`goose-providers`/`goose-provider-types`（LLM 层）、`goose-context-management`（压缩）、`goose-mcp`（内建 MCP 服务）、`goose-local-inference`（llama.cpp/MLX）、`goose-roaming`（P2P）、`goose-sdk`（UniFFI）、`goose-cli`、`goose-test`。
- **核心执行模型 = 状态机 op 管线**：`agents/state_machine/` 把一轮回复拆成有序可重入 op（ops_llm/ops_toolcalling/ops_compaction/ops_steer/ops_tool_approval/ops_maxturns/ops_retry/ops_slash_command/ops_recipe…），操作作用于持久化会话（`crates/goose/src/agents/state_machine/mod.rs`）。
- **工具面 = 三源统一 MCP 抽象**：外部进程扩展（rmcp）、平台内建扩展（in-process 实现 `McpClientTrait`）、builtin registry（duplex 通道，`src/builtin_extension.rs`）。
- **会话持久化 = SQLite**（sqlx + WAL + `schema_version` 迁移，`src/session/session_manager.rs`，5271 行）。

## 特性清单

### 会话 / 持久化
- **SQLite 会话库 + 七类 SessionType** — User/Scheduled/SubAgent/Hidden/Terminal/Gateway/Acp 一库统管所有来源（`session_manager.rs:52`）；跨会话全文检索（`chat_history_search.rs`）。
- **回合上下文注入（moim）** — 每回合给请求注入 `<turn-context>` 块：当前时间、cwd、compaction 状态、**turn 预算**（提示模型预算见底要收敛），仅上下文 ≥32k 的会话启用（`src/agents/moim.rs:6,37`）。
- **会话命名/导出/导入** — 前 3 条消息 LLM 起名并剥 XML 标签（`src/session/session_naming.rs`）；HTML/Markdown 导出（`export_html/`、`export_markdown.rs`）+ 多格式导入（`import_formats/`）。
- **chatrecall 扩展** — 语义/关键词召回旧会话历史（`src/agents/platform_extensions/chatrecall.rs`，feature 门控）。

### 上下文管理
- **双阈值自动压缩** — `DEFAULT_AUTO_COMPACT_TOKEN_LIMIT = 225_000`（刻意压在 OpenAI 272k 长上下文价格档下方）× 可配 threshold 取 min（`crates/goose/src/context_mgmt/mod.rs:27`）。
- **结构化压缩独立 crate** — `goose-context-management`（structured/summarize/templates）。
- **工具对压缩** — 工具调用/响应对批量为 10 做摘要（`GOOSE_TOOL_PAIR_SUMMARIZATION` 开关）。
- **大工具输出落盘** — >200k 字符的工具响应写临时文件、替换为文件引用（`src/agents/large_response_handler.rs:5`）。
- **token 计数** — tiktoken CoreBPE + LRU 缓存（`src/token_counter.rs`）；models.dev 远端模型目录异步刷新 + 本地缓存（`src/model_catalog.rs`）。
- **hints** — 启动加载 `AGENTS.md`/`GOOSE_HINTS` + 子目录 hint 追踪（`src/hints/`）。

### LLM 协议兼容 / provider
- **声明式 provider** — 30+ 开源网关一 JSON 一 provider，`include_dir!` 编译进二进制（`crates/goose-providers/src/declarative.rs`）；anthropic/openai/google/ollama/openrouter/bedrock 等硬编码 provider。
- **ACP 反向 provider（独有）** — 把 Claude Code/Codex/Copilot/Cursor CLI/amp/pi 等外部 agent 包成 Provider（`crates/goose/src/providers/claude_acp.rs`、`codex_acp.rs` 等），即「用订阅套壳别的 agent」。
- **OAuth 订阅通道** — Claude/ChatGPT/Gemini 订阅登录（`gemini_oauth.rs`、`chatgpt_codex.rs` 等）。
- **端侧推理** — llama.cpp + MLX + CUDA/Vulkan，含聊天模板、无工具调用模型的 toolshim 仿真（`crates/goose-local-inference/`）。
- **重试/steer/空响应处理** — 状态机内独立 op（`ops_retry.rs`/`ops_steer.rs`/`ops_empty_response.rs`）。
- **语音** — WebRTC 实时语音会话（`src/live_voice/`）+ 本地 whisper 听写（`src/dictation/whisper.rs`）。

### 子 agent / 编排
- **summon + delegate** — 平台扩展把 recipe/agent 当子代理派发，`delegate` 工具可覆写 provider/model/max_turns/extensions/working_dir（`src/agents/platform_extensions/summon.rs:32,499`）；子代理是持久化 `SessionType::SubAgent` 会话，前台子代理可从会话恢复。
- **后台子代理通知** — `subagent_execution_tool/notification_events.rs` + CLI 任务展示。
- **checks / goose review** — `.agents/checks/*.md` 与 `.agents/REVIEW.md` 定义审查项，每项由 check 子代理执行（默认 25 turns）（`src/checks/mod.rs`、`goose-cli/src/commands/review/`）。
- **平台内建扩展族** — todo/summarize/scheduler/ext_manager/analyze（tree-sitter 调用图）/code-mode/apps（`src/agents/platform_extensions/mod.rs:31`）。
- **cron 调度** — tokio-cron-scheduler 定时跑 recipe（`src/scheduler/full.rs`）；agent 自己也有 schedule 管理工具（`src/agents/schedule_tool.rs`）。

### 工具体系 / 审批 / 安全
- **四档审批模式** — Auto/Approve/**SmartApprove**/Chat（`crates/goose-provider-types/src/goose_mode.rs:22`）。
- **LLM 权限判定** — SmartApprove 用模型判定「只读」工具免批（虚拟工具 `platform__tool_by_tool_permission`，`src/permission/permission_judge.rs:41`）。
- **Inspector 管线** — 每次工具调用过 ToolInspector 链，产出 Allow/Deny/RequireApproval + confidence + finding_id（`src/tool_inspection.rs:11`）。
- **安全三件套（独有）** — 注入扫描 `PromptInjectionScanner`、**egress 外泄检测**（工具参数外发敏感数据拦截，`src/security/egress_inspector.rs`）、**adversary LLM 拦截**（按默认规则拦 shell：外传/超范围破坏/恶意/提权，`src/security/adversary_inspector.rs`）。
- **工具循环检测** — 同名同参重复调用监控（`src/tool_monitor.rs`）。
- **code-mode** — TS/Bash 沙箱执行 + 工具披露折叠压上下文（`src/agents/platform_extensions/code_execution.rs`）。

### 扩展生态
- **hooks（Open Plugins 规范）** — 插件 `hooks/hooks.json`，matcher + `${PLUGIN_ROOT}` 命令动作，stdin 收 JSON 事件（`src/hooks/mod.rs:1`）。
- **skills** — SKILL.md（agentskills.io spec）作为 MCP 客户端暴露（`src/skills/mod.rs`）。
- **recipe** — YAML/JSON 任务包：instructions/prompt/extensions/settings/subrecipes + 校验/模板/goose:// deeplink（`src/recipe/`）。
- **斜杠命令三源** — Builtin/Recipe/Skill 统一注册（`src/slash_commands/types.rs:2`）。
- **扩展恶意软件检查** — `src/agents/extension_malware_check.rs`。

### headless / 自动化 / 交互
- **Telegram 网关** — 远程聊天驱动 goose（`src/gateway/`）。
- **ACP server** — IDE 经 Agent Client Protocol 驱动（`src/acp/server.rs`）。
- **roaming（独有）** — iroh QUIC P2P ACP 传输：远端驱动/委托任务，WireGuard 式公钥 allowlist，免端口转发（`crates/goose-roaming/`）。
- **handoff memo（独有）** — 会话移交外部 ACP agent 时把历史做**预算化+脱敏+截断**（窗口 30% 且 ≤64k token、图按 1600 token 计、近 5 轮工具响应保留）（`src/acp/handoff.rs:17`）。
- **GDK SDK** — UniFFI 编译 Python/Kotlin 绑定 + `ObservabilityHook` 生命周期事件（`crates/goose-sdk/`）。
- **安全自更新** — SHA256 + **sigstore bundle/trust-root 验证** release 产物（`goose-cli/src/commands/update.rs:19`）。
- **doctor** — 环境自检（`src/doctor.rs`）；evals/harbor — terminal-bench-2 基准对比工具（89 任务、成本/turns 报表）。

## 独特亮点（别家少见）

1. **ACP 三栖**：ACP server（IDE 驱动自己）+ ACP client（把 Claude Code/Codex 整个 agent 当 provider）+ iroh P2P roaming 传输——agent 互操作位面最全。
2. **供应链级自更新**：release 资产 sigstore trust-root + bundle 验证后才替换二进制。
3. **安全 inspector 三件套**：prompt-injection 扫描 + egress 外泄检测 + LLM adversary 拦 shell，统一管线并进审批决策。
4. **handoff memo**：跨 agent 会话移交时对历史做 token 预算/脱敏/截断的「有界备忘录」。
5. **声明式 provider**：一 JSON 接入一个网关并编译进二进制，40+ provider 维护面摊薄。
6. **turn 预算自感知**：`<turn-context>` 注入剩余回合数，模型据此自主收敛。
7. **check-as-subagent 的 repo 规则审查**（`.agents/REVIEW.md`）。
8. **UniFFI 跨语言 GDK + 类型化观测 hook**。

## 版本与活跃度

- workspace 版本 **1.54.0**（根 `Cargo.toml`）；子 crate 由 release-plz 独立发版。
- HEAD `3bd8520`（2026-10-09），PR 序号 1.2 万+，活跃度非常高；Linux Foundation 基金会治理 + 官网文档站 + Discord，**活跃维护中**。

## 存疑 / 仅文档宣称

- **lead/worker 双模型**：仅见 2025-08 博客（`documentation/blog/2025-08-11-llm-tag-team-lead-worker-model/`）宣称，现行代码 grep 无任何 lead/worker 机制，判断已被 summon/delegate 子代理取代——**仅文档（博客）宣称，代码未见**。
- `documentation/docs/experimental/` 的 goose-mobile / remote-access / vs-code-extension 仅为实验文档页，Rust 侧未见实现。
