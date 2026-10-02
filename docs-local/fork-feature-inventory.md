# fork 已有特性基线清单（feature inventory）

> **用途**：同类开源项目特性移植预研（kimicode-port / step-code-port / workflow-pi-port 式）
> 开工前，先对照本清单下「已有」判定，避免把 fork 已有的设计误报为缺口；
> 预研完工后，把新澄清的「已有 / 不做」行**回填**进本清单（§3/§5）。
> 每条尽量带锚点或来源 crate；锚点优先引自已复核的预研文档，新写锚点需 grep 复核。
> **纪律**：本清单只写「有什么、在哪」，不写否定结论（「没有 X」归各预研文档）。
> 基线：main `c3e6c3c`（2026-10-02）；workspace 成员 102 crate（`crates/build` 1 + `crates/codegen` 89 + `crates/common` 12）。

## 1. crate 功能域地图

主程序：`xai-grok-shell`（CLI 主程序 bin）+ `xai-grok-pager-bin`（`xai-grok-pager` 全屏 TUI bin）+ `xai-grok-pager-minimal`（`grok --minimal` scrollback 原生渲染）。

### 会话与持久化
| crate | 职责 |
|---|---|
| `xai-chat-state` | Actor 型会话状态管理（快照原语 `types.rs:26`）；LOCAL：失败计数账本链 |
| `xai-grok-session-events` | 每 session 的 JSONL 类型化事件日志（遥测面，非会话历史 journal） |
| `xai-sqlite-journal` | SQLite journal 模式选择器（WAL vs rollback，按文件系统判定）；**不是**会话历史 journal |
| `xai-grok-compaction` | 传输无关的压缩引擎（common，chat/build 共用） |
| `xai-compaction-transcript` | 压缩分段 Markdown 渲染 + 分段存储命名约定 |
| `xai-grok-active-sessions` | grok-home 下锁保护 JSON 的活跃 TUI 会话注册表 |
| `xai-grok-foreign-sessions` | 外部 coding agent 会话的有界元数据发现 |
| `xai-grok-session-search` | SQLite FTS5 会话全文索引（lease 引导 + 增量 upsert + BM25） |
| `xai-grok-memory` | 跨会话记忆（V1/V2；FTS5 + 可选向量检索降级链，见 agent-memory-design/ADOPTION.md） |
| `xai-prompt-queue` | 输入排队 wire 类型（合并规则，回合结束发送） |
| `xai-interjection-core` | 回合中 steer 注入缓冲与格式化（Ctrl-S 机制层） |
| `xai-message-delivery-core` | 源类型化消息投递值与操作授权 |

### Agent 与 workflow
| crate | 职责 |
|---|---|
| `xai-grok-agent` | Agent 构建、definition 解析、system prompt 组装（plan mode 工具注册 `config.rs:148-149`） |
| `xai-agent-lifecycle` | host 无关的 agent lifecycle hooks（贡献者收 data-only 输入） |
| `xai-grok-subagent-resolution` | 子 agent definition/runtime/prompt/resume 的共享解析 |
| `xai-workflow` | Rhai 脚本化 workflow 引擎：agent()/parallel()/journal 重放/预算 reserve-release/await_user（`lib.rs:17-18`、`host.rs:54-55`） |
| `xai-grok-bundle` | 已发布 subagent bundle（personas/roles/agents/skills）的校验和缓存 |
| `xai-grok-config` / `xai-grok-config-types` | 配置加载（requirements > user > managed、TOML merge）/ 叶子配置类型（goal 开关 `lib.rs:491-500`） |
| `xai-grok-models` | 内置默认模型 ID（default_models.json） |

### 采样与流
| crate | 职责 |
|---|---|
| `xai-grok-sampler` | Actor 型采样/推理层：HTTP 流式 + retry + 流分类（`stream_classify.rs`）+ Length salvage（`client.rs:2438-2448`） |
| `xai-grok-sampling-types` | 采样/chat-completion API 纯数据类型；LOCAL：国模 think 标记 + ChatCompletions gateway quirk（十四期） |

### 工具体系与执行
| crate | 职责 |
|---|---|
| `xai-grok-tools` | 内置工具库（`implementations/grok_build/`：bash/scheduler/todo/ask_user_question 等） |
| `xai-grok-tools-api` | 工具 protobuf API 定义 |
| `xai-tool-types` / `xai-tool-protocol` / `xai-tool-runtime` | 工具描述规范类型 / Computer Hub wire 协议 / 统一 Tool trait + 分派 + 错误分类 + 搜索索引 |
| `xai-grok-hooks` | 运行时 hooks：文件发现 + 命令执行 + 策略（7 事件、pre_tool_use allow/ask/deny、fail-open） |
| `xai-hooks-plugins-types` | hooks/plugins ACP 扩展 DTO（wire 层） |
| `xai-grok-sandbox` | OS 级沙箱（Landlock/Seatbelt，经 nono） |
| `xai-grok-shell-terminal` | local/ACP/PTY 终端 runner（自 shell 拆出） |
| `ptyctl` / `ptyctl-cli` | 基于 alacritty_terminal 的无头 PTY 控制器及其 CLI |
| `xai-tty-utils` | TTY 安全 spawn 工具（脱离控制终端、压制交互分页器） |
| `xai-grok-shell-session-support` | MCP gateway 目录/调用缓存 + 文件访问跟踪 |
| `xai-hunk-tracker` | 文件 hunk（diff）跟踪，agent/外部归因 |

### 扩展生态
| crate | 职责 |
|---|---|
| `xai-grok-mcp` | MCP 集成（rmcp 隔离舱）：stdio + streamable HTTP 双传输、OAuth、凭据存储（`servers.rs`） |
| `xai-grok-plugin-marketplace` | 插件市场源配置 + 插件发现（FS 回退索引） |
| `xai-acp-lib` | ACP 编辑器协议库 |
| `xai-computer-hub-core` / `-sdk` / `-mcp-adapter` | Computer Hub 传输/ToolRegistry 抽象 / 连接池+重连+工具运行时 / MCP 工具桥接 |

### TUI / pager
| crate | 职责 |
|---|---|
| `xai-grok-pager` | 主全屏 TUI（视图/弹窗/斜杠命令；`views/tasks_pane.rs`、`actions/defaults.rs:425` `/rewind`） |
| `xai-grok-pager-render` | pager 渲染层（品牌协议选择 `terminal/image.rs` 等） |
| `xai-grok-pager-diff` | TUI diff hunk 构造 |
| `xai-grok-markdown` / `-core` | 流式终端 Markdown 渲染器 / 无头 markdown 分析（同 pulldown-cmark 配置） |
| `xai-grok-mermaid` | Mermaid 源 → PNG 栅格渲染（可换引擎） |
| `xai-ratatui-inline` / `xai-ratatui-textarea` | ratatui inline 组件 / 文本域组件 |
| `xai-grok-gboom` | `/gboom` 彩蛋 raycaster 游戏 |
| `xai-grok-pager-pty-harness` | pager e2e/bench 共享 PTY harness + 场景库 |
| `xai-grok-dashboard-store` | SQLite 持久 dashboard 工作区（成员/布局/分组） |
| `xai-grok-announcements` | 公告类型/持久化/格式化 |
| `xai-grok-voice` | 语音听写（流式 STT） |

### workspace / 文件系统 / 进程
| crate | 职责 |
|---|---|
| `xai-grok-workspace` | host 本地 workspace 核心（FS/VCS/执行/发现），含 workspace-server bin |
| `xai-grok-workspace-client` / `-daemon` / `-types` | hub 代理 RPC 客户端 / workspace-server 守护进程生命周期 / workspace wire 类型 |
| `xai-grok-diag-server` | guest 内诊断 HTTP 服务（/ready /statusz /logs） |
| `xai-fsnotify` | 本地文件系统事件源（单因果语义 FsEvent 流） |
| `xai-fast-worktree` | CoW clone 高性能 git worktree 创建 |
| `xai-gix-status` | gix status 共享助手（RLIMIT_NPROC 线程预算） |
| `xai-fuzzy-file-search` | ignore 感知遍历 + nucleo 模糊匹配 + 后台 daemon |
| `xai-codebase-graph` | tree-sitter 查询驱动的代码图谱生成 |
| `xai-dirs` / `xai-grok-paths` | home/grok-home 解析（USERPROFILE 优先）/ 类型安全路径包装 |
| `xai-grok-file-lock` | grok-home 有界非阻塞咨询文件锁 |
| `xai-system-power` | 跨平台睡眠/唤醒通知 |
| `xai-crash-handler` | 跨平台崩溃处理（Unix 信号 + Windows SEH）+ 启动崩溃检测 |

### 网络 / 账号 / 安全
| crate | 职责 |
|---|---|
| `xai-grok-login` | 登录子系统：登录流、token 刷新、凭据 provider、auth 存储 |
| `xai-grok-auth` | auth 依赖反转缝（HttpAuth + AuthCredentialProvider traits） |
| `xai-grok-env` | 后端环境：endpoint URL 预设、GROK_*/XAI_* 读取器、凭据路径 |
| `xai-grok-http` | 共享 reqwest 客户端与 User-Agent |
| `xai-grok-extra-ca` | TLS 策略：rustls 钉扎 + `GROK_EXTRA_CA_BUNDLE`；LOCAL：`proxy_hosts` 出口代理（一期） |
| `xai-grok-secrets` | 出站数据正则脱敏（Sentry/Mixpanel/产品事件；`sanitizer.rs`） |
| `xai-grok-image` | 图片校验与转码（工具与客户端共用） |

### 遥测 / 观测 / 版本
| crate | 职责 |
|---|---|
| `xai-grok-otel` | OpenTelemetry 地基：W3C trace 传播、OTLP HTTP client、tracing→OTLP 层 |
| `xai-tracing` / `xai-tracing-macros` | tracing 初始化 / 时间戳与时序宏 |
| `xai-mixpanel` / `xai-grok-telemetry` | Mixpanel HTTP 上报 / 遥测引擎（产品事件 + Mixpanel + Sentry） |
| `xai-grok-status-line` | 状态行契约：`[ui.status_line]` 配置 + agent→client payload |
| `xai-token-estimation` | 纯共享 token 估算原语 |
| `xai-grok-update` / `xai-grok-version` | 自更新 / lockstep 版本号（LOCAL：自动更新默认关，五期） |
| `xai-grok-feedback` | 反馈分类 + 本地草稿 + 会话 trace 归档 |
| `xai-file-utils` | 本地数据采集：上传队列与 blob 存储 |

### 基础 / 测试 / 构建
| crate | 职责 |
|---|---|
| `xai-grok-shared` | shell 与下游客户端（pager-render）共享工具 |
| `xai-grok-test-support` / `xai-test-utils` | mock 推理服务 + SSE 生成器 + ACP stdio client + 无头 runner / hermetic git |
| `xai-circuit-breaker` | 熔断器（common） |
| `xai-proto-build` | protobuf 构建（LOCAL：Windows 构建修复，一期） |

## 2. 已有特性清单（按功能域，重点域）

锚点未标注来源者引自已复核预研文档（step-code-port/survey.md、kimicode-port/README.md、workflow-pi-port/README.md），新写锚点已 grep 复核。

- **会话与持久化**：append-only `updates.jsonl` + `RewindMarker`（kimicode-port §P1 摸底更正）；`/rewind` 破坏性截断 + **prompt 放回编辑器已有**（`pager/app/dispatch/rewind.rs:396`）；compaction 引擎（`xai-grok-compaction`）+ 压缩分段渲染（`xai-compaction-transcript`）+ 压缩请求半答并行 tool call 回填（LOCAL `feat/local-compaction-toolpair-repair`）；快照原语（`xai-chat-state/src/types.rs:26`）；会话 FTS5 全文搜索（`xai-grok-session-search`）；跨会话记忆（`xai-grok-memory`，V1/V2 + 检索降级链）；输入队列（`xai-prompt-queue`）；steer 注入（`xai-interjection-core/src/lib.rs:1-7`）。
- **采样与流**：HTTP 流式 + retry 分类（`xai-grok-sampler`，`retry.rs:245-257`）；Length 截断 salvage（`client.rs:2438-2448`）；流分类 `stream_classify.rs`；Responses/chat_completions 双协议；i18n 文案（中文）；LOCAL：国模 think 泄漏修复 + gateway quirk（十四期）、Responses 事件方言归一化（`fix/local-responses-event-dialect`）、think_split 修复 ×2（`fix/local-think-split-code-span` 等）。
- **工具体系**：内置工具面含 plan mode（`EnterPlanModeTool`/`ExitPlanModeTool`，`xai-grok-agent/src/config.rs:148-149`）、clarify（`grok_build/ask_user_question/`）、todo + TodoGate 催办、scheduler（interval 型，持久化 `grok_build/scheduler/types.rs:311`，与 bash 工具同目录）；bash 审批 = 参数级 `ToolRequirement` 表达式（`xai-grok-tools/.../bash/mod.rs:1489`）+ hooks allow/ask/deny；OS 沙箱（`xai-grok-sandbox`）；hooks 7 事件 fail-open（`xai-grok-hooks`）；工具输出压缩（六期，`tool-output-compression-plan.md`）；LSP 诊断合并去抖（二期）。
- **TUI / pager**：ratatui 原生全屏 TUI + `--minimal` 双渲染模式；`/stats` 三时间窗 + 模型用量 ledger + 卡片性能指标（LOCAL）；`/rewind`（见上）；`/style` 极简风格正交覆盖层（十五期）；`/lang` i18n 全量中文化（翻译表 `pager/src/slash/i18n.rs`）；`/import-claude`（`pager/src/slash/commands/import_claude.rs:1-8`，覆盖面比 Step-Code 更宽）；`/gboom`；dashboard 工作区（`xai-grok-dashboard-store`）；欢迎屏熊猫 logo（v1.0.39 比例修复）+ witty loading phrases；语音听写。
- **workflow 与子 agent**：Rhai 引擎 + journal 确定性重放（`engine.rs:268 host_call` request_hash）+ token/agent 双预算（`xai-workflow/src/lib.rs:17-18`、`host.rs:54-55`）+ 子 agent **确定性 resume**（workflow-pi-port 结论：领先 pi）；tasks pane（`pager/src/views/tasks_pane.rs`）；subagent bundle 缓存（`xai-grok-bundle`）；隔离 spawn（`shell/src/agent/subagent/child_runtime.rs`）。
- **扩展生态**：MCP 双传输 + OAuth + 401 重试 + 进程组收割（`xai-grok-mcp/servers.rs`）；插件市场（`xai-grok-plugin-marketplace`）；ACP（`xai-acp-lib`）；skills（bundle + shell `skills_watcher` 特性）；Computer Hub 三件套。
- **遥测与观测**：OTLP/Mixpanel/Sentry（`xai-grok-otel`/`xai-mixpanel`/`xai-grok-telemetry`）+ 出站脱敏（`xai-grok-secrets/src/sanitizer.rs`）；状态行 payload 契约（`xai-grok-status-line`）+ pager 内置渲染（二期）+ ttft/tps；LOCAL：TTFT 参考点前移（`feat/local-perf-ttft-request-anchor`）+ 0ms 伪样本过滤（`fix/local-perf-ttft-zerosample`）+ 输出流修复（`feat/local-perf-ttft-output-streams`）+ model.display_name 覆盖（`feat/local-statusline-catalog-model-name`）；`/usage` 周额度估算（`feat/local-usage-quota-estimate`，三模块：tools 采样落盘 + shell T1 + pager T3）；limit-probe 限流头落盘（`feat/local-limit-probe`，`limit-inference.md`）。
- **账号与配额**：登录流/token 刷新/凭据存储（`xai-grok-login`）；auth 依赖反转缝（`xai-grok-auth`）；endpoint 预设与凭据路径（`xai-grok-env`）；配额端点调研账 `docs-local/usage/quota-endpoints-and-credentials.md`；用量跳变审计 playbook（`quota-pct-jump-audit.md`）。

## 3. LOCAL 专题自产特性（上游没有；来源 = PATCHES.md 登记 / AGENTS.md 待办）

| 特性 | 来源 |
|---|---|
| i18n 全量中文化（斜杠命令 → P0–P4 四期–十二期 → 十六期 T3 扫尾，翻译表 1417+ 组；`/lang` 切换） | PATCHES 四/五/七/九/十/十一/十二/十六期；规约 `ui-i18n-plan.md`；同步补回 `feat/local-sync-i18n-restore` |
| `/stats` 时间窗化（5h/day/week 标签页 + 参数）+ 模型用量 ledger + 卡片性能指标对齐 + 缓存命中率 | `feat/local-stats-modal-i18n`；PATCHES 2026-09-20/21 各节 |
| `/usage` 周额度 token 估算（billing 采样落盘 → Δpct 反推下界 → 面板） | `feat/local-usage-quota-estimate`（PATCHES 2026-09-27）；`usage-quota-estimate-todo.md` |
| limit-probe：被动落盘限流头/429 现场 → 离线反推 TTL/RPM/TPM | `feat/local-limit-probe`；`limit-inference.md` |
| 状态行 TTFT 参考点前移 + 0ms 伪样本过滤 + 输出流修复 | `feat/local-perf-ttft-request-anchor` / `fix/local-perf-ttft-zerosample` / `feat/local-perf-ttft-output-streams` |
| 状态行原生渲染 + item 扩展（tokens/cache/think 脚本退役）+ display_name 覆盖 | PATCHES 二期/五期；`feat/local-statusline-catalog-model-name` |
| Responses 流事件方言归一化（第三方网关非标事件容错） | `fix/local-responses-event-dialect`（PATCHES 2026-10-02） |
| 国模适配：think 标记泄漏修复 + ChatCompletions 网关 quirk | PATCHES 十四期 |
| think_split 修复 ×2（多字节 panic、反引号 code-span 误折叠） | `fix/local-think-split-code-span` 等（v1.0.37-preview.2/.6） |
| 压缩请求半答并行 tool call 回填 | `feat/local-compaction-toolpair-repair` |
| `/style` 极简风格与 agent 变体解耦（正交覆盖层） | PATCHES 十五期 |
| 工具输出压缩（实验性）+ LSP 诊断合并去抖 | 六期 / 二期 |
| 自动更新默认关、`proxy_hosts` 出口代理、Windows 构建与路径修复批次 | 五期 / 一期 / 九期等 |
| 欢迎屏熊猫 logo（盲文格采样宽高比修复）+ witty loading phrases | PATCHES 2026-09-20；AGENTS.md v1.0.39 条 |
| OpenCode Go 网关 UA 标识 | `feat/local-opencode-go-ua-marking` |
| 会话默认 agent 尊重 `[agent] name`（respect_config_agent；已被上游 defer_builtin_agent_profile 取代，同步时撤销） | PATCHES 十三期 |

## 4. 已判定「不移植 / 已有」汇总表（合并自既有预研矩阵）

| 特性 | 判定 | 出处 |
|---|---|---|
| `/goal` 目标生命周期（budget/pause/resume/clear） | ✅ 已有且更强（completion classifier、`no_progress_paused`、`token_budget`） | step-code-port survey.md:86 |
| plan mode / clarify / todo+TodoGate | ✅ 已有 | step-code-port survey.md:87-89 |
| Claude/Codex MCP 配置自动导入 | ✅ 已有（fork 覆盖更宽，`/import-claude`） | step-code-port survey.md:90 |
| secret 出站脱敏 | ✅ 已有（`xai-grok-secrets`，可对照补模式） | step-code-port survey.md:91 |
| steer 注入运行中回合 | ✅ 机制已有（`xai-interjection-core`；Ctrl-S 键位入口待对照） | step-code-port:92 + kimicode-port:59 |
| 备用屏布局系统 | ✅ 已有（ratatui 原生全屏 TUI，架构不同） | step-code-port survey.md:93 |
| MCP catalog / OAuth / 插件市场 / 遥测 | ✅ 已有 | step-code-port survey.md:96 + workflow-pi-port:39 |
| 编排原语 agent()/parallel()/journal 重放/预算/await_user | ✅ 已有且领先 pi（确定性 resume） | workflow-pi-port README TL;DR 1 |
| 输入队列 | ✅ 已有（`xai-prompt-queue`） | kimicode-port:58 |
| lifecycle hooks | ✅ 已有（7 事件 fail-open；kimi 多出事件位登记为候选增强） | kimicode-port:61 / D3 |
| 明文文件记忆 + 按需检索 + 作用域分层 | ✅ 已有，无需重做存储架构 | agent-memory-design/ADOPTION.md §3 |
| stdout takeover / 背压 | ❌ 不做（Node 特有） | step-code-port survey.md:94 |
| steppage 静态站发布 | ❌ 不做（产品绑定） | step-code-port survey.md:95 |
| codemode（QuickJS 模型写 JS 调工具） | ❌ 不移植（D1：双运行时破坏 journal 确定性；吸收按分支 KV + 大输出旁路两思想） | workflow-pi-port D1 |
| extension 运行时可编程 API | ❌ 不移植（hooks/plugins 是事件钩子） | workflow-pi-port:45 |
| virtual models 按请求路由 | 不需要（`AgentOpts.model/effort` 已覆盖） | workflow-pi-port:43 |
| 视频输入 | ❌ 不做（无 provider 能力位） | kimicode-port D5 |
| minidb / 持久化四接口 / SSH kaos / ACP / node-sdk | ❌ 不做（sqlite-journal、xai-acp-lib 已覆盖） | kimicode-port:67 |
| `/mcp-config` 对话式 MCP 配置 | 不照搬（D4，只吸收"内置能力写成 SKILL.md"思想） | kimicode-port D4 |
| cron 表达式调度 / ultraloop 关键词入口 | 不立项（fork scheduler interval 型覆盖主场景；P5/P6 按需随手） | step-code-port D5 |

## 5. 已知盲区（「未核实」≠「没有」，后续预研先核再判）

- fork headless 模式遇审批阻塞的现有行为（step-code-port 未核实项 §1）。
- `xai-grok-shell` 子 agent spawn 链路是否有 env 级防递归标记与 idle watchdog（step-code-port D4）。
- workflow 前台逐 agent 行级进度投影 vs Step `progress.ts`（step-code-port 未核实项）。
- `xai-grok-compaction` 分层与 Step request-time projection 的重叠度（step-code-port 未核实项）。
- `xai-grok-subagent-resolution` 现状（kimi 委托图约束对照面，kimicode-port:57）。
- 审批 "approve for session" 的作用域模型现状（kimicode-port:60）。
- `xai-grok-models` 能力位裁剪现状（kimi kosong capability 对照面，kimicode-port:64）。
- Ctrl-S steer 的键位入口是否已接线（机制层已有；kimicode-port:59 订正注）。

## 来源

- `docs-local/step-code-port/survey.md`、`docs-local/kimicode-port/README.md`、`docs-local/workflow-pi-port/README.md`（+ 两份设计原型分文档）、`docs-local/agent-memory-design/{README,ADOPTION}.md`
- `docs-local/PATCHES.md`（一–十六期 + 专题修复节）、`AGENTS.md` 待办索引、`docs-local/ui-i18n-plan.md`、`docs-local/limit-inference.md`、`docs-local/usage-quota-estimate-todo.md`、`docs-local/tool-output-compression-plan.md`
- crate 职责取自各 `Cargo.toml` description / `src/lib.rs` 头部注释（2026-10-02 快照）
