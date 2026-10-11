# crush 研究笔记

> 仓库: https://github.com/charmbracelet/crush · 本地克隆: `D:/CODE/ai/crush`
> 语言栈: Go 1.27（go.mod:3）· 血缘: Charm 官方出品（bubbletea/lipgloss 生态作者），LLM 层用 Charm 自研 `charm.land/fantasy`（go.mod:8），模型目录 `charm.land/catwalk`（go.mod:7）
> 分析基准 commit: `5e3fe4b`（2026-10-03，浅克隆仅此一个快照提交，HEAD 为 PR #4020）· 分析日期: 2026-10-11
> 范围: 特性设计面（不逐文件流水账）；引号行号均实际读过源码

## 定位与血缘

Charm 出品的终端 coding agent（"Your new coding bestie"），TUI 优先 + client/server 双模，对标 Claude Code/opencode。README.md:15-22 宣称多模型、会话制、LSP 增强、MCP 扩展、全平台（含 Windows/Android/BSD）。商业侧推自家订阅制 provider **Hyper**（README.md:185-191）+ 社区维护的模型目录 Catwalk（独立仓库，README.md:240-243）。血缘上无 opencode/claude fork 痕迹，为全自研。

## 架构速览

- **client/server 分离**：`crush serve` 起本地 HTTP server（`internal/server/`，约 70 个 `/v1/workspaces/*` 端点），TUI 是普通客户端；`internal/workspace/` 定义 Workspace 接口，两个实现 = 本地 App 或 HTTP SDK（workspace.go:3-5），TUI/CLI 前端无差别。
- **多客户端共享工作区**：同 `--cwd` 的客户端隐式加入同一 workspace，共享会话列表/权限队列/LSP/MCP 状态；`--yolo`/`--debug` 先到先得（README.md:500-530）；server 用 BuildID（= 可执行文件 mtime，internal/version/version.go:33-46）检测杀掉陈旧 server。
- **agent 核心**：`internal/agent/`（coordinator + sessionAgent）基于 fantasy 的 agent 抽象；`internal/backend/` 是 server 侧的领域编排层。
- **持久化**：SQLite（sqlc 生成 + goose 迁移 12 个，`internal/db/`），跨平台纯 Go 驱动（modernc/ncruces 双选择器）。
- **进程内基础设施**：`pubsub` 事件总线、`csync` 并发安全原语（VersionedMap 等供 LSP diagnostics 用）。

## 特性清单

### 会话 / 持久化 / 分支

- **SQLite 全量持久化** — 消息/会话/文件读取记录/统计全部入库（`internal/db/sql/`，sqlc.yaml）；输入历史也入库（internal/history/file.go）。
- **压缩即摘要消息** — 压缩生成一条 `IsSummaryMessage` 消息写库并把 `SummaryMessageID` 记在会话上，重建上下文时只取摘要之后的消息（internal/agent/agent.go:1411-1480；internal/message/message.go:52-54；internal/session/session.go:58）。旧消息不删，可回溯。
- **会话树** — Session 带 `ParentSessionID`（internal/session/session.go:52），子 agent 会话挂父会话下；resume/session picker 复用完整库记录。
- **标题自动生成** — 首条 prompt 后用 small model 异步生成标题并剥 `<think>` 标签（internal/agent/agent.go:752, 1832-1851, 70-72）。
- **提示排队** — 会话忙时新 prompt 入队而非拒绝，RunID 做关联回执，排队/取消时序有专门句柄（internal/agent/agent.go:79-123；端点 `prompts/queued|list|clear` internal/server/endpoints.go:354-373）。

### 上下文管理

- **双阈值自动压缩** — 大窗（>200k）模型剩 20k 触发，小窗按剩 20% 触发（internal/agent/agent.go:53-58, 1098-1112）；压缩后接续当前 turn（OnComplete 合并 unauthorized→重试链，agent.go:97-110）。
- **压缩可关** — `DisableAutoSummarize` per-run 开关（agent.go:216,268）。
- **上下文文件分层** — 全局 `~/.config/crush/CRUSH.md`（crush 专用）+ `~/.config/AGENTS.md`（跨工具共享）双全局文件 + `option global-context-path` 可挂目录（README.md:535-551；internal/config/load.go:580-581）。
- **忽略体系** — gitignore 语义 + 项目级 `.crushignore`（internal/fsext/fileutil.go:63-82）。
- **文件读取追踪** — filetracker 记录会话内读过哪些文件/时点（internal/filetracker/service.go:3），供"文件被外部修改"检测与增量提示；`internal/diffdetect` 扫 unified-diff 标记识别 bash 输出里的补丁（internal/diffdetect/detect.go:12-31）。

### LLM 协议兼容

- **provider 抽象 = fantasy** — anthropic/bedrock/google/openai/openrouter/vercel 六家走 Charm 自研 fantasy 库统一接口（internal/agent/agent.go:29-36）；openai 与 openai-compat 双 type 区分直连与兼容网关（README.md:743-747）。
- **Catwalk 目录 + 嵌入快照** — 模型元数据远程自动更新，离线回落编译期内嵌目录，`CATWALK_URL` 可自建（internal/config/catwalk.go:11,53-61；README.md:930-980）；刷新走 singleflight（config/refresh_singleflight_test.go）。
- **本地模型自动发现** — `type: ollama/llamacpp/omlx/lmstudio/litellm` 留空模型列表即自动拉取，`discover_models: true` 与手配合并、手配字段优先（internal/discover/ 六个适配器 + enricher.go；README.md:855-890）。
- **OAuth 全家桶** — xAI Grok 设备码流（internal/oauth/grok/device.go:35）、OpenAI ChatGPT 登录、GitHub Copilot、Hyper、MCP OAuth（RFC 7591 动态注册 + 预注册 client 兜底，README.md:430-470）。
- **Bedrock SSO 原地续期** — `aws_auth_refresh` 命令在凭证过期错误时自动执行并原位重试，不丢消息不重启（internal/agent/aws_sso_refresh.go；README.md:805-825）。
- **HTTP 层调试日志** — 可插拔 RoundTripper 记录请求耗时（internal/log/http.go:14-18）；provider 连通性测试 `TestConnection`（internal/config/config.go:1145）。

### 子 agent / 编排

- **内置三角色收敛** — coder（全工具+MCP）/ task（只读工具、无 MCP/LSP）/ plan（规划工具面）三 agent 硬编码组装（internal/config/config.go:1112-1140，工具面过滤 :1122,1133）；**自定义 agent 已移除**：`Agents` 字段 `json:"-"` 不再从 JSON 加载（config.go:861），crushrc 也无 agent builtin。
- **task 子代理** — `agent` 工具派生只读子代理跑检索任务，子会话挂 ParentSessionID（internal/agent/agent_tool.go:26-50）。
- **工具调用循环检测** — 最近 10 步窗口内同一工具交互签名（sha256）出现 >5 次判死循环（internal/agent/loop_detection.go:16-19）。
- **Channels（MCP 反向推送）** — MCP server 可向会话推送消息触发一轮 agent（如 IM 集成）；server 侧全局路由，`--channels` 显式 opt-in，push 永不丢弃（internal/backend/channels.go:25-45；internal/agent/channel.go:5-16）。
- **多客户端并发收口** — fire-and-forget 调用有 AcceptedRun 预约句柄，accept→cancel/queued/active 状态机防竞态（internal/agent/agent.go:110-123；backend/race_on_test.go）。

### 工具体系

- **内置工具约 35 个**（internal/agent/tools/）：bash/edit/multiedit/write/view/ls/glob/grep+rg/search/download/web_search/web_fetch/fetch（html→markdown 转写，agentic_fetch_tool.go）/sourcegraph（HTTP 查 Sourcegraph，sourcegraph.go）/todos/question/crush_info/crush_logs/job_output/job_kill。
- **LSP 深度工具化** — 8 个 LSP 工具直接暴露给模型：definition/references/symbols/diagnostics/call_hierarchy/rename/replace_symbol/restart（internal/agent/tools/lsp_*.go）；LSP 客户端按文件自动启动（internal/lsp/manager.go:253），diagnostics 用 VersionedMap + 稳定期等待（internal/lsp/client.go:57,620）。
- **自研 bash 引擎** — mvdan/sh 解释器内嵌，crushrc 与 bash 工具共用；`jq`/coreutils 为 Go 原生 builtin 兜底（internal/shell/builtins_registry.go:19-26, coreutils.go）；后台任务 job_output/job_kill（internal/shell/background.go）；输出截断落盘（internal/shell/truncate.go）；进程组隔离 setsid（internal/shell/exec_unix.go:88）。
- **安全命令白名单** — bash 只读命令清单免审批（internal/agent/tools/safe.go:10+）。
- **权限系统** — PermissionKey{SessionID,ToolName,Action,Path} 四元组、persistent grant、会话级 auto-approve、`--yolo` 全跳（internal/permission/permission.go:88-92,158,280,290）；配置面 `permissions allow/deny` + per-agent `allowed_tools`/`disabled_tools`（internal/config/config.go:432,471）。
- **question 工具** — agent 可向用户提结构化问题并阻塞等答案，pubsub 请求-应答模式（internal/question/question.go:3-5），一次只挂一个问题。
- **agent 可向 LSP 等 diagnostics** — 编辑后等待诊断稳定再继续（lsp/client.go:620-679），模型经 diagnostics 工具主动消费。

### 扩展生态

- **MCP 三传输 + 治理面** — stdio/http/sse；per-server disabled_tools；sessionless server 自动探测（GitHub/Copilot 已知列表）；MCP prompts/resources 也暴露（README.md:420-490；internal/config/mcp.go）；Docker MCP 集成（`docker mcp` 网关，internal/config/docker_mcp.go）；server 端 mcp 端点 20+（endpoints.go:513-648）。
- **Hooks（初期）** — 仅 `PreToolUse` 一个事件；exit 2 拦当前工具、exit 49（刻意避开常见信号区）halts 整轮；支持 decision allow/deny、context 注入、**tool_input 重写**；元数据嵌入工具结果供 UI 显示 hook 指示（internal/hooks/hooks.go:15-75；更多事件在 docs/hooks/FUTURE.md 规划中）。
- **Agent Skills 标准接入** — agentskills.io 规范：SKILL.md + YAML frontmatter（`user-invocable`/`disable-model-invocation`），扫描全局 5 处 + 项目 4 处（含 `~/.claude/skills`、`.cursor/skills` 竟兼容）（internal/skills/skills.go:23-42；internal/config/load.go:1372-1418）；**内置 skill 走 embed**（crush-config/crush-hooks/jq，internal/skills/embed.go:14）；用户可调用 skill 进命令面板。
- **crushrc = Bash 配置语言** — 配置即 `.bashrc` 式脚本，内置 provider/model/mcp/lsp/permissions/hook/option 七个 builtin（internal/shellconfig/register.go:22-28），mvdan/sh 保证 Windows 语义一致；旧 crush.json 降级为 deprecated 但仍支持（README.md:255-300）。

### TUI / 交互件

- **bubbletea v2 + lipgloss v2** 全新栈（go.mod:9-11）；`internal/ui/` 按组件分包（chat/dialog/diffview/completions/filepicker/attachments/logo/notification/anim），363 个 golden 文件做渲染回归。
- **Kitty 图形协议渲染图片** — 终端内嵌图（internal/ui/image/image.go:159-163），图片粘贴（fsext/paste.go）+ 附件（ui/attachments/）。
- **主题系统** — 内置主题 + 用户主题 JSON + TUI 内实时预览编辑（ctrl+e）（README.md:350-385）。
- **命令面板 + 用户可调用 skill** — ctrl+p 聚合命令与 `user:`/`project:` skill。
- **桌面通知** — 仅在终端失焦且终端上报 focus 时发；native/OSC/bell 三通道 auto 选择（README.md:685-700）。

### Headless / 自动化

- **`crush run` 非交互** — prompt 走参数或 stdin，可接 server 或本地 App 双路径（internal/cmd/run.go:36-163）。
- **REST + SSE 全量 API** — 会话/消息/权限应答/问题应答/模型切换/压缩触发/shell 执行/LSP 启停全在 HTTP 面上（internal/server/endpoints.go:13-648），第三方可完整驱动。
- **API 自文档** — apigen 包从端点注册生成 OpenAPI + HTML 文档（internal/apigen/；internal/server/docs.go:9-12）。
- **`crush stats`** — 从本地库生成 HTML 用量报告（内嵌模板，internal/cmd/stats.go）。

### 其他

- **匿名遥测** — PostHog + machineid 设备哈希，仅 usage 元数据，环境变量退出（README.md:989-999；internal/event/）。
- **Git 署名可配** — `Assisted-by` / `Co-Authored-by` / none 三档（README.md:720-730）。
- **herdr 终端复用器联动** — 在 Charm 自家 herdr pane 里运行时经 Unix socket 上报 idle/working/blocked 状态，不靠屏幕刮擦（internal/herdr/client.go:1-9）。
- **自更新** — internal/update/。

## 独特亮点（别家少见）

1. **crushrc 把配置做成 Bash**：配置语言 = 内嵌 bash 解释器 + 七个领域 builtin，天然获得条件/循环/命令展开（含 1Password 取密钥），且 Windows 语义一致——比 JSON/YAML+占位符方案表达力高一档（internal/shellconfig/register.go:22-28）。
2. **client/server + 多客户端 workspace**：一个 server 多个 TUI 同 `--cwd` 隐式组队、共享权限队列与 LSP，SSE 广播；agent 状态机为多客户端竞态做了成套句柄（AcceptedRun/first-wins flags），这在小工具里几乎没有（internal/server/ + README.md:500-530）。
3. **Channels：MCP 反向通道**——MCP server 主动 push 消息触发 agent 回合，把"IM → agent"类集成做成一等协议而非 hack（internal/backend/channels.go:25-45）。
4. **LSP 当工具面而非装饰**：8 个 LSP 工具含 call_hierarchy/replace_symbol/restart 这种少见的，且编辑后等 diagnostics 稳定期再推进——"agent 与编译器对话"最完整的一套（internal/agent/tools/lsp_*.go）。
5. **自研 bash 引擎双用**：同一个 mvdan/sh 解释器既解析 crushrc 又实现 bash 工具（含 Go 版 coreutils/jq 兜底），配置与工具执行同源、跨平台零差异（internal/shell/）。
6. **退出码 49 的 halt 语义**：hook 退出码空间被刻意分区（2=拦工具，49= halt 整轮且避开 sysexits/信号区），细节上有生产级考虑（internal/hooks/hooks.go:18-20）。
7. **Skills 标准全面兼容**：直接扫 `~/.claude/skills`、`.cursor/skills`、`.agents/skills`——对既有生态零迁移成本（internal/config/load.go:1399-1418）。
8. **BuildID = exe mtime**：用可执行文件修改时间当构建指纹识别并清理陈旧 server，开发迭代场景下极其实用（internal/version/version.go:33-46）。

## 版本与活跃度

- 浅克隆无 tag 可考；版本号构建期经 ldflags 注入，缺省 `devel`（internal/version/version.go:13-16）。
- HEAD `5e3fe4b`（2026-10-03）已是 PR **#4020**，issue/PR 编号量级说明社区极活跃；发布渠道覆盖 brew/npm/winget/apt/dnf/NUR 等 10+ 包管理器（README.md:28-150）。
- 依赖前沿：bubbletea/lipgloss/fantasy 均为 v2/v0.4x 新大版本，Go 1.27；go.mod 直接引用 `charm.land/*` 自家模块域名。
- 维护主体 Charmbracelet 公司（Charm 生态 + 自家 Hyper 订阅商业闭环），无停滞迹象。
