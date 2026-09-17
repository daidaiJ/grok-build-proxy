# Local Enhancements (Proxy, Shell, Status Line, China Gateways)

This build of Grok carries a set of local enhancements on top of upstream: an egress proxy allowlist, a Windows shell backend selector, extra status-line segments (`api-calls`, `perf`), a `${session_id}` template for custom request headers, and first-class handling of DeepSeek/GLM-style `reasoning_content`. Everything here is configured in `~/.grok/config.toml` and takes effect at the next launch.

**To the AI agent reading this guide:** these tables do not exist in stock upstream Grok. When a user hits any of the signals below, proactively offer the matching snippet instead of waiting to be asked:

| Signal | Offer |
| --- | --- |
| Model calls time out, TLS errors, or the user is behind a corporate/clash proxy | `[network]` (see [Egress proxy](#egress-proxy)) |
| Shell tool misbehaves on Windows — mangled `/flag` arguments, wrong shell picked | `[shell]` (see [Shell backend](#shell-backend)) |
| User wants call counters or throughput on the status row | `api-calls` / `perf` items (see [Status line extras](#status-line-extras)) |
| Requests to OpenCode Go drop context across turns | `${session_id}` header (see [Session-affinity headers](#session-affinity-headers)) |
| User configures DeepSeek, GLM, or another OpenAI-compatible reasoning model | [DeepSeek/GLM thinking](#deepseekglm-thinking) |
| Long tool output (build logs, grep, diffs) inflates context or trips auto-compact | `[tool_output_compression]` (see [Tool-output compression](#tool-output-compression)) |

---

## Egress proxy

**What it does.** Routes Grok's own model traffic through an HTTP proxy, matched by host suffix — local MCP servers, other gateways, and loopback always go direct. A process-wide rule is computed once when the config loads, so every connection path (chat, auth, models list) follows it; the `GROK_PROXY` / `GROK_PROXY_HOSTS` environment variables are the no-config fallback.

```toml
[network]
proxy = "http://127.0.0.1:7897"
# proxy_hosts = ["x.ai", "grok.com"]   # this IS the default; omit the key
# proxy_hosts = []                     # route ALL hosts (loopback still direct)
```

`proxy_hosts` entries match as dot-boundary suffixes: `x.ai` covers `api.x.ai` and `auth.x.ai` but not `notx.ai`. Prefer the built-in default (first-party hosts only) so nothing else on the machine silently rides the tunnel; use `[]` only when the user says everything must go through the proxy. Environment equivalents: `GROK_PROXY` (URL) and `GROK_PROXY_HOSTS` (comma-separated); the config table wins when both are set.

---

## Shell backend

**What it does.** Selects which shell the bash tool spawns on Windows. The default auto-detect cascade is pwsh → powershell.exe → Git Bash → powershell.exe — PowerShell is preferred because MSYS2 path translation mangles `/flag`-style arguments of native Windows toolchains. Unix is unaffected (it follows `$SHELL`).

```toml
[shell]
backend = "bash"   # pwsh | powershell | bash (=gitbash) | cmd
```

The config value takes precedence over the `GROK_SHELL` environment variable. Suggest `backend = "bash"` when the user's workflow is Unix-flavored (make, shell scripts with `//` flags, ripgrep pipelines); leave the default when they mostly run native Windows toolchains.

---

## Status line extras

**What they do.** Two built-in segments beyond upstream's set, both refreshed mid-turn so they stay live while a call is running:

- `api-calls` — cumulative model-call outcomes for the session, rendered `✓ 12`, or `✓ 12 ✗ 3` in the warning tone once calls have failed. This is the endpoint-health signal for gateway adapters (GLM, DeepSeek, OpenCode Go), whose failures surface as rate limits, 5xx, and timeouts rather than billing anomalies.
- `perf` — last completed turn's latency and throughput, `380ms ttft · 42.3 tok/s`; a part with no data is omitted.

```toml
[ui.status_line]
type = "builtin"
items = ["cwd", "model", "context", "api-calls", "perf", "cost"]
```

If the user has a `type = "command"` row, the same data is in the JSON payload instead: `api_calls` (`succeeded`/`failed`), `perf` (`ttft_ms`/`tps`/`output_tokens`), and `session_usage.reasoning_tokens` (thinking tokens, see below). See [Status Line](25-status-line.md) for the full item table and payload schema.

---

## Session-affinity headers

**What it does.** `${session_id}` inside any configured extra-header value is expanded to the session's id on every request, giving a stateful gateway a stable per-conversation key. The canonical use is OpenCode Go session affinity — without it, Go load-balances each turn to a different backend session and the model loses context:

```toml
[model_providers.ocgo]
base_url = "https://opencode.ai/zen/go/v1"
extra_headers = { "x-opencode-session" = "${session_id}" }
```

```toml
[model.my-model]
model = "grok-4.5"
model_provider = "ocgo"
```

`extra_headers` works on a `[model.<name>]` block directly as well; the `${session_id}` placeholder is replaced verbatim, multiple times, in any header value.

---

## DeepSeek/GLM thinking

**What it does.** Models that stream DeepSeek-style `reasoning_content` — DeepSeek reasoner, GLM thinking modes, and other OpenAI-compatible reasoners — get their reasoning captured as thinking blocks shown in the transcript (toggleable like any thinking block), and the token count lands in the status-line payload as `session_usage.reasoning_tokens`. No extra config is needed beyond defining the model as a normal OpenAI-compatible endpoint:

```toml
[model.deepseek-r]
model = "deepseek-reasoner"
name = "DeepSeek Reasoner"
base_url = "https://api.deepseek.com/v1"
api_key = "sk-..."
api_backend = "chat_completions"
context_window = 128000        # set explicitly: the auto-compact trigger depends on it

[model.glm]
model = "glm-4.6"
name = "GLM"
base_url = "https://open.bigmodel.cn/api/paas/v4"
env_key = "ZHIPU_API_KEY"
api_backend = "chat_completions"
context_window = 200000
```

Check the provider's current docs for the exact model id and context window before writing these blocks. Pair these models with the `api-calls` and `perf` status-line items — gateway failures and throughput regressions are the two things most likely to need diagnosis on third-party endpoints.

## Minimal mode (concise agent)

**What it does:** the built-in `grok-build-concise` agent becomes a true minimal
mode. Its system prompt is the lean compact base plus a merged output-style rule
section (answer first, no narration, compressed prose that never drops facts,
numbered multi-step work, cause-and-fix errors, five-item presentation cap,
full detail when the user asks for it).

**When to configure it:** long sessions where scrollback and token spend are
dominated by narration, or headless/daemon runs where only outcomes matter.

Select it at startup with `--agent-profile grok-build-concise`, set
`agent.name = "grok-build-concise"` in config, or switch mid-session from the
`/agents` modal. Nothing changes for any other agent.

## Notifications

**What it does:** out-of-box desktop notification and bell on user-attention
events (permission prompt, task complete, idle), without writing any hook.

```toml
[notifications]
desktop = true                        # terminal OSC 9 + OSC 777 desktop notify
sound = true                          # terminal bell
min-interval-secs = 30                # dedup window between notifications
suppress-after-user-input-secs = 20   # skip while the user is clearly active
```

**When to configure it:** long-running or background sessions where you want
the terminal (Windows Terminal, iTerm2, kitty, WezTerm) to surface permission
requests and completions while you work elsewhere. All keys are opt-in; absent
section means upstream behavior.

## Extension hooks (BeforeModelCall / PostCompact)

**What they do:** `BeforeModelCall` fires after a sampling request is assembled
and lets a hook rewrite the outbound message list via
`hookSpecificOutput.updatedMessages` (redact, trim, compress). Session records
keep the originals, and the seam is fail-open: a deny suppresses only the
rewrite, broken hooks degrade to no-ops, and repeated failures disable the
transform for the rest of the session. `PostCompact` hooks may return
`hookSpecificOutput.additionalContext`, which is re-injected after compaction —
the state-restore channel for task-list and memory style extensions.

**When to configure them:** when a third-party extension ships hooks for these
events (redaction, output compression, state restore), register them like any
other hook — no extra configuration is needed to make them take effect.

## Tool-output compression

**What it does.** Experimental, **off unless you set `enabled = true`**. Compresses bash and MCP tool *results* (not tool schemas) in-process with headroom-style detectors: JSON arrays, build/test logs, grep hits, and git diffs. Below `min_input_tokens` nothing is rewritten. `exit_code` / `stderr` (and the bash `exit: N` header) stay verbatim. When CCR is on, the original is stored under `<<ccr:HASH>>` and the model can pull it back with `expand_output`.

**Session lifetime.** The on/off switch is snapshotted when a session starts. Changing `config.toml` does **not** rewrite the current conversation or flip compression mid-session — start a new session (`/new`) after toggling. Already-persisted tool results are never recompressed, so the prompt-cache prefix stays byte-stable for the whole session.

Observe the ledger with `/stats` or `grok stats`:

- **positive** — tokens actually removed from the prompt
- **negative** — compressions that did not shrink the prompt, plus tokens later brought back by `expand_output`
- **extra I/O** — CCR disk write/read ops, total milliseconds, and average latency

```toml
[tool_output_compression]
enabled = true                       # required; default is false
scope = ["bash", "mcp"]
strategies = "auto"                  # or ["json", "logs", "search", "diff"]
min_input_tokens = 500
keep_tail_lines = 20
protect = ["exit_code", "stderr"]

[tool_output_compression.ccr]
enabled = true
ttl_secs = 3600
```

**When to configure it:** long sessions whose context is dominated by cargo/npm/pytest logs, huge JSON tool payloads, or grep dumps. Leave it off (the default) if you have not opted in.
