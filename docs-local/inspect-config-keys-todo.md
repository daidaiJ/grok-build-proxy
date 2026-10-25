# `grok inspect` 配置键表缺口（已收口 2026-10-25）

> **状态（2026-10-25 收口）**：待办 1 已修（分支 `fix/local-inspect-privacy-warning`，
> PATCHES 六十二期——`[privacy]` 节整体纳入 `is_non_serde_config_path` 豁免，pager
> 拥有的节不再报 unrecognized）；待办 2 已处置（`~/.grok/bin/` 刷为 fork `1.0.69`，
> 旧上游 1.0.40 备份为 `*.bak-1.0.40-upstream`）。本文件保留为环境陷阱参考。
> 基线 = fork 构建 `D:\tool-cli\grok2\grok2.exe` `1.0.69 (ac8598508ca6)`（2026-10-25 10:51）。
> 行号仅作定位，施工以文本搜索为准（上游同步会让行号漂移）。

## 待办 1（已修）：`[privacy]` 被报成 unrecognized

fork 构建跑 `grok inspect`，`Config Warnings` 只剩一条：

```
[privacy] — unrecognized config key
```

但 `[privacy] privacy_banner_acked` 是**文档化的、由 pager 自己写进用户 config** 的键
（`~/.grok/docs/user-guide/26-config-reference.md:510`：「The pager reads user `config.toml`
only」）。即 inspect 对宿主自己拥有的节报假警告，用户每次 inspect 都会看到，容易误判成配置写错。

原锚点（2026-10-25 核实）：

| 事实 | 位置 |
|---|---|
| 假警告产出点（`unrecognized_keys` → `ConfigWarningKind::UnknownField`） | `crates/codegen/xai-grok-shell/src/agent/config.rs:1699`、`:1780`（`deserialize_collecting_unrecognized` 之后） |
| `PrivacyConfig`（`privacy_banner_acked` 字段所在） | `crates/codegen/xai-grok-shell/src/util/config/mcp.rs:52`、`:73` |
| pager 落盘该键的路径 | `crates/codegen/xai-grok-shell/src/util/config/persist.rs:58`（`config.privacy == PrivacyConfig::default()` 判定） |
| 文档口径 | `~/.grok/docs/user-guide/26-config-reference.md:506-510` |

**修法（定案）**：`is_non_serde_config_path`（config.rs）对 `privacy` 与 `privacy.*`
整节豁免——语义 = 该节由 pager persist 层拥有并 round-trip，agent `Config` 永不 serde
建模，节内任何键都不可能构成 unrecognized；前缀豁免沿 `features.` 既有先例，pager 未来
给 `PrivacyConfig` 加字段也不会再冒假警告。取舍：用户在 `[privacy]` 下手写 typo 键同样
静默（该节非安全面，宽容优于噪音；pager round-trip 深合并不会丢数据）。
单测 `privacy_section_does_not_warn_as_unrecognized`（守卫：相邻真未知键仍报）。

## 待办 2（已处置）：PATH 上的 `grok` 是过期上游构建

`which grok` → `C:\Users\panda\.grok\bin\grok.exe`，原为**上游 1.0.40**（2026-09-21，
`agent.exe` 同文件副本；`~/.bashrc` 的 `grok()` 函数也硬指向该路径）。三个坑（2026-10-25 实测）：

- `grok inspect` 报 **9 条** warning，其中 `[api_dump]` / `[network]` / `[model_groups]` /
  `[subagents.model_groups]` / `use_proxy` / `accepts_images` 全是假警告（fork 构建只报
  `[privacy]` 一条，见待办 1）。
- 启动时按自身内嵌文档重写 `~/.grok/docs/user-guide/`，**剪掉 fork 的
  `28-welcome-branding.md` / `29-local-enhancements.md`**（27 篇 vs 29 篇）。跑一次 fork 的
  `grok2.exe`（任意子命令即可）会把 29 篇重新同步回来。
- headless `grok -p` 与 ACP `grok agent stdio` 的工具集是旧版的，用它探测 fork 特性会得出错误
  结论：实测两者都答「没有 `spawn_subagent` 工具」，而 TUI 侧 subagent 一切正常。

**处置（2026-10-25 已做）**：`~/.grok/bin/grok.exe` + `agent.exe` 均覆盖为 fork
`1.0.69 (ac8598508ca6)`（同 `D:\tool-cli\grok2\grok2.exe`），旧版备份
`grok.exe.bak-1.0.40-upstream` / `agent.exe.bak-1.0.40-upstream` 留在同目录。
处置后实测：`grok inspect` 警告 9 → 1（仅剩 `[privacy]`，随六十二期修复归零）、
`grok doctor --json` `totalThemes: 19`（fork 主题全量）、user-guide 29 篇。

**遗留环境提醒**：① 若将来再跑上游安装命令（或上游自更新机制）可能把 `~/.grok/bin/`
刷回上游版——复现「inspect 多条假警告 / user-guide 变 27 篇」即此因，按本文件处置重刷；
② fork 发新版本后记得同步刷新 `~/.grok/bin/`（当前 1.0.69，privacy 修复随下一版进入）。
