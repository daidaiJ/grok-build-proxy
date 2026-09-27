# Qwen Code Vision Bridge 对照

> 本地克隆：`D:/CODE/ai/qwen-code`
> 阅读日：2026-09-27（与专题评估同期，晚于 `docs-local/agent-cli-tracking/notes/qwen-code.md` 的 2026-09-19 总览）。
> 用途：本 fork 施工时对照语义。不把 TS 源码搬进 Rust。

## 源码与设计文档

| 件 | 路径 |
|---|---|
| 转写核心 | `packages/core/src/services/visionBridge/vision-bridge-service.ts` |
| 工具结果 | `packages/core/src/services/visionBridge/tool-result-vision-bridge.ts` |
| 图 part 工具 | `packages/core/src/services/visionBridge/image-part-utils.ts` |
| 常量 | `packages/core/src/utils/vision-bridge-constants.ts`（最多 4 张；单张 base64 ~9.9 MiB） |
| 用户图接线 | `packages/cli/src/ui/hooks/use-llm-stream.ts`（`applyVisionBridgeIfNeeded`）、ACP `packages/cli/src/acp-integration/session/Session.ts`、非交互 `nonInteractiveCli.ts` |
| 调度器入口 | `packages/core/src/core/coreToolScheduler.ts`：`bridgeToolResultImages` |
| 设置 | `visionModel`；`/model --vision` |
| 设计 | `docs/design/2026-07-13-pdf-vision-bridge-fallback.md`、`2026-07-21-tool-result-vision-bridge.md`、`full-turn-multimodal-routing.md` |

## 门控

`shouldRunVisionBridge`：主模型有效输入模态 `image !== true`，且 `getDefaultVisionBridgeModel()` 有值。

自动挑选：只在**同一 endpoint / 同一 authType** 的目录里找识图模型；找不到就关桥。显式 `visionModel` 视为授权跨供应商，notice 必须带 endpoint host。

识图判定：`isVision === true` 或 `modalities.image === true`，否则按模型名默认表。

整回合接管另要 `capabilities.agent === true`，且不是 fastOnly / voiceOnly / imageOnly。

## 三条路径

1. **用户输入**：有图且应跑桥 → 转写，替换 parts，去掉 inline 图。若视觉模型 agent-capable 且调用方能切模 → 保留原图、本回合改走该模型。
2. **工具结果**：共享 helper，scheduler 与 ACP 执行器共用。识图主模型原样通过。否则逐条 `functionResponse` 转写，描述追加到 `output`/`error`，嵌套 image part 删除。失败：成功的 tool call 仍成功，图改成 unavailable note。
3. **PDF**：仅当文本抽取失败或单页仍超 ~12k token。最多渲 4 页；转写按原 PDF 页码分段。失败恢复**原始 PDF 错误**，渲页不到达文本模型。普通 PNG 的 `read_file` 不在工具内调 VL，只在文本主模型+有桥时保留像素给路径 2。

回合预算：用户图 / PDF / 工具图共用一个 cap（keyed 在 AbortSignal 上）。投机 follow-up 只剥图、不调 VL。

## 失败与披露

结果三态：`ok` / `failed` / `skipped`。失败仍 `applied: true` 且 parts 无图。超时 30s、最多 2 次。输出 cap 2048 tokens。

Notice 含：张数、省略数、模型 id、host、是否已 egress。转写块带 untrusted 声明，并禁止再 `read_file` 去打开转写里的路径。

Prompt：图内文字是 DATA；不要回答用户问题；用户问题只作 focus hint（截断 2000 字）。

## 对本 fork：抄 / 不抄

**抄语义**

- 门控看主模型识不识图，不看套件名。
- 任何将进入文本模型请求的 image part 必须转写或剥除（用户图不够，工具图也要）。
- 失败剥图、回合继续、tool 仍算成功。
- 显式配置才跨供应商；notice 带 host。
- 转写标 untrusted；图内文字不当指令。
- 普通 `read_file` 图不在工具内部调 VL，收口在写入模型上下文前。

**不抄实现**

- Gemini `Part` / `inlineData` 形状（Grok 已有 `ContentPart::Image` + `ImageContent`）。
- 整回合 `model\0baseUrl\0` 选择器与关 compact。
- 30s 超时、用户图 4 张上限、同供应商自动挑模型、`/model --vision`。
- OpenTUI / web-shell / daemon 多面 parity（本 fork 只有 pager TUI + ACP）。

**已有、Qwen 没有的**

- 会话级描述缓存（内容+prompt 指纹）。
- 图落盘到 `session/assets` + `<image_files>` 路径信封。
- 240s / 4096 tokens / 用户图 16 张。
- `finalize_image_describe_sampler_config` 对 BYOK 端点的 404 防护。
