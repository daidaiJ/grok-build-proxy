# 代码索引：codegraph + cbm（2026-11-20）

目标：新会话理解本仓库时先查预索引，少 grep、少整文件读。cbm 索引常驻；
codegraph 全量索引在本机试跑后**未保留**（见下节，2026-11-20 清理）。
cbm 缓存在 `~/.cache/codebase-memory-mcp`（本机实际路径以 `list_projects` /
日志为准）；`.codegraph/` 已在 `.gitignore`，需要时再建。

## 分工

| 问题 | 工具 | 说明 |
| --- | --- | --- |
| 架构热点、模块边界、分层、聚类 | cbm `get_architecture` | grep 要十几次调用才能拼出来 |
| 全仓复杂度排行、多跳调用链 | cbm `query_graph` | Cypher，按 `complexity` / `cognitive` 排序 |
| commit 影响半径（diff 驱动） | cbm `detect_changes` | 传完整父 SHA |
| 单符号定义、调用方、callees、影响半径 | codegraph `query` / `callers` / `callees` / `impact` | 需先建索引；本机全量索引不常驻 |
| 受影响测试 | codegraph `affected` | 同上 |
| 读源码验真 | Read | 图谱与代码冲突时以代码为准 |

## cbm（架构层）

- 版本 `0.11.0-fork.4`；项目名 `D-CODE-ai-grok-build-proxy`（由仓库路径派生）。
- 2026-11-20 索引：fast 模式，**136,163 节点 / 1,019,819 边**；94 个文件按设计
  排除；2 个 partial parse（`xai-grok-pager/src/app/dispatch/session/lifecycle.rs`
  999–1000 行、`xai-grok-tools-api/proto/grok-tools.proto` 局部）。采信图谱前先跑
  覆盖闸门。
- 常用命令：

```bash
codebase-memory-mcp cli list_projects
codebase-memory-mcp cli get_architecture '{"project":"D-CODE-ai-grok-build-proxy","aspects":["hotspots","boundaries","layers","clusters"]}'
codebase-memory-mcp cli query_graph '{"project":"D-CODE-ai-grok-build-proxy","query":"MATCH (c)-[:CALLS]->(f) WHERE f.name = \"context_composition\" RETURN labels(c) AS kind, c.qualified_name AS caller, c.file_path AS file, c.start_line AS line"}'
codebase-memory-mcp cli check_index_coverage '{"project":"D-CODE-ai-grok-build-proxy","scopes":["."]}'
codebase-memory-mcp cli index_repository --repo-path . --mode fast   # 手动刷新，约 76s
```

- 陷阱：
  - `Function` 的文件属性是 `file_path`，不是 `file`；拼错属性会显式报错。
  - 调用方可能是 `Method`，查 CALLS 时不要写死 `(c:Function)`。
  - fast 模式会把同名方法聚合（热点里 `RecentProbe.unwrap` fan-in 12850 是例子），
    下结论前用 Read 验真。
  - 空结果先查属性名/coverage，再 grep 复核；`graph=missed` 是覆盖盲区。
  - 原始 JSON 传参有 deprecation 警告；未来版本改用 `--args-file` 或 flag。

## codegraph（符号层）

- 版本 `0.9.7`；索引目录为仓库根 `.codegraph/`（SQLite，已 gitignore），
  本次试跑后已删除，当前没有常驻 codegraph 索引。
- 本机 CLI 陷阱：gash 里直接执行 PATH 上的 `codegraph` 会按 cwd 找
  `node_modules/@colbymchenry/codegraph/npm-shim.js` 并报 `MODULE_NOT_FOUND`；
  用绝对路径 `/d/Programs/nodejsgc/node_global/codegraph`（或 cmd 里的
  `codegraph.cmd`）。
- 常用命令：

```bash
CG=/d/Programs/nodejsgc/node_global/codegraph
$CG status .
$CG query context_composition
$CG callers process_conversation_turn_inner
$CG impact apply_replay_diet
$CG context "cost ledger composition"
$CG sync .            # git pull / 改代码后的增量同步
$CG index -f -q .     # 全量重建
```

- 2026-11-20 全量试跑结果（已清理）：
  - 扫描 3,852 个文件；“Parsing code” 阶段约 75 分钟到 99%，随后进入
    “Resolving refs” 第二阶段并重置百分比，9 分钟后仅 9%，整体超过 85 分钟
    仍未完成，手动停止。
  - 停止时 DB 437 MB、135,849 节点 / 201,651 边（引用未解析完）。符号定义
    查询可用，`callers` 返回空（误判），因此这种半成品索引不可用于调用链。
    `.codegraph/` 已整目录删除，释放约 450 MB。
- 慢的根因：本机 D: 是机械盘（HDD），解析阶段进程 CPU 仅约 100s / 45min，
  主要是 I/O 等待；SQLite 的 WAL + 逐文件提交带来约 12 GB 写放大。
- 本仓库 4,711 个 tracked 文件里 4,478 个在 `crates/`（95%），`crates/codegen`
  也有 4,218 个；按目录“只索引主代码”几乎省不了时间。真要再用 codegraph，
  建议只对单个 crate（如 `xai-grok-pager`）建 scoped 索引，或把索引放到 SSD
  再全量；日常调用链查询优先用 cbm。

## 已验证的查询（2026-11-20）

- cbm `query_graph`：`context_composition` 定义位于
  `xai-chat-state/src/actor/state.rs:113`；调用方正确给出
  `SessionActor.process_conversation_turn_inner`（`turn.rs:2669`）与单测。
- cbm `search_graph`：`process_conversation_turn_inner` 命中 Method，
  范围 `turn.rs:2669-4275`。
- cbm `get_architecture`：能返回 hotspots / boundaries / layers / clusters；
  分层结果含测试夹具目录噪声（`grok/skills/*` 等），看 `crates` 聚类为主。
- codegraph（半成品索引，已清理）：`query context_composition` 能命中
  `xai-chat-state/src/actor/state.rs:113` 与两处测试；`callers
  process_conversation_turn_inner` 返回 0，印证引用解析未完成时不能信调用链。
  索引目录已删除，需要时按上面的 scoped 方案重建。

## 维护

- cbm：会话开始 hook 自动 fast 索引；大改后可手动跑上面的 `index_repository`。
- codegraph：本机不常驻。确实需要符号级查询时，按 crate 建 scoped 索引；
  用完后可 `rm -rf <crate>/.codegraph` 清掉，避免再留下几百 MB 半成品。
- 两者都是本地缓存，不提交；换机或删缓存后按本文重建。cbm 是默认方案，
  codegraph 只在 cbm 不覆盖的单符号/受影响测试场景临时启用。
