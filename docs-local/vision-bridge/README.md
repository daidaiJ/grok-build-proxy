# 视觉桥（Vision Bridge）

> 目的：让纯文本主模型也能处理用户图 / 工具返回图——另开一个识图模型转写成文字，
> 原图不得进入文本 provider。对照兄弟目录 Qwen Code 的 Vision Bridge，落在本 fork
> 已有的 `image_describe` 管线上，而不是整包移植。
>
> **状态（2026-09-27）**：评估完成，**未开工**。本文档是决策留痕 + 施工计划。
> 调研快照 2026-09-27；基线当时 `main`（`git rev-parse HEAD` 以施工日为准）。
> 下文路径与符号名以文本搜索为准，行号会随上游同步漂移。

## 本目录用途

- 收纳本 fork「视觉桥」专题：对照、定案、分期施工与验收。
- **范围**：会话内图像如何到达模型。含用户粘贴/拖图、`read_file` 图/PDF 渲页、MCP 等工具结果里的 inline 图。
- **不做**：整回合把 agent 切到视觉模型（Qwen full-turn takeover）、`/model --vision` TUI、独立 VL 工具、视频/音频。

## 文档导览

- [plan.md](plan.md) — 施工计划：现状锚点、定案、T1–T3、验收、纪律。
- [notes/qwen-code.md](notes/qwen-code.md) — 兄弟目录 `D:/CODE/ai/qwen-code` 的实现对照（抄什么、不抄什么）。

## 一句话结论

本仓库 **Cursor 套件**已经有完整描述桥（`image_describe` + `models.image_description` + 缓存 + 落盘），但 grok-build 把 `is_cursor_harness()` 写死为 `false`，日常路径永远把原图塞给主模型。要做的是把门控从「套件名」改成「主模型是否识图」，并让工具结果走同一套失败闭合。Qwen 贡献的是产品语义，不是代码。

## 分期一览

| 期 | 内容 | 估时 | 依赖 |
|---|---|---|---|
| T1 | 模型 `accepts_images` + 用户图转写（打开已有管线） | 2–4 人天 | 无 |
| T2 | 工具结果 / `read_file` / PDF 渲页：转写或剥图 | 3–5 人天 | T1 的能力位与描述 API |
| T3 | 用户可见披露 + BYOK 默不打 xAI + 文档 | 1–2 人天 | T1；可与 T2 并行后半 |

整回合视觉 agent 接管明确 **不做**，见 [plan.md](plan.md) §不做。

## 分支与纪律

- 功能代码：`feat/local-vision-bridge`（或按期拆 `feat/local-vision-bridge-t1`）。
- main 只收本目录文档。改动同步文件须登记 `docs-local/PATCHES.md`。
- Windows 测试入口：`scripts-local/ctest.sh -p xai-grok-shell --lib <filter>`，禁止裸 `cargo test`。
