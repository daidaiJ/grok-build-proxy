# 插件与 harness 内核调研（2026-11）：opencode Code Mode / zcode v4 / dsh

> **2026-11-28 范围修订：** 本文以插件体系与 harness 内核为主体。agentic loop 与 token 成本的
> 专项结论、成本算式与优先级见
> [`../agentic-loop-token-cost-2026-11/README.md`](../agentic-loop-token-cost-2026-11/README.md)。
> 本文 §5 的 P0 四项按成本口径属于正确性/UX，不计入成本 P0。
>
> **状态：调研入库，未开工（2026-11-28）**。范围是三家的插件体系与 harness 内核，
> 含 opencode 基于解释器的 Code Mode、zcode v4 的插件矩阵与内核契约、dsh 的 Cordis
> 微内核与 PTC。TUI/桌面视觉、模型能力适配只在与内核相关时提及。
>
> 基线：opencode `v2.4.2`（2026-11-26）+ `dev` head `eeeeaa0db3c1`（2026-11-28）；
> zcode `v4.0.0`（本地 `a4b44451`，2026-11-23）；dsh `v0.2.3-alpha.2`（本地
> `4b399c39`，2026-11-22）。
>
> 取证方式：opencode 本地工作树停在 `v2.0.32`，本轮新文件经 GitHub API 按
> `v2.4.2` tag 与 `dev` 提交读取，并已浅取 `v2.4.2` tag 供本地复核；zcode、dsh
> 本地克隆已 fast-forward 到上述 head。三家均未做运行时实测，结论来自源码、设计文档
> 与 release notes。

## 0. 结论先行

- **opencode 把 Code Mode 做成了产品化的工具调用层**：模型写一段受约束的
  JavaScript 风格程序，程序里通过 `tools.*` 调工具；语言层已拆成独立的
  `psychoscript` 包，自带字节码 VM、挂起/恢复、幂等/持久工具，宿主通过工具目录和
  `search(...)` 控制模型可见面。配套插件 API 覆盖 server、TUI、GUI 三条运行时。
- **zcode v4 的插件体系以“能力投影”为主**：Claude Code 插件兼容层、MCP Apps UI
  Plugin、Gen UI、桌面 Extension Host、workspace 级启停与信任门，分别把插件内容投影到
  既有能力域；运行时代码集中在 Extension Host 与 MCP server 两类边界。内核侧的重点是
  prompt snapshot、工具 schema 字节稳定、`is_error` 冷恢复和 MCP/skills 动态目录。
- **dsh 把“一切皆插件”推进到内核**：Cordis 提供 service/typed event/reversible
  effect，profile/bundle/patch 负责组合，插件管理器与 HMR 负责生命周期；PTC 用
  TypeScript `run_code` + 生成 SDK 实现程序化工具调用，和 opencode 的 Code Mode 是
  同一类问题的两种工程解法。
- **本仓近期值得拿的四个内核件（正确性/UX 口径）**：工具结果 `is_error` 全链路、
  工具目录/指令的增量渲染、按 key 的会话级指令条目、Hook 上下文与失败路由。
  Code Mode 适合作为主循环的独立 `execute` 工具重新评估，先划清与 workflow journal 的边界。
  这四项在 99% 缓存命中下不改变主要成本；成本优先级见上述专项文。

## 1. opencode：Code Mode 与插件运行时

### 1.1 版本线索

| 版本 | 日期 | 与本题相关的变更 |
|---|---|---|
| `v2.2.0` | 2026-10-27 | Desktop GUI plugins；`ctx.session.instructions.entry.put/list/remove` 按 key 管理会话指令，存活于压缩/回滚，下一步以小增量注入，不重建 system prompt；插件事件按类型过滤；等插件加载完再返回 agent/provider 列表；`execute` 被拒时 MCP 工具直接暴露；TUI OpenTUI 0.7 |
| `v2.3.0` | 2026-11-13 | 后台 subagent 默认；freeform 工具输入；`session.fork`；`ToolContext.signal`；`@opencode/plugin/effect` 导出 `Tool`；插件清理失败不再阻塞替换；Code Mode 命名空间当工具调用时给建议 |
| `v2.4.0` | 2026-11-23 | 内置桌面浏览器；automations；视频生成；插件按 range/tag 自动更新（`name@latest`、semver、Git 分支）；插件在后台加载；Code Mode getter/setter 与错误栈；`full_codemode` 下可调用 `session.interrupt` 停止其他会话、后台 job 和 subagent；assistant part 按行持久化；`options.codemode` 可给 Code Mode 形态单独 description/input/output；Hook 收到 `info` 与 `codemode` |
| `v2.4.2` | 2026-11-26 | GPT-6.2 系统提示；`generate_image`/`generate_video` 移入 Code Mode，每次请求少约 700 token；automations 失败后继续调度；Promise 插件调用支持 `signal` |

### 1.2 Code Mode 的执行模型

**定位。** `packages/codemode` 是一个围绕工具调用设计的轻量纯解释器，语言层是
`psychoscript`。它只运行自己实现的语法子集，程序不能直接访问网络、文件系统、进程或
应用 API；外部世界只能经宿主提供的工具和 extension globals 进入。未完成的异步工作在
程序结束时被中断；最后一条顶层表达式按 REPL 语义成为结果；不支持的语法返回带位置的
`UnsupportedSyntax` 诊断。

**工具与目录。** `Tool.make({ input, output, execute, freeform })` 定义工具；input/output
可以是 Effect Schema，也可以只是给模型看的 JSON Schema。嵌套 record 形成 namespace，
`Namespace.make` 可补描述。`CodeMode.make()` 返回可复用 runtime，`runtime.catalog()`
给出每个工具的精确 path、描述和生成的 TypeScript 签名；`search(...)` 是同步内建，支持
精确 path、namespace、空查询浏览和分页，结果可直接用、await 或链式 `then/catch/finally`。

**边界与限制。** 每次执行返回 `Result`，含 value/logs/warnings/truncated/toolCalls 和
诊断类型（`ParseError`、`UnknownTool`、`InvalidToolInput`、`ToolCallLimitExceeded`、
`TimeoutExceeded`、`ToolFailure` 等）。限制项 `timeoutMs`、`maxToolCalls`、
`maxOutputBytes` 默认都是 unlimited；timeout/工具调用超限产生对应诊断，输出截断则保留
成功结果并带 in-band 标记。
工具参数跨边界按数据复制，支持 plain data、Date、Map、Set、URL、Headers、Error；
函数或 Promise 必须先 await 或转成数据。

**挂起与恢复。** `runtime.program(code, bindings?, { runId })` 创建程序，`run()` 跑到结束
或挂起。每次宿主调用有稳定 key `<runId>:<callId>`，同时传给 tool execute 和 hook。
`program.suspend()` 把状态存成 JSON snapshot，可在同版本、同 host globals 的另一进程
`runtime.resume(...)`。工具分三类：

- 普通工具：执行期间拒绝挂起；
- `Tool.idempotent`：挂起时中断，恢复后用同一 key 重跑，不重复计入 `maxToolCalls`；
- `Tool.durable({ start, wait, cancel })`：`start` 返回 JSON data 后可挂起，恢复时重新
  `wait`，真正取消时才 `cancel`。

**扩展与 OpenAPI。** `Extension.make` 把宿主函数挂成程序全局（如 `fetch`），可标记
`Extension.async`、`Extension.constructible`，可用 `snapshot.encode/decode` 把函数结果
保存过挂起。`import`/`require` 按 Node 模块名解析到宿主 globals，不加载 npm 包。
`OpenAPI.fromSpec` 把 OpenAPI 3.x 操作转成工具，点号 `operationId` 形成 namespace，
不支持的参数编码、非 JSON body、WebSocket/SSE、二进制响应进入 `skipped`，认证由宿主
解析，不暴露给模型。

**主循环接入。** `packages/core/src/codemode/instructions.ts` 注入 `# Code Mode` 段：
列出的工具只能在 `execute` 里的 JavaScript 中调用，其他工具保持直接调用；目录不完整时
提示模型先用全局 `search(...)`，并强调结果里的 `path` 是完整调用表达式。目录更新按
delta 渲染，`UPDATE_BUDGET = 1000`，只重述新增/变更/删除的 namespace 和工具，新增签名
共享预算，超出部分提示“还有 N 个，用 search”。`packages/core/src/codemode/tool.ts`
构造保留名 `execute`，描述为“在受限 Code Mode runtime 里运行 JavaScript”，输入是
freeform `{ type: "text", field: "code" }`；嵌套工具调用各自有 `CallID`，共享权限和
上下文，标记 `codemode: true`。

**full_codemode。** 该模式额外提供 `Bun.file`/`Bun.write`/`Bun.Glob` 和
`timeout`/`background` 输入；脚本可转后台，完成时发 synthetic notification；后台 job
带恢复元数据 `{ kind: "codemode", sessionID, jobID }`。默认 Code Mode 仍然只有宿主
工具和 extension globals；`full_codemode` 是显式放开的文件能力层。模型策略在该模式下
隐藏 `glob`/`grep`/`webfetch`，让程序通过 shell/fetch global 完成搜索与抓取。

**自动化里的静态扫描。** `packages/codemode/src/codemode/scan.ts` 静态提取
`tools.<path>` 调用，按首次出现顺序返回；拒绝语法错误、动态下标、把 `tools` 别名传递、
`import`/`require` 工具。automations 用它判断计划代码会碰哪些工具，不执行代码。

**最新 dev 增量。** `4eba154a`（2026-11-28）给工具加 `context.tool.call(name, input)`：
工具可以调用其他可达工具，形成嵌套调用；嵌套调用有自己的 `CallID`，权限与父调用绑定，
共享进度与取消，Hook 和 `CodeModeTool.Parent` 可见，并新增 `Tool.Calls` 类型。这个能力
把“程序化调用”从 Code Mode 扩展到普通工具内部。

### 1.3 插件 API

`@opencode/plugin` 2.4.2 的入口按运行时拆分：`.`（Promise）、`./effect`、`./host`、
`./runtime`、`./tui`、`./gui`、`./gui/main`、`./gui/host`、`./gui/builtin` 等。插件用
`Plugin.define({ id, setup })` 注册，`ctx.options` 读配置，setup 可返回同步或异步清理
函数；注册可单独 `dispose()`。

**Transform hooks** 按域提供编辑器：`agent`、`command`、`integration`、`mcp`、`model`、
`provider`、`reference`、`skill`、`tool`、`vcs`、`websearch`。编辑器是同步的，异步数据
要在注册 transform 前加载；数据变化后调用对应域的 `reload()` 重跑 transform。

**Runtime hooks** 拦截实时操作：

- `ctx.aisdk.hook("sdk" | "language")` 替换 AI SDK 实例或 language model；
- `ctx.session.hook("context")` 在 provider dispatch 前改工具描述、删工具；
- `ctx.session.hook("retry")` 改重试决策；
- `ctx.session.hook("experimental.idle")` 在会话空闲前决定是否注入 synthetic 输入，
  `event.background` 列出仍在跑的 shell/subagent；
- `ctx.tool.transform` 添加插件自有工具，`options: { codemode: false }` 让工具保持直接
  调用；
- Effect 侧的 session hooks 还覆盖 `fork`、`prompt`、`compaction`、`generate`、`title`、
  `model.request`、`http.request/response`、`experimental.ws.*`、`experimental.idle`。

**Hook 域与失败语义。** `packages/core/src/plugin/hooks.ts` 把 hook 分成 `aisdk`、
`session`、`permission`、`shell`、`tool` 五个域；只有 `tool.execute.before` 能以
`Tool.Error` 失败。`permission.evaluate` 可改 effect/message，`shell.create.before` 可改
command/cwd/timeout/shell/env。`tool.execute.before/after` 事件带 session/agent/message/
call ID、input、`codemode` 标志和工具定义 `info`；嵌套调用的权限绑定父调用。

**宿主与热重载。** `packages/plugin/src/host.ts` 解析插件的 `server`、`tui`、`rpc`、
`gui-experimental` 入口，按 package.json 或目录名给 id；`runtime.ts` 让插件共享宿主的
`effect` 和 `@opencode/plugin` 模块实例，拒绝插件 `node_modules` 里的副本；
`source.ts` 对插件文件/目录做指纹，只重载变化的依赖图。v2.4.0 起支持自动更新 range/tag
和异步加载，清理失败不再阻塞替换。

**GUI/TUI 插件。** 桌面与 Web 的非核心功能全部是内置 GUI 插件，第三方使用同一个
`@opencode/plugin/gui` SDK：`Plugin.define` 声明 `os`、`stores`、`icons`、`i18n`，
window 入口和 main 入口分离，`ctx.layout/sessions/storage/desktop`、`ctx.use/provide`
契约、`ctx.add` 注册表、`ctx.main(Ipc)`，SolidJS 渲染。TUI 侧有 `./tui` 入口和 OpenTUI
依赖。`packages/plugin-browser/README.md` 把桌面浏览器经 Code Mode 工具暴露：server 持有
工具、调用作用域和权限，desktop 持有 tab/CDP/evaluate/文件，browser skill 在桌面浏览器
接入前隐藏。

**会话级指令条目。** v2.2.0 的 `ctx.session.instructions.entry.put/list/remove` 是内核
侧最值得注意的插件面：插件按 key 挂指令，条目存活于压缩与回滚，下一步以小更新注入，
不重建 system prompt、不额外起一轮模型调用。它把“插件想长期影响行为”与“前缀缓存不能
被打破”这两个目标拆开了。

### 1.4 内核侧变化

- **工具 schema 只转换一次**：v2.2.0 的 `#58798`/`#58848`，以及 dev `804b0f23`
  （2026-11-27）继续收敛重复转换；工具目录和指令按 delta 渲染，避免每次请求重发全量。
- **freeform 工具输入**：v2.3.0 支持模型直接输出字符串，Code Mode 的 `execute` 用它
  接收 `code`，省掉 JSON 包装和转义。
- **模型策略决定工具面**：`packages/core/src/tool/model-policy.ts` 按模型选择 `patch`
  或 `edit/write`，`full_codemode` 下隐藏 `glob/grep/webfetch`；Code Mode 工具与直接工具
  的划分由 session 模型策略决定（`#64515`）。
- **插件加载与列表**：v2.2.0 曾让 agent/provider 列表等插件加载完再返回；v2.4.0 起
  SDK 端恢复后台加载，启动后列表可能短暂为空或不完整，长连接客户端按 `*.updated`
  事件刷新。插件生命周期与目录读取解耦，避免启动被慢插件拖住。
- **后台 job 与恢复**：`packages/core/src/codemode/job.ts` 给 `full_codemode` 脚本提供
  2 分钟阻塞阈值、后台执行、恢复元数据和完成通知；这使程序化调用能覆盖长任务，而不必
  把长任务塞进一次模型请求。
- **MCP 工具在 `execute` 被拒时直连**：v2.2.0 的兼容路径保证“策略不允许 Code Mode”时
  工具仍可达；`packages/core/src/tool.ts` 的 `codeModeAvailable(policy)` 以 `execute`
  是否被 deny 为准。

### 1.5 评价

opencode 的强项是把 Code Mode 做成了**可运营的工具面**：目录、搜索、签名、预算、诊断、
挂起、持久工具、后台 job、权限与 Hook 都有产品级接口。它的语言层是自研子集，安全边界
清晰，代价是语言覆盖与维护成本。插件侧的强项是**运行时域拆分**：server/TUI/GUI 各自
有 API，transform 与 runtime hook 分工明确，宿主模块共享和热重载做得细。对本仓的直接
可借点集中在工具目录/指令增量、会话级指令条目和 Hook 上下文；整套 JS 运行时的移植
优先级较低。

## 2. zcode v4：插件矩阵与内核契约

### 2.1 插件类型

zcode v4 的“插件”是多种能力包的集合，按宿主能力域投影：

| 类型 | 形态 | 运行边界 | 证据 |
|---|---|---|---|
| Claude Code 兼容插件 | `.claude-plugin/plugin.json` + `.zcode-plugin/plugin.json` 双清单；skills、commands、MCP servers、hooks、agents | 内容投影到既有能力域；hooks 从所有启用插件注入；P0 未做 per-plugin trust | `apps/zcode-cli/docs/design/v2/plugin-compat.md` |
| 官方内置插件 | workspace package，构建后 seed 到 `~/.zcode/cli/plugins/cache/zcode-plugins-official/...` | CLI 从 cache 启动 MCP server；用户只启停，不从 workspace 路径执行 | `design-builtin-plugin.md` |
| UI Plugin | 插件 MCP server + MCP Apps 页面；工具 `_meta.ui.resourceUri`、清单 `ui.surfaces`、`app.callServerTool()`、`updateModelContext()`、`sendMessage()`、`createSamplingMessage()` | 桌面本地工作区；页面与宿主经 MCP Apps 通信；Web/手机/远程工作区保留普通 MCP 工具记录 | `UI_PLUGIN.md` |
| Gen UI | Agent 按问题生成 HTML/JS 页面，宿主注入 `window.zcode` | 页面在会话专用输出目录，宿主加载；不提供 `callTool`/MCP 资源/Node/任意文件；状态按 workspace+session 隔离，`setWidgetState` 上限 16 KiB | `UI_PLUGIN.md` §Gen UI |
| Extension Host | 插件清单 `extension` + 顶层 `slots`；一个 App 唯一 utility process | 扩展代码在独立进程，经 Window Host 的 `extensions-service` 访问 App 能力；不按插件隔离进程；远程 workspace 不跑扩展代码 | `docs/plugins/extension-host.md` |
| Workspace 级插件 | `<workspace>/.zcode/config.json` 的 `plugins.enabledPlugins`；选项存私有文件 | 仓库只决定启停，`plugins.options` 一律忽略；`plugins.dirs` 只产生待信任候选 | `workspace-level-plugin-architecture.md` |
| 插件市场/发现 | 官方/个人 marketplace、官方 seed、agent plugin discovery（BM25 搜索 + 推荐卡） | 安装/更新/卸载是 Host User inventory；推荐不自动安装，最多一次 mutation | `agent-plugin-discovery.md`、`plugin-market-interactions-scope.md` |

**Claude Code 兼容层的边界。** P0 只投影 skills、custom commands、MCP servers、hooks、
agents 五类；高风险的 Claude 插件特性先识别、诊断，等 zcode 长出对应运行时再支持。
hooks 的信任判定集中在 adapters plugin loader 的 `canRunPluginHooks`，当前 per-plugin
trust 未实现。这个做法把“生态兼容”和“任意代码扩展”分开了。

**Extension Host 的结构。** 一个 App 一个扩展进程，Electron Main 管生命周期，Window
Host 管业务服务，扩展进程与 Window Host 之间用一对 `MessageChannelMain` 直连，跑
`@zcode/rpc` 的 `extension-host` 与 `extension-window` 两个频道。能力只经能力表的
`call`/`subscribe` 两个通用入口，不按能力加 RPC 方法；能力网关统一做查表、校验、授权、
审计、分派。插槽由壳层组件渲染，扩展只提供标题、图标、排序和动态值（text/tooltip/
visible），不接受插件 HTML/SVG/样式。扩展 API 版本由 `EXTENSION_API_VERSION` 管，
插件清单 `extension.engines.zcode` 必须满足；`@zcode-dev/plugin-api` 1.4.0 是纯类型包，
版本与宿主 API 版本一致。当前 `packages/shared/src/extension-host/manifest.ts` 的
`EXTENSION_API_VERSION` 为 `1.4.0`；同目录 `CONTRACT.md` 仍写 `1.3.0`，属于文档滞后。

**Workspace 级启停与信任。** 解析顺序是
`workspace.plugins.enabledPlugins[pluginId] ?? user.plugins.enabledPlugins[pluginId] ??
manifest.defaultEnabled`；显式 false 是有效覆盖。仓库里的 `plugins.options` 被忽略，
普通 option/credential/secret 只存用户私有文件；`plugins.dirs` 里的项目目录插件默认
待信任，用户在设置页逐个确认后才在新会话加载。已安装的 User/Marketplace 插件没有
workspace trust gate；打开项目不会自动安装插件；workspace 配置引用缺失 package 时显示
`missing`，不自动下载。

**官方 seed 与发现。** 官方插件在构建期 seed 到 CLI cache，运行时从 cache 执行 MCP
server，workspace 路径只作为源码，不进入执行。agent plugin discovery 用 BM25 检索官方
目录，按显式需求返回推荐卡；卡片确认后才安装/启用/恢复，跳过可恢复，双端点击只接受
first-answer-wins，不自动安装。

### 2.2 内核契约

**System prompt contract 与 prompt snapshot。** zcode 把主 agent 的系统提示分成稳定的
`system[0]` CLI 前缀、静态 `system[1]`、动态 `system[2]`；工具 schema、项目指令、skills、
当前日期放在默认 system blocks 之外，作为 history-backed attachments。主会话发送
prompt snapshot 里记录的系统块；当前代码实时渲染的文本只在 snapshot 缺失或失效时
使用。snapshot 只在 provider/model 匹配时使用，成功压缩或会话模型切换后清除并重建，
fork 复制父会话的 snapshot，rewind 不改。这样新版本的系统提示只会在下一次成功压缩或
模型切换后进入已有会话，避免中途改前缀导致缓存失效。

**工具变更链。** 工具 schema、顺序和描述是固定字节，直接进入缓存前缀：
`@zcode/contracts` 的 `toToolJsonSchema` 统一用 zod 4 生成 draft 2020-12，根节点固定
`type: "object"`，union 用 `anyOf`，元数据键在前；工具名单按 en-US 名字排序，名单外按
注册顺序，MCP 工具按名字排序排最后。工具描述只写已支持能力，WebSearch 不写真实月份。
升级后第一个请求 tools 段缓存失效一次，已发送历史不改写。

**`is_error` 持久化。** 工具正常返回但输出是错误时，part 状态仍是 `completed`；live
请求的 tool_result 带 `is_error: true`，冷恢复原来按成功回放。v4 在结算时写
`metadata.providerIsError: true`，冷恢复按它还原 `is_error`；Hook 选
`PostToolUseFailure` 与 tool_result 的 `is_error` 用同一判定（Bash 非零/超时/取消、
MCP `isError: true`）。Hook 的 additionalContext 不再拼进 tool_result，而是写成
`tool_hook_context` 附件，排在该批 tool result 之后，持久化后 resume 位置不变。

**MCP 与 skills 动态目录。** v4 增加 MCP `config-loader`/`config-source`、连接池、
`tool-catalog-cache`、resource subscriptions、transport errors；skills 增加
`catalog`/`scan`/`roots`，支持 live catalog。MCP 工具名以 `mcp__` 开头并排最后，目录
缓存与连接状态解耦，避免连接顺序改变 tools 段字节。skills 目录变化按指纹回放，只补
缺失项，减少整段重发。

**会话存储与压缩。** session store 走 SQLite baseline + migrations，消息/会话/usage/
permission 分 repository；补充了 read-state hash resume、compaction/microcompaction
的持久化字段。压缩仍走“重写历史”路线，和工具结果的 `providerIsError` 一起保证冷恢复
后的请求字节与 live 一致。

### 2.3 评价

zcode 的插件体系覆盖面最广，但每一层都尽量把插件限制在既有能力域：Claude 兼容层只做
五类投影，UI Plugin 走 MCP Apps，Extension Host 走独立进程 + 能力表，workspace 级只
给启停和信任门。它对本仓最有价值的是**内核契约**：prompt snapshot 的冻结/重建时机、
工具字节稳定与排序规则、`is_error` 的 live/冷恢复一致性、Hook 上下文与 tool_result
分离。这些都能在 Rust 侧独立落地，不依赖 zcode 的插件运行时。

## 3. dsh：Cordis 微内核与 PTC

### 3.1 Cordis：一切皆插件

dsh 的架构说明把 Cordis 描述为“插件向共享 context 贡献 service、typed event 和
reversible effect”；模型适配器、工具注册表、会话日志、agent loop 本身都是插件，可以
从配置替换，没有需要打补丁的特权核心。注册是 effect，插件卸载时回滚。

**组合模型。** 一个运行中的 dsh 是启动时按有序层组装出来的插件树：

- **profile**：Harness home 里的命名组合，列出 bundle、外部插件和用户自己的
  `cordis.patch.yml`；内置 `web`、`headless`、`sdk`、`sdk-minimal`、`acp`。
- **bundle**：Cordis 配置行和挂载代码的分发格式，插入的内容仍可被上层 patch。
- **patch**：按 entry id 替换整行配置或插入新行；`preset` patch 在声明内部的
  `config.plugins` 上应用同样的层优先级。

层顺序是 profile 的有序 bundle → profile patch → home patch → `--patch` overlay。
HMR 由 YAML 控制：base 开 config-only `dsh-hmr`，headless/SDK/ACP 关闭，`sdk-minimal`
不包含。Plugin Manager 在 Web 与 agent 中可用；插件开关只改 profile patch 里最后一个
匹配 override 的 `disabled`，没有差异时不写。

**核心包与 turn flow。** `core/session` 是 append-only `SessionEvent` 日志，
`core/system-prompt` 负责 prompt section 与工具 schema 组装，`core/tools` 是 scoped
registry + guarded execution pipeline，`core/agent` 是 Agent 接口与 live registry，
`core/agent-loop` 是默认驱动，`core/scope` 是 per-agent scoped registration 原语。
一个 step 是一次模型请求加它调用的工具；turn 是零或多个 step。循环按
`turn/start → claim → assemble prompt → agent/pre-step → step/start → agent/request →
commit system/message → stream → tool/call* → tools/pre-execute → tools/execute →
tools/post-execute → step/end` 推进；`agent/pre-step`、`agent/request`、`llm/stream`、
`tools/*` 是 waterfall，`agent/turn-stopping` 是串行无 next。

**动态 Cordis 包。** extensions 子系统允许 agent 定义带版本的 Cordis package，运行
host 和 browser 两半，并在写代码前查询已批准的 runtime metadata。`ctx.dynamicCordisRunner`
提供 `define`、`undefine`、`run`、`runHostHalf`、`getClientCode`、`resolveRequestRun`、
`settleUserRun`、`stop`、`inventory`、`snapshot`；动态包在 `node:vm` 中运行，安装/运行
走审批，host 代码在 workspace sandbox 外以 in-process 方式执行。UI 侧用 slot 注册表
（`root` 是唯一内建声明），注册跟随 Cordis effect 生命周期，dispose 时递归收起子 slot；
`ctx.cordisInspect` 提供 host/browser 两侧的只读 inspect provider。

### 3.2 PTC：程序化工具调用

**入口与 SDK。** `@deepseek-ai/dsh-tool-executor-ptc` 在挂载兼容 PTC runtime 与 tool
registry 后，只把 `run_code` 暴露为直接工具；其他工具由 scoped restrictions 生成
TypeScript SDK，程序里用 `await tools.name(args)` 调用。`run_code` 参数是 `code`（async
TypeScript 函数体，erasable syntax，类型标注只作提示，运行前剥掉）和 `description`。
SDK 声明 `inspect(...values)`，只把选中的值/图片放进结果；其他中间值留在执行环境。

**调度与预算。** 根 `run_code` 调用串行且互斥，`maxParallelSubCalls` 控制每个程序内
并行安全子调用（默认 10，1 为串行）；`maxSubCallsPerMinute` 默认 1024，每次 `run_code`
的允许调用数是 `floor(timeoutMs / 60000 × maxSubCallsPerMinute)`，默认 120s 即 2048。
这是总配额，不是滚动窗口；队列中的调用、非法参数、失败工具都计数，下一次尝试会中止
整个程序、取消活跃子调用、丢弃排队调用，返回 `CODE_RUN_FAILED`。`timeoutMs` 默认
120,000ms、上限 600,000ms，包含嵌套工具和审批等待；更宽的 `sandbox_permissions` 需要
非空 justification 并在程序启动前审批。

**结果与图片。** `run_code` 结果按 emission order 输出 text/image；text 保留换行，相邻
text 直接拼接。宿主 binding 返回 `PtcImage`（attachment + execution-world path），程序
看到 `Image`（path、width、height）。`PtcInspectableValue` 把 lossless JSON 值与宿主
文本关联，直接 inspect 未修改的容器时显示宿主文本，字段/副本/嵌套值走普通格式化。
输出超限时 `outputTruncated: true`，正常完成的程序仍是成功结果，附截断提示；失败保留
原错误类型与截断提示。

**运行时。** Node provider 支持 per-call（每次全新）、persistent-in-memory（默认，按
live Agent 保留变量）、Script 模式 `scriptPersistence: true`；Agent dispose 时释放。
PTC 模式要求 `ctx.ptcRuntime.language === 'typescript'`，不满足时在 prompt assembly 和
执行阶段拒绝，不回退 native 工具。`/run-code` 的 `createRunCodeTool` 允许插件注册私有
binding 的程序工具，私有定义不进 tool registry，嵌套调用仍走正常审批、guard、dispatch
log、截图、deferred context 和 turn completion。

**token/cache 影响。** PTC 的固定每请求成本与可见定义成正比：用生成 SDK 文本 + 一个
transport schema 换掉端工具 schema，不承诺普遍下降；调用配额不增加 prompt/schema 文本。
前缀在 PTC 模式、生成 SDK、transport schema、可见工具集不变时稳定；模式或过滤变化会从
第一个变化的 prompt/schema token 起失效。中间值 execution-local，不能从 session replay
重建，只有外层 `run_code` 结果有硬上限。

### 3.3 评价

dsh 的微内核在“可替换性”上最彻底：配置层、插件生命周期、事件、作用域、HMR、插件管理
器形成一套完整组合模型；代价是概念多、启动链长、对使用者的 Cordis 理解要求高。PTC 的
工程完成度高，尤其是生成 SDK、`inspect`、调用预算、持久 REPL、图片描述符和取消语义；
它把程序化调用做成一种**执行器**，语言本身由 TypeScript 提供。对本仓而言，dsh 更适合
作为“插件边界与程序化调用”的参考架构，完整 profile/bundle/patch 组合模型的移植成本
过高。

## 4. 横向对照

| 维度 | opencode | zcode v4 | dsh | 本仓现状 |
|---|---|---|---|---|
| 插件单元 | npm 包，`Plugin.define`；server/TUI/GUI 分入口 | Claude 兼容包、官方内置包、UI Plugin、Extension | Cordis package；profile/bundle/patch 组合 | hooks（命令/HTTP，7 事件 fail-open）、marketplace、MCP、skills |
| 插件代码执行 | in-process，共享宿主 `effect` 模块 | Extension Host 独立 utility process；UI Plugin 走 MCP server | in-process Cordis；动态包 `node:vm`；host/browser 两半 | 事件钩子；无运行时可编程 API |
| 工具面/程序化调用 | Code Mode `execute` + psychoscript；嵌套调用、挂起/恢复、durable | 未见通用 `execute/run_code` 形态；UI Plugin 页面可调 MCP 工具 | PTC `run_code` + TypeScript SDK；持久 REPL、预算、沙箱 | 直接工具调用 + Rhai workflow（编排者脚本） |
| prompt/cache 纪律 | 工具目录/指令 delta；schema 一次转换；会话指令条目 | prompt snapshot；工具字节稳定/排序；`is_error` 冷恢复 | system-prompt section + systemUpdates；PTC 前缀稳定 | replay diet、P1 压缩、deferred tools；缺 keyed 指令条目 |
| UI 扩展 | GUI plugins（SolidJS/Electron/Web）、TUI plugins | UI Plugin（MCP Apps）、Gen UI、Extension slots | Web/Desktop 插件、UI slots | TUI；无插件 UI 运行时 |
| 会话持久化 | 事件/part 存储；后台 job 恢复 | SQLite session store + prompt snapshot + `providerIsError` | append-only SessionEvent 日志 | chat-state JSONL；账本 |
| 安全边界 | 纯解释器；`full_codemode` 显式放开 Bun 文件 | 进程/能力表/workspace trust；UI 页面无 Node | Cordis effect + 审批 + `node:vm` + sandbox policy | OS 沙箱、bash 静态分析、hooks fail-open |
| 与本仓的最近邻 | Code Mode 工具面 + 插件 API | 内核契约 + workspace 插件治理 | 插件组合 + PTC 执行器 | — |

## 5. 对本仓的取舍

### 5.1 先做（正确性/UX）：四个内核件

> 本节按 wire 正确性与 UX 排序。成本口径下，`is_error`、指令条目与 Hook 上下文不改变
> 主要 token 支出；成本 P0/P1 见
> [`../agentic-loop-token-cost-2026-11/README.md`](../agentic-loop-token-cost-2026-11/README.md)。

**P0-1 工具结果 `is_error` 全链路。** 这是 zcode v4 已经落地的契约，和插件无关，收益
直接落在 Anthropic wire、冷恢复和 Hook 失败路由上。落地面：

- `xai-grok-sampling-types` 的 `ToolResultItem` 增 `#[serde(default)] is_error`，
  Anthropic `ContentBlock::ToolResult` 增 `is_error: Option<bool>`；
- `xai-grok-shell` 的中心成功/错误路径用 `ToolOutput::is_error()` 落位；
- `xai-chat-state` 持久化 `providerIsError`，冷恢复按它还原；
- `is_mcp_error_result` 改成同一谓词，`PostToolUseFailure` 与 tool_result 共用判定。

这与 `docs-local/zcode-v4-port/README.md` 的 R2 是同一件事，可直接并线，避免重复设计。
验收按该文：Bash `exit 1` 的 live Anthropic body 带 `is_error: true`，冷恢复字节一致，
旧 JSONL 默认 false，Hook 路由覆盖 Bash/MCP。

**P0-2 工具目录与指令的增量渲染。** opencode 的 `UPDATE_BUDGET = 1000` 和 zcode 的
tools 段字节稳定可以合成一套：

- 工具 schema 只转换一次，按稳定顺序（名字 en-US 排序，MCP 最后）序列化；
- 目录变化时只发新增/变更/删除的 namespace/工具，新增签名共享预算，超出部分提示
  `search`；
- 指令段变化走 delta，不重写 system prompt；目录完整时明确告诉模型“不要猜工具名”。

这与 `docs-local/token-economy-port/` 的 P1 和 `deferred-tool-exposure.md` 同向。先在
deferred tools 上做 A/B，观察首轮命中与 `$/call`，再决定是否扩展到全工具面。

**P0-3 按 key 的会话级指令条目。** opencode v2.2.0 的
`ctx.session.instructions.entry.put/list/remove` 值得做一个 Rust 对应物：

- 以 `(plugin_id, key)` 存储指令文本，写入 chat-state，随会话持久化；
- 压缩/回滚后条目仍在，下一步以追加的 system update 注入，不重建完整 system prompt；
- 变更不额外起一轮模型调用；
- 与现有 `push_system_reminder` 的差别是“按 key 可替换/删除”，而不是只追加。

落地前先确认本仓的 prompt cache 断点与 compaction 回放路径；如果条目会导致 system
前缀变化，应退化为“只追加、不改写”的版本。验收：长会话中增删条目后，下一次请求的
缓存前缀只在条目位置之后变化，压缩后条目仍可见。

**P0-4 Hook 上下文与失败路由。** zcode 把 Hook 的 additionalContext 从 tool_result
里拿出来，写成 `tool_hook_context` 附件，持久化后 resume 位置不变；Hook 失败判定与
tool_result 的 `is_error` 共用。本仓已有 7 事件 fail-open，可先补：

- `PostToolUseFailure` 的触发条件与 `ToolOutput::is_error()` 对齐；
- Hook 文本作为独立附件/提醒，不占 tool_result 预算；
- 单条 context 超限时落盘，附件只放 `<persisted-output>` 外壳。

### 5.2 需要立项评估：Code Mode 的主循环版本

**先处理既有决策。** `docs-local/workflow-pi-port/README.md` 的 D1 拒绝 codemode，
理由是 QuickJS 会成为第二脚本运行时，破坏 workflow journal 的
`request_hash(kind, payload)` 重放确定性。这个理由针对的是 **workflow 引擎里的脚本**。
opencode/dsh 的 Code Mode 是**主循环的工具执行器**：模型写程序，程序经稳定 call key
调工具，嵌套调用仍走正常权限、Hook、dispatch log。两者可以分开：

- workflow journal 继续只认 Rhai，`engine.rs` 的 `host_call` 不引入第二种脚本；
- 主循环新增一个可选 `execute` 工具，嵌套调用以 `(run_id, call_id, tool, args)` 记录，
  会话恢复按 call log 重放/对账；
- `execute` 的运行时可以是 Rhai（复用现有依赖，但模型要学 DSL）或嵌入式 JS
  （Boa/rquickjs，模型熟悉，但增加二进制体积与 Windows 构建面）。

**建议的验收门槛**（不达标就不立项）：

1. 同一组长任务，`execute` 版本的输入 token 比直接工具调用低 15% 以上，且首轮缓存
   命中不下降；
2. 嵌套调用的权限、审批、Hook、取消与直接调用一致；子调用有稳定 ID，可在账本中
   对账；
3. 程序不能直接访问 fs/net/process；文件能力只在显式 `full_codemode` 等价开关下开放；
4. 程序挂起/恢复只发生在 durable/idempotent 工具上；普通工具执行期间拒绝挂起；
5. Windows 构建、二进制体积、启动时间在可接受范围；
6. 与 `xai-workflow` 的 Rhai 运行时互不干扰，journal 重放测试保持全绿。

**分期建议。** 第一期只做单次 `execute` + `tools.*` + `maxToolCalls`/`timeout`/
`maxOutputBytes`，不做挂起/恢复；第二期再评估 durable 工具。第一期的价值已经覆盖
“多次工具调用折叠成一次模型请求”的主要成本收益。

### 5.3 插件运行时：外部进程优先

本仓的 hooks 是事件钩子，marketplace/MCP/skills 已经覆盖了大部分“插件贡献能力”的
场景。opencode 的 in-process 插件 API 和 zcode 的 Extension Host 都说明一件事：**插件
需要稳定的能力域和生命周期；把任意代码放进主进程会放大风险**。本仓可走：

- **MCP/JSON-RPC 外部进程**：插件作为 server 提供工具、资源、提示词；宿主只做目录、
  权限、健康检查、重启；这与现有 `xai-grok-mcp` 复用度最高。
- **WASM 插件**（需要时）：用 wasmtime 提供纯函数型 transform/hook，宿主导入受控能力；
  适合格式化、解析、策略判断，不适合长时间持有会话状态。
- **typed hook domains**：把现有 7 事件扩成 `session`/`tool`/`permission`/`shell` 等域，
  transform 与 runtime hook 分开；插件自有工具继续走 MCP 注册，不引入 dylib ABI。
- **重载与版本**：参考 opencode 的 source 指纹和 zcode 的 workspace trust，做“变更
  检测 + 显式信任 + 重启/热重载”三档，避免插件更新静默改变行为。

### 5.4 workspace 级启停与发现

zcode 的两级 scope 适合本仓的 `/plugin` 与 marketplace：

- workspace 配置只放 `enabledPlugins`，仓库里的 options 一律忽略；选项/密钥存用户私有
  文件；
- 项目目录插件默认待信任，用户逐个确认后才加载；已安装的 User/Marketplace 插件不打
  workspace trust gate；
- 打开项目不自动安装；缺失 package 显示 `missing`；
- 发现侧用 BM25/关键词检索已安装与可用插件，推荐卡确认后才安装；不自动安装，最多一次
  mutation。

这部分是产品治理，不涉及运行时内核，适合在 marketplace 现有骨架上迭代。

### 5.5 参考层，不急于移植

- **zcode Extension Host / UI Plugin / Gen UI**：依赖 Electron/utility process/MCP Apps，
  本仓是 Rust TUI，移植收益低。可借的是“能力表 + 插槽 + 进程边界”的设计语言，未来做
  GUI 时再评估。
- **dsh profile/bundle/patch**：组合模型完整但概念重，适合作为配置分层与插件治理的
  参考；本仓现有 config 层不需要整体替换。
- **dsh 动态 Cordis 包 / node:vm**：作为“sandboxed plugin code”的参考，等本仓确定
  WASM/外部进程路线后再对照。

## 6. 决策表

> 下表是内核/正确性口径；成本口径的排序见专项文 §5–§6。两套口径不互相替代。

| 项 | 价值 | 成本 | 主要风险 | 建议 |
|---|---|---|---|---|
| `is_error` 全链路 | 高：wire/冷恢复/Hook 一致 | 1–2 人天 | 旧 JSONL 兼容、provider 差异 | **P0，立即并线 zcode-v4 R2** |
| 工具目录/指令 delta | 高：长回合输入成本 | 3–5 人天 | 目录变化导致缓存失效、排序不稳 | **P0，先 deferred tools A/B** |
| 会话级指令条目 | 中高：插件长期行为 + 缓存 | 3–5 人天 | 压缩/回滚语义、前缀变化 | **P0/P1，先做只追加版** |
| Hook 上下文/失败路由 | 中高：工具结果干净、可恢复 | 2–3 人天 | 附件排序、落盘失败 | **P0，随 `is_error` 一起** |
| Code Mode `execute` | 高：工具调用折叠、长任务 | 10–20 人天（Rhai） | 第二运行时、权限/恢复复杂 | **P1，先 1 周 spike + 验收门槛** |
| 外部进程/WASM 插件 | 中高：生态与隔离 | 8–15 人天 | 协议、生命周期、Windows | **P1，先 MCP 化 plugin host** |
| workspace 启停/信任 | 中：团队治理 | 5–8 人天 | 配置兼容、信任 UX | **P1/P2，marketplace 迭代** |
| Extension Host/Gen UI | 低（当前 TUI） | 高 | 平台绑定 | **参考层** |
| dsh profile/bundle/patch | 低–中 | 高 | 概念负担 | **参考层** |

## 7. 未决问题与证据缺口

- opencode 本地 `dev` fetch 因网络 `early EOF` 失败，工作树仍在 `v2.0.32`；`v2.4.2`
  tag 已浅取，本轮新文件以 GitHub API 读取为准。如果进入实现，需要拉全 `dev` 源码并跑
  `packages/codemode` 测试。
- `psychoscript` 仓库未克隆，语言支持清单（test262 覆盖、getter/setter、Buffer、Date
  locale、错误栈）只从 opencode 的 README/commits 侧面确认，未逐项核对。
- zcode 的插件运行时未实机跑过；Extension Host、UI Plugin、workspace trust 的结论来自
  设计文档与源码结构，缺少真实插件安装/启停/恢复的运行时证据。
- dsh PTC 未跑过；Node provider 的持久 REPL、内存上限、取消清理、图片 path 映射只从
  文档确认，缺少实测。
- 本仓的 prompt cache 断点、compaction 回放、chat-state JSONL 与“会话级指令条目”的
  交互需要先做零编译的证据核对，再决定 P0/P1。
- Code Mode 的 Rhai vs 嵌入式 JS 选择需要一次 1 周 spike：同一组任务测 token、缓存
  命中、延迟、二进制体积、Windows 构建；没有数据前不立项。

## 8. 参考

### opencode（`anomalyco/opencode`，`v2.4.2`）

- `packages/codemode/README.md`：语言边界、Tool/Namespace、CodeMode.make、挂起/恢复、
  durable/idempotent、Extension、OpenAPI、limits、diagnostics。
- `packages/core/src/codemode/instructions.ts`：`# Code Mode` 指令、partial catalog、
  `UPDATE_BUDGET = 1000`、目录 delta。
- `packages/core/src/codemode/tool.ts`：`execute` 工具、freeform、full_codemode、
  嵌套 CallID、background。
- `packages/core/src/tool.ts`、`packages/core/src/tool/runtime.ts`、
  `packages/core/src/tool/model-policy.ts`：direct vs codemode、`codeModeAvailable`、
  `full_codemode` 工具隐藏。
- `packages/core/src/codemode/job.ts`：后台 job、2 分钟阻塞阈值、恢复元数据。
- `packages/codemode/src/codemode/scan.ts`：automations 的静态工具扫描。
- `packages/plugin/src/README.md`、`packages/plugin/src/effect/README.md`：Promise/Effect
  插件 API、transform/runtime hooks、reload。
- `packages/plugin/src/effect/session.ts`、`tool.ts`、`permission.ts`、`shell.ts`：
  session/tool/permission/shell hook 形状。
- `packages/plugin/src/host.ts`、`source.ts`、`runtime.ts`：入口解析、指纹重载、宿主
  模块共享。
- `packages/gui-plugins/README.md`、`packages/plugin-browser/README.md`：GUI 插件与
  Code Mode 浏览器工具。
- Release notes：`v2.2.0`、`v2.3.0`、`v2.4.0`、`v2.4.2`；dev commit `4eba154a`
  （`context.tool.call`）。

### zcode（`zai-org/ZCode`，`v4.0.0` / `a4b44451`）

- `UI_PLUGIN.md`：UI Plugin 与 Gen UI 的能力、API、边界。
- `apps/zcode-cli/docs/design/design-builtin-plugin.md`：官方内置插件的 seed/注册点。
- `apps/zcode-cli/docs/design/v2/plugin-compat.md`：Claude Code 插件兼容范围与 hooks
  信任判定。
- `docs/plugins/extension-host.md`：扩展进程、插槽、能力表、Window Host、API 版本。
- `docs/plugins/workspace-level-plugin-architecture.md`：User/Workspace 两级 scope、
  enabled/options 合并、`plugins.dirs` 信任。
- `docs/plugins/agent-plugin-discovery.md`：BM25 发现、推荐卡、安装/启用/恢复。
- `packages/plugin-api/src/CONTRACT.md`：`@zcode-dev/plugin-api` 纯类型包与版本规则。
- `apps/zcode-cli/docs/design/v2/loop/system-prompt-contract.md`：静态/动态 system
  blocks、attachments、cache 边界。
- `apps/zcode-cli/docs/design/v2/loop/prompt-snapshot.md`：snapshot 的写入/使用/压缩/
  fork/rewind 规则。
- `apps/zcode-cli/docs/design/v2/tool/00-tool-change-chain.md`：工具 schema/顺序/描述
  字节稳定、Hook 上下文、`is_error`。
- `apps/zcode-cli/docs/design/v2/session-persistence.md`：`providerIsError` 与冷恢复。
- `apps/zcode-cli/docs/design/v2/hooks.md`：失败定义、聚合规则。
- `docs/openai-responses-tool-continuation-boundary.md`：工具续写边界。

### dsh（`deepseek-ai/deepseek-harness`，`v0.2.3-alpha.2` / `4b399c39`）

- `docs/cordis-primer.md`：Cordis 的 service/event/effect。
- `docs/subsystems/core.md`：核心包、agent/agent-loop、turn flow。
- `docs/subsystems/system-prompt.md`：prompt section、systemUpdates、compaction 重建。
- `docs/subsystems/tools.md`、`docs/subsystems/session.md`：工具注册表与 append-only
  session log。
- `packages/ptc-runtime/ptc-runtime/README.md`：PTC runtime、binding、`PtcImage`、
  `PtcInspectableValue`、输出/取消。
- `packages/core/tool-executor-ptc/README.md`：`run_code`、SDK、预算、并发、沙箱、失败分类。
- `packages/extensions/`：动态 Cordis 包、host/browser 两半、`node:vm`、审批、slots、
  inspect registry。
- `packages/boot/plugin-manager/README.md`：profile/bundle 开关、`restart-required`。

### 本仓既有文档

- `docs-local/zcode-v4-port/README.md`：zcode v4 的 P1/P2 增补（推理修复、`is_error`、
  压缩文件回补、MCP 状态等）。
- `docs-local/dsh-port/README.md`：dsh 上下文工程（in-history system prompt、工具追加、
  reasoning replay）。
- `docs-local/opencode-port/README.md`：opencode v2 TUI 工作台移植调研。
- `docs-local/agent-cli-tracking/notes/opencode.md`：opencode 1.x/2.x 机制笔记。
- `docs-local/workflow-pi-port/README.md`：D1 codemode 决策、exposure 三档、canonical
  context edit。
- `docs-local/token-economy-port/`：P1 压缩、replay diet、账本与观测。
- `docs-local/fork-feature-inventory.md`：本仓 hooks/marketplace/MCP/skills/ACP/Rhai
  现状。
