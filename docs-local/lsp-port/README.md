# LSP 同类调研 + `/lsp` workspace 级启停施工（2026-10-12）

> 状态：**施工完成（本分支 `feat/local-lsp-workspace-toggle`），活体 TUI 验证待做**。
> 上游提案：[`agent-cli-tracking/ADOPTION.md` §7.6.1](../agent-cli-tracking/ADOPTION.md)（2026-10-11 登记，
> 本文档是它的调研+落地正文）。
> 用户口径：调研范围只取真有 LSP 的同类；only-cc-lite（上下文压缩库，非 coding agent）与
> MiMo-Code（设计不佳）不取参照。

## 1. 调研范围与结论速览

父目录浅克隆逐仓扫描（`*lsp*` 文件名 + `language.?server|langserver` 全文两轮）：

| 仓库 | LSP | 形态 |
|---|---|---|
| **crush**（Charm, Go） | ✅ 最深 | 8 个 LSP 工具直出模型 + 懒启动管理器 + 诊断稳定期等待 |
| **opencode**（effect/ts） | ✅ 成熟 | builtin+config 合并、per-(root,server) 多实例、编辑回喂诊断 |
| **qwen-code**（ts） | ✅ 最全 | 统一 `lsp` 工具 14 操作 + 热重载增量 reconcile + 查询时磁盘文档同步 |
| goose / kimi-code / zcode / Step-Code / minimax-code | ❌ | 两轮扫描 0 命中 |
| only-cc-lite / MiMo-Code | — | 用户口径剔除 |

三家共同的设计底座（也是 fork 已对齐的部分）：**server 配置文件驱动（不内置语言假设）+ agent 用统一工具消费 + 诊断流回喂**。

## 2. 各家设计深读

### 2.1 qwen-code（参照价值最高：两份设计文档 + `/lsp` 先例）

- **配置面**：项目根 `.lsp.json`（语言键 → server 配置）+ 扩展 `plugin.json` 的 `lspServers`；
  字段含 `command/args/transport(stdio|tcp|socket)/env/initializationOptions/settings/
  extensionToLanguage/workspaceFolder/startupTimeout(10s)/shutdownTimeout(5s)/restartOnCrash/maxRestarts/trustRequired`。
  用户文档 `docs/users/features/lsp.md`。
- **启用开关**：`--experimental-lsp` CLI 旗标，**设计文档明说不做运行时切换**
  （`hot-reload/lsp-runtime-reinitialization.md` Non-goals：「If LSP was not enabled at startup,
  there is no service to reload」）——与 ADOPTION 7.6.1「会话边界切换」拍板互为印证。
- **`/lsp` 与 `/status`**：`/status` 出一行摘要（disabled / enabled N/M ready / no servers
  configured），`/lsp` 出 per-server 表格（Server/Command/Languages/Status，READY/FAILED+原因）。
  **qwen 的 `/lsp` 是只读状态面板**；本 fork 提案是启停开关+状态，语义更强。
- **热重载（设计文档，2026 落地）**：`.lsp.json` 专用 watcher（300ms 去抖、canonical 语义 diff、
  解析失败保留旧状态、ENOENT=空配置）；`LspServerManager.reconcileServerConfigs` 按
  **per-server 归一化配置哈希**增量 reconcile（added/removed/restarted/unchanged/failed），
  拒绝 stop-all/start-all（会丢 warm state）；reconcile 串行队列防启动/停止竞态；
  重启后 **replay `textDocument/didOpen`** 保持文档上下文。
- **安全准入**：**先准入后 spawn**；工作区 `.lsp.json` 是工作区可控输入 → **强制 `trustRequired: true`
  不许自降**（扩展配置才允许声明 false）；env 黑名单大小写不敏感
  （`NODE_OPTIONS/LD_PRELOAD/LD_LIBRARY_PATH/DYLD_*`），探针解析裸命令名时不用配置提供的 PATH
  （防劫持 `clangd --version` 探针）；相对路径逃逸工作区拒绝；CLI allow-list 若引入必须是
  会话级上限（运行时只能收窄）。
- **查询时磁盘文档同步**（`lsp-disk-document-sync.md`）：每次查询前读盘发 didOpen/didChange
  （版本单调），覆盖「任何写入者改了文件」的场景；**call hierarchy 条目带 HMAC 出处签**
  （server 名+文本摘要+版本，键序无关），陈旧偏移拒绝并提示重新 prepare，绝不冒充「无调用」。
- **信任联动**：workspace 未信任（`/trust`）时 server 不启动；per-server `trustRequired` 可豁免。

### 2.2 crush（工具面最深）

`internal/agent/tools/lsp_*.go` 8 工具直出：definition / references / symbols / diagnostics /
**call_hierarchy / rename / replace_symbol / restart**（后三个 fork 没有对应工具，见 ADOPTION 7.1.13 缓档）。

- **懒启动**：首次文件访问触发 `canAutoStart`（internal/lsp/manager.go:253）——
  ① 通用命令名（如 `lsp`）黑名单不自动启；② 文件类型过滤先于 PATH 搜索（省 stat 风暴）；
  ③ **recentlyUnavailable 负缓存**（标记后一段时间内不再尝试，防止每个文件访问都白探测）。
- **初始化上下文独立于请求**：用 `context.Background()+timeout`（默认 30s），请求取消不杀初始化
  （manager.go:225 注释点明）。
- **诊断稳定期等待**（client.go:620 `WaitForDiagnostics`）：1s 首变窗口内无变化=server 不会推 →
  提前返回；有变化后等 **300ms settle**（50ms 轮询 version 不变）才放行 agent——
  「编辑后 agent 与编译器对话」的时序核心。
- 编辑/文件工具后 `RefreshDocuments` 重读磁盘推 didChange（client.go:600 附近）。
- 配置：`crushrc`（bash 方言）`lsp` builtin + 旧 crush.json 兼容；per-server `disabled` 键。

### 2.3 opencode（生命周期与合并模型）

`packages/opencode/src/lsp/`（lsp.ts 507 行核心）：

- **builtin+config 合并**：内置 server 表（tsserver/gopls 等）；`cfg.lsp === true` 全开，
  `cfg.lsp` 为对象时逐 server 覆盖（`disabled` / `command[]` / `extensions` / `env` /
  `initialization`），falsy 全关。**`disabled` 键 = per-server 关闭的先例**。
- **per-(root, server) 多实例**：client 按 `root + serverID` 键控，`server.root(file)` 逐文件解析
  项目根 → **monorepo 天然多根**；race 防护三件套：`broken` 负缓存集合、`spawning` 单飞 map、
  spawn 完成发现已有 client 时杀掉重复进程保旧。
- **编辑回喂**：`edit.ts` / `apply_patch.ts` 在工具结果尾部 `touchFile(path, "document")` →
  `waitForDiagnostics` → 有错则追加 `LSP errors detected in <file>, please fix:\n<块>`——
  诊断作为工具输出的一部分回流，模型立即自修。
- **实验旗标**：`experimentalLspTy` 切换 pyright/ty 二选一（`filterExperimentalServers`）。

## 3. fork 现状锚点（施工前基线）

- **工具面**：统一 `lsp` 工具（`xai-grok-tools/src/implementations/grok_build/lsp/mod.rs`，
  ToolId `"lsp"`，操作 goToDefinition / findReferences / hover / goToImplementation /
  documentSymbol / workspaceSymbol）；后端 `implementations/lsp/`（manager/client/dispatch/
  diagnostics/watched_files/restart/format 等）。
- **配置面**：`~/.grok/lsp.json` + 项目 `.grok/lsp.json` + 插件贡献（`config.rs
  load_servers_with_plugins_sourced`），`folder_trust::filter_untrusted_project_lsp` 门禁——
  与 qwen 的 trust 模型同构。
- **诊断回喂**：已有（LOCAL 二期「诊断合并去抖」：150ms quiet window 注入去抖 +
  `take_answered_items`，PATCHES.md），对应 crush 的 settle 等待 / opencode 的编辑回喂。
- **门控**：`[features] lsp_tools`（默认关，env `GROK_LSP_TOOLS`，remote 可覆盖），解析链
  `pin > env > config > remote > default`（`xai-grok-config-types/src/registry.rs` `Feature::resolve`）。
  消费点唯一：`agent_ops.rs:4541`（session spawn 构建 definition 时）。
- **缺口**（= ADOPTION 7.6.1）：无 workspace 粒度开关、无运行时交互入口、无状态查询。

## 4. `/lsp` 施工（本分支）

设计沿 ADOPTION 7.6.1 草案，施工定案：

- **语义**：`/lsp on|off|status`（`clear` 追加：删除覆盖恢复跟随全局）。写
  `~/.grok/feature-overrides/<encode_cwd_dirname(cwd)>.json`（`{"lsp_tools": bool}`，
  键可扩展到其他 per-workspace feature；原子 temp+rename 写）。
- **优先级**：`pin > env(GROK_LSP_TOOLS) > workspace 覆盖 > config > remote > default`——
  用户当前工作区的显式选择压过环境默认；进程级 env 与 MDM pin 保持最高
  （qwen 的「准入上限不可运行时放宽」同思想）。
- **生效点**：session spawn 构建 definition 时（`resolve_lsp_tools_for_cwd`，agent/config.rs），
  决策带 `tracing::debug` 日志（tier + 值）可活体排查。**对本会话不生效，下次会话起效**——
  qwen 设计文档的 Non-goal 印证了会话边界切换是正确粒度。
- **新会话门**：仅「零用户回合」会话可 on/off（`/clear` 即 `/new` 别名，天然归一）；
  判定 = session 存在且 `prompt_history` 空 + 未在加载 + Idle + 无在途/排队/持有 prompt。
  带历史回合拒绝并提示。status 只读不受门控。
- **status 展示**：本会话 LSP 工具是否已注册（`available_tools` 含 `"lsp"`，shell 经
  `meta.tools` 广播）+ 当前 workspace 覆盖值 + 下次会话走向。
- **落点**：shell `util/workspace_overrides.rs`（新，读写）+ `agent/config.rs`
  `resolve_lsp_tools_for_cwd` + `agent_ops.rs` 换消费点；pager `slash/commands/lsp.rs`（新）+
  `Action::LspConfigure` + `dispatch/router.rs` 处理臂（cwd 取 `agent.session.cwd`，
  无会话时 `app.cwd`）+ i18n 键。**不扩 `CommandExecCtx`**（50 个字面构造点，编译级联不值；
  cwd/门控信息在 dispatch 层取）。
- **worktree 会话**：按会话真实 cwd 落键（worktree 路径自成 workspace）；文件极小，不做回收。

## 5. 调研思想的后续吸收判定（不随本次施工）

| 思想 | 来源 | 判定 |
|---|---|---|
| `.lsp.json` 热重载（哈希增量 reconcile + didOpen replay） | qwen | **缓**——fork 会话边界切换已消解主诉求；若日后要「改配置不重开会话」再做，reconcile 骨架照抄其设计文档 |
| 诊断稳定期等待双计时器（1s 首变 + 300ms settle） | crush | **已有等价物**（注入层 150ms quiet window 去抖）；若回喂滞后实测明显再对齐 |
| call_hierarchy HMAC 出处签 | qwen | **缓**——fork 无 call_hierarchy 工具（7.1.13 范围）；补工具时一并考虑 |
| per-(root,server) 多实例 monorepo 多根 | opencode | **缓**——fork 单 root 模型；多根诉求出现再立项 |
| 编辑工具结果尾部追加诊断块 | opencode | **已有等价物**（诊断 reminder 注入） |
| call_hierarchy/rename/replace_symbol 三工具 | crush | **缓**（= ADOPTION 7.1.13，等 `/lsp` 落地后的使用反馈） |
| per-server `disabled` 键（crushrc/opencode） | crush/opencode | **留档**——fork 的 lsp.json 若日后要逐 server 开关可参照 |

## 6. 施工记录

- 分支 `feat/local-lsp-workspace-toggle`；PATCHES.md 三十三期。
- 验证：shell GATE_FORCE `workspace_overrides` 4/4（往返/键控/非法 JSON 幂等写）+
  `lsp_tools` 3/3（覆盖压 config / 默认关上开 / pin 压覆盖）；pager `lsp` 6/6
  （命令解析 2 + dispatch 门控 4：带历史拒绝 / 回合中拒绝 / 写清往返 / status
  三态）+ `translations_table` 1/1；双 crate check --all-targets 0 error
  0 warning；i18n 扫描 exit 0。
- 待办：活体 TUI 验证（`/lsp on` → /clear → 看工具清单出现 `lsp`；`/lsp status`
  三态显示；带历史回合拒绝）。
