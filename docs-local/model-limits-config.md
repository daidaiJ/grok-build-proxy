# 自定义模型的 context_window / max_completion_tokens：兜底语义与真实值查法

> 记录日期 2026-09-22。起因：本机 `~/.grok/config.toml` 挂了 7 个第三方模型，**其中一个
> 都没写 `context_window`**，导致 auto-compact 按兜底值提前触发，白白浪费 4/5 窗口。
> 本文记录兜底语义（含代码锚点）与「不烧 token 拿到真实上限」的查法，供后续新增模型时照做。

## 一、`context_window`：不写就走兜底，而兜底值因来源而异

`context_window` 直接驱动 auto-compact 触发点（默认 85%，见
`[session] auto_compact_threshold_percent`）。它不是显示字段，写错就是真金白银的窗口浪费。

**三个兜底值，取决于"这个模型从哪来"**（这是最容易踩的坑）：

| 模型来源 | 兜底值 | 代码锚点 |
|---|---|---|
| 用户在 `[model.*]` 里**新建**的模型（不在内建/prefetched 目录中） | **200_000** | `crates/codegen/xai-grok-shell/src/agent/config.rs:3704` |
| 跟随远端 `/models` 拉取、对端没给 `contextWindow` | **256_000** | `crates/codegen/xai-grok-shell/src/remote/client.rs:627`（`DEFAULT_CONTEXT_WINDOW`） |
| 内建模型 JSON 缺字段 | **200_000** | `crates/codegen/xai-grok-shell/src/agent/config.rs:3702-3704` |

注意 `config.rs:3432` 那条 `tracing::debug!` 只在「新建模型且未写 context_window」时打印
（`default = 200_000` 字样）——**是 debug 级，默认看不到**。所以配漏了通常毫无提示，
除非把日志开到 debug。

对照 `docs/user-guide/11-custom-models.md:117` 的文档声明：

> When you define a new model and omit `context_window`, Grok defaults to **200,000** tokens,
> so set it explicitly to match your provider.

文档说的 200,000 只覆盖了第一种来源；第二种（远端拉取）实际是 256,000，文档未提。
同类不一致还有 `docs/user-guide/11-custom-models.md:100/395` 与
`05-configuration.md:267` 的示例一律写 `context_window = 128000`，而 `xai-chat-state`
内部默认同样是 `128_000`（`crates/codegen/xai-chat-state/src/types.rs:182`），
`web_fetch` 的兜底亦为 `128_000`
（`crates/codegen/xai-grok-tools/src/implementations/grok_build/web_fetch/config.rs:72`）。
**200k / 256k / 128k 三个值散落在不同代码路径**，写模型时不要依赖任何一个。

## 二、`max_completion_tokens`：是 `Option<u32>`，不写即"不发送"

- 类型：`Option<u32>`（`crates/codegen/xai-grok-shell/src/agent/config.rs:1074`）。
- Chat Completions 路径**直接透传**，`None` 表示省略该字段、交由服务端默认：
  `crates/codegen/xai-grok-sampling-types/src/conversation/chat_completions.rs:349`
  → `max_tokens: req.max_output_tokens`。
- ⚠️ Anthropic 格式路径 (Messages API) 则不同，是 `unwrap_or(0)`：
  `crates/codegen/xai-grok-sampling-types/src/conversation/messages.rs:420`。
  自建 `api_backend = "chat_completions"` / `"responses"` 的模型不走这条，
  但把同一模型切到 Anthropic 格式时要留意这里会把缺省变成 `max_tokens: 0`。
- 全局兜底键存在：`models.max_completion_tokens`（`26-config-reference.md:416`），
  但本机未设。

结论：不写不会报错，但等于把输出上限完全交给对端默认；不同网关默认值不同，
长回答可能被提前 `finish_reason = "length"` 截断。

## 三、真实上限的查法（不烧 token）

**原则：查元数据接口 / 官方文档，不要发试探请求去"撞"上限。**
主动发超限请求试探会真实消耗 token 且可能触发限流，是错误做法。

### 1) OpenRouter 模型目录（覆盖面最广，一次拿全）

```bash
curl -sS "https://openrouter.ai/api/v1/models" | python -c "
import json,sys
for m in json.load(sys.stdin)['data']:
    tp = m.get('top_provider') or {}
    print('%-38s %12s %12s' % (m['id'], m.get('context_length'), tp.get('max_completion_tokens')))
"
```

字段：`context_length` + `top_provider.max_completion_tokens`。本机实测一次返回 444 个模型。
**免费、无鉴权、不消耗推理额度。**

### 2) 火山方舟（Ark）——最好用，直接给精确 token_limits

```bash
curl -sS "https://ark.cn-beijing.volces.com/api/coding/v3/models" \
  -H "Authorization: Bearer $ARK_API_KEY" | python -c "
import json,sys
for m in json.load(sys.stdin)['data']:
    if m.get('token_limits'):
        print(m['id'], json.dumps(m['token_limits']))
"
```

返回含 `token_limits`：
`{context_window, max_input_token_length, max_output_token_length, max_reasoning_token_length}`
——比 OpenRouter 更细，连 reasoning 上限都有。同样是列表接口，不烧 token。

### 3) Command Code 网关

```bash
curl -sS "https://api.commandcode.ai/provider/v1/models" \
  -H "Authorization: Bearer $COMMANDCODE_API_KEY"
```

给 `context_length` 与 `supported_endpoints`，**但不给 max output**。输出上限需回退到
OpenRouter 或厂商文档。

### 4) 厂商官方文档（兜底交叉验证）

- DeepSeek 官方：1M 上下文，`deepseek-v4-flash` 旧名已下线，请求会转到 V4.1-Flash
- 智谱官方：GLM-5.3 支持 1M 上下文、最大输出 128K
- 火山方舟文档：「glm-5.3、deepseek-v4-flash、deepseek-v4-pro、kimi-k2.8-preview
  支持 1M 上下文窗口」
- OpenAI 官方：GPT-5.6 Luna = 1,050,000 上下文 / 128,000 输出

⚠️ **文档里的"128K/384K"是口语，元数据里是 `131072` / `393216`。** 举例：
GLM-5.3-Flash 文档写"最大输出 128K"，方舟 `token_limits` 给的是 `131072`；
DeepSeek 文档写"384K 输出"，实际是 `393216`。**配配置时用元数据里的精确整数**，
不要用文档的约数（差 288/3456 个 token 看似无害，但边界请求会被拒）。

## 四、本机配置现状（2026-09-22 补齐后）

```
model                            backend                  ctx    max_out
gpt-5.6-luna                     responses            1050000     128000
mimo-v2.5-pro                    chat_completions     1050000     131072
deepseek-v4-flash                chat_completions     1048576     393216
glm-5.3-flash                    chat_completions     1048576     131072
deepseek/deepseek-v4-flash       chat_completions     1048576     393216
deepseek/deepseek-v4.1-flash     chat_completions     1048576     384000
xiaomi/mimo-v2.6-flash           chat_completions     1048576     131072
```

补齐前 4 个 Command Code / Ark 模型的这两项**全部为空**（走兜底 200k，真实 1M）。
配置备份：`~/.grok/config.toml.bak-20260922-224106-prebackend`。

### 两处易错点（本机实测踩到）

1. **`gpt-5.6-luna` 原本配的 `context_window = 272000` 是错的**。272K 是 Codex 的
   **计费档位**（billing tier），不是模型硬上限；OpenAI 官方与 OpenRouter 均为 `1050000`。
2. **同一模型在不同网关的上限可以不同**：`deepseek/deepseek-v4.1-flash` 在 Command Code
   是 `384000`，在火山方舟（`deepseek-v4-1-flash-260910`）是 `393216`。
   按各自网关的口径分别填，不要强行统一。

### 未验证项

- `gpt-5.6-luna` 走的是第三方转售网关 `token-unlimited.com`，其真实可用上限可能低于
  OpenAI 官方标注（转售网关常有额外截断），**未实测**。上面按官方口径填。
- 小米自家网关（`token-plan-cn.xiaomimimo.com`）的 `/models` **只返回 id，不含长度字段**，
  `mimo-v2.5-pro` 的值取自 OpenRouter 口径，未与该网关交叉验证。

## 五、新增自定义模型的检查清单

1. `context_window` 必填，取**该网关**的精确值（Ark 看 `token_limits`，其他看 OpenRouter）
2. `max_completion_tokens` 建议填，否则输出上限交给服务端默认
3. `api_backend` 明确指定 —— 注意第三方网关的 Responses 事件方言问题，
   见 [`upstream-responses-event-compat.md`](upstream-responses-event-compat.md)
4. 若该模型还要用 `web_search`，`api_backend` 必须是 `responses`
   （代价是可能踩上条链接里的事件方言兼容性 bug）
5. 配完复核：`python -c "import tomllib;d=tomllib.load(open(r'C:/Users/panda/.grok/config.toml','rb'));print(d['model'])"`
