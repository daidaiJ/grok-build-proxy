# Claude Code 泄露源码经典主题配色移植预研（dark 档已落地：`deep-dark`，PATCHES 四十二期；light/daltonized 待拍板）

来源：Claude Code v2.1.88 经 npm `cli.js.map` 意外泄露（2026-03-31，~1906 个 TS/TSX
源文件）。本文素材取自公开存档仓
[`qw0258/claude-code-v2.1.88`](https://github.com/qw0258/claude-code-v2.1.88) 的
`01-claude-code-source-crack/claude-code-source/src/utils/theme.ts`（本机临时缓存
`D:\cargo-tmp\cc-theme.ts`，2026-10-15 抓取）。ghuntley 反混淆仓与 sanbuphy 仓均不含
TUI 主题定义，qw0258 份最全。

## 1. 供体盘点（v2.1.88 内置主题）

`THEME_NAMES = dark / light / light-daltonized / dark-daltonized / light-ansi /
dark-ansi`，另有 `auto` 跟随系统明暗（fork 已有 system_appearance 等价物）。
Theme 类型约 90 槽：品牌色（claude/claudeShimmer）、权限/建议蓝（permission/
suggestion/claudeBlue）、planMode、promptBorder、text/inverseText/inactive/
subtle、success/error/warning/merged、diff 六槽（Added/Removed × 普通/Dimmed/
Word）、subagent 专用八色（Tailwind 600 系）、userMessageBackground(+Hover)/
messageActionsBackground/selectionBg/bashMessageBackground/memoryBackground、
rate_limit_fill/empty、fastMode、briefLabel×2、rainbow 七色(+shimmer)。

**供体没有终端背景槽**：全部主题假设背景跟随终端（注释明说用显式 RGB 是为了
避开用户自定义 ANSI 的不一致），只有各面板底色（userMessageBackground 等）。
fork 的 bg_* 六槽需在品牌色系内手调派生（Nord/codex 先例打法）。

## 2. 移植判定

| 供体主题 | 判定 | 理由 |
|---|---|---|
| dark | **✓ 首选** | Claude 橙 `#D77757` 品牌识别度强，现 19 主题无一撞车（最近的 Ayu 暖橙偏金日落系，色相可区分）；UI 语义面完整（权限蓝/planMode 鼠尾草绿/成功绿/错误红/琥珀警告各就各位） |
| light | ✓ 次选 | 黑字白底 + 同一橙 accent；success `#2C7A39` / error `#AB2B3F` / warning `#966C1E`；fork 已有两个浅色（Latte/Solarized Dark 之外）先例，管线通 |
| dark-daltonized / light-daltonized | 可选 | 无障碍新类别（success→蓝 `#3999FF`、diffAdded→深蓝 `#004466`、warning→黄橙 `#FFCC00`，红绿依赖全拆掉），fork 目前零覆盖；工作量与普通主题相同，价值取决于是否有色觉障碍用户需求 |
| light-ansi / dark-ansi | ✗ 不移植 | 16 色手写 variant 与 fork 的 `Theme::quantized(color_support)` 自动降档机制冗余；同 codex 移植时「按用户决定弃用 256 色无障碍 variant」先例 |

## 3. dark 主题槽位映射草案（供体 → fork 64 槽）

- `claude #D77757` → accent_user / accent_assistant / accent_system / link_fg / md_task_unchecked（品牌橙家族）
- `claudeShimmer #EB9F7F` → running（同族提亮）
- `permission #B1B9F9` → accent_running / fuzzy_accent / accent_model / path（浅蓝紫家族）
- `planMode #48968C` → accent_plan；`remember #B1B9F9` → accent_remember
- `success #4EBA65` → accent_success / diff_insert_fg 语义对；`error #FF6B80` → accent_error / diff_delete_fg 语义对
- `diffAddedWord #38A660` / `diffRemovedWord #B3596B` → diff_insert_fg / diff_delete_fg；`diffAdded #225C2B` / `diffRemoved #7A2936` / Dimmed 对 → diff_insert_bg / diff_delete_bg / gutter（供体是深底语义，天然是 bg 槽料）
- `userMessageBackground #373737` → bg_light；`bashMessageBackground #413C41` / `memoryBackground #374146` → bg_hover / bg_visual 参考
- `selectionBg #264F78` → selection_border 参考（供体是选区底色语义）
- `promptBorder #888888` → prompt_border / command / warning 档参考；`inactive #999999` → text_secondary；`subtle #505050` → gray_dim；`text #FFFFFF` → text_primary
- `rate_limit_fill #B1B9F9` / `rate_limit_empty #505370` → usage 面板进度条配色参照（fork 无独立槽，落 accent 或 md_muted 时参考）
- `fastMode #FF7814`、`chromeYellow #FBBC04`、rainbow 七色、subagent 八色：fork 无对应槽，不搬（subagent 八色对应的 agent 自定义颜色功能 fork 未立项）
- **需派生**（donor 无料，Nord/codex 先例：品牌色系内手调）：bg_base / bg_dark / bg_terminal（近黑 #141414 系）、bg_highlight / bg_hover、md_code / md_code_bg / md_text、md_heading h1–h6 五梯度及 modifiers、scrollbar、paste 三槽、gray / gray_bright、accent_thinking / accent_tool / accent_skill / accent_verify

light 主题同法：text `#000000` / userMessageBackground `#F0F0F0`（Hover `#FCFCFC` 专为 256 色量化保间距设计，truecolor 下按 Latte 先例重排）/ diffAdded `#69DB7C`（浅底绿）等。

## 4. 落地面与工作量

管线照抄 codex/ayu 先例：`theme/` 新文件 + `ThemeKind` 新枚举值 + mod/cache/
syntax/settings/doctor/i18n/mermaid 八处接线（PATCHES 三十六期清单）。单主题
净开发 ≈ 半天（映射表已备，主要是派生槽手调 + 主题对照测试）。

## 5. 待拍板

1. 移植范围：仅 dark / dark+light / 加 daltonized 两档。
2. 命名与档位：`claude`（含商标考量时可叫 `anthropic` 或 `cc-dark`）。
3. bg 派生基调：donor 假设终端近黑，bg_base 取 #141414 还是更黑的 #0F0F0F。
