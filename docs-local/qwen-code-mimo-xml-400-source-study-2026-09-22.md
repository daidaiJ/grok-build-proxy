# qwen-code 源码调研：MiMo 工具调用泄露 / XML 输出隔离 / 400 会话恢复（2026-09-22）

- 源码基准：`D:\pro\qwen-code`，main @ `64dd058`（2026-09-22 检出，只读调研）
- 关联记录：[third-party-model-toolcall-issues-2026-09-22.md](third-party-model-toolcall-issues-2026-09-22.md)（Grok Build 侧现象、curl 实锤与 issue/PR 清单；本文是其源码深挖姊妹篇）。
  注：该文**不在 main 上**——它所在的 tool-call 提交线已从 main 剃掉，现存于
  `backup/main-before-mimo-strip-20260922` 与 `fix/local-toolcall-issues-20260922`；main 上
  的对应记录是 [mimo-v26-toolcall-loop-analysis.md](mimo-v26-toolcall-loop-analysis.md)。
- 回答两问：① 工具调用泄进推理/正文、原始 XML 泄露给用户与 agent 上下文，qwen-code 怎么处理；② kimi-k3 400 后会话为何恢复不了，qwen-code 有哪些恢复机制、缺口在哪。

## 概览：故障形态 → 机制映射

| 故障形态 | 机制 | 挂载点 |
|---|---|---|
| tool_calls 泄进 `reasoning_content`（MiMo FAQ 自认的不稳定） | 请求侧强制回放 reasoning_content，缺失注入空串 | `provider/mimo.ts`、`provider/utils.ts` |
| 正文吐原始 `<invoke name=…>` XML（应走结构化 tool_calls） | XML 工具调用恢复 fallback，恢复后重写历史 | `core/xml-tool-call-fallback.ts`、`llm-chat.ts:6330` |
| 协议标签 / JSON 形态 tool call 泄露进用户流 | `LeadingProtocolTagLeakDetector` 前缀缓冲判定 | `llm-chat.ts:1684-1780` |
| 畸形 tool call（空 name / 坏 args / 坏 index） | 流式暂扣 + 入历史前拒收 + 独立预算重试 | `converter.ts:1885`、`streamingToolCallParser.ts` |
| 已入历史的孤儿 tool_call（无配对 tool 响应） | 请求构建期清洗 | `converter.ts:2100-2270` |
| 400 后会话恢复 | fail-fast + 三个带内特判（thinking / media / 溢出压缩） | `llm-chat.ts:5204`、`pipeline.ts:1651` |

---

## 一、MiMo provider：请求侧适配

`packages/core/src/core/openaiContentGenerator/provider/mimo.ts`（`determineProvider` 在
`openaiContentGenerator/index.ts:88` 优先命中）：

1. **识别**：baseUrl hostname 等于/后缀于 `xiaomimimo.com`（后缀匹配有防伪装测试，
   `mimo.test.ts:66-75`：`xiaomimimo.com.evil.example` 不命中），或模型名 `mimo-*` 前缀
   （自定义网关场景）。
2. **reasoning_content 强制回放**：`buildRequest` 对每条历史 assistant 消息调
   `ensureReasoningContentOnAssistantMessage`（`provider/utils.ts:11-25`）——缺
   `reasoning_content` 字段就补 `''`。注释原文：思考模式的 OpenAI 兼容 API 要求每个历史
   assistant 轮都带回该字段，即使当轮没有可见推理文本。这正是 MiMo 官方"多轮保留全部
   reasoning_content"要求的落地；缺字段的后果是 400 或工具调用行为退化。
   测试覆盖三种形态：tool-call 轮缺则注入、有则原样保留、纯文本轮也注入
   （`mimo.test.ts:100-168`，与 DeepSeek provider 同构，共享同一工具函数）。
3. **反向对照**：Mistral/Cerebras 收到该字段会 400，由各自 provider 用
   `stripReasoningContent` 在出站边界剥掉（`provider/utils.ts:31-44`）。即该字段是 400
   雷区，注入还是剥离完全按 provider 分派——fork 移植时这是 per-provider wire 修正的样板。
4. **splitToolMedia 默认 true**（`mimo.ts:66-72`）：tool 结果里的媒体拆成后续 user 消息，
   满足严格 chat 请求；用户显式配置优先。

**边界（重要）**：qwen-code **不**从推理内容里恢复工具调用。converter 把
`reasoning_content` 转成 thought part（`converter.ts:1363-1383`），而下文 XML fallback 只扫
可见正文（`contentText` 过滤 `thought`）——泄进推理里的 tool call XML **不会被恢复**。
缓解靠三层：请求侧回放卫生（上面第 2 条）、结构化 tool_calls 通道保持权威、以及
"思考完没调出工具"时的回合级重试（见第四节 NO_TOOL_RESULT_PROGRESS）。

---

## 二、正文 XML 工具调用恢复（死调用不断链）

模块：`packages/core/src/core/xml-tool-call-fallback.ts`（PR #8037 引入，issue #8003）。

**触发五门**（`llm-chat.ts:6334-6341`）：`streamError === null`、本轮无结构化 tool call、
有 finish reason、可见正文非空、正文匹配 `<invoke name="…">` 方言。即：模型本该调工具却把
调用写成了正文。

**解析细节（每条都对应一个真实翻车案例）**：
- 实体解码五个预定义实体，`&amp;` 最后解（否则 `&amp;lt;` 错解成 `<`）——不解码则
  `edit` 的 old_string 永远匹配不上文件、`write_file` 把字面 `&lt;` 写进源码；
- 只剥开/闭标签后各一个换行，其余空白保留（#8003：整段 trim 掉了 old_string 的缩进）；
- 围栏代码块（``` / ~~~，CommonMark §4.5 规则）内的 invoke 块跳过——那是"文档化格式"不是
  调用；invoke 块内部行不参与围栏状态（参数值里的三连反引号不会开一个永不关的围栏）；
- 无参数 invoke 块不恢复、保留为正文；
- 参数值只在长得像 JSON（`{`/`[` 开头）时 `JSON.parse`，失败按原字符串保留——避免把文件名
  `"null"`、端口 `"8080"` 强转类型。

**意图守卫**（`tryRecoverXmlToolCalls`，fallback.ts:196-210）：剥掉所有 invoke 块后剩余
prose 占比 > 0.8 → 拒绝恢复（模型在讲解格式，不是在调工具），记
`recovery was rejected (prose ratio too high or no parameterized blocks)` 警告日志。

**恢复后的历史重写**（`llm-chat.ts:6342-6447`），两处防泄露设计直接对应"XML 泄进 agent
上下文"的事故：

1. **统一可见文本谓词** `isVisibleTextPart = (part) => Boolean(part.text) && !part.thought`
   （llm-chat.ts:6197-6211 有事故复盘注释）：早期 `contentText` 计算用
   `part.text && !part.thought`、移除循环却用更严的 `isValidNonThoughtTextPart`（额外排除带
   `thoughtSignature` 的 part），谓词分裂导致"带 thoughtSignature 但非 thought 的 part"被扫到
   却没被移除——**原始工具调用 XML 与恢复出的 functionCall 并存、一起进了持久历史**。
   现在 contentText 计算、移除循环、恢复后重算三处共用同一谓词，保证"参与 contentText 的
   part 集合 = 恢复移除的集合"。
2. **悬挂推理协同**：移除循环前先捕获 `hadTrailingDanglingThought`（真正的流截断悬挂，而非
   移除制造的假尾部），移除后、重插 remainingText 前的那个唯一窗口里调
   `dropDanglingUnsignedTrailingThought(parts, true)`——顺序是承重的：先重插文本会让悬挂
   episode 不再是尾部，检查 no-op，落盘成 `[thought(unsigned), text, functionCall]`，
   被"拒绝尾随无签名推理"的 provider 永久 400（测试 `llm-chat.test.ts:23035/23105/23232`
   分别锁定：前序签名 episode 保留、悬挂无签名丢弃、签名尾随保留）。
3. 合成 chunk（恢复出的 functionCall）延迟到所有 throw 点之后才 yield——协议泄露/流校验
   失败走重试路径时不会执行两遍恢复出的调用。

**残余泄露面（诚实边界）**：prose 守卫拒绝恢复时，XML 原文留在正文里照常展示给用户；
`remainingText` 只回收被恢复的参数化 invoke 块。另外当前方言仅
`<invoke>/<parameter>`，不覆盖 `<tool_call>` 序列化文本（#10692 指出系统提示词还主动教授该
格式）——Grok Build 侧观察到的泄漏形态落在这个缺口里。

---

## 三、输出边界：用户流与 agent 上下文双隔离

**用户可见流**：
- `LeadingProtocolTagLeakDetector`（llm-chat.ts:1684-1780）：流式缓冲判定。前缀为
  `<analysis`/`</analysis`/`<summary`/`</summary`（内部协议标签）→ leaked，缓冲整段吞掉；
  以 `{`/`[` 开头进入 json 态继续缓冲，`finish()` 时用 `hasLeakedToolCallTags` 扫
  `}`/`]` 后跟闭合序列（regex 源码：`/[}\]]\s*<\/parameter>\s*<\/function>/`，带 JSON
  字符串感知，字符串里的该序列不误判）判定"JSON 形态的 tool call 泄露"→ leaked。
  判定期间 `accept()` 返回空串，用户流看不到泄露前缀（llm-chat.ts:5956）。leaked 且无
  tool call → `PROTOCOL_TAG_LEAK` InvalidStreamError；leaked 且有 tool call →
  `pendingProtocolParts` 整体丢弃（llm-chat.ts:6162-6166），畸形协议片段既不给用户看也
  不进历史。
- 正文中的 invoke XML 由第二节的恢复/移除回收。

**agent 上下文（历史/JSONL/resume）**：
- `contentText` 只取非 thought part，恢复重写后落盘 `consolidatedHistoryParts`；
- `stripThoughtPartsFromContent`（llm-chat.ts:1663）在压缩路径第 2 步剥离 model 轮的
  thinking parts（llm-chat.ts:2839-2845，"Step 2: strip thinking parts from model turns"），
  另一处在 5679；
- 会话标题/摘要/续写投机分别在 `sessionTitle.ts:270`、`sessionRecap.ts:134`、
  `followup/speculation.ts:286` 剥离工具调用与隐藏推理；
- 导出（`omni/export.ts:45`）明确只导出可见文本，thought parts 排除。

---

## 四、畸形 tool call 拦截与 400 会话恢复（k3 相关）

### 4.1 kimi-k3 是什么

`providers/presets/moonshot.ts:32-47`：`thinkingMandatory: true`（思考无法关闭，线上永不发
disable 形状）、`reasoning_effort: low|high|max` 默认 max、`canDisable: false`、1M 上下文。

### 4.2 入历史前：畸形 tool call 拒收

`converter.ts:1857-1898` + `StreamingToolCallParser`（`streamingToolCallParser.ts:345-352`）：

- 流式过程中出现 nameless tool call / 冲突 identity / thinking 标签候选时，
  `shouldHoldParts` 把 parts 扣进 `pendingUntrustedResponseParts` 暂存区——**不进用户流、
  不进历史**；
- `finish_reason` 到达时终检：`hasInvalidToolCallIndex()` ∥ `hasNamelessToolCall()` ∥
  (`finish_reason === 'tool_calls'` 且 0 个完整调用) → 抛
  `InvalidStreamError('Model response contained a malformed tool call.', 'MALFORMED_TOOL_CALL')`；
  另有 args 非法（`hasInvalidToolCallArguments`）参与 thinking 标签消毒分支判定；
- Anthropic 协议侧同样抛 `MALFORMED_TOOL_CALL`（`anthropicContentGenerator.ts:1227`），
  跨协议一致。

**处置**：`InvalidStreamError` 走独立重试预算 `INVALID_STREAM_RETRY_CONFIG`
（llm-chat.ts:672-678）：transient 类 **4 次**、2s 起步退避；重试前
`popPendingPartialAssistantTurn()` 弹掉部分轮，历史干净重来（llm-chat.ts:4411-4458）。
预算耗尽 → break → 回合失败上抛。**只要走流式路径，空 name/坏 args 的调用根本进不了历史**
——这是与 Grok Build Issue 2（畸形调用已入历史 → 重放 400）的本质差别。

### 4.3 请求边界：孤儿 tool call 清洗（治标不治本的那一环）

`converter.ts cleanOrphanedToolCalls`（约 2100-2270，出站构建 messages 时执行）：

- assistant 的 `tool_calls` 只保留**有相邻配对 tool 响应**的（`validToolCalls` 过滤）；
- 全部孤儿 → 删 tool_calls 字段、保留正文/推理内容（converter.test.ts:3765/3800 锁定）；
- 孤儿 tool 响应、孤儿 split-media 消息一并删除；连续 assistant 先合并再清洗。

**能力边界**：它只按"有无配对响应"过滤，**不校验 name/args 合法性**。一条已入历史、带
响应的空 `function.name` 调用会原样重放 → provider 400 → 会话卡死。这正是关联文档 Issue 2
的根因链；上游修复是 PR #11158（改 converter + streamingToolCallParser + llm-chat，"良性
畸形调用就地恢复、不再整轮失败"，关联 #10689，据关联文档为 OPEN、Kimi K3 实测触发）。
本次检出的 main 尚是"拒收 + 4 次重试"行为，未含就地恢复。

### 4.4 400 的分类与带内恢复（"恢复"恢复在哪）

**第一道门——重试分类**（`llm-chat.ts:5204-5213`）：status 400 只有
`classifyRetryError(...).kind === 'transport'`（无响应体、裹着底层网络错误的伪 400）才进
`retryWithBackoff`；真客户端 400 快速失败——重发同一 payload 对内容性 400 无意义，这是
有意设计。

**第二道门——pipeline 两个带内特判**（`pipeline.ts executeWithErrorHandling`，1626-1691），
失败 catch 里先于 `handleError` 尝试改写请求重发一次：
1. `isRequiredThinkingError`（pipeline.ts:238-249）：400 且报文含 `enable_thinking` +
   `must be/restricted to true` → 记入 `requiredThinkingModels`，去掉 disable 形状重发
   （只在上一次 wire 带了 `enable_thinking:false`/`reasoning_effort:'none'` 时才可能触发——
   k3 因 `thinkingMandatory` 永不带 disable 形状，此门对 k3 不开）；
2. **媒体降级**（#10693，端到端测试 `http400-media-wedge.test.ts`）：400 且 wire 请求真带
   inline 媒体（`wireRequestHasMediaContent`）→ `modalities: {}` 降级成占位符重发一次；
   再失败才上抛。
3. **溢出压缩**（llm-chat.ts:3869-3874）：`getContextLengthExceededInfo` 识别出的上下文溢出
   （含 400 报文里 token 上限措辞）与 413 载荷超限（#10380/#10408）→ 一次性压缩后重试。
   这是内容性 400 里唯一有自动恢复的类别——因为它改变 payload。

**第三道门之后**：`EnhancedErrorHandler`（errorHandler.ts）只做日志/脱敏/超时包装，然后
rethrow——无恢复。回合失败；**下一回合原样重放历史**。若 400 由历史内容触发（如空 name
tool_call），且不在上述三个特判内 → 每次都 400 → 会话卡死。microcompact 只清老 tool
**结果**内容，不动 assistant tool_calls 结构，因此也解不开——与关联文档 Issue 2 的 Grok
Build 卡死完全同型。

**qwen-code 的答案总结**：400 会话"能不能恢复"取决于畸形内容有没有被挡在历史之外
（4.2 挡住了流式路径）+ 400 是否命中三个带内特判之一。缺口 = 非流式/旁路入历史的畸形调用
（等 #11158）与三特判之外的内容性 400（靠 compaction 偶然洗掉，无定向修复）。

### 4.5 "思考了但没调工具"的回合恢复

流校验（llm-chat.ts:6459+）：工具结果后的轮若只有 thought、无正文也无新 tool call →
`NO_TOOL_RESULT_PROGRESS` 走 transient 预算重试（注释引 #7039）；预算耗尽后**接受安静完成**
而非失败整个 run（#9026，部分模型家族合法地静默结束）。普通用户轮 thought-only 始终有效。
这是"工具调用失败（压根没发出来）"场景的兜底重试环。

---

## 五、对 grok-build-proxy 的移植要点（增量，接关联文档补丁清单）

1. **入历史闸门优先于出边清洗**：qwen-code 的真正防线是流式暂扣 + `MALFORMED_TOOL_CALL`
   拒收（空 name 根本不入历史），`cleanOrphanedToolCalls` 只是兜底且不校验合法性。fork 应
   先做"装配产物入历史前校验"（关联文档建议 3），再考虑出边 sanitize。
2. **出边清洗最低配**：按关联文档建议 1，发送前把空 `function.name`/坏 args 的 tool_call
   降级为 assistant 文本——这是解锁已卡死会话的最小修复；qwen-code 现有代码在 #11158 合并
   前同样没有这一步。
3. **400 恢复要"改 payload 才重发"**：qwen-code 的模式是 fail-fast + 只在能改变请求形状时
   重发一次（去 disable 形状 / 媒体降级 / 压缩），content 不变的盲目重试被明确禁止。fork 的
   重试器可直接套这个判据。
4. **XML 恢复的三个防误报**照抄：prose 占比守卫、围栏跳过、无参数块保留；两个防泄露照抄：
   单一谓词贯穿"扫描=移除"、恢复 chunk 延迟过 throw 点。
5. **协议标签边界**：`LeadingProtocolTagLeakDetector` 的"前缀流式缓冲 + JSON 形态
   tool-call 闭合标签嗅探"是通用输出边界的可移植实现（对应关联文档 issue #10559 方向），
   比逐格式打补丁收敛。
6. **reasoning_content 注入/剥离按 provider 分派**：MiMo/DeepSeek 注入、Mistral/Cerebras
   剥离，共享一对工具函数；fork 接国模端点时把"该字段是 400 雷区、方向因 provider 而异"
   写进 provider trait。

## 附：本次调研核对过的关键文件

| 文件 | 关注点 |
|---|---|
| `core/openaiContentGenerator/provider/mimo.ts` + `mimo.test.ts` | MiMo 识别、reasoning_content 注入、splitToolMedia |
| `core/openaiContentGenerator/provider/utils.ts` | 注入/剥离一对工具函数 |
| `core/xml-tool-call-fallback.ts` + `.test.ts` | XML 恢复全量逻辑与 #8003 用例 |
| `core/llm-chat.ts`（6150-6530、1684-1780、5204、672、4380-4460、3840-3900、2820-2860） | 恢复集成、协议泄露检测、400 分类、重试预算、溢出恢复、压缩剥离 |
| `core/openaiContentGenerator/converter.ts`（811-870、1340-1420、1840-1910、2100-2270） | assistant 形状、reasoning→thought、MALFORMED_TOOL_CALL、孤儿清洗 |
| `core/openaiContentGenerator/pipeline.ts`（238-249、1591-1691） | thinking/媒体两个 400 带内重试 |
| `core/openaiContentGenerator/errorHandler.ts`、`core/invalid-stream-error.ts` | 无恢复的最终上抛、错误类型集 |
| `providers/presets/moonshot.ts` | kimi-k3 预设 |
| `core/openaiContentGenerator/http400-media-wedge.test.ts` | #10693 媒体 400 端到端复现 |
