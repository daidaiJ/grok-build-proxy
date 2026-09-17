# 工具输出智能压缩 — 主干规划

> 状态：一期已落地（简化实现，未引入 only-cc-lite git 依赖）。显式
> `[tool_output_compression] enabled = true` 才生效；开关在**会话启动时快照**，
> 只作用于该会话整段生命周期，不改写已入库的 tool_result（提示缓存前缀字节稳定）。
> `/stats` 与 `grok stats` 展示正向节省、负向回灌/无收益，以及 CCR 额外 I/O 延迟。

## 目标与定位

在 grok CLI 的 ToolResult 管道内（进程内、结果层）压缩工具输出，降低 context 压力、
推迟 auto-compact 触发。**默认关闭，per-toolset 开关，可逆。**

## 四条硬约束（全部来自上游事故，见 PATCHES.md「设计约束来源」）

1. **只压结果，不碰定义** — 工具 schema/description 是 `search_tool` BM25 延迟加载的
   索引面，压缩层永远不得触碰（headroom #746：代理触发客户端全量物化工具 schema，+25K 基线）。
2. **进程内，不走外部代理** — 不做改 base_url 的 wrap/proxy 形态（#746 / #1869 wrap 对
   opencode 零节省 / #987 按协议漏压，三种死法全是外挂层结构性问题）。
3. **检索纯 pull 式** — 压缩注入 `<<ccr:HASH>>` 类标记，模型需要原文时主动调
   `expand_output(hash)` 工具取回；严禁代理侧主动回注（#2186：CCR 主动回注旧摘要，token 反涨）。
4. **无损字段白名单** — `exit_code`、`stderr`、错误堆栈尾部、退出信号直通不压
   （本机 rtk 实测教训：压缩层报 "Go build: Success" 而退出码非 0）。

## 策略来源

直接依赖 `daidaiJ/only-cc-lite`（Rust 库，零 ML 依赖，git dependency 可直接引入；
headroom 的裁剪版，策略已按编码场景取舍）：

| 内容类型 | only-cc-lite 模块 | 典型压缩率 |
|---|---|---|
| JSON 数组 | `transforms/smart_crusher`（serde_json 解析） | 60-90% |
| 构建/日志 | `transforms/log_compressor`（ERROR/WARN/Traceback 感知） | 50-80% |
| 搜索结果 | `transforms/search_compressor`（`file:line:` 模式） | 50-80% |
| Git diff | `transforms/diff_compressor` + `unidiff_detector` | 40-60% |
| 自动路由 | `transforms/content_detector` | — |
| 可逆存储 | `ccr/`（SQLite + BLAKE3 24hex + TTL，WAL 并发安全） | — |
| 重要性分级 | `signals/tiered` + `line_importance`、`transforms/adaptive_sizer`/`live_zone` | — |

Token 计数用字符密度估算（chars/cpt 按模型族校准），不需要 tokenizer 依赖。

## 目标配置面

```toml
[tool_output_compression]
enabled = false                      # 默认关
scope = ["bash", "mcp"]              # 只压这两类输出；内置工具自有预算
strategies = "auto"                  # 或 ["json", "logs", "search", "diff"]
protect = ["exit_code", "stderr"]    # 无损字段
min_input_tokens = 500               # 低于阈值不压
keep_tail_lines = 20                 # 头尾保留

[tool_output_compression.ccr]
enabled = true                       # 开启才注册 expand_output(hash) 检索工具
backend = "sqlite"                   # 落 xai-sqlite-journal 同款地基
ttl_secs = 3600
```

## 落地切面（合并友好：T2，新模块 + 一处注册）

- **管道位置**：`xai-grok-tools` 的 ToolOutput 持久化/发送前，紧邻现有 truncation。
  新模块 `implementations/output_compression/`（新文件），registry 注册一行。
- **检索工具**：`expand_output(tool_use_id/hash)` 新工具，ccr.enabled 时注册。
- **指标**：压缩节省量并入 signals（`compression.saved_tokens` 计数器），供 /stats 二期消费。
- **验收**：only-cc-lite 自带 benches；新增回归——exit_code/stderr 无损、deferral 索引
  面字节不变、expand_output 可取回原文。

## 明确不做

- 不压模型自己的消息/思考内容（compaction 管道已覆盖会话级压缩）。
- 不压工具定义/描述（约束 1）。
- 不做外部代理形态（约束 2）。
- 一期不接 `relevance/embedding`（only-cc-lite 里的可选模块），bm25/hybrid 留二期。
