---
handoff_id: compact-instructions
status: released-awaiting-live-verification
---

# Handoff：压缩指令 `/compact <instructions>`

> 交接日期：2026-11-13。**已发版并部署 v1.0.75**：merge `828d5f5e`（PR #11），
> release run `43373939909` success（13m57s），本机 `D:\tool\cli\grok2.exe` =
> `grok 1.0.75 (828d5f5e0486)`。剩余：活体验收（§5 第一项），需新会话。

## 1. 目标

让手动压缩支持"保留重点"指令：`/compact 保留数据库迁移、失败命令和未决问题`，
把用户文本传进摘要 prompt 的 `user_context` 槽位，压缩摘要按重点保留。

## 2. 当前进度与状态

- **当前阶段**：v1.0.75 已发版并部署，待活体验收。
- **已完成**：pager 命令/effect、ACP 类型、shell handler/命令链路、prompt 合并、
  单元测试、用户文档、PATCHES 七十期、CI 过滤器扩展。
- **额外覆盖（交接时未列）**：shell 自解析路径 `BuiltinAction::Compact` 也携带
  指令，旧式客户端把 `/compact ...` 当 prompt 文本发来时不丢文本。
- **未完成**：合并 + 新版本活体验收（摘要 prompt 与摘要内容体现重点）。

## 3. 关键证据

- 客户端命令显式拒绝参数：`crates/codegen/xai-grok-pager/src/slash/commands/compact.rs`
  （`NO_ARGS = "/compact takes no arguments."`，`submission_refusal` 与 `run` 都拦）。
- 摘要 prompt 已有槽位：`xai-grok-shell/src/session/helpers/session_compact.rs:248-284`
  （`build_compaction_prompt(user_context)` → "User-provided context for this
  compaction" 段）。
- 唯一生产者是 goal：`acp_session_impl/goal.rs:1079 goal_compaction_user_context()`
  拼 objective / next step；手动压缩传 `None`：
  `session/compaction.rs:611 .run_compact_inner(None, Manual, false)`。
- 请求链：`Effect::Compact`（pager `app/effects/mod.rs:1755`）→
  `x.ai/compact_conversation` 参数只有 `sessionId` →
  `CompactConversationRequest`（`session/acp_types.rs:34`）→
  `SessionCommand::CompactSession`（`session/commands.rs:520`）→
  `run_loop.rs:1189` → `run_compact()`。

## 4. 施工步骤（建议顺序）

1. **pager 命令**：`slash/commands/compact.rs` 去掉 NO_ARGS 拦截，允许
   `/compact [instructions]`；`run` 原样保留参数（queued text 带参数）。
2. **pager 调度**：`app/dispatch/queue.rs` 的 command 分支（约 :552-610）从
   `parse_invocation(&queued.text)` 取 args，带进
   `Effect::Compact { agent_id, session_id, instructions }`。
3. **pager effect**：`app/effects/mod.rs:1755` 的参数 JSON 增加
   `"instructions": Option<String>`（省略时不发或发 null）。
4. **ACP 类型**：`session/acp_types.rs:34` 的 `CompactConversationRequest` 增加
   `#[serde(default)] pub instructions: Option<String>`（旧客户端省略即 None）。
5. **shell handler**：`extensions/memory.rs:336 handle_compact` 透传到
   `SessionCommand::CompactSession { respond_to, instructions }`；
   `session/commands.rs:520` 加字段；`run_loop.rs:1189` 调
   `run_compact(instructions)`。
6. **compaction 入口**：`session/compaction.rs:583 run_compact` 加参数，
   `:922 run_compact_inner` 加参数（只有 2 个调用点：manual :611、:2381；
   其余传 None）。在 `:981` 处把用户指令与 goal context 合并：
   建议顺序 `用户指令在前 + "\n" + goal context 在后`，两者都为空则 None。
7. **测试**：
   - pager：`/compact 保留 X` 被接受、无参不变、usage 文案更新；
   - ACP：带/不带 `instructions` 的反序列化；
   - shell：摘要 prompt 含用户指令段，空参数时不出现；
   - 自动压缩/`SwitchModelCompact` 路径不带用户指令，行为不变。
8. **文档**：用户指南 `/compact` 段（`~/.grok/docs/user-guide/04-slash-commands.md`
   的来源文档）改为 `/compact [instructions]`；若有 i18n 文案，跑扫描。

## 5. 验收标准

- [ ] `/compact 保留数据库迁移和失败命令` → 摘要 prompt 出现该文本（日志或
      `[api_dump]` 可见），摘要内容体现重点。**待合入发版后活体验收。**
- [x] 裸 `/compact` 行为与现在完全一致 — 命令单测、drain 单测（`None`）、
      既有带图/补全用例；prompt 无 `User-provided context` 段断言。
- [x] 自动压缩、切模型压缩（`SwitchModelCompact`）不带用户指令 —
      `run_compact_only` 传 `None`，`run_compact_inner` 自动调用点为 `None`。
- [x] 编译/测试 — 远程 CI run `43359242671` 双 job success：Linux 全量 shell 套件
      + pager 新增过滤器，Windows `cargo check`。
- [x] 旧客户端（不带 instructions）调用 `x.ai/compact_conversation` 正常 —
      `#[serde(default)]` + 反序列化单测；pager 仅在 `Some` 时写该字段。

## 6. 设计决策 / 待定

- **作用域**：仅本次手动压缩，不持久化、不影响自动压缩。若以后要给自动压缩
  固定重点，再加 `[compaction] instructions = "..."`（本次不做）。
- **长度**：建议上限 4,000 字符，超限拒绝并提示（与摘要预算 32,768 不冲突）。
- **与 goal 合并**：两者都有时都保留；用户指令优先呈现。
- **空字符串**：视为无指令，不产生空段落。

## 7. 资产与环境

| 资产 | 路径 | 用途 |
|---|---|---|
| 仓库 | `D:\pro\grok-build-proxy` | 改动目标 |
| 分支 | 建议 `feat/local-compact-instructions`（从最新 main 开） | 分支纪律：代码不进 main |
| 配置 | `~/.grok/config.toml` | 本日已改：300k 窗口、80% 压缩、`[compaction.pruning] enabled=false`；与本次功能无关，勿回滚 |
| 参考 | `docs-local/token-economy-audit-2026-11/` | 压缩/缓存经济性背景 |

## 8. 注意事项（已踩过的坑）

- **工作区有未提交文档**：当前在 `fix/local-send-feedback-cache-prefix`（已远端合并），
  工作区里有 AGENTS.md 治理、token-economy 文档、archive 快照、技能评估等未提交
  改动。开工前先把这些文档归到 main（docs-only 允许），再开功能分支。
- **`/compact` 是 queued command**，不是 prompt；参数要走 `parse_invocation`，
  不要当普通文本发送。
- **ACP 兼容**：新字段必须 `#[serde(default)]`，旧客户端与旧 shell 互操作。
- **prompt 槽位已有**，不要重复造第二个段落；`build_compaction_prompt` 已处理
  `Some/None`。
- **不要动裁剪语义**：本功能只影响摘要输入文本，不改 `[compaction.pruning]`。

## 9. 用户偏好（原文）

- "现在压缩不支持指令标明保留重点"——希望手动压缩能指定重点。
- 之前确认：压缩时裁剪工具结果收益可忽略且伤摘要保真，保持
  `[compaction.pruning] enabled = false`。

## 10. 建议使用的 Skill / MCP

- **Skill**：`tdd`（先写命令解析与 prompt 断言）、`verify-this`（验收证据）。
- **MCP**：`codebase-memory-mcp`（若要追调用链）；本任务文件集中，grep 足够。

## 11. 下个会话聚焦重点

1. 先把未提交文档收口到 main，再开 `feat/local-compact-instructions`。
2. 按 §4 顺序实现，先跑 pager 单测，再跑 shell 定向测试。
3. 用一次真实 `/compact 保留 …` 活体验证摘要 prompt 与摘要内容。

## 12. 明确标注为验证的事项

### 已执行、待验证
- `/compact <instructions>` 端到端活体验收（摘要 prompt 文本 + 摘要内容体现重点），
  合入并发版后用新会话验证；`[api_dump]` 可见 `User-provided context` 段。

### 实现落点（PR #11）
- 远端分支 `feat/local-compact-instructions`（origin 与 `pandazhangS` fork 均有）；
  PR https://github.com/daidaiJ/grok-build-proxy/pull/11 已 MERGED，合入提交
  `828d5f5e`（本机 `git merge --no-ff` + 推 main，token 无 PR merge 权限）。
- main 首次 build（`43363285019`）在 Linux 步骤 25 分钟超时被杀，日志确认全部
  单测与 doctest 已通过、无失败用例；随后把该步骤上限提到 40 分钟
  （`8b3d6c8b`，merge `d42a8abb`），main build `43367116994` 双 job success
  （linux 39m56s，其中 Test touched crates 23m24s；pager 步骤 9m18s）。
- 发版与部署：tag `v1.0.75`（指 `828d5f5e`），release run `43373939909`
  success（13m57s），4 个资产；Windows zip sha256
  `2ea66d927e6c99415070ff5914b86387d808c58c26a7dc2fd3be8f96d350b770`
  与 `.sha256` 一致。本机替换 `D:\tool\cli\grok2.exe`，备份
  `grok2.exe.1.0.74-aae2d5692699.bak`；`grok2.exe --version` =
  `1.0.75 (828d5f5e0486)`。`~/.grok/bin/{grok,agent}.exe` 仍是官方 1.0.30，未动。
- 未在本地编译（按本轮约定走远程 CI）：Linux `Test touched crates` success 覆盖
  新增 shell 单测；`Test pager additions` 新增
  `compact_drain compact_no_args compact_with_args compact_whitespace_only
  compact_rejects_instructions slash_compact_with_trailing_text
  compact_completable_with_optional_args`。
- 偏差说明：长度上限（4,000 字符）在 pager 与 shell ACP 边界各校验一次，共用
  `session_compact::MAX_COMPACTION_INSTRUCTIONS_CHARS`；i18n 旧的预置 usage 键
  `/compact compaction instructions` 改为 `/compact [instructions]`。

### 已完成且已验证
- [x] `/compact` 拒绝参数 — 源码 `compact.rs` 的 `NO_ARGS` 与 `submission_refusal` 实读确认。
- [x] 摘要 prompt 有 `user_context` 槽位 — `session_compact.rs:248-284` 实读确认。
- [x] 唯一生产者是 goal — `goal.rs:1079`、`compaction.rs:611` 实读确认。
- [x] 请求链锚点 — pager effect / ACP 类型 / SessionCommand / run_loop 均已定位。
