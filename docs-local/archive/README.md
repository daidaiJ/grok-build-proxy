# 归档（已收口的一次性记录）

> **维护约定（2026-09-30 定）**：`docs-local/` 根层与专题目录只放**活跃**文档——
> 有未完成待办、仍会被后续施工引用的规约/账本、或持续追加的 wiki。问题排查、
> 事故复盘、审计报告这类**一次性记录在收口（修复落地并验证）当天移入本目录**，
> 防止前两级目录被过时内容堆满造成上下文腐坏。
> 归档 ≠ 废弃：文内结论仍可引用，跨文相对链接在同目录内依然有效。

> **索引只追加（append-only）**：新归档条目一律**追加在表尾**，禁止插入中间、
> 重排序、或修改已有条目的任何字段（含收口原因）；写错就往下补一条勘误条目，
> 不回头改。归档文档本身同理——收口后发现的补充以"后记"小节追加在文末，
> 不改正文。下表条目顺序 = 归档时间顺序（早 → 晚）。

| 文件 | 收口原因 | 归档日期 |
|---|---|---|
| [ci-cache-incident.md](ci-cache-incident.md) | Windows release 缓存事故（9→57 分钟）：对策（sccache/rust-cache 配置）已落地并复核，事故记录留档供再动 CI 时对照 | 2026-09-30 |
| [ci-failures-2026-09-20.md](ci-failures-2026-09-20.md) | 一次性 CI 失败交接，PATCHES.md 已完成对账（"没编 sampling-types"假设被推翻） | 2026-09-30 |
| [think-split-quoted-marker-fold.md](think-split-quoted-marker-fold.md) | `<think>` 误折叠已修复并发版（`v1.0.37-preview.6`，活回合验证通过）。**注意：`.handoff/` 目录未入库已不存在，本文是该主题唯一存活的完整记录** | 2026-09-30 |
| [utf8-boundary-audit-2026-09-20.md](utf8-boundary-audit-2026-09-20.md) | think_split 中文边界 panic 已修 + 全库 UTF-8 字节切片审计完毕，无未处置站点 | 2026-09-30 |
| [compaction-dangling-toolcall-400.md](compaction-dangling-toolcall-400.md) | 压缩请求悬空 tool_call 400 已修（prep 补答兜底），验证记录在文内 | 2026-09-30 |
| [mimo-v26-toolcall-loop-analysis.md](mimo-v26-toolcall-loop-analysis.md) | MiMo v2.6 工具调用洪水的根因分析；属上游/网关侧问题，本仓库侧（压缩 400）已修、其余无活跃工作流。留作 MiMo 系模型再接入时的对照 | 2026-09-30 |
| [qwen-code-mimo-xml-400-source-study-2026-09-22.md](qwen-code-mimo-xml-400-source-study-2026-09-22.md) | 姊妹篇源码调研，随 mimo 分析同组归档。⚠️ 文内引用的 `third-party-model-toolcall-issues-2026-09-22.md` 从未入库且已不在工作区，链接是死的 | 2026-09-30 |
