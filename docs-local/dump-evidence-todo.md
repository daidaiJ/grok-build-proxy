# `/dump` 现场取证转储（设计，TODO）

> **状态（2026-09-24）**：设计调研完成，**未施工**。来源是
> `docs-local/mimo-v26-toolcall-loop-analysis.md` 里那句「`/dump` 现场取证命令：方案已备
> （见会话 plan），未施工」——原会话 plan 没有落盘，本文件是重新调研后的完整方案。
> LOCAL 特性；施工时按分支纪律开 `feat/local-dump-evidence`，基线取当时 `main`。
> 本文所有量级数字来自本机实测（脚本 `logs/dump-proto.py`，见文末附录），
> 不是估算；行号仅作定位参考，施工**以文本搜索为准**。

## 一、问题定义：什么叫"现场够用"

`/dump` 的产物有两个消费者，诉求正好相反：

- **取证者**（一个没有本会话上下文的新 agent 会话，或人）：要求**信息足够**——
  拿到 dump 后能独立回答一份固定的问题清单，不必回头找用户补料。
- **消费方式**：要求**对 agent 友好**——它能进上下文，能被 grep/read_file 直接吃，
  不需要人工先整理。

于是判据可以写死成两条可验收的约束：

1. **充分性**：给定 dump 目录，一个空上下文 agent 能在 ≤5 次工具调用、≤30k token 内
   回答下面这份清单（清单来自真实故障，不是假想）：
   - 工具调用/结果是否配对完整（悬空 `tool_calls`）；
   - 是否退化成循环（**按输入签名去重**，而非相邻等值——轮转模式会击穿等值检测）；
   - 每轮 reasoning token 是否塌缩、输入 token 是否爆炸、单轮耗时是否异常；
   - assistant 消息重放时是否带 `reasoning_content`；
   - 协议伪标记（`<tool_call>` / `<function=` / `<parameter=`）泄进了哪个通道、几次；
   - 正文/思考两通道的字节分布（`think_split` 类切分 bug 的第一指纹）；
   - 版本、模型、后端、配置项（脱敏后）、工作区路径。
2. **友好性**：单一入口、字节预算明确、每个数字都能指回原始出处（文件+行或文件+偏移）、
   原始层不截断只标注、schema 带版本号且每行一条 JSON（可 grep 可管道）。

### 量级现实（实测，2026-09-24）

同一台机上会话 `01a0d161`（13 轮、270 次工具调用）：

| 项 | 实测 |
|---|---|
| 会话目录原始总量 | **25,560,362 B / 168 个文件** |
| 其中最大单文件 | `terminal/chatcmpl-tool-b0b2….log` = **4,325,376 B**（占 95%） |
| `events.jsonl` | 11 MB，131,858 行，其中 **130,299 行是同一种 `phase_changed`** |
| `updates.jsonl` | 6,742,229 B，1,149 条记录（270 次调用各 2 条：`tool_call` + `tool_call_update`） |
| `chat_history.jsonl` | 1.1 MB，545 条 |
| 最大单个文本 chunk | 62,578 B（思考通道），>10k 的 chunk 有 7 个 / 246 个 |

把整个目录丢给 agent 不可行：25 MB ≈ 600 万 token 量级，而其中 95% 是一个工具的输出、
52% 的行是同一种心跳。**"信息足够"的瓶颈从来不是字节不够，而是信噪比。**

## 二、现状盘点：已有什么、缺什么

### 已有的四个面（不要重复造）

| 面 | 入口 | 形态 | 对 agent 的可用性 |
|---|---|---|---|
| 转录导出 | `grok2 export <sid>`（`export_cmd.rs`）、TUI `/export`（`dispatch/transcript.rs`）、`/transcript`（`$PAGER`） | Markdown 对话 | 只有对话，**无任何元数据/时序/用量** |
| 全量打包 | `grok2 trace <sid> --local`（`trace_cmd.rs`，`build_session_tar`） | 整目录 tar.gz + memtrace + `trace_config.json` + `export_metadata.json` | **全量倾倒**：无索引、无聚合、无预算；面向上传/支持，不面向 agent |
| 采样日志 | `--log-sampling` / `GROK_LOG_SAMPLING=1`（隐藏 flag，默认关）→ `~/.grok/logs/sampling.jsonl`（`sampler/src/sampling_log.rs` + `telemetry/src/sampling_log.rs`） | 每条 SSE 原始 `data`（`event="sse_chunk"`，三种 backend 都覆盖）+ 请求元数据（model/base_url/auth_type/`auth_prefix` 截断 20 字符）+ 空响应/doom-loop 诊断 | 唯一的**线级真相**；但共享 5 MB 滚动上限（`unified_log::MAX_SIZE`） |
| 会话统计 | `signals.json`（1.7 KB 扁平计数器）、`usage.json`（按轮、按模型） | JSON | **这两个就是"agent 友好"的正面样板**：小而全、直接可判读 |

### 现场文件清单（每个文件的角色）

| 文件 | 角色 | 对它有用的取证问题 |
|---|---|---|
| `updates.jsonl` | **通道真相**：`agent_message_chunk` / `agent_thought_chunk` / `user_message_chunk` / `tool_call` / `tool_call_update` / `turn_completed` | 通道字节比、伪标记落点、每轮 token 与耗时 |
| `events.jsonl` | **时序真相**：`turn_started/ended`、`first_token`、`tool_started/completed`（带 `duration_ms`+`outcome`）、`permission_*`、`mcp_*`、`loop_started` | TTFT、工具耗时、权限流、MCP 健康；噪声巨大 |
| `chat_history.jsonl` | **agent 视角的会话**（system/user/assistant/**reasoning**/tool_result） | reasoning 是否为独立记录、工具结果原文 |
| `terminal/<id>.log` | 每次工具调用的终端输出落盘 | 单次调用的原始输出（含 4.3 MB 那种离群值） |
| `usage.json` | 按轮 + 按模型的 token 账 | 输入爆炸、缓存命中、reasoning 塌缩 |
| `signals.json` | 会话级计数器（循环/doom-loop/取消/压缩/工具集合） | 一屏给结论 |
| `summary.json` / `prompt_context.json` / `system_prompt.txt` / `plan.json` | 标题、提示上下文、系统提示、计划态 | 复现提示层差异 |
| `compaction/` + `compaction_requests/` | 压缩输入与被压内容 | "压缩后行为变了"类问题 |
| `rewind_points.jsonl` | 回滚点（316 KB） | 定位用户回滚点 |

### 三个真实缺口

1. **出站请求体从未落盘**。`sampler/src/client.rs` 里 `request_body` 只在本地构造后直接发走
   （L1276/L1418）；`raw_body` 只在**响应反序列化失败**的错误路径被打印（L927/L1343/L1705）。
   已核查的 `sampling_log` 事件只有：`client_new`/`client_post` 元数据、`sse_chunk`（响应侧）、
   空响应与 doom-loop 诊断——**没有出站请求体**。而 MiMo 取证三问里
   「`tool_calls`/`tool_result` 配对完整性、轮转签名、`reasoning_content` 是否回传」
   问的正是**发给服务端的那个 body**。`chat_history.jsonl` 不能替代它：
   线上 body 还要过 `strip_leaked_tool_call_text`、空 name 过滤、孤儿 tool_result 降级、
   `conversation_to_chat_messages` 的 Reasoning 折叠、`patch_reasoning_text_types` 等一串改写。
2. **没有任何东西做跨文件 join**。上面每个问题都要同时读 2–3 个文件才能答：
   一次调用的输入在 `updates.jsonl`、耗时在 `events.jsonl`、输出在 `terminal/<id>.log`；
   而两边的 id 还**不同名**（本会话里 `call_00_…` 与 `chatcmpl-tool-…` 混用，145 个终端日志对 270 次调用）。
3. **没有面向 agent 的入口与预算控制**。`trace` 是"给支持工程师的压缩包"，不是"给 agent 的索引"。

## 三、设计：三层结构（Facts / Index / Raw）

原则：**派生层有损但要诚实，原始层无损但要索引**。一次采集落三个东西，互不替代。

```
$GROK_HOME/dumps/<session-id>-<utc-stamp>/
├── INDEX.md        人类与 agent 的唯一入口（预算：≤ 4 KB / ≈ 1k token）
├── facts.json      机器可读事实表（每行一条记录，可 grep/jq；带 schema_version）
├── raw/            原始层：会话目录（硬链接优先，跨卷则拷贝）+ 可选采样的 sampling 片段
└── manifest.json   采集元数据：时间、grok 版本、源路径、每个文件字节数/sha256/行数/脱敏说明
```

### 层 1：`facts.json`——把"要 join 的东西"预先 join 好

字段与它回答的问题一一对应（实测产出 3.0 KB）：

| 段 | 字段 | 回答的问题 |
|---|---|---|
| `session` | id / cwd（可脱敏）/ grok 版本 / os / arch / 模型集合 / 后端 / 生效配置摘要 | 复现前提 |
| `channels` | 每通道 chunk 数与字节数（`agent_message_chunk` 14,476 B vs `agent_thought_chunk` 478,932 B） | 切分/折叠类 bug 第一指纹 |
| `markers` | 按通道统计 `<tool_call>`/`<function=`/`<parameter=`/`<think>` 出现次数 | 泄露源在网关还是 serving、落在 content 还是 reasoning |
| `turns[]` | 每轮：`stop_reason`、`model_calls`、`input/output/reasoning tokens`、`elapsed_ms` | reasoning 塌缩、输入爆炸、单轮异常耗时 |
| `calls[]` | 每次调用：`id`（**两套 id 都记**）、工具名、`input_digest`、`input_bytes`、`duration_ms`、`outcome`、`output_bytes`、`output_path`（指向 `terminal/<id>.log`） | 配对完整性、逐调用耗时/失败/输出体积离群 |
| `loops` | `distinct_inputs` / `repeat_ratio` / `top_repeated[]` | **#2482 型轮转循环**（实测本例 270 调用 / 270 种输入 = 1.0x，健康） |
| `anomalies[]` | 派生出的"值得先看"的指针：最大 chunk、最大工具输出、失败调用、`phase_changed` 占比等 | 省掉 agent 自己找异常 |
| `sources` | 每个数字的来源：`{file, lines, bytes, sha256}` | 可校验、可追溯 |

### 层 2：`INDEX.md`——`≤4 KB` 的入口，含"怎么下钻"

不是"摘要"，而是**结论文档 + 下钻指令**。实测 1,787 B 就装下了：
原始规模、轮数/调用数/失败数、循环签名（repeat ratio 与阈值判读）、通道字节表、
marker 普查、逐轮 token/耗时表、事件聚合（把 130,299 行 `phase_changed` 收成一行计数）、
以及一段固定格式的「下钻指南」：想查什么 → 去 `facts.json` 的哪一段 → 需要原文时
`grep -n <id> raw/updates.jsonl`。

### 实测效果

| 产物 | 字节 | 相对原始 |
|---|---|---|
| `raw/` | 25,560,362 | 1x |
| `facts.json` | 3,043 | 1/8,400 |
| `INDEX.md` | 1,787 | 1/14,300 |
| **派生层合计** | **4,830** | **1/5,292** |

也就是说：**进 agent 上下文的永远是那 4.8 KB，25 MB 只在需要时按指针去 grep。**
这就是"信息足够 + agent 友好"的同时达成方式——不是折中，是分层。

## 四、两条约束的具体化（充分性四条 / 友好性六条）

### 让"信息足够"（四条）

1. **通道齐全**：对话两通道 + 工具输入/输出 + 时序 + 用量 + 版本/配置 + 系统提示摘要。
   任何一个缺失都会把 agent 逼回来问用户（那正是现状）。
2. **同一事实两个来源**：凡关键结论都要能交叉验证（`updates.jsonl` vs `events.jsonl`；
   派生层 vs 原始层）。`facts.json` 的每个数字都带 `sources` 指针。
   **实测已抓到一处两源打架**（同一会话）：`events.jsonl` 里 `tool_completed` 有
   **7 次 `outcome="error"`**（另 263 次 success），而 `signals.json` 的
   `toolFailureCount` 只有 **1**。两个数都不假——前者是工具级非零退出，后者大概只统计
   冒泡成回合错误的那些。派生层必须**两个都报、并标注各自来源**，不许二选一，
   否则 agent 会拿着一个自相矛盾的数字去下结论。
3. **只标注、不偷偷截断**：任何裁剪都留显式标记 `<<truncated: N bytes, full: raw/<file>#L<line>>>`
   （与仓库既有 `<<ccr:HASH>>`/压缩账本的约定一致）。截断策略优先"头+尾+总量"。
4. **采集不改现场**：只读打开；不写回会话目录；不重放会话（`/dump` 不触发任何模型调用）；
   采集顺序确定（同一输入两次 dump 的 `facts.json` 除时间戳外逐字节相同，可做回归断言）。

### 让"agent 友好"（六条）

1. **单一入口**：`INDEX.md` 第一屏就够决策；`facts.json` 是它的机器可读孪生。
2. **预算写明**：文件头部直接标注自身字节数与估算 token，agent 一眼知道能不能全文读。
3. **每个数字可追溯**：`{file, line}` 或 `{file, offset}`，杜绝"信我"式结论。
4. **预置异常指针**：`anomalies[]` 让 agent 不必自己遍历 25 MB 才发现那一个 4.3 MB 输出。
5. **稳定 schema + 版本**：`schema_version` 单调递增，字段只增不改义；JSONL 形态便于 grep/jq。
6. **脱敏在采集时完成并显式声明**：`manifest.json` 列出被抹掉的字段名（不给值），
   免得 agent 花轮次去找密钥、也免得 dump 分享出去泄密。

## 五、采集范围与脱敏

| 数据 | 默认 | 理由 |
|---|---|---|
| 对话两通道全文 | 保留 | 就是现场 |
| 工具输入 / 输出（`terminal/<id>.log`） | 保留（超阈值只入索引 + 指针） | 定位工具层故障必需 |
| 错误文本、HTTP 状态与 body（非 HTML 部分） | 保留 | 400/401 类问题只看这个 |
| 模型名 / 版本 / 后端 / base_url 主机名 | 保留 | 复现前提 |
| 工作区路径 | 保留（`--redact=strict` 可切） | 会话目录名本身已编码路径，切了也只是形式 |
| API key / Bearer 头 / cookie / 部署密钥 | **一律剥离**，只记"存在"与截断前缀(≤4 字符) | 分享与外泄风险 |
| `sampling.jsonl` 里的 `auth_prefix`(20 字符)/`x_api_key_prefix`(12 字符) | **一律剥离** | 已是部分明文，不该随 dump 走 |
| 代理 URL 中的凭据 | 剥离 | 同上 |
| `.env` 类文件内容 | 不采集 | 与产品既有读权限规则保持一致 |

`--redact=strict|standard|none` 三档：`strict` 额外切路径与主机名（对外分享用），
`standard` 为默认（剥密钥、留路径），`none` 仅供本机自用（并应在 `manifest.json` 里显式警示）。

## 六、施工分期

| 期 | 内容 | 落点 | 验收 |
|---|---|---|---|
| **T1** | 派生层生成器：读会话目录 → `INDEX.md` + `facts.json` + `manifest.json`；CLI `grok2 dump <sid> [--out DIR]` | 新 crate `xai-grok-dump`（只依赖 `xai-grok-shell` 的 storage/replay 读路径），pager CLI 挂子命令 | 对 `01a0d161` 产出与本文附录实测一致；`facts.json` 二次运行幂等；单测覆盖 join（两套 id 命名空间）、repeat_ratio、marker 普查 |
| **T2** | 出站请求体落盘（**当前唯一答不了 MiMo 三问的缺口**）：在 `sampler` 的 `request_body` 构造点后追加一条脱敏后的日志事件，受独立开关控制（建议 `GROK_DUMP_WIRE=1`，与 `--log-sampling` 解耦），写**按会话/按请求分文件**而非共享滚动日志 | `crates/codegen/xai-grok-sampler/src/client.rs`（`chat_completions` / `responses` / `messages` 三处）+ 新 writer | 请求体可据 id 与 `updates.jsonl` 对齐；脱敏断言（无 `sk-`/`Bearer `）；开关关闭时零开销（快路径早退） |
| **T3** | `/dump` TUI 命令：`Action::DumpEvidence` + `dispatch`，复用 `/export` 的 fs 层（tilde/mkdir/错误不回声用户路径）；支持 dump**当前活跃会话**（故障现场常常已经不可交互） | `slash/commands/dump.rs`、`app/dispatch/`、`app/actions.rs` | 命令注册与补全；无会话时报错文案对齐 `/export`；dump 期间不打断当前回合 |
| **T4** | `--bundle` 打包（对齐 `trace` 的 tar 形态）与三档脱敏落地 | `xai-grok-dump` | 打包后解压可独立读；`--redact=strict` 下路径/主机名不可见 |
| **T5** | **回归夹具**：把一个已知故障会话（think_split 折叠现场、或 MiMo 循环现场）作为 golden 输入，断言 `facts.json` 的关键字段（通道字节比、marker 计数、repeat_ratio、悬空调用数） | 测试 | 夹具进 CI（注意：CI 当前不跑本地主题测试，见 AGENTS.md §11） |

T1 与 T2 相互独立，可并行；T3 依赖 T1；T4/T5 收尾。**建议先做 T1**：它立刻把
现有 25 MB 现场变成 4.8 KB 可读物，也是 T2 产物的天然容器。

## 七、与 `trace` 的关系（不重复造）

- `trace` 保持现有语义：**全量 + 上传**，面向支持与远程排障，不引入聚合与预算（它是"留档"）。
- `dump` 是 `trace` 的**可读层 + 选择性子集**：同源（会话目录）不同用途。
- 实现上应复用 `build_session_tar` 与 `add_directory_to_tar` 的打包路径，
  避免出现第二套 tar 逻辑（T4 的 `--bundle` 直接调它再加派生层）。

## 八、待定决策（需拍板）

1. **命名**：`/dump` 与 `grok2 dump` 是否沿用（`dump` 在仓库里已被当动词用于 debug 场景，
   但作为命令名不冲突）；备选 `/evidence`、`/forensics`。
2. **默认是否带 `raw/`**：带则目录 25 MB、体感重；不带则下钻断链。倾向**默认带硬链接**
   （同卷零成本），跨卷时降级为"仅索引 + 源路径指针"并在 `INDEX.md` 说明。
3. **`sampling.jsonl` 的 5 MB 滚动上限**：是否改为按会话分文件。不改的话，
   长会话的早期线级证据在采集时早已被裁掉，`--wire` 只能是 best-effort。
4. **请求体落盘的粒度与体积**：逐请求全量会显著增大落盘量（本会话 240 次调用、输入合计 4,000 万 token，
   全量 body 落盘是 GB 级）。候选：只落最近 N 次、只落在失败/重试/空响应触发的请求、或只落"结构摘要 + 哈希"。
   **这一条直接决定 T2 的可行性与成本，建议单独讨论。**
5. **是否纳入 `/feedback` 草稿流程**：反馈草稿与本 dump 有重叠（都需脱敏现场）。

## 九、纪律与验证要求

- 一切改动走 feature 分支，`docs-local/PATCHES.md` 登记（T2 改的是采样热路径，属同步文件高复发区）。
- 新增 crate 要过 `scripts-local/ctest.sh -p <pkg> --lib`（L0 白名单需登记）与 `cargo check`。
- `facts.json` schema 变更必须带 `schema_version` 递增 + 夹具同步更新。
- 不得为了"agent 友好"而静默丢信息：任何裁剪都留标记与原始指针（本文 §四.3）。

## 附录：本次调研的实测数据与原型

原型脚本（本次会话新增，未纳入施工范围）：`logs/dump-proto.py`，输入一个会话目录，
输出 `INDEX.md` / `facts.json` 的文本与字节数。对本机会话
`01a0d161-ff0b-7e21-bf5c-72c1fb131272` 的实测：

```
raw total : 25,560,362 B / 168 files
INDEX.md  : 1,787 B
facts.json: 3,043 B
ratio     : 5292x
```

关键实测结论（写入 §一层级设计的依据）：

- 13 轮 / 270 次工具调用；失败数**两源不一致**（`events.jsonl` 7 次 `outcome=error`，
  `signals.json` `toolFailureCount` 1）；`distinct_inputs` = 270（**repeat ratio 1.0x**，非循环）。
- 通道字节：thought 478,932 / message 14,476 / user 1,493 → 思考通道比正文大 33 倍。
- 伪标记：`<tool_call>` 思考通道 7 次 + 正文 1 次；`<function=` 5 + 1；`<think>` 字面量思考通道 278 次。
- 逐轮用量可见"洪水"指纹：第 6 轮 65 次模型调用 / 输入 10,935,344 token / 输出 63,152 / 674 秒；
  第 1–4 轮 reasoning token = 0，第 5–6 轮才出现 27,400 / 45,004。
- 噪声量化：`events.jsonl` 131,858 行中 130,299 行是 `phase_changed`（98.8%）。
- 体量集中在离群点：`terminal/` 共 4,563,764 B，其中一个文件 4,325,376 B（95%）。
