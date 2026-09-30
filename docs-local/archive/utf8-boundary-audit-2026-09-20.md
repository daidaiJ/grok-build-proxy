# UTF-8 字节切片审计（2026-09-20）

起因：`xai-grok-sampler/src/stream/think_split.rs:160` 在中文流式正文上 panic
（`byte index 1 is not a char boundary; it is inside '这' … of \`这篇\``）。修掉之后
按"同类隐患全仓排查"做了一轮审计，本文是结果台账。

**结论**：除 think_split 之外，同类站点**全部是上游自带代码**（blame 落在开源导入
提交 `c68e39f6` "Publish harness and TUI open-source"，2026-07-16；或上游同步
`a5589e95` "Synced from monorepo"，2026-08-05）。按 2026-09-20 定的口径，上游遗留
**暂不修，只记录**（避免把无关改动混进正在验证的预览版）；fork 本地补丁引入的立即修。

## 处置口径（2026-09-20）

- **本地补丁引入** → 立刻修 + 推送 + 随预览版发。
- **上游原有** → 不动代码，只登记本文；等上游同步/专门批次再清。

判断依据用 `git blame`：本仓库是 shallow clone，根提交 `c68e39f6` 即上游导入点，
`a5589e95` 是第一次 `Synced from monorepo`。落在这些提交上的行 = 上游代码；落在
`feat(local)`/`fix(local)` 等本地提交上的 = 自研。参考量级：导入之后本地共 131 个
提交，其中 76 个带 `local` 标记。

## 已修（本地补丁，随 v1.0.37-preview.2）

| 站点 | 来源 | 处置 |
|---|---|---|
| `xai-grok-sampler/src/stream/think_split.rs:160` | 本地补丁 `d75e966d`（整个文件为该补丁新增） | `991e1467` 加 `is_char_boundary` 守卫 + 2 个回归测试；`23d4eb98` 记 PATCHES.md |

根因：holdback 窗口扫描按**字节**枚举后缀起点（`start = buf_len - n`），把不落在
字符边界上的偏移直接喂给 `&self.buffer[start..]`。delta 尾部非 ASCII（中文正文、
`│ ✓` 这类表格字形）即崩，无标记的普通正文一样会崩，探测窗口 1..11 字节。

## 上游遗留清单

### A. 生产可达（已逐行读码确认）

| # | 站点 | 触发输入 |
|---|---|---|
| A1 | `xai-grok-tools/src/implementations/grok_build/scheduler/interval.rs:15` | `s.split_at(s.len() - 1)`。`interval` 是 scheduler 工具的自由字符串参数（`SchedulerCreateInput.interval`，schema 无 pattern），模型/用户给 `"5分"`、`"1小时"` 即 panic。同文件 `interval_to_human`（:45）有同样的写法，但只在 `is_interval_token` 通过后调用（ASCII），修 A1 时一并核。 |
| A2 | `xai-grok-tools/src/implementations/opencode/read/mod.rs:304` | `&line_text[..MAX_LINE_LENGTH]`（2000 字节），只判 `line_text.len() > MAX_LINE_LENGTH`。任何 >2000 字节且第 2000 字节落在字符内部的行（约 700 个汉字）即 panic。**同 crate 的 `codex/read_file/text_utils.rs:26` 用了 `take_at_char_boundary`，这里是漏改的副本**。 |
| A3 | `xai-grok-tools/src/implementations/opencode/grep/mod.rs:330` | 同上，作用于 grep 命中行 `m.line_text`（rg 输出 = 任意文件内容）。 |

### B. 生产可达（触发路径由子代理报告，blame 已确认上游；B1/B2 我自己读过码）

| # | 站点 | 触发输入 |
|---|---|---|
| B1 | `xai-grok-pager/src/slash/commands/loop_cmd.rs:37` | `is_interval_token` 里 `split_at(s.len() - 1)`，入参是 `/loop` 参数的首个空白分隔 token。**`/loop 中文 写点东西` 直接崩**（`中文` 6 字节 → split_at(5) 落在 `文` 内）。修法要注意早退：`interval_to_human`(:45) 的安全依赖它先校验通过。中文用户命中率最高，建议优先。 |
| B2 | `xai-grok-pager-render/src/util.rs:157` | `parse_schedule_interval_secs` 对 `"every "` 之后**不含空白**的 rest 做 `split_at(rest.len() - 1)`。入参 `human_schedule` 来自服务端 `ScheduledTaskCreated`；当前宿主用 `interval_to_human` 只产生英文，故需服务端下发非英文（如 `every 5分钟`，7 字节 → split_at(6) 落在 `钟` 内）才触发。 |
| B3 | `xai-grok-login/src/api_key_probe.rs:24` | `key_suffix`: `if len > 12 { &t[len - 12..] }`，作用于 `XAI_API_KEY` 一类环境变量值（>12 字节且第 `len-12` 字节落在多字节字符内即崩），用在 API key 探针的 telemetry JSON 里。同源实现 `xai-grok-auth/src/bearer_fragment.rs:10` 已改成按字符计数并留了"`éabcdefghijk` 会 panic"的回归测试，这个副本因 import cycle 没同步。 |
| B4 | `xai-grok-memory/src/storage.rs:200` | `&session_id[..session_id.len().min(8)]`（`write_daily_log`）。正常 session id 是 uuid；但 `SessionId` 是 `#[serde(transparent)]` 无校验 newtype，ACP 侧 `preferred_agent_session_id` 可传入非 ASCII id，`v2_capture::validate_session_id` 也只查字节长度和控制字符。 |
| B5 | `xai-grok-markdown/src/render.rs:1059` | `self.text[..cp_byte.min(self.text.len())]` 用的是**未 snap** 的 `last_checkpoint`；同函数 475 行之前对同一个值做过 snap（注释说 pulldown 的 checkpoint 字节"应该"是对齐的），而本 crate 里已有的回归测试记录了真实发生过的 mid-char checkpoint（`render.rs:1216-1220` 表情符号场景，当时只改了另一个消费者）。触发未确证，属"同源风险点"。 |

### C. 测试专用（不影响用户会话，但会让测试红）

- `xai-grok-shell/src/session/goal_classifier.rs:1995` — `rfind(|c: char| c.is_whitespace() || c == '`')` 之后 `i + 1`；`is_whitespace()` 命中 U+00A0/U+3000 等多字节空白时越界。
- 夹具/断言消息类：`xai-grok-tools/.../codex/read_file/slice.rs:142`、`xai-grok-tools/src/computer/local/shell_state.rs:810,872`、`xai-grok-tools/.../grok_build_hashline/edit/apply.rs:973`、`xai-grok-tools/.../grok_build/read_file/mod.rs:1313`、`xai-grok-pager-pty-harness/tests/pty_e2e/common.rs:102`、`xai-grok-markdown/src/latex_delimiters.rs:1136`、`xai-grok-pager/src/views/new_worktree_dialog.rs:239`、`xai-grok-sandbox/src/profiles.rs:907`。
  这些只在"测试失败需要打印消息"时求值，或夹具恒为 ASCII，当前不触发。

### D. 契约型（published 函数对非 ASCII 调用者不安全，当前调用者都是 ASCII）

- `xai-grok-workspace/src/worktree/mod.rs:770` `truncate_label`（`pub`，仅被 ASCII 化的 `sanitize_label` 调用）
- `xai-file-utils/src/queue.rs:2055`（session id 尾 8 字节）
- `xai-grok-pager/src/plugin_cmd.rs:224`（git OID 前 7 字节）、`xai-grok-pager/src/sessions_cmd.rs:253-254`（`created_at`/`updated_at` 截 10 字节，正常是 RFC3339，但值来自可手改的 session JSON）
- `xai-fast-worktree/src/bin/pool_perf_bench.rs:590`、`src/bin/cli.rs:168`（dev bin / git SHA）

## 统一修法

仓库里已有几种现成模式，优先复用，不要新造：

- `xai-grok-tools/src/util/truncate.rs` 的 `floor_char_boundary` / `ceil_char_boundary`
  （`str::floor_char_boundary` 的 polyfill，注释说明等工具链升到 1.91 可删）、
  `truncate_str` / `truncate_str_with_marker`
- `xai-grok-tools/.../codex/read_file/text_utils.rs` 的 `take_at_char_boundary`
- `xai-grok-shell/src/**/helpers/chat.rs` 的本地 `floor_char_boundary`
- `xai-grok-sampler/src/doom_loop_recovery.rs:62`、`xai-grok-hooks/src/event.rs:645`、
  `xai-chat-state/src/compaction_utils.rs:245`、`xai-grok-compaction/src/intra_compaction/fit.rs:434`
  等处的 `while !s.is_char_boundary(i) { i -= 1 }` 手写循环

"取最后一个字符"语义要写 `let last = s.chars().next_back(); s.split_at(s.len() - last.len_utf8())`，
**不要** `split_at(s.len() - 1)`（A1/A3/B1/B2 都是这个形状）。

另外注意语义混淆：tools 里的截断阈值是**字节数**，但文案写着 `chars`
（`read/mod.rs:307` 的 `(line truncated to 2000 chars)`）——修的时候别把两者当成同一口径。

## 验证前提（2026-09-20 实测，给下一批修 A1–A3 的人）

- 本机能编 `xai-grok-tools` 的测试目标：`cargo check -p xai-grok-tools --all-targets`
  实测 0 error（1m23s）。`WIN-TEST-GATE.md` 首轮扫描（2026-09-19）把它列进
  COMPILE-BREAK 的结论**已过时**，别据此跳过本机回归（该文档 2026-09-20 的
  "全目标检查基线"一节已覆盖此点）。
- 但 `xai-grok-tools` 不在 `docs-local/win-whitelist.txt`，跑 lib 测试要走逃生门：
  `GATE_FORCE=1 scripts-local/ctest.sh -p xai-grok-tools --lib`。
- **CI 不跑 tools 的测试**：`build.yml:62` 只有 `cargo check --locked -p xai-grok-tools`
  （不带 `--all-targets`）。所以 A1–A3 修完后回归必须在本机执行，CI 绿不代表测试通过。

## 审计方法（上游同步后可复用）

扫描模式（rg，`crates/` 下 96 个 crate）：

```
\[[a-zA-Z_][a-zA-Z0-9_]*(\.len\(\))?\s*[-+]\s*[0-9a-zA-Z_]+[^]]*\.\.
\[[^]]*\.\.[^]]*\]
\.\.=?[0-9]+\]        \[[0-9]+\.\.
\.truncate\(|\.split_at\(|\.drain\(\.\.|\.replace_range\(|\.insert_str\(
floor_char_boundary|is_char_boundary          # 对照已有守卫
```

判定规则：

1. 接收者必须是 `str`/`String` —— `Vec`/`&[u8]`/`Vec<char>` 的字节索引永远不会 panic，
   这是最大的假阳性来源（`.truncate(` 全仓 182 处，绝大多数是 `Vec`）。
2. 索引必须被**证明**落在字符边界：`find`/`rfind`/`match_indices` + ASCII 字面量长度、
   `char_indices()`、`is_char_boundary`/`floor_char_boundary` 守卫、trim 派生偏移、
   或来源本身是纯 ASCII（hex/uuid/时间戳/ASCII 字面量前后）。
3. 可疑形状：`s.len() - k` 纯字节算术；`bytes().position(..)`/`as_bytes()[i]` 扫描出来的
   偏移回喂 `&str`（尤其 `+ n` / `- 1`）；跨行列累计偏移；对模型输出/用户文本/工具输出
   按"字节预算"截断。

量级与覆盖：`len() - k` 强信号 19 处、`.truncate(` 182 处、已有 `is_char_boundary` 守卫
69 处（markdown 21 / shell 19 / pager 18 / ratatui-textarea 7 / tools 7）。pager 系的
`[..]` 宽匹配有 676 处，超出单次工具输出，靠分目录 + 窄模式二次扫覆盖。

**已知盲区**（下次审计要补）：

- 纯 assert 消息里的内插值只做了抽样（只在测试失败时求值）
- `xai-grok-shell` 的 `.truncate(` 53 处按"接收者类型"批量判为 `Vec`，未逐处读
- 本仓库是 shallow clone，无法 blame 到导入点之前，判断"上游/本地"只能靠 `c68e39f6`/`a5589e95`
  这两个锚点提交；上游同步后应重跑本审计，新导入代码可能重新引入同类站点
