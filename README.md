# Grok Build 社区增强版（`grok2`）

[Grok Build](https://x.ai/cli)（`grok`）的社区 fork。上游是 SpaceXAI 的终端 AI 编程 agent，本仓库在保持上游全部能力的前提下，做了一批**本地增强**：让第三方 / 国产模型成为一等公民（不买 grok 订阅也能用）、供应商成本透明（`/usage` 套餐用量、`/stats` API 成本列）、中文界面、Windows 可用性修复、以及若干上下文与交互上的改进。

二进制发行名是 **`grok2`**，与官方 `grok` 并存互不干扰；自动更新默认关闭，官方安装器不会把本构建覆盖掉（见 [默认行为差异](#默认行为差异)）。

- [这是什么](#这是什么)
- [快速开始](#快速开始)
- [用第三方模型（不买 grok 订阅）](#用第三方模型不买-grok-订阅)
- [本地新特性](#本地新特性)
- [配置速查](#配置速查)
- [默认行为差异](#默认行为差异)
- [内置文档与 skill](#内置文档与-skill)
- [从源码构建](#从源码构建)
- [开发约定](#开发约定)
- [来源与许可](#来源与许可)

---

## 这是什么

- **上游同步 + 本地补丁**：本树由上游 monorepo 定期同步（`SOURCE_REV` 记录对应上游 commit）。所有本地改动都登记在 [`docs-local/PATCHES.md`](docs-local/PATCHES.md)，同步后按该表核对重放；`docs-local/` 目录上游不存在，不会冲突。
- **可配置优先**：增强项默认不改变上游行为，或提供显式开关（例外见"默认行为差异"）。
- **面向国内/第三方模型使用场景**：出口代理白名单、`${session_id}` 会话亲和头、DeepSeek/GLM 思考回传、中文界面等，都是为了在不在 x.ai 网络与账号体系内的环境下稳定工作。

> 上游文档里写的 `grok` 命令，在本 fork 中对应 `grok2`；配置文件路径、字段名全部一致。

## 快速开始

1. **拿到二进制**：从本仓库的 Releases 下载 `grok2-vX.Y.Z-x86_64-linux.tar.gz` 或 `grok2-vX.Y.Z-x86_64-windows.zip`（附 `.sha256`），或[从源码构建](#从源码构建)。放到 PATH 上即可，推荐命名 `grok2`。
2. **写配置**：`~/.grok/config.toml`（`GROK_HOME` 已设置时用 `$GROK_HOME/config.toml`）。最小可用配置见下一节。
3. **跑**：`grok2`。首次进入某个目录会有一次目录信任确认；除此之外**不会**弹浏览器登录，直接用。没有任何第三方模型配置时，欢迎页会引导进 [`/auth` 面板](#本地新特性)添加供应商（五步写回 `config.toml`，免重启生效）；Grok 账号登录命令是 `/grok`（原 `/login`，不留旧名）。

```bash
grok2                          # 打开 TUI
grok2 -p "总结这个仓库"        # 无头模式
grok2 models                   # 列出模型并显示当前认证状态
grok2 stats --days 7           # 用量与性能报表（本地账本）
grok2 --help
```

## 用第三方模型（不买 grok 订阅）

**机制**：客户端启动时只看"第一条被广告的认证方式"（`xai-grok-pager/src/acp/mod.rs` 的 `startup_auth_metadata()`）。只要模型目录里**存在任意一个自带凭据的模型**，`xai.api_key` 就会被排在第一位（`xai-grok-shell/src/agent/auth_method.rs` 的 `build_auth_methods()`），登录屏因此不出现。运行时每个模型各自解析凭据（`resolve_credentials()`）：模型自己的 key → 命名 provider → grok 会话 token → 全局 `XAI_API_KEY`。所以第三方模型和 grok 模型可以在同一会话里混用；只要第三方模型写了自己的凭据，grok 会话 token 就不会发到该端点（反过来，一个什么凭据都不写的自定义模型会被当作"自带网关"，用手上的会话 token 去打它的 base_url）。

### 最小配置

```toml
[models]
default = "ds-flash"              # 必改：否则新会话仍用内置 grok-4.6（打 x.ai，无账号会 401）

[model.ds-flash]
model = "deepseek-v4-flash"       # 发给对方 API 的模型 id
base_url = "https://api.deepseek.com/v1"
name = "DeepSeek Flash"
api_key = "sk-..."                # 首次配置用内联 key 最稳
context_window = 128000           # 不写按 200000 算，影响自动压缩时机
```

验证：

```bash
grok2 models        # 顶部应显示：Model 'ds-flash' is using its own API key.
grok2 -p "ok"       # 真跑一次
```

`grok2 models` 顶部若显示 `You are not authenticated.`，说明凭据没被识别——最常见原因是只写了 `env_key` 而该环境变量没导出。

### 常用坑

| 现象 | 原因 → 处理 |
| --- | --- |
| 仍弹登录屏 | 只用 `env_key` 且变量未导出 → 首次配置用内联 `api_key`，或确保启动 shell 里已 export |
| 首次对话 401 | `[models] default` 没改，仍走内置模型 → 指到第三方模型键 |
| Anthropic 端点 401 | 它用 `x-api-key` 而非 Bearer → `api_backend = "messages"` + `extra_headers = { "x-api-key" = "sk-ant-...", "anthropic-version" = "2023-06-01" }` |
| 工具调用异常 | 部分 BYOK 网关不吃流式工具调用 → 该模型加 `stream_tool_calls = false` |
| 误选内置 grok 模型就 401 | `[models] hidden_models = ["grok-4.6", "grok-4.5"]` 收进选择器（`-m` 仍可用），或 `disabled_models` 彻底移除 |
| 联网检索/标题生成仍打 x.ai | `[models] web_search`、`session_summary`、`image_description`、`prompt_suggestion` 默认都是 `grok-4.6` → 一并指到第三方模型键 |

细节：`api_key` 是明文写进 `config.toml`，库外密钥建议用 `env_http_headers`（值只在内存里）或 `env_key`；别把带 key 的配置提交进仓库。

### 进阶

```toml
# 多个模型共用同一网关：base_url / 凭据 / 头只写一次
[model_providers.my-gw]
base_url = "https://gw.example.com/v1"
env_key = "GW_API_KEY"
extra_headers = { "x-opencode-session" = "${session_id}" }   # fork 扩展：展开为会话 id

[model.gw-flash]
model = "deepseek-v4-flash"
model_provider = "my-gw"

# 密钥由 CLI 动态签发（网关侧 SSO / 短期 token）
[model_providers.my-gw2]
base_url = "https://gw2.example.com/v1"
[model_providers.my-gw2.auth]
command = "/usr/local/bin/gw-token"
token_ttl_secs = 3600

# 只想用自家模型目录（跳过全部内置模型）
[endpoints]
models_base_url = "https://models.example.com/v1"

# 彻底不出现浏览器登录（fail-closed：没有 key 就直接报错）
[auth]
preferred_method = "api_key"
```

字段全表与各提供商（OpenAI / Anthropic / Ollama / Together / 自建）示例见内置文档 `11-custom-models.md` 与 `26-config-reference.md`；另有一份可直接被 agent 自动加载的 skill：[`third-party-models`](#内置文档与-skill)。

> `disable_api_key_auth = true` 是相反方向的开关（企业强制走 IdP），第三方模型场景不要设。

## 本地新特性

### 模型与网关

- **第三方 / BYOK 模型一等公民**：`[model.*]` 支持 `api_key` / `env_key`（含数组）/ `api_backend`（`chat_completions` / `responses` / `messages`）/ `extra_headers` / `env_http_headers` / `query_params`；`[model_providers.<id>]` 提供 provider 级默认值，`[auth_provider.<name>]` 与 `[model_providers.<id>.auth] command` 支持按需铸 token。
- **国产模型思考回传**：DeepSeek / GLM / Kimi 等 OpenAI 兼容推理模型的 `reasoning_content`（含 `reasoning` / `reasoning_text` 别名字段）会作为思考块进入对话，并在状态行计入 `reasoning_tokens`；跨 chunk 拆分的 `</think>` / `</thinking>` 标记由流式状态机剥离，不会泄漏进正文。
- **网关健壮性**：未知 `finish_reason` 不再打断整条流；DeepSeek 的扁平缓存命中字段（`prompt_cache_hit_tokens`）计入缓存统计；网关不发工具调用 id 时自动合成 `call_{index}`，保证结果可配对。
- **失败模型调用计数**：每次终态失败记入会话账本，状态行以 `✗ n` 呈现（网关限流/5xx/超时的第一手信号）。
- **`/stats` 用量聚合**：按 5 小时 / 日 / 周 × 模型聚合输入、输出、缓存读写 token 与缓存命中率，以及 TTFT、TPS 的 p50/p90；账本落在 `grok_home/cache/model-usage.jsonl`（只覆盖主循环推理，subagent 调用暂不计入）。CLI 形态：`grok2 stats --days 7 --model deepseek`。
- **`/auth` 供应商配置面板**：五步向导（协议 → base URL → SK → 模型目录勾选 → 确认写回），`toml_edit` 定点写回 `[model.<id>]`，注释与手工字段保留、id 冲突拒绝整批；目录回填 `context_window` / `max_completion_tokens`（与新建兜底相同才省略），目录标注支持思考的模型自动带 `supports_reasoning_effort = true`；写回后显式重载模型目录，免重启生效。
- **`/usage` 供应商套餐用量**：当前模型属于「同一把 SK 可查套餐用量」的供应商时（首批：OpenCode Go / Command Code / 智谱 GLM / Kimi / MiniMax），Usage limit 面板显示该供应商套餐余量并标注来源，替代 SuperGrok 块；按模型 `base_url` host 自动判定，缓存 `[provider_usage] cache_minutes`（缺省 5 分钟），TTL 到期先显旧值再后台刷新。

### 网络与代理

- **`[network]` 出口代理**：进程级生效（模型请求、认证、模型目录都走同一条路径），按 host 后缀匹配；默认只代理 `x.ai` / `grok.com`，第三方端点与 loopback 直连。环境变量等价物 `GROK_PROXY` / `GROK_PROXY_HOSTS`。单个模型可显式选择是否走隧道：`[model.<id>] use_proxy = true`（**默认 false 恒直连**——即使配了进程代理，未显式启用的模型也直连）。

```toml
[network]
proxy = "http://127.0.0.1:7897"
# proxy_hosts = ["x.ai", "grok.com"]   # 缺省即此值；[] = 所有 host
```

### Windows 可用性

- **`[shell] backend`**：选择 bash 工具使用的 shell，**仅 Windows 生效**。取值 `pwsh` | `powershell` | `bash`（=Git Bash，别名 `gitbash`/`git-bash`）| `cmd`（别名 `cmd.exe`）；缺省自动级联 pwsh → powershell.exe → Git Bash → powershell.exe。优先于环境变量等价物 `GROK_SHELL`；取值不认识、或选了 `bash` 但本机没装 Git Bash 时，打告警并退回级联。跑 make/脚本类工作流建议 `bash`——Git Bash 自带 `grep`/`head`/`sed`/`awk`，且子进程注入 `MSYS_NO_PATHCONV=1` / `MSYS2_ARG_CONV_EXCL=*`，`/flag` 形式参数不会被 MSYS2 路径转换改坏；反之原生 Windows 工具链（`MSBuild /t:Build`、`cl.exe /nologo`）建议留默认的 PowerShell。取值在进程内首次使用时固定，**改完需重启 `grok2`**。Unix 上该表被忽略，shell 仍由 `$SHELL`（`GROK_SHELL` 可给绝对路径）决定。完整取值表与各 shell 行为差异见内置文档 [`29-local-enhancements.md`](crates/codegen/xai-grok-pager/docs/user-guide/29-local-enhancements.md#shell-backend)。
- **构建与测试**：`xai-proto-build` 修复了 protoc 在 Windows 上的 Unix 路径 panic；路径语义、目录身份、测试线程栈等一批环境族问题按 `docs-local/WIN-TEST-GATE.md` 的流水线处理，测试统一走 `scripts-local/ctest.sh`（自动注入 `RUST_MIN_STACK` / `TMP`，并按 `docs-local/win-skip.txt` 台账跳过已知必挂用例）。

```toml
[shell]
backend = "bash"   # pwsh | powershell | bash(Git Bash) | cmd；缺省自动级联
```

### 上下文与成本

- **`/stats` API 成本列**：用量卡片按模型显示本次窗口的美元成本，数据取官方目录价——models.dev 主源、OpenRouter API 兜底，只留一方原厂价（转售商整库不留）；配置里的裸网关 id、`vendor/model` 全串、`-20260826` 部署戳、`_`/`.` 分隔符变体都能按置信档对上同款原厂条目。价格目录落 `~/.grok/cache/prices.cache`（TTL 一天，`[model_catalog] cache_minutes` 可调），进程启动后台预热一次/天；取数失败宁用旧价，单源失败打 warn 日志（带生效源）。合计行剔除无价模型并标注条数。
- **工具输出压缩（实验，默认关）**：`[tool_output_compression] enabled = true` 后，对 bash / MCP 的**结果**做 headroom 风格压缩（JSON / 日志 / grep / diff），`exit_code` / `stderr` 无损保留；原输出存为 `<<ccr:HASH>>`，可用 `expand_output` 取回。开关在会话启动时快照。
- **状态行原生指标**：内置 `model` / `api-calls` / `tokens` / `cache` / `think` / `perf`，默认出行为 builtin（`in 47k out 3.2k`、`cache 95.7%`、`think 28.1%`、`✓ 12 ✗ 3`、`380ms ttft · 42.3 tok/s`）。也可写 `type = "command"` 脚本消费同一份 JSON payload。

```toml
[ui.status_line]
type = "builtin"
items = ["model", "api-calls", "tokens", "cache", "think", "perf"]
```

### LSP 语言服务器

- **开箱即用**：`lsp` 工具默认开启（上游 `[features] lsp_tools` 默认关），零配置可用。内置五个知名语言服务器的 PATH 自动检测——Rust `rust-analyzer`、Go `gopls`、Python `pyright` → `basedpyright` → `pylsp`（按序三选一）、Java `jdtls`、C/C++ `clangd`——装了哪个用哪个。显式配置恒压 builtin：`~/.grok/lsp.json`（用户）、`<cwd>/.grok/lsp.json`（项目，受目录信任门控）、插件贡献均可按名覆盖，且任一配置 server 认领了某语言的文件扩展后该语言的 builtin 自动让位。检测只查 PATH 候选不执行二进制；服务器在会话启动时后台拉起，崩溃自动重启并回放已打开文档。
- **统一 `lsp` 工具**：`goToDefinition` / `findReferences` / `hover` / `goToImplementation` / `documentSymbol` / `workspaceSymbol` 六个只读操作，按文件扩展路由到对应服务器。编辑 / patch 后的诊断被动回喂：多批诊断在 150ms 静默窗内合并成一次注入，连续编辑不重复 drain。
- **`/lsp` 命令**：裸 `/lsp` 打开统一操作面板（usage 同款窗口 chrome）——状态显示（门禁来源、工作区覆盖、server 清单、会话后端）、`e` 一键启用/关闭、server 自动发现清单（配置 + builtin PATH 检测）、空格勾选 + Enter 确认（取消勾选的写入本工作区禁用清单，只记排除项，空清单 = 恢复全量跟随）、`c` 清除本工作区全部覆盖（含禁用清单）恢复跟随全局；翻转、重置与确认对零回合会话即时热加载。无 option 子命令——启用/关闭/状态/勾选全部在面板内完成。**零回合会话写完立即热加载**——重建 harness 换 LSP 后端并重广播工具清单，无需 `/clear`；已有回合的会话报「热加载跳过」，改动对新会话生效。优先级：策略 pin > `GROK_LSP_TOOLS` > `/lsp` 工作区覆盖 > `[features] lsp_tools` > 服务端设置 > 默认开。

### 交互与界面

- **`/rewind` 分支树 undo**：rewind 从破坏性截断升级为分支树——被弃回合保留在会话文件里（append-only + 分支标记），`/rewind` 列表对旧分支点标 ↩、不可回退点标 ⚠，对旧分支点再次 rewind 即 redo 切回原路继续；跨 compaction 检查点回放语义一致。
- **中文界面**：斜杠命令、快捷键栏、设置/用量/MCP/扩展/会话选择/权限与计划审批等弹窗默认中文（1400+ 词条）。`/lang` 中英即时切换；`GROK_LANG=en` 固定英文。
- **主题目录扩容（社区色板移植）**：在上游 6 主题（GrokNight/GrokDay/TokyoNight/RosePineMoon/OscuraMidnight/Terminal）之外，从同类 coding agent（opencode 主题库）移植 11 套社区好评色板——Catppuccin Mocha/Latte（Latte 是第二个浅色主题）、Dracula、Gruvbox Dark、Nord、One Dark、Everforest、Kanagawa、Vesper、Night Owl、Solarized Dark。全部 ~60 个颜色槽位逐一映射（donor 未定义的 hover/visual 带、边框细档、plan 金、md 六级标题梯在其自身色系内手调派生），全部标 truecolor 门槛（256 色终端隐藏，同 TokyoNight 策略）；`/theme`、`/settings`、`auto_dark/light_theme` 与 config 校验自动收编。
- **欢迎屏定制**：熊猫头 logo、副标题与退出告别语；`28-welcome-branding.md` 是给 AI 看的复刻配方（换成你自己的 logo/文案）。
- **趣味等待文案**：思考/回复阶段按轮换词表随机取词（中英各一套），不再固定 "Thinking…"。
- **`/style` 极简输出风格**：与 agent 变体解耦的覆盖层——任意 agent 上叠加"答案先行、零旁白、多步编号、错误给 cause+fix"规则；状态持久化在 `<grok_home>/output_style.json`，首次模型调用前切换对本会话立即生效。
- **通知开箱**：`[notifications]`（`desktop` / `sound` / `min-interval-secs` / `suppress-after-user-input-secs`，全部 opt-in），走终端 OSC 9 / OSC 777 / BEL，无需写 hook。
- **`/agents` 弹窗列出全部内置变体**（含 `grok-build-concise`），不再有隐藏名单；`[agent] name` 作为会话默认 agent 会被尊重（不再被 plan 模式旗标静默覆盖）。

### 扩展与钩子

- **`BeforeModelCall`**：每次请求组装完成后、发送前改写消息列表（出去脱敏 / 回来还原），会话记录永不改动；fail-open，坏 hook 降级为 no-op。
- **`PostCompact`**：压缩后可通过 `additionalContext` 重注入状态（任务列表 / 记忆类扩展的恢复通道）。
- **workflow 可见面编辑**：spawn prompt 可经 `replace_visible(seq, digest)` / `hide_visible(seq)` 对早前子代理的可见输出做原位替换 / 隐藏（journal 直记、重放确定），迭代式 workflow 不再把历史全文反复塞进 prompt。
- **workflow 子代理工具面白名单**：`AgentOpts.tools` 按子代理角色显式裁剪声明面（canonical tool id，支持裸名），与 capability 过滤叠加只减不增；fan-out 场景省掉"每 agent × 全量工具面"的重复 token。deferred 延迟发现（`defer_tools` / `search_tools`）为后续切片。
- **headless 审批降级续跑**：无人值守回合遇权限审批阻塞默认终止；`--non-interactive-denial continue` 把阻塞审批转成 PolicyDeny 失败工具结果，agent 带着拒绝原因换路续跑。
- **本地命令**：`/auth`（供应商配置面板）、`/usage`（供应商套餐用量）、`/stats`（用量、成本与压缩收益）、`/lang`（中英切换）、`/style`（极简风格）、`/lsp`（LSP 工具启停与状态）、`/grok`（Grok 账号登录，原 `/login`）、以及 `grok2 stats` 独立子命令。

## 配置速查

| 配置 | 作用 |
| --- | --- |
| `[models] default` / `[model.<key>]` | 选模型、接第三方端点（见上一节） |
| `[models] hidden_models` / `disabled_models` / `allowed_models` | 收进选择器 / 移出目录 / 白名单 |
| `[model.<id>] use_proxy` | 单模型显式走进程出口代理（默认 false 恒直连） |
| `[model_catalog] cache_minutes` | 模型目录价格缓存时长（缺省 1440 = 一天；缓存文件 `~/.grok/cache/prices.cache`） |
| `[provider_usage] cache_minutes` | `/usage` 供应商套餐用量缓存（缺省 5 分钟） |
| `[features] lsp_tools` | `lsp` 工具总开关（默认开；`GROK_LSP_TOOLS` 等价，`/lsp` 可按工作区覆盖） |
| `[network] proxy` / `proxy_hosts` | 出口代理与 host 白名单（默认 `x.ai`、`grok.com`） |
| `[shell] backend` | `pwsh` \| `powershell` \| `bash`(Git Bash) \| `cmd`（仅 Windows；优先于 `GROK_SHELL`，改完需重启） |
| `[ui.status_line] type` / `items` | 状态行开关与段集合 |
| `[notifications]` | 桌面通知与提示音（opt-in） |
| `[tool_output_compression]` | 工具输出压缩（默认关） |
| `[auth] preferred_method = "api_key"` | 彻底不出现浏览器登录（fail-closed） |
| `[cli] auto_update = true` | 恢复自动更新（默认关） |

## 默认行为差异

相对上游，本 fork 的开箱默认值有五处不同，全部可改回：

| 项 | 本 fork 默认 | 恢复上游行为 |
| --- | --- | --- |
| 界面语言 | 中文 | `GROK_LANG=en`，或 `/lang` 切换 |
| 状态行 | 开启（builtin 行，6 个段） | `[ui.status_line] type = "disabled"` |
| LSP 工具 | 默认开启（builtin 自动检测，无需配置） | `[features] lsp_tools = false`，或 `GROK_LSP_TOOLS=0`，或 `/lsp off` |
| 自动更新 | 关闭 | `[cli] auto_update = true` |
| 工具输出压缩 | 关闭 | `[tool_output_compression] enabled = true`（这是本地新增特性） |

其余默认值与上游一致。

另有一处**命名差异**（不可配置）：`/login` 更名 `/grok`（Grok 账号登录），不留旧名。

## 内置文档与 skill

- **TUI 内文档**：`/docs`（或直接读 `<grok_home>/docs/user-guide/`）。其中 [`29-local-enhancements.md`](crates/codegen/xai-grok-pager/docs/user-guide/29-local-enhancements.md) 是本地增强总表（含"信号 → 该配什么"对照表），[`11-custom-models.md`](crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md) 是第三方模型全量说明。
- **skill `third-party-models`**：把"用第三方模型替代 grok 登录"的完整流程（机制、最小配置、常见坑速查、验证命令、本 fork 的额外能力）写成可被 agent 自动加载的 skill。放在 `~/.grok/skills/third-party-models/SKILL.md`，之后：`/third-party-models` 手动调用，或在你说"接自己的模型 / 不用 grok 账号 / 配 deepseek"时自动触发。
- **仓库内开发资料**：`docs-local/PATCHES.md`（本地补丁账本，上游同步后按表重放）、`docs-local/WIN-TEST-GATE.md`（Windows 测试门控与分诊流水线）、`docs-local/provider-onboarding-api-cost-todo.md`（`/auth` 面板 + `/stats` 成本列 + 模型目录价格的设计与拍板账本）、`docs-local/ui-i18n-plan.md`、`docs-local/tool-output-compression-plan.md`、`docs-local/agent-cli-tracking/`（同类 agent CLI 的调研笔记）、`docs-local/agent-memory-design/`（agent 软件记忆模块的专题调研：七家落地对照、工程取舍、优化点与演进方向，含 Reddit / HN 社区风评）、`docs-local/startup-perf/`（grok2 启动耗时专题：span 画像 + ptyctl 驱动的复测流程、冷/热缓存实测，主因 `fetch_models_blocking` 与缓解选项）、`docs-local/usage/`（供应商套餐用量专题：各家 Coding Plan 用量接口与凭据调研 + `/usage` 面板显示供应商套餐量的施工方案）。

## 从源码构建

要求：

- **Rust**：工具链由 [`rust-toolchain.toml`](rust-toolchain.toml) 固定，`rustup` 首次构建自动安装。
- **[DotSlash](https://dotslash-cli.com)**：让 [`bin/`](bin/) 下的 hermetic 工具（主要是 `bin/protoc`）自动下载运行；构建前装好并确保 `dotslash` 在 `PATH`。也可自备 `protoc`（Windows 上本机无 DotSlash 时，把真实 protoc 放进 `PATH` 即可，见 `docs-local/PATCHES.md`）。

```bash
cargo build -p xai-grok-pager-bin --release   # 产物 target/release/xai-grok-pager（发行时改名 grok2）
cargo run -p xai-grok-pager-bin               # 直接跑 TUI
cargo check -p <crate>                        # 快速验证，别整仓编
```

Linux 与 Windows 均由 CI 构建发行版（`.github/workflows/release.yml`，推 `v*` tag 触发；Windows 侧用 cargo-xwin 在 Linux runner 上交叉编译成 `windows-msvc`）。

## 开发约定

- **分支纪律**：`main` 只收文档类改动；功能 / 修复一律走 `feat/local-*` 分支，验证通过后合入 main 并按 `vX.Y.Z` 打 tag 发版。
- **同步文件**：上游同步会覆盖同步文件，对它们的本地补丁要在 `docs-local/PATCHES.md` 有据可查，便于同步后重放（本地插入行带 `// LOCAL:` 注释）。
- **Windows 测试**：一律走 `scripts-local/ctest.sh -p <pkg> --lib`（L0 白名单 + L1 环境注入 + L2 `win-skip.txt` 跳过），禁止裸跑 `cargo test`；上游套件级回归留给 Linux CI。
- 上游不接受外部 PR，本仓库只维护 fork 侧的增量。

## 来源与许可

- 上游：[xai-org/grok-build](https://github.com/xai-org/grok-build)；本树对应上游 commit 记录在 [`SOURCE_REV`](SOURCE_REV)。
- 第一方代码遵循 **Apache License 2.0**（见 [`LICENSE`](LICENSE)）；第三方与 vendored 代码保留原许可，见 [`THIRD-PARTY-NOTICES`](THIRD-PARTY-NOTICES)、[`third_party/NOTICE`](third_party/NOTICE) 与 [`crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md`](crates/codegen/xai-grok-tools/THIRD_PARTY_NOTICES.md)。
- 本仓库是**非官方**发行：不提供 grok 账号、订阅或额度；用第三方模型时请遵守各提供商的服务条款。
