# 用户可见命令提示的硬编码二进制名（diagnostics 别名族）——open

> 2026-10-17 立案。伴随「resume 提示未跟随二进制名」修复（同日随
> feat/local-resume-hint-bin-name 收口的两处：session_title_resolve 的
> `grok --resume`、startup_failure 的 `grok leader kill`）发现。

## 现象

用户把发行二进制改名（如 `grok2`）后，以下用户可见文案仍写死 `grok`：

- `crates/codegen/xai-grok-pager/src/diagnostics/fix.rs`
  - `SSH_WRAP_FIX_COMMAND = "grok doctor fix terminal.ssh-wrap"`（诊断面板
    Remediation 命令，用户照抄执行）
  - `SSH_WRAP_ONE_OFF = "grok wrap ssh <host>"`（面板文案让用户手跑）
  - `SSH_WRAP_ALIAS_POSIX / _FISH = "alias ssh='grok wrap ssh'"`（**doctor
    fix 写进用户 shell 配置的别名**——改名后写出的别名直接失效，比文案更重）

## 为什么本次不修

别名常量参与 doctor fix 的幂等检测与写入面（配置产物），改名自适应需要把
「写入配置的二进制名」与「检测既有别名的匹配」一起换算，且要考虑用户先以旧名
写过别名的迁移；影响面超出本次 bugfix，单独立项。

## 修法方向（拍板后施工）

统一走 `app::cli::current_binary_name()`（argv[0] 文件名，`.exe` 剥除，
缺省 `grok`）；doctor fix 写别名时用当时进程名，检测端按「任一历史名」容忍
（`grok wrap` 与实际名都算已配置）。

## 相关

- 同类已修：`app/session_title_resolve.rs`（`grok --resume <session-id>`）、
  `app/startup_failure/render.rs`（`grok leader kill`）。

## 2026-10-18 追认：退出族两处已修（同修复分支）

用户实测报告：退出会话时的 resume 提示仍显示 `grok --resume <id>`。定位为四十八期
漏网的两处（同一退出提示族），已在 `fix/local-exit-hint-bin-name` 分支修复：

- `app/mod.rs` `print_exit_resume_hint`——普通退出后的「Resume this session with:」
  两行（minimal / 默认），改 `cli::current_binary_name()`。
- `app/screen_mode_relaunch.rs` `screen_mode_relaunch_resume_hint`——screen-mode
  relaunch 失败兜底的 pasteable 命令（`GROK_SCREEN_MODE=… grok … --resume`）。

测试同步改动态二进制名断言（测试进程 argv[0] 非确定，期望串按同一函数现算）。
`doctor fix.rs` 别名常量族维持 open（修法方向见上节）。

**2026-10-18 收口**：修复分支 `fix/local-exit-hint-bin-name`（merge `86c3c43`）
合入 main，随 tag `v1.0.63` 发版（CI build run `39097429149`）；退出提示活体
抽查项（grok2 退出显示 `grok2 --resume <id>`）随新客户端。本 issue 剩余 open
面 = `doctor fix.rs` 别名常量族（P3）。
