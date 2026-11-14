# Deferred built-in tools (2026-11-14)

## What changed

Zero-use built-in tool schemas are no longer merely hidden from the per-request
declaration. They are **deferred**: kept out of the direct tool list, indexed in
the `search_tool` catalog under the `built-in` group, and callable through
`use_tool`. The list lives in
`crates/codegen/xai-grok-shell/src/session/tool_index.rs` as
`DEFERRED_BUILTIN_TOOLS`:

| Tool | 30-day calls | Declaration tokens (o200k) |
| --- | ---: | ---: |
| `workflow` | 0 (also config-disabled) | 1,385 |
| `reference_to_video` | 0 | 1,320 |
| `send_feedback` | 0 | 985 |
| `lsp` | 0 | 555 |
| `image_edit` | 0 | 478 |
| `image_to_video` | 0 | 396 |
| `image_gen` | 0 | 252 |
| `scheduler_create` | 0 | 456 |
| `scheduler_delete` | 0 | 94 |
| `scheduler_list` | 0 | 61 |

`scheduler_*` was added on the user's 2026-11-14 instruction after the first
deferral round kept it declared.

## Mechanism

- `deferred_builtin_metadata()` maps the matching `ToolDefinition`s to the same
  metadata shape as MCP tools (`server_name = "built-in"`).
- `ToolMetadataSnapshot.deferred_builtins` is a separate section: MCP refreshes
  replace `tools`/`servers` but never this field.
- `Bm25ToolSearchIndex::search_snapshot` merges MCP tools and deferred built-ins
  into one BM25 corpus; `list_server_summaries` stays MCP-only so the connected
  server reminder does not announce a fake server.
- `ToolSearchIndex::is_deferred_builtin()` lets `use_tool` accept a bare built-in
  name and route it through `InnerDispatch` instead of the MCP `server__tool`
  path. The native-tool corrective error still fires for declared native tools.
- The snapshot fills lazily in `prepare_tool_definitions_inner` from the
  unfiltered definitions before the declaration is filtered.
- The list is static, so the declared tool surface stays byte-identical per
  turn; prefix caching is unaffected. `search_tool`/`use_tool` descriptions now
  mention deferred built-ins.

## Expected effect

- Declared surface stays at 20 tools / 5,667 tokens (o200k); no additional
  token saving compared with plain hiding.
- A deferred tool costs one `search_tool` round trip when it is actually needed,
  then normal `use_tool` execution.
- `scheduler_*` stays reachable; `/loop` prompts that name `scheduler_create`
  rely on the model discovering it through the search description.

## Verification

- Linux + Windows build `43571074230` green for commit `b8a4f1d9` (Linux
  27m53s, Windows 10m41s).
- Live check on `grok 1.0.78 (b8a4f1d98363)`: a one-shot session's
  `tool_definitions.json` has 17 tools / 5,070 tokens with no deferred names
  (single-turn mode does not register `scheduler_*`, so an interactive session
  keeps 20 tools);
  `search_tool("scheduler_create")` returned the `built-in` group and schema;
  `use_tool` with `scheduler_list` returned "No scheduled tasks."
- Unit tests: deferred entries are searchable by exact name, `is_deferred_builtin`
  answers correctly, `list_server_summaries` omits the `built-in` group,
  `deferred_builtin_metadata` follows the shared list, and `use_tool` dispatches
  a deferred name through `InnerDispatch`.
- Live check after deploy: new session `tool_definitions.json` stays at 20 tools
  with `scheduler_*` absent; `search_tool("scheduler_create")` returns the entry;
  `use_tool` executes it.
