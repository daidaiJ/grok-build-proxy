# WebDAV 同步（用量 + 主机无关配置）+ `/sync`（评估与 TODO）

> **状态（2026-09-21）**：评估完成，**未开工**。本文是决策留痕 + 施工计划。
> 结论：技术可行，成本集中在「哪些配置算主机无关」「合并冲突」「明文密钥外泄」三处，
> 不在 WebDAV 协议本身。分期 T1→T4，净开发 6–8.5 人天，含测试与联调 10–12 人天。
>
> 调研快照 2026-09-21，基线 `main` = `f8e1fea3`。**下文行号仅作定位参考，施工时以文本搜索为准**
> （上游同步提交会让行号漂移）。文档落在 `docs-local/`，上游不存在此目录。

## 需求原话与拆解

用户提出：给本 fork 加 WebDAV 同步——同步**用量信息**与**核心配置**（模型供应商配置等与所在主机
无关的部分），服务端按**主机 MAC 或 hostname** 分子目录存储，TUI 用 `/sync` 选择从哪台机器恢复、
做简单同步。

拆成四个可独立验证的问题：

1. **主机标识**用什么（MAC？hostname？还是现成的机器 id）。
2. **同步什么**（配置里哪些 key 可移植、哪些是主机本地态、哪些是密钥）。
3. **协议与合并**（WebDAV 子集、追加型账本并集、TOML 三方合并、冲突如何呈现）。
4. **密钥与凭据**（config.toml 里的 `api_key` 是明文；WebDAV 口令存哪）。

## 可复用的现成件（全部已核实）

| 需求 | 现成实现 | 位置 |
|---|---|---|
| 机器标识 | `agent_id()`：硬件派生 UUIDv5，首算落盘 `$GROK_HOME/agent_id`（0600） | `xai-grok-telemetry/src/id.rs`（`load_or_compute_agent_id` / `compute_machine_hash`） |
| 配置加锁读改写 | `lock_config_writes()`（`SAVE_LOCK` + flock `.config-init.lock`）+ `save_config_locked()`（原子 rename、保留未建模字段的深合并）+ `update_config()` | `xai-grok-shell/src/util/config/persist.rs` |
| HTTP 客户端 | `shared_client()` / `shared_upload_client()`，工厂为 `xai_grok_extra_ca::build_reqwest_client`（TLS 根、extra CA、`[network] proxy` 策略） | `xai-grok-http/src/lib.rs` |
| 斜杠命令 + 模态 + 后台任务 | `slash/commands/stats.rs`（命令样板）、`app/actions.rs`（`Action::ShowStats`）、`views/modal.rs`（`ActiveModal`）、`app/modals.rs`（事件路由）、`app/dispatch/status.rs`（open 函数） | `xai-grok-pager/src/*` |
| CLI 子命令 | `stats_cmd` 模块 + main.rs 接线（`xai_grok_pager::stats_cmd::run`） | `xai-grok-pager/src/stats_cmd/mod.rs`、`xai-grok-pager-bin/src/main.rs` |
| 后台传输 / 待办队列 | `xai-grok-shell/src/upload/`（trace、`memory.tar.gz` 上传、manifests、`drain_pending_uploads`） | `xai-grok-shell/src/upload/` |
| 依赖齐全的宿主 crate | reqwest + toml + toml_edit + tokio + `xai-grok-config`/`-http`/`-telemetry` + uuid + sha2 + base64 | `xai-grok-shell/Cargo.toml` |

依赖检索结论（`Cargo.lock` 全仓搜）：`reqwest 0.12`（可用 `Method::from_bytes(b"PROPFIND")` 发任意方法）、
`toml_edit`、`md-5`、`hmac`、`pbkdf2`、`ring`（含 AEAD）、`hostname`、`mac_address`、`uuid`、
`zip`/`zstd`/`flate2` 均已在锁文件内，**无需联网拉新 crate**。缺的是：WebDAV 客户端封装、
AEAD 封装、系统凭据库（keyring，锁文件里没有）。

## 数据分类（同步白名单的依据）

按本机 `config.toml` 与 `$GROK_HOME` 实际盘点（2026-09-21）：

| 类别 | 内容 | 处理 |
|---|---|---|
| 可移植 | `[model.*]`、`[model_providers.*]`（密钥字段除外）、`[models]`、`[ui]`（主题/状态行/时间戳）、`[features]`、`[tools]`、`[skills].disabled`、`[plugins].enabled`、`[agent]`、`slash-mru.json` | 同步，按 key 三方合并 |
| 主机相关 | `[mcp_servers.*].command`（本机绝对路径，如 `codegraph.cmd`、`2native-ssh-mcp.exe`）、`[shell].backend`、`[network].proxy`、`[skills].paths`、`trusted_folders.toml`、`[ui.status_line] type="command"`、`[model_providers.*.auth].command` | 默认不同步；要同步则按机器目录单独存 |
| 密钥/缓存/会话 | `config.toml` 内的 `api_key`、`[mcp_servers.*.headers]` 里的 key、`auth.json`、`settings_cache.json`、`models_cache.json`、`sessions/` | 默认排除（见 T4） |
| 用量 | `cache/model-usage.jsonl`（本机 60 KB）、`sessions/**/usage.json`（本机 39 个文件 / 100 KB） | 同步；**整个 `sessions/` 是 93 MB，禁止整体同步** |

**规模实测（本机）**：`config.toml` 3366 B，内含 **4 个不同密钥、8 处明文**
（含 `[mcp_servers.context7.headers]`）；`cache/model-usage.jsonl` 60401 B；
`sessions/**/usage.json` 39 个 / 100845 B；`sessions/` 全量 93479234 B。

用量侧还有一处**路径耦合**要处理：`sessions/<URL 编码的 cwd>/<session_id>/usage.json` 的目录名是
本机绝对路径派生的（`xai-grok-config/src/paths.rs` 的 `encode_cwd_dirname`，长路径走
`{slug}-{blake3_16}` + `.cwd` 文件）。同步时要剥离路径，只留 会话/日/模型 维度，
`stats_cmd` 的读取逻辑（`crates/codegen/xai-grok-pager/src/stats_cmd/mod.rs`，遍历
`sessions/**` 最多 4 层）是可复用的参考。

## 设计定案（施工前必须锁定，不要默默选）

### D1 主机标识：不用 MAC，也不用 hostname

- MAC 会被 Wi-Fi 隐私随机化、多网卡（有线/虚拟网卡/蓝牙）搅乱，取值本身还要平台 API；
  hostname 在默认名与容器场景下高度重复。
- 现成的 `agent_id()` 是硬件派生（Windows 走 PowerShell 查 WMI：
  `Win32_ComputerSystemProduct.UUID` + BIOS/主板序列号 + CPU id；macOS 走硬件 UUID；
  Linux 走 `/etc/machine-id` + `$HOSTNAME`），有**两个坑**：
  ① 首次计算依赖 PowerShell/WMI，被策略禁用时 `mid` 返回错误 → 静默退化成**随机 UUIDv4**
  （仍会落盘，故本机稳定，但不再是"可复现的同一台机器"）；
  ② 它是**机器级**而非用户级，同机多用户同值，且换主板/刷 BIOS 会换号 → 在用户眼里
  "平白多出一台新主机"。
- **定案**：服务端目录主键用**本机随机生成的 sync id**（`uuid v4`，落盘 `$GROK_HOME/sync-id`，0600）；
  把 `agent_id`、hostname、平台写进服务端 `machines.json` 作为**别名**，供 `/sync` 展示与
  "本地 id 丢失后按别名认领已有目录"。不把 `agent_id` 当目录名（它是已上报的稳定设备指纹，
  上传到第三方网盘属额外隐私暴露；真要写就写哈希）。

### D2 目录布局

```
<base>/
  machines.json                     # 机器索引：sync_id → {hostname, platform, agent_id_hash, last_seen}
  <sync_id>/
    manifest.json                   # 本次快照的：schema 版本、时间戳、各类文件的内容哈希
    config.portable.toml            # 白名单抽取出的配置子集（T4 后可能是 .toml.enc）
    usage/model-usage.jsonl         # 追加型账本（并集去重后）
    usage/sessions/<session_id>.json
```

按 `<sync_id>` 分子目录是需求原话要的"按主机分目录"；`machines.json` 是让人认得出是哪台机。

### D3 合并语义（三类数据三种策略）

| 数据 | 策略 |
|---|---|
| `model-usage.jsonl`（append-only） | 按记录唯一 id 并集去重（缺 id 时按内容哈希），本地保留全量，远端只存并集 |
| `sessions/**/usage.json`（小 JSON） | 按 session_id 并集，冲突回合取 max/最新 |
| TOML 配置 | **按 key 路径**三方比较（base=上次同步的 manifest 哈希、local、remote），`toml_edit` 手术式写入以保住注释与写法；合并前一律落 `config.toml.bak-<时间戳>`（仓库已有 `.bak-*` 习惯） |

**纪律**：pull 侧落盘**必须**走 `lock_config_writes()`，禁止自己 `fs::write`；
注意 `save_config_locked()` 只覆盖 `cli / models / ui / harness / session / skills / telemetry /
features / privacy / consent / toolset.ask_user_question`，**`[model.*]`、`[model_providers.*]`、
`[mcp_servers.*]`、`[plugins]`、`[network]`、`[shell]`、`[tools]`、`[agent]` 不经此路径写出**——
所以配置同步要么扩这个函数（改动同步文件，需登记 PATCHES.md），要么在 sync 模块内用
`toml_edit` 自写并在同一把锁下操作。

### D4 传输：WebDAV 只用一个子集

`PROPFIND Depth:1`（列目录）、`MKCOL`（递归建目录）、`GET`、`PUT`（优先带 `If-Match`/ETag
条件写，服务端不支持则退回时间戳 + 内容哈希比对）、`DELETE`。不用 `LOCK`/`UNLOCK`（兼容性差，
按"单写者 + 条件写"假设设计）。

认证先只做 **Basic over HTTPS**（Nextcloud/坚果云用"应用密码"）；Digest 留 T4 之后——
reqwest 不带 Digest，需手写（`md-5` 已在锁文件），而 MD5 Digest 本身也弱。
服务端差异要在夹具里覆盖：Nextcloud 对不存在的集合返回 404 而非 405；路径按段转义；
`Depth: 1` 的 XML 响应解析要容忍命名空间前缀变化。

### D5 触发与生效提示

写侧账本是 shell 每次成功推理追加，push 要取快照或按 offset 增量，禁止边写边读整文件。
pull 完成后**必须**提示"配置已更新，重启 `grok2` 生效"——模型目录在进程初始化时解析
（见 `third-party-models` 技能"重启进程"一节），不提示用户会以为同步失败。

---

## T1：WebDAV 传输层 + 假服务测试夹具

### 目标

一个可独立验证的 WebDAV 客户端子集（D4），配一个进程内假 WebDAV 服务作单测夹具
（真实网盘无法进自动化回归）。

### 改动点

1. 新模块 `crates/codegen/xai-grok-shell/src/sync/webdav.rs`（LOCAL；**不新增 workspace member**，
   见 `PATCHES.md` 顶部约定）：`Client` 结构（base url、Basic 凭据、超时、路径前缀）+
   `list() / mkdir_all() / get() / put_if_match() / delete()`。
   - HTTP 走 `xai_grok_http::shared_client()` 或 `xai_grok_extra_ca::build_reqwest_client`
     （继承 TLS/extra CA/`[network] proxy` 策略，别新建裸 client）。
   - 自定义方法：`reqwest::Method::from_bytes(b"PROPFIND")`。
   - `PROPFIND` 响应解析：优先用现成 XML 解析依赖（先查 `Cargo.lock` 里是否已有
     `quick-xml`/`roxmltree`）；没有则加，**不要手写正则解析 XML**。
2. 夹具 `crates/codegen/xai-grok-shell/src/sync/test_server.rs`（`#[cfg(test)]`）：
   基于 axum（已在依赖树）实现最小 WebDAV，含 ETag、`Depth: 1`、404-vs-405 分支、
   可注入故障（超时/500/412）。

### 验收标准

- `cargo check -p xai-grok-shell` 通过。
- `scripts-local/ctest.sh -p xai-grok-shell --lib sync::webdav` 通过，覆盖：列目录、
  递归建目录、条件写冲突（412）、404 当空目录、路径含空格/CJK/`#`/`%` 的转义往返。
- 不新增裸 `reqwest::Client::new()`（grep 自查）。

## T2：清单、合并内核与 `grok2 sync` CLI

### 目标

`grok2 sync status|diff|push|pull`（先做 CLI，TUI 后置）：白名单抽取/合并、用量并集、
dry-run diff。**这是最有价值的最小切片**——协议与合并逻辑在无 TUI 的情况下就能验完。

### 改动点

1. `crates/codegen/xai-grok-shell/src/sync/manifest.rs`：白名单定义（按 D3 的表格，
   以 `[section]` + key 路径表达）、抽取（`extract_portable(&TomlValue) -> TomlValue`）、
   合并（`merge_portable(local, remote, base)`），纯函数 + 表驱动单测。
2. `crates/codegen/xai-grok-shell/src/sync/usage.rs`：`model-usage.jsonl` 并集去重、
   `usage.json` 归并；剥离 cwd 路径。
3. `crates/codegen/xai-grok-shell/src/sync/layout.rs`：`sync-id` 读写（0600，
   参考 `fs_atomic::write_atomically`）、`machines.json` 读写、目录布局（D2）。
4. `crates/codegen/xai-grok-pager/src/sync_cmd/`：CLI 门面，照 `stats_cmd` 的结构
   （`clap::Args` + `run()` + `--json`），在 `xai-grok-pager-bin/src/main.rs` 接线。
5. 密钥处理按 T4 的开关：一期默认**剥离** `api_key`/`headers` 里的密钥，
   命令输出显式提示"N 个密钥未同步"。

### 验收标准

- `cargo check -p xai-grok-pager -p xai-grok-shell` 通过。
- `sync::manifest` 单测：白名单外的 key 不被带走；本地注释在合并后仍在；
  冲突项按 D3 策略裁决；空/坏 TOML 不 panic。
- `sync::usage` 单测：并集幂等（跑两次结果不变）、重复记录去重、跨机记录按日/模型聚合正确。
- 端到端（对夹具）：`push` → 清空本地 → `pull` → 配置与用量等价；`pull` 前自动备份存在。
- **不碰**真实网盘；真实 e2e 手动跑一次并记进本文档（服务端类型 + 结果）。

## T3：`/sync` TUI（斜杠命令 + 模态 + 后台任务）

### 目标

`/sync` 打开模态窗口：远端机器列表（`machines.json`）→ 选中后看快照摘要（时间/文件/大小/
与本地 diff 概要）→ 选择 拉取 / 推送 / 仅预览。对齐 `agents_modal` / `usage_modal` 的观感。

### 改动点

1. 命令 `crates/codegen/xai-grok-pager/src/slash/commands/sync.rs`：`slash_meta!`
   （名称 `sync` 当前**未被占用**，已查注册表与 `slash/commands/` 目录）、
   `takes_args: true`（`/sync push|pull|status`）、`run()` 返回 `CommandResult::Action(Action::ShowSync)`；
   注册进 `slash/commands/mod.rs` 的 `builtin_commands()`。
2. `views/sync_modal.rs`：抄 `views/stats_modal.rs`（`State` + `render()` + `route_key()` +
   `Outcome` 枚举），底部复用 `views/modal_window.rs` 框架；加进度/错误态。
3. `views/modal.rs` 加 `ActiveModal::SyncInfo { .. }` 与标题映射；`app/modals.rs` 补键盘路由/
   滚动/关闭；`app/dispatch/status.rs` 加 open 函数；`app/actions.rs` 加 Action。
4. 网络调用**必须异步**：`tokio::spawn` 起任务，经 channel 把进度/结果回灌 UI（参考
   `app/event_loop.rs` 的 voice pipeline 与 `app/app_view.rs` 的异步结果注入），
   禁止阻塞渲染线程；失败要有可见 toast，不能静默。
5. 文案全部经 `crate::slash::i18n::tr`/`tr_str`（否则 `ui-i18n-plan` 扫尾会漏），
   翻译表加进 `slash/i18n.rs`。

### 验收标准

- `cargo check -p xai-grok-pager` 通过、`scripts-local/ctest.sh -p xai-grok-pager --lib slash::` 通过。
- 手动验证：`/sync` 开窗、可切换机器、预览 diff 与 `grok2 sync diff --json` 一致、
  推送/拉取有进度与结果提示、Esc 关闭、80/120/200 列宽不溢出。
- 无网络/服务端 401/404 时给出可读错误，不 panic、不卡 UI。
- 提示"重启生效"在 pull 成功后出现。

## T4：加密与凭据（可选开关，但默认排除密钥必须落地）

### 风险前提

`config.toml` 现有 4 个不同密钥、8 处明文。整段同步 = 把全部供应商密钥交给网盘。
三个选项，**默认取第 1 个**：

1. **只同步结构、剥离密钥**（本机改用 `env_key` 指向环境变量——即 `third-party-models`
   技能推荐的"库外密钥"路径）。成本最低。
2. **加密载荷**：口令 → PBKDF2-HMAC-SHA256 或 Argon2 → `ring` 的 AES-256-GCM /
   ChaCha20-Poly1305 加密 `config.portable.toml` 与密钥段。需定：盐/nonce 存放、
   口令错误提示、口令丢失后的语义（不可恢复要写明）。
3. 混合：结构明文同步，密钥单独 `secrets.enc` 按需拉取。

### 改动点

- 本机 WebDAV 口令的存放：不进 `config.toml`（明文）；候选是 Windows Credential Manager /
  DPAPI（`windows` crate 已在 workspace 依赖里，需加 feature）与 Linux secret-service
  （`zbus` 已在依赖里）；其次才是 0600 的独立凭据文件 + 明确告知。
- 依赖新增（如需）：`argon2` / `chacha20poly1305` 等**不在** `Cargo.lock`，属真正的新依赖，
  加之前先确认能联网取包（本机此前所有 crate 都来自既有锁文件）。

### 验收标准

- 加密开关关：远端与抓包中均不出现任何密钥明文（对夹具断言请求体）。
- 加密开关开：口令错 → 明确报错且不改本地文件；正确口令 → 往返等价；
  `grok2 sync status` 在无口令时能给出可读提示。
- 凭据读取失败（无 keyring/无 GUI 会话）时有回退路径且提示清晰。

---

## 备选路线（成本差一个量级，先比再上）

| 路线 | 覆盖需求 | 估算 |
|---|---|---|
| tar/zip 导出导入（`/sync export` 产出单文件，用户自己丢网盘） | 换机、多机复用模型配置 | ~1 天；零服务端交互、零凭据管理、零密钥外泄风险 |
| 只同步用量（不含配置） | 多机用量汇总看 `/stats` | ~1 天；无密钥问题 |
| Git 私有仓库代替 WebDAV | 全部需求，冲突解决免费、有审计 | 2–3 天（fork 已有 git 基建） |
| WebDAV（本方案） | 全部需求，无需 git 环境、手机可看 | 10–12 人天 |

若动机只是"换机/两台机复用模型配置"，**先做导出导入 + 用量同步**性价比高一个量级；
WebDAV 的不可替代场景是"没有 git、想用坚果云/Nextcloud、要自动同步"。

## 通用验证与纪律

按 `AGENTS.md` 第 4 条（Windows 测试门禁，MANDATORY）：

- 编译验证（快）：`cargo check -p xai-grok-shell -p xai-grok-pager`；
  行为验证按模块过滤：`scripts-local/ctest.sh -p xai-grok-shell --lib sync::`、
  `scripts-local/ctest.sh -p xai-grok-pager --lib sync`。**禁止裸跑 `cargo test`**。
- 网络功能无法在本机真实回归 → **夹具优先**（T1），真实 e2e 手动跑并记进本文档。
- 改共享 crate（`xai-grok-shell`）级联重编，每个 T 一次做完、独立提交（AGENTS.md 第 10 条）。
- LOCAL 约定（`PATCHES.md` 顶部）：所有插入行带 `// LOCAL:`，新配置字段 `#[serde(default)]`
  且追加在 struct 尾部，**不新增 workspace member**；改动同步文件（`slash/`、`views/`、
  `app/`、`util/config/persist.rs`）需在 `PATCHES.md` 登记，便于上游同步后重放。
- 上游 `Synced from monorepo` 会覆盖同步文件里的本地改动——sync 代码尽量整目录新增
  （`src/sync/` 整块新文件），减少对同步文件的侵入面。

## 待定决策（实现时须先定，不要默默选一个）

1. **密钥**：默认剥离（推荐），还是要做加密同步？
2. **目标服务端**：Nextcloud / 坚果云 / 自建 —— 决定认证与 404-405 兼容分支的取舍。
3. **依赖**：是否接受为 Digest 认证、加密、系统凭据库各加 1–2 个**新**依赖
   （`md-5`/`ring`/`zbus`/`windows` 已在锁文件或 workspace 依赖内）。
4. **是否先走备选路线**：导出导入 + 用量同步（~2 天）先满足多机复制，WebDAV 押后。
5. **`/sync` 语义边界**：是"手动一次"还是带自动同步（定时/启动时）——影响是否需要
   后台常驻任务与冲突 UI。
