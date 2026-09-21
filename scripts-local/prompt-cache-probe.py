#!/usr/bin/env python3
"""Prompt-cache probe for third-party models configured in ~/.grok/config.toml.

Answers the question "/stats shows cache hit 0% for model X — provider or us?" by
printing the raw `usage` of repeated requests in three scenarios:

  repeat  same body twice                     -> 2nd call should report cached_tokens
  tail    same prefix, different user message -> prefix should still hit
  tools   same messages, one extra tool       -> tells whether tools sit in the prefix

Only reads base_url / api_key from ~/.grok/config.toml (never prints the key).
See docs-local/prompt-cache-notes.md for the findings this reproduces.

Usage: python scripts-local/prompt-cache-probe.py [--model KEY] [--max-tokens N]
"""

from __future__ import annotations

import argparse
import json
import re
import urllib.error
import urllib.request
from pathlib import Path

CFG_PATH = Path.home() / ".grok" / "config.toml"
FILLER = "Shared prefix probing needs byte-identical leading tokens. " * 100


def load_model(key: str) -> dict[str, str]:
    text = CFG_PATH.read_text(encoding="utf-8")
    block = re.search(
        r'^\[model\."' + re.escape(key) + r'"\]\n((?:[^\[\n].*\n|\n)*)', text, re.M
    )
    if not block:
        raise SystemExit(f"model {key!r} not found in {CFG_PATH}")
    return dict(re.findall(r'^([a-z_]+)\s*=\s*"([^"]*)"', block.group(1), re.M))


def tool(name: str, description: str) -> dict:
    return {
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": {
                "type": "object",
                "properties": {"arg": {"type": "string", "description": "argument"}},
                "required": ["arg"],
            },
        },
    }


TOOLS_A = [
    tool("alpha_tool", FILLER),
    tool("beta_tool", "second tool"),
    tool("gamma_tool", "third tool"),
]
TOOLS_B = TOOLS_A + [tool("delta_tool", "extra tool added later")]


class Prober:
    def __init__(self, cfg: dict[str, str], max_tokens: int) -> None:
        self.url = cfg["base_url"].rstrip("/") + "/chat/completions"
        self.key = cfg["api_key"]
        self.model = cfg["model"]
        self.max_tokens = max_tokens

    def call(self, label: str, *, system: str, user: str, tools: list | None) -> None:
        body: dict = {
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "max_tokens": self.max_tokens,
            "stream": False,
        }
        if tools:
            body["tools"] = tools
        req = urllib.request.Request(
            self.url,
            data=json.dumps(body).encode(),
            headers={"Authorization": f"Bearer {self.key}", "Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                usage = json.loads(resp.read()).get("usage") or {}
        except urllib.error.HTTPError as err:  # noqa: PERF203
            print(f"  {label:<44} HTTP {err.code} {err.read()[:200]!r}")
            return
        cached = (usage.get("prompt_tokens_details") or {}).get("cached_tokens")
        print(f"  {label:<44} prompt={usage.get('prompt_tokens')} cached={cached}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", default="ark-glm-5.3-flash", help="model key in config.toml")
    parser.add_argument("--max-tokens", type=int, default=8)
    args = parser.parse_args()

    cfg = load_model(args.model)
    prober = Prober(cfg, args.max_tokens)
    sys_prompt = "You are a coding agent." + FILLER
    print(f"--- {cfg['model']} @ {cfg['base_url']} ---")

    print("[repeat] same body twice")
    prober.call("call 1 (cold)", system=sys_prompt, user="Reply with: ok", tools=None)
    prober.call("call 2 (identical)", system=sys_prompt, user="Reply with: ok", tools=None)

    print("[tail] same prefix, different tail")
    prober.call("call 1 (cold)", system=sys_prompt, user="Reply with: one", tools=TOOLS_A)
    prober.call("call 2 (different user msg)", system=sys_prompt, user="Reply with: two", tools=TOOLS_A)

    print("[tools] same messages, tools array changes")
    prober.call("tools +1 (cold)", system=sys_prompt, user="Reply with: one", tools=TOOLS_B)
    prober.call("tools unchanged again", system=sys_prompt, user="Reply with: one", tools=TOOLS_A)


if __name__ == "__main__":
    main()
