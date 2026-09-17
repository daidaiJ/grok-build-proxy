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

## 二期方向：低损/无损组合（2026-09-17 调研定稿）

> 目标升级为**低损甚至无损**。三个信息源：本地机制消融（`mech_ablation_report`
> 测试）、headroom/only-cc-lite/DCP/context-mode 源码、四家社区风评
> （HN/Reddit/GitHub issues）。

### 本地消融结论（token 轴 + 结构保留轴）

| 机制 | 重复型 JSON | 唯一型 JSON | 日志(错误埋中) | 日志(重复) | 保留率 |
|---|---|---|---|---|---|
| 一期基线 head/tail | 92.7% | 93.4% | 78.8% | 72.3% | 重复型 JSON **丢唯一项**；search 每文件 3 条上限**丢尾部命中** |
| M1 精确去重 | 91.3% | 25.1% | — | — | 全保留（无损） |
| M2 adaptive-k（膝点） | 87.9% | 23.0% | — | — | 全保留 |
| M3 重要性打分限预算 | — | — | 93.0% | 92.0% | 全保留 |
| M4 模板折叠 xN | — | — | 93.1% | 92.4% | 全保留（可逆重建） |

关键读数：**唯一型内容"低损=低省"是物理极限**（保全部只能省 23-25%），
无损收益全部来自"重复信息的折叠"；日志类 M4 双轴碾压一期基线。

### 社区风评要点（机制取舍依据）

- headroom #3545：search 压缩熔接行 → **行号↔内容假配对**（agent 行动坐标被毁，
  比丢内容更危险）。#3580：代码当 prose 被 ML 压缩静默删词。#3590：小结构化输出
  压缩后像真数据被误读。#3625：**截断时未写 CCR marker**。#3544/#3560：有 marker
  但客户端调不了 retrieve。#3587：1886 请求零次检索——marker 成本可能白付。
- headroom 无损层 `lossless_compaction.py` 是正解：格式原生（grep 仍是 grep）、
  **运行时往返自校验、失败退回原文**；`cross_turn_dedup.py` 跨轮逐字去重
  （前缀单调 + keep-earliest，缓存字节安全）。
- tsheadroom 保守化配方：不装 ML 通道、只压 tool_result 大块、fail-open、
  实测压缩率 ~40%（vs 宣传 60-95%）。
- brandonbarker 实测：**重写历史 → cache bust 123 vs 14，净省 ≈0**；
  headroom 自报 savings 计量 ~1.9x 高估（/stats 数字只当相对指标）。
- DCP（模型主动 compress + 自动清理）：issue 重灾区——摘要膨胀反烧 738k token
  （#573）、静默丢数据（#534）、原地替换破 cache（#614/#604）、保护白名单
  Windows 路径分隔符从未匹配成功（#592）。模型主动调用的路线**不采用**。
- context-mode（事前沙箱，仅 stdout 进上下文）：理念好、零缓存伤害，但覆盖不了
  MCP 工具，FTS5 召回依赖模型写对脚本，且有删错数据的 bug（evict 排序反了）。
  本 fork 不做——rtk hook 已覆盖同场景。

### 二期组合（低风险排序：无损层 → 低损层 → 有损兜底）

1. **无损层**（新，源 headroom lossless_compaction，全部可逆 + 往返自校验失败退回原文）：
   ANSI 剥离；重复行折叠 `... (repeated N times)`（即 M4 泛化）；JSON 数组精确
   去重 `xN`（M1）；search 结果路径前缀提升为标题（行数全保留，修一期 3 条上限）；
   diff `index` 行剥离。
2. **低损层**（源 only-cc-lite）：折叠后仍超预算时，日志用重要性打分限预算
   （M3，错误/堆栈永远优先于配额）；JSON 用 adaptive-k 膝点决定保留数
   （M2，锚点头尾 + 去重序填充）。
3. **有损兜底**（一期已有，加帽）：generic head/tail 仅在前两层后仍超预算时启用，
   引入 `max_lossy_ratio`（默认 0.25，headroom F2.2 保守档）；**任何截断路径
   必写 CCR marker**（#3625 教训），`expand_output` 注册与 marker 严格同生共死。
4. **不采用**：ML/prose 压缩通道；模型主动压缩（DCP 病全部命中我们约束）；
   回溯改写历史/live_zone 面（cache 经济学实测净亏）；子进程沙箱（rtk 已覆盖）；
   跨轮逐字去重（要动已发送历史，同上，留观察）。

### 配置面增量

```toml
[tool_output_compression]
max_lossy_ratio = 0.25   # 有损兜底最多压到原文的 25%；0 = 只用无损层
# 其余沿用一期；lossless 层随 enabled 总开关生效，不单独设开关
```

验收补充：每个无损变换的往返重建单测；marker-in-all-truncations 回归；
search 行号↔内容不熔接回归。
