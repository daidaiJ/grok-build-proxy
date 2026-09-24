# Grok Build 社区增强版（`grok2`）

[Grok Build](https://x.ai/cli)（`grok`）的社区 fork。上游是 SpaceXAI 的终端 AI 编程 agent，本仓库在保持上游全部能力的前提下，做了一批**本地增强**：让第三方 / 国产模型成为一等公民（不买 grok 订阅也能用）、中文界面、Windows 可用性修复、以及若干上下文字与交互上的改进。

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
3. **跑**：`grok2`。首次进入某个目录会有一次目录信任确认；除此之外**不会**弹浏览器登录，直接用。

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

### 网络与代理

- **`[network]` 出口代理**：进程级生效（模型请求、认证、模型目录都走同一条路径），按 host 后缀匹配；默认只代理 `x.ai` / `grok.com`，第三方端点与 loopback 直连。环境变量等价物 `GROK_PROXY` / `GROK_PROXY_HOSTS`。

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

- **工具输出压缩（实验，默认关）**：`[tool_output_compression] enabled = true` 后，对 bash / MCP 的**结果**做 headroom 风格压缩（JSON / 日志 / grep / diff），`exit_code` / `stderr` 无损保留；原输出存为 `<<ccr:HASH>>`，可用 `expand_output` 取回。开关在会话启动时快照。
- **LSP 诊断合并去抖**：多批诊断在 150ms 静默窗内合并成一次注入，连续编辑的 750ms 窗口内不再重复 drain，减少重复注入与阻塞。
- **状态行原生指标**：内置 `model` / `api-calls` / `tokens` / `cache` / `think` / `perf`，默认出行为 builtin（`in 47k out 3.2k`、`cache 95.7%`、`think 28.1%`、`✓ 12 ✗ 3`、`380ms ttft · 42.3 tok/s`）。也可写 `type = "command"` 脚本消费同一份 JSON payload。

```toml
[ui.status_line]
type = "builtin"
items = ["model", "api-calls", "tokens", "cache", "think", "perf"]
```

### 交互与界面

- **中文界面**：斜杠命令、快捷键栏、设置/用量/MCP/扩展/会话选择/权限与计划审批等弹窗默认中文（1400+ 词条）。`/lang` 中英即时切换；`GROK_LANG=en` 固定英文。
- **欢迎屏定制**：熊猫头 logo、副标题与退出告别语；`28-welcome-branding.md` 是给 AI 看的复刻配方（换成你自己的 logo/文案）。
- **趣味等待文案**：思考/回复阶段按轮换词表随机取词（中英各一套），不再固定 "Thinking…"。
- **`/style` 极简输出风格**：与 agent 变体解耦的覆盖层——任意 agent 上叠加"答案先行、零旁白、多步编号、错误给 cause+fix"规则；状态持久化在 `<grok_home>/output_style.json`，首次模型调用前切换对本会话立即生效。
- **通知开箱**：`[notifications]`（`desktop` / `sound` / `min-interval-secs` / `suppress-after-user-input-secs`，全部 opt-in），走终端 OSC 9 / OSC 777 / BEL，无需写 hook。
- **`/agents` 弹窗列出全部内置变体**（含 `grok-build-concise`），不再有隐藏名单；`[agent] name` 作为会话默认 agent 会被尊重（不再被 plan 模式旗标静默覆盖）。

### 扩展与钩子

- **`BeforeModelCall`**：每次请求组装完成后、发送前改写消息列表（出去脱敏 / 回来还原），会话记录永不改动；fail-open，坏 hook 降级为 no-op。
- **`PostCompact`**：压缩后可通过 `additionalContext` 重注入状态（任务列表 / 记忆类扩展的恢复通道）。
- **本地命令**：`/stats`（用量与压缩收益）、`/lang`（中英切换）、`/style`（极简风格）、以及 `grok2 stats` 独立子命令。

## 配置速查

| 配置 | 作用 |
| --- | --- |
| `[models] default` / `[model.<key>]` | 选模型、接第三方端点（见上一节） |
| `[models] hidden_models` / `disabled_models` / `allowed_models` | 收进选择器 / 移出目录 / 白名单 |
| `[network] proxy` / `proxy_hosts` | 出口代理与 host 白名单（默认 `x.ai`、`grok.com`） |
| `[shell] backend` | `pwsh` \| `powershell` \| `bash`(Git Bash) \| `cmd`（仅 Windows；优先于 `GROK_SHELL`，改完需重启） |
| `[ui.status_line] type` / `items` | 状态行开关与段集合 |
| `[notifications]` | 桌面通知与提示音（opt-in） |
| `[tool_output_compression]` | 工具输出压缩（默认关） |
| `[auth] preferred_method = "api_key"` | 彻底不出现浏览器登录（fail-closed） |
| `[cli] auto_update = true` | 恢复自动更新（默认关） |

## 默认行为差异

相对上游，本 fork 的开箱默认值有四处不同，全部可改回：

| 项 | 本 fork 默认 | 恢复上游行为 |
| --- | --- | --- |
| 界面语言 | 中文 | `GROK_LANG=en`，或 `/lang` 切换 |
| 状态行 | 开启（builtin 行，6 个段） | `[ui.status_line] type = "disabled"` |
| 自动更新 | 关闭 | `[cli] auto_update = true` |
| 工具输出压缩 | 关闭 | `[tool_output_compression] enabled = true`（这是本地新增特性） |

其余默认值与上游一致。

## 内置文档与 skill

- **TUI 内文档**：`/docs`（或直接读 `<grok_home>/docs/user-guide/`）。其中 [`29-local-enhancements.md`](crates/codegen/xai-grok-pager/docs/user-guide/29-local-enhancements.md) 是本地增强总表（含"信号 → 该配什么"对照表），[`11-custom-models.md`](crates/codegen/xai-grok-pager/docs/user-guide/11-custom-models.md) 是第三方模型全量说明。
- **skill `third-party-models`**：把"用第三方模型替代 grok 登录"的完整流程（机制、最小配置、常见坑速查、验证命令、本 fork 的额外能力）写成可被 agent 自动加载的 skill。放在 `~/.grok/skills/third-party-models/SKILL.md`，之后：`/third-party-models` 手动调用，或在你说"接自己的模型 / 不用 grok 账号 / 配 deepseek"时自动触发。
- **仓库内开发资料**：`docs-local/PATCHES.md`（本地补丁账本，上游同步后按表重放）、`docs-local/WIN-TEST-GATE.md`（Windows 测试门控与分诊流水线）、`docs-local/ui-i18n-plan.md`、`docs-local/tool-output-compression-plan.md`、`docs-local/agent-cli-tracking/`（同类 agent CLI 的调研笔记）。

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
