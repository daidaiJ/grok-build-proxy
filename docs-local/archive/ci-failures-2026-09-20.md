# CI 失败交接（2026-09-20）

给下一个会话用。任务：修 `daidaiJ/grok-build-proxy` 最近 4 个失败 GitHub Actions，不是 runner/网络问题。

- Repo: https://github.com/daidaiJ/grok-build-proxy
- Actions: https://github.com/daidaiJ/grok-build-proxy/actions
- 分析日期: 2026-09-20
- 分析时 `main` HEAD: `1815e39`（`merge: Windows all-targets zero-warning baseline + incremental-crash self-heal`）
- 该 commit **没有**修下面两类错误；`build #59` 当时 in progress，linux shell 测试大概率再挂

不要改这个交接文档来“记修复”。修完后在 `docs-local/PATCHES.md` 记一笔。

---

## 先看这 4 个 run

| 工作流 | run | SHA | 失败 job / step |
|---|---|---|---|
| release #26 | https://github.com/daidaiJ/grok-build-proxy/actions/runs/35501876294 | `1815e39` tag `v1.0.37-preview.1` | linux + windows `Build release binary` |
| release #25 | https://github.com/daidaiJ/grok-build-proxy/actions/runs/35495100026 | `20523be` 同一 tag | 同上 |
| build #58 | https://github.com/daidaiJ/grok-build-proxy/actions/runs/35495090055 | `20523be` `main` | linux `Test shell (in-process harness)`；windows **过了** |
| build #57 | https://github.com/daidaiJ/grok-build-proxy/actions/runs/35492899506 | `16f167b` `main` | 同上 |

Windows build job 只测 portable crates，不编 `xai-grok-shell` harness，所以 #57/#58 windows 绿、linux 红。release 两边都编 pager-bin，所以 linux/windows 同挂。

拉日志：

```bash
gh run view 35501876294 --repo daidaiJ/grok-build-proxy --log-failed
gh run view 35495090055 --repo daidaiJ/grok-build-proxy --log-failed
```

---

## 根因时间线

1. `16f167b` — merge `/stats` + style overlay。给 `SessionActor` 加了字段，测试字面量没补；`output_style.rs` 把 chrono API 用在 `Instant` 上。
2. `20523be` — merge agent-CLI P0。sampler / sampling-types 冲突没消干净。commit message 自己写了：

```
# Conflicts:
#   crates/codegen/xai-grok-sampler/src/stream/chat_completions.rs
#   crates/codegen/xai-grok-sampler/src/stream/collect.rs
#   crates/codegen/xai-grok-sampler/src/stream_classify.rs
#   crates/codegen/xai-grok-sampling-types/src/types.rs
#   docs-local/PATCHES.md
```

3. `1815e39` — 只清 Windows warning / incremental-crash，上面两类都还在。

同一 tag `v1.0.37-preview.1` 已经打过两次失败。修完后要 **force-move tag** 才会重跑 release。

---

## 修复 1：release — `ChatChunkDelta.reasoning_text`

`cargo build --locked --release -p xai-grok-pager-bin` 编不过 `xai-grok-sampler`。linux/windows 同一 3 个错：

```
error[E0609]: no field `reasoning_text` on type `ChatChunkDelta`
  --> crates/codegen/xai-grok-sampler/src/stream/chat_completions.rs:194:31
                     .or(delta.reasoning_text)
help: a field with a similar name exists: reasoning_content

error[E0026]: struct `ChatChunkDelta` does not have a field named `reasoning_text`
  --> crates/codegen/xai-grok-sampler/src/stream_classify.rs:15:13
             reasoning_text,

error[E0282]: type annotations needed
  --> crates/codegen/xai-grok-sampler/src/stream_classify.rs:25:55
             || reasoning_text.as_deref().is_some_and(|text| !text.is_empty())
```

### 字段现状（`1815e39` 的 `types.rs`）

`ChatChunkDelta` 在 `crates/codegen/xai-grok-sampling-types/src/types.rs`：

- `reasoning_content: Option<String>` — 主字段
- `reasoning: Option<String>` — dialect 回退
- `reasoning_text: Option<String>` — Kimi 风格，LOCAL(deepseek-compat) 注释块里

CI 编译器看到的类型 **没有** `reasoning_text`（rustc 只提示 `reasoning_content`）。两种可能，都要处理：

1. **源码不一致（主因）**：sampler 读 `reasoning_text`，当时编进来的 types 只有 `reasoning_content`。merge 一边改名、一边加 Kimi 字段，没对齐。
2. **cache 次因**：release 日志 `Updating crates.io` 后直接编 sampler，**没编** sampling-types。`chetan/git-restore-mtime-action` + `Swatinem/rust-cache` 可能让 sampling-types rlib 陈旧、sampler 对着旧 rlib。见 `docs-local/ci-cache-incident.md`。

### 该怎么改

Sampler 扫描三个字段，与 `types.rs` 一致，不要只留一个名字：

```rust
delta.reasoning_text
    .or(delta.reasoning_content)
    .or(delta.reasoning)
```

同步改：

- `crates/codegen/xai-grok-sampler/src/stream/chat_completions.rs` ~194
- `crates/codegen/xai-grok-sampler/src/stream_classify.rs` ~15 和 ~25
- 顺手看 `stream/collect.rs`（conflict 列表里有）

确认 `ChatChunkDelta` 在 types.rs 里三个字段都在、没有 cfg 藏起来。本地先：

```bash
cargo check -p xai-grok-sampler --locked
cargo build --locked --release -p xai-grok-pager-bin
```

若本地过、CI 仍报 no field：cache 问题。可 bump rust-cache key，或让 sampling-types 有实质改动逼 cargo 重编。不要只靠“感觉 mtime 对了”。

---

## 修复 2：build linux — `SessionActor` + `Instant`

`cargo test` 编 `xai-grok-shell` (lib test) 挂，12 个错，两类。

### A. E0063 × 11 — 测试字面量缺字段

`/style` overlay 给 `acp_session::SessionActor` 加了：

- `first_model_call_done`
- `output_style_applied`

测试 `SessionActor { ... }` 没写。优先改公共构造，再扫剩余字面量。

| 文件 | 行（#58 / SHA `20523be`） |
|---|---|
| `crates/codegen/xai-grok-shell/src/session/compaction_inline_auto_compact_flow_tests.rs` | 83 |
| `crates/codegen/xai-grok-shell/src/session/acp_session_tests/support.rs` | 318 |
| `crates/codegen/xai-grok-shell/src/session/acp_session_tests/replay_buffer_send_update_tests.rs` | 79 |
| `crates/codegen/xai-grok-shell/src/session/acp_session_tests/inline_auto_compact_flow_tests.rs` | 78, 515 |
| `crates/codegen/xai-grok-shell/src/session/acp_session_tests/memory_config_tests.rs` | 157 |
| `crates/codegen/xai-grok-shell/src/session/acp_session_tests/cancel_running_task_tests.rs` | 127, 750, 1091, 2679 |
| `crates/codegen/xai-grok-shell/src/session/acp_session_tests/idle_resume_tests.rs` | 135 |

HEAD 行号可能已漂。搜：

```bash
rg "SessionActor \{" crates/codegen/xai-grok-shell
rg "first_model_call_done|output_style_applied" crates/codegen/xai-grok-shell
```

补字段用 struct 默认（`false` / `None`，以类型为准）。`support.rs` 的 helper 改完，部分测试会一起好。

### B. E0599 × 1 — 用错时间 API

```
error[E0599]: no method named `timestamp_nanos_opt` found for struct `std::time::Instant`
  --> crates/codegen/xai-grok-shell/src/agent/output_style.rs:67:121
std::time::Instant::now().timestamp_nanos_opt().unwrap_or_default()
```

`timestamp_nanos_opt` 是 chrono `DateTime` / `NaiveDateTime` 的，不是 `std::time::Instant`。

改法（二选一，与周围 ID 生成风格一致）：

- `std::time::SystemTime::now().duration_since(UNIX_EPOCH)` 的 nanos
- `chrono::Utc::now().timestamp_nanos_opt()`

不要继续用 `Instant`。

### 本地验证

```bash
cargo test -p xai-grok-shell --locked --lib
```

Windows 上这条也会编 shell；CI windows job 不跑它，linux 必跑。

---

## 建议落地顺序

1. 修 `output_style.rs:67`（单点、必炸 lib）。
2. 补 `SessionActor` 测试字段，先 `support.rs`。
3. 对齐 sampler 三个 reasoning 字段。
4. 本地 `cargo check -p xai-grok-sampler --locked` + `cargo test -p xai-grok-shell --locked --lib`。
5. push `main`。等 build 绿。
6. **force-move** `v1.0.37-preview.1` 到新 SHA，否则 release 不会按新 commit 再发。

```bash
git tag -f v1.0.37-preview.1 <new-sha>
git push -f origin v1.0.37-preview.1
```

确认仓库允许 force-push tag。

---

## 不要做的事

- 不要为了过 CI 把 shell 测试从 linux job 拿掉。
- 不要把 `reasoning_text` 全局改成 `reasoning_content` 而不保留 Kimi/dialect 回退；`types.rs` 三个字段是有意的。
- 不要只 rerun failed jobs；源码没改，还会挂。
- 不要假设 `1815e39` 已修（只修了 Windows warning）。

---

## 验收

- [ ] `cargo check -p xai-grok-sampler --locked` 过
- [ ] `cargo test -p xai-grok-shell --locked --lib` 过
- [ ] GitHub `build` linux `Test shell` 绿
- [ ] GitHub `release` linux + windows `Build release binary` 绿（需新 tag 或 force-move）
- [ ] `docs-local/PATCHES.md` 记了这两处 merge 残留
