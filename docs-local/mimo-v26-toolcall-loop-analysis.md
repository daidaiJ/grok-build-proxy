# MiMo v2.6 工具调用洪水/泄露：根因分析与最小复现方案（2026-09-23）

> 症状：`xiaomi/mimo-v2.6-flash` 在 agent 会话中工具调用泄露/死循环/多轮 400。
> 证据来自社区一手 issue 抓取核实（#2482、#2436 全文已读），非搜索摘要转述；
> 小米官方无根因声明，相关 issue 全部仍开放。

## 根因分析

### 结论

模型侧回归：v2.6 的 agent RL 训练把工具调用倾向推过头，开放型任务下思考通道塌缩；
叠加 agent 框架通用的「整代自由采样、工具结果不回灌」机制，退化为重复调用吸引子。
网关/API 格式严格性（400）与 serving 侧 parser 泄露是伴生问题，不是主因。

### A/B 对照（决定性证据）

MiMo-Code#2482：同机、同仓库、同日上午——

| 模型 | 行为 |
|---|---|
| v2.5-pro | 296 次调用 / 267 种输入，正常完成文档审计 |
| v2.6-flash | 单 generation 4120 次调用 / 仅 35 种输入（同批文件 read 各约 370 次）；127,952 输出 token 中仅 48 reasoning token |

客户端不变、仅换模型即复现 → 权重/训练回归。flash 与 pro 都中招；
常在「v2.5 正常会话之后的第一个开放式 v2.6 turn」爆发。

### 因果链

1. **整代自由采样**：工具结果本地边流边执行，step 结束才随下次请求回灌 → sampler 全程拿不到新信息，重复无外力打断。
2. **reasoning 塌缩**（v2.6 签名）：洪水代 48 reasoning token / 127k 输出。「已读过」的自检在思考通道，塌缩后循环无人踩刹车。v2.5 不塌。
3. **轮转模式击穿检测器**：A,B,C 轮转 + 每次全新 `call_*` id → 「相邻 3 个调用等值」型 doom_loop 永不命中；#2436：loop_streak 若先 hash reasoning 文本，thinking 下每轮 hash 不同，检测被提前短路。
4. **截止形态**：只能等输出上限（131k）自然结束，末调用 JSON 截断成 `invalid`；下一 turn `input 1,028,090 / output 0 / finish=length`，会话砖死（context poisoning，#2436），换模型也救不回。
5. **半答 run → 400**：回合在并行调用仍在飞时中止，历史留下悬空 `tool_calls` → 网关 400。本仓库压缩路径已修（见 `compaction-dangling-toolcall-400.md`），普通回合自愈靠 `repair_dangling_tool_calls`。

### 伴生问题：API 格式严格性（MiMo-V2-Flash#8，OpenRouter/直连均复现）

- 带 `tool_calls` 的 assistant 消息 `content: null` 被拒（OpenAI 规范允许），需 `content: ""`。
- `role:"tool"` 只收字符串 content，数组/parts 形态报 `text is not set`。
- tools schema 多余字段（如数组的 `items`）也 400。首轮正常、第二轮起炸。
- thinking 默认开启，多轮必须回传完整 `reasoning_content`，缺失 → 400 或指令遵循明显退化。
- 自托管（vLLM/SGLang）：tool-call XML 泄进 `<think>` 或正文（parser 只扫 content 段）。

### 官方动作（全在框架侧，模型侧无下文）

MiMo-Code 0.1.15 与 v2.6 同日发布：flood guard（单响应超额调用不再执行）、仅纯读/搜类工具允许并行、失败调用跳过后续依赖；main 分支另有单代 16 次硬上限（#2463，未发版）。

## 最小复现方案

### 路线一：确定性探针（400 格式族，脚本可直跑）

沿用 `compaction-dangling-toolcall-400.md` 已验证的探针模式（同一网关、同一 UA 要求），
三用例只差一个字段，逐例隔离格式敏感点：

```python
#!/usr/bin/env python3
# scripts-local/mimo-v26-format-probe.py <api-key>
# 预期：A=400（content:null 被拒）B=200（content:"" 通过）C=400（tool 数组 content）
import json, sys, urllib.request, urllib.error

BASE, MODEL = "https://api.commandcode.ai/provider/v1/chat/completions", "xiaomi/mimo-v2.6-flash"
TOOLS = [{"type": "function", "function": {"name": "ls", "description": "list dir",
          "parameters": {"type": "object", "properties": {}}}}]

def post(messages):
    req = urllib.request.Request(BASE, data=json.dumps(
        {"model": MODEL, "messages": messages, "tools": TOOLS}).encode(),
        headers={"Content-Type": "application/json", "Authorization": f"Bearer {sys.argv[1]}",
                 "User-Agent": "Mozilla/5.0"})  # 浏览器样式 UA，缺了 Cloudflare 403 code 1010
    try:
        with urllib.request.urlopen(req) as r: return r.status, json.load(r)
    except urllib.error.HTTPError as e: return e.code, json.load(e)

call = {"id": "call_1", "type": "function", "function": {"name": "ls", "arguments": "{}"}}
cases = {
    "A_content_null": [  # OpenAI 合法、MiMo 拒绝的形态
        {"role": "user", "content": "列出当前目录"},
        {"role": "assistant", "content": None, "tool_calls": [call]},
        {"role": "tool", "tool_call_id": "call_1", "content": "a.txt"}],
    "B_content_empty": [  # 对照：仅 content 换 ""
        {"role": "user", "content": "列出当前目录"},
        {"role": "assistant", "content": "", "tool_calls": [call]},
        {"role": "tool", "tool_call_id": "call_1", "content": "a.txt"}],
    "C_tool_parts": [  # 对照：仅 tool content 换 parts 数组
        {"role": "user", "content": "列出当前目录"},
        {"role": "assistant", "content": "", "tool_calls": [call]},
        {"role": "tool", "tool_call_id": "call_1",
         "content": [{"type": "text", "text": "a.txt"}]}],
}
for name, msgs in cases.items():
    status, resp = post(msgs)
    err = (resp.get("error") or {}).get("message", "") if isinstance(resp, dict) else ""
    print(f"{name}: HTTP {status} {err[:120]}")
```

判读：A 挂 B 过 → 确认 content:null 敏感；C 挂 → 确认 tool parts 敏感；全过 → 400 另有因
（大概率悬空 tool_call 或 reasoning_content 缺失，转路线二取证）。

### 路线二：洪水行为触发（非确定性）

#2482 明说没有一行 prompt 能必现。社区最可靠的触发面：

- v2.6-flash/pro + 开放式任务：读多文件仓库并重写文档/做全面调研；
- 常在 v2.5 正常会话后的第一个 v2.6 开放 turn；
- 现象指纹（出现即命中）：单 step 时长逼近 1 小时、输出逼近 131k、reasoning token 极低、
  调用数数百上千且输入种类 < 50、随后 turn output=0。

取证要点：dump 现场请求体，核对 ① `tool_calls`/`tool_result` 配对完整性
② 带调用轮转签名（输入集合是否 < 50 种）③ assistant 消息是否回传了 `reasoning_content`。

## 与本仓库的关联与后续

- 悬空 tool_call 400（压缩路径）：已修 `feat/local-compaction-toolpair-repair`。
- 检测面建议：按「输入签名集合」做循环判定，而非相邻等值——现有等值型检测对轮转模式全盲。
- `/dump` 现场取证命令：方案已备（见会话 plan），未施工；施工后本探针可与 dump 产物互为印证。

## 参考链接

- https://github.com/XiaomiMiMo/MiMo-Code/issues/2482 （洪水法证主帖）
- https://github.com/XiaomiMiMo/MiMo-Code/issues/2436 （loop_streak 被 reasoning 短路）
- https://github.com/XiaomiMiMo/MiMo-Code/issues/2497、#2486、#2475、#2463
- https://github.com/XiaomiMiMo/MiMo-V2-Flash/issues/8 （格式 400）
- r/opencode 2026-09-21 帖、r/LocalLLaMA vLLM 修复帖（社区侧证）
