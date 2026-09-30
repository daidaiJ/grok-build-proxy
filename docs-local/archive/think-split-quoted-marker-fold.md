# 正文引用的 `<think>` 被当成控制标记 → 回答尾部被折叠成思考块（2026-09-24，fix/local-think-split-code-span）

结论先写：这不是 TUI 的渲染 bug，也不是模型把答案写进了推理通道，而是**本地
`think_split` 切分器把正文字面量当成了控制标记**。修复见 `docs-local/PATCHES.md`
同名条目；本文只承载证据链与复现配方。

## 一、现象（用户视角）

可见回答**恰好在某个落单反引号处截断**，其后的内容（连同 `##` 标题、表格、清单）
整段出现在一个折叠的 “Thought for Xs” 思考块里。用户先后以 `'` 与反引号描述，
实为同一处——折叠发生前，正文最后一个可见字符就是那个反引号。

## 二、判定签名（10 秒判定法）

扫会话落盘事件流 `~/.grok/sessions/<编码 cwd>/<session-id>/updates.jsonl`，找这样的
相邻对：

- 前一条 `agent_message_chunk` 的正文以反引号结尾；
- 紧接的 `agent_thought_chunk` 以反引号开头；
- 且 `<think>` 字面量**两个通道都不出现**。

第三条是关键：`update_chunk_merge.rs` 与 `persistence.rs::maybe_merge_notification`
都只合并同类型 chunk，message/thought 之间不会互相吞字，所以这个 7 字符的缺口只可能
由切分器 drain 掉——排除"上游网关自己把正文塞进 reasoning 字段"（那样标记会原样留在
正文里）。复现扫描（本机 76 份会话，命中 6 处）：

```python
import json, glob, os
BT = chr(96)
for f in sorted(glob.glob(os.path.expanduser('~/.grok/sessions/D%3A*/*/updates.jsonl'))):
    prev = None
    for i, line in enumerate(open(f, encoding='utf-8')):
        u = (json.loads(line).get('params') or {}).get('update') or {}
        name, c = u.get('sessionUpdate'), u.get('content') or {}
        t = c.get('text') or '' if isinstance(c, dict) else ''
        if name in ('agent_message_chunk', 'agent_thought_chunk'):
            if prev and prev[0] == 'agent_message_chunk' and name == 'agent_thought_chunk' \
                    and BT in prev[1][-4:] and t.lstrip().startswith(BT):
                print(f, i, repr(prev[1][-60:]), '|', repr(t[:60]), len(t), '## ' in t)
            prev = (name, t)
```

## 三、现场清单

| 会话 | 行号 | 正文尾部（message） | 思考块开头（thought） | 落入思考块的字符数 |
|---|---|---|---|---|
| `01a0d161` | 43 | `…1. **模板补丁**：强制每轮以 ` | `` ` 开头，修复流式空响应；… `` | 1125（含 `## 四`、`## 五` 两节） |
| `01a0c38e` | 224 | `…reasoning delta(`reasoning_content`、` | `` `)和工具调用参数 delta 只标记… `` | 323 |
| `01a0c38e` | 230 | `…`reasoning_content`/` | `` ` 的 reasoning delta 和工具调用… `` | 355 |
| `01a0c38e` | 327 | `…**只有可见文本 delta 会记流时间戳**——reasoning delta(`reasoning_content`、` | `` `、ThinkingDelta)和工具调用参数… `` | 694（含 `## `） |
| `01a0c38e` | 332 | 同上 | 同上（该轮另一次） | 718（含 `## `） |
| `01a0c849` | 718 | `…**泄漏恢复而非纯剥离**：内容里的 ` | `` ` 标签按思考处理不进输出（`contentOnlyThinkingTagLeaks`… `` | 676 |

六处的拼接口径一致：正文尾部反引号 + **被吃掉的 `<think>`** + 思考块开头反引号。以
`01a0d161` 为例，原句是「强制每轮以 `` `<think>` `` 开头，修复流式空响应」——模型在
回答里引用 chat template 补丁的写法，行内代码跨度里就是那个字面量。

## 四、根因链路（逐跳，可对照源码）

| # | 位置 | 行为 |
|---|---|---|
| 1 | `xai-grok-sampler/src/stream/chat_completions.rs:59` | 每条 chat-completions 响应建一个 `ThinkTagSplitter`（第三方端点都走这条路径） |
| 2 | 同文件 `:226` | 每个 `content` delta 交给 `splitter.feed()` |
| 3 | `stream/think_split.rs` `feed` 的 `find_earliest(&buffer, &OPEN_TAGS)` | 在缓冲区**任意位置**匹配 `<think>`/`<thinking>`，不看 markdown 上下文 → 引用被当控制 |
| 4 | 同文件 `finish` | 流结束时未闭合的块整块 flush 成 reasoning（qwen-code `final` 语义），剩余 token 全数改道 |
| 5 | `chat_completions.rs:246-262` | 改道部分发成 `ChannelToken{ channel: Reasoning }` |
| 6 | `xai-grok-shell/.../sampling_events.rs:213-239` | `Reasoning` → `send_thought_chunk` → `AgentThoughtChunk` |
| 7 | `xai-grok-pager/src/acp/tracker.rs:1123` | `handle_thought_chunk` 追加进 `RenderBlock::Thinking` |
| 8 | `blocks/thinking.rs:553` | 回合结束 `finished_display_mode() = Collapsed` → “Thought for Xs” |

## 五、为什么是"引用 vs 控制"的歧义

同一段字节有两种相反语义：网关把推理块内联进 `content`（控制标记，必须改道），模型在
正文里写文档／复现／教学时引用它（普通文本，必须留在正文）。纯字符串匹配无法区分，因此
需要一个带内判据——而正文通道本身就是 markdown（`blocks/agent.rs` → `MarkdownContent`），
模型引用含 `<` 的字面量时只有代码跨度与围栏可用。同类先例见
[`qwen-code-mimo-xml-400-source-study-2026-09-22.md`](qwen-code-mimo-xml-400-source-study-2026-09-22.md:62)
（qwen-code 的 XML 工具调用恢复同样跳过围栏内的 invoke 块）。

两条错误方向代价不对称，决定了守卫该站哪一侧：

- 把引用当控制（旧行为）：正文截断 + 后续上千字符被藏进折叠块 → **答案丢失**；
- 把控制当引用（守卫代价）：真标记被当正文 → 推理文本显示为可见文本，**答案不丢**。

## 六、修复与边界

`stream/think_split.rs` 新增 `CodeScan`：按可见文本通道增量跟踪行内代码跨度与
反引号/波浪号围栏，`in_code()` 为真的候选标记按普通文本发回正文。跨度按行跟踪（未闭合
的反引号只致盲本行），围栏到闭合行；跨 delta 的 run 用 pending 状态承载。有意不覆盖：
其它引用形态（`"<think>"`、`**<think>**`）仍按标记处理；position 口径（只在回合开头认
标记）因会推翻既有 `text_before_think_block_stays_text` 用例而未采用。

验证：`ctest.sh -p xai-grok-sampler --lib` 272 例全过（新增 9 例，其中 5 例在把
`in_code()` 临时置恒 `false` 后立刻转红，守卫承重有反证）。

## 七、现场验证（release 产物上的活回合）

同一条提示词（让模型在正文里用行内代码引用标记），分别在 `v1.0.37-preview.5`（修复前）与
`v1.0.37-preview.6` 产物上跑活回合：preview.5 命中本签名——正文 83 字符截断于落单反引号、
其后的 466 字符（含 `## 四`/`## 五`）整段改道 reasoning；preview.6 同提示词正文 562 字符
完整、签名 0 命中。围栏轮（fenced 代码块内标记 + 行内跨度）全文留在正文；裸
`<think>…</think>` 真标记仍整块改道 reasoning，说明真标记路径未被误伤。TUI 轮用 `--minimal`
（scrollback 原生渲染）捕获：回合结束的折叠思考块（“Thought for 3.9s”）里只有英文规划，
答案正文（含两节标题与标记字面量）在折叠块之外完整显示。`"<think>"` 与 `**<think>**` 当场
复现折叠，与第六节「有意不覆盖」一致。七轮对照表、复现脚本与产物路径见
`.handoff/think-split-quoted-marker-fold.md` §13。
