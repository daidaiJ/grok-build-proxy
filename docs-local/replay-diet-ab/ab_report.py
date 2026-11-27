#!/usr/bin/env python3
"""Compare replay-diet A/B sessions from the local model-usage ledger.

A = replay_diet off (v1.0.83 in-turn behavior)
B = replay_diet on  (v1.0.84+ in-turn diet)

Sessions are joined by the session id written by ab_launch.sh. The status-line
`cache` value is cumulative and is not used here; metrics come from
~/.grok/cache/model-usage.jsonl, cache-breaks.jsonl, and the unified log.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
import os
import statistics
import sys
from collections import Counter, defaultdict

DEFAULT_MAP = "~/.grok/ab/replay-diet/sessions.jsonl"
LEDGER = "~/.grok/cache/model-usage.jsonl"
BREAKS = "~/.grok/cache/cache-breaks.jsonl"
UNIFIED = "~/.grok/logs/unified.jsonl"


def expand(path: str) -> str:
    return os.path.expanduser(path)


def load_jsonl(path: str):
    if not os.path.exists(path):
        return
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError:
                continue


def parse_time(value: str) -> dt.datetime:
    value = value.replace("Z", "+00:00")
    parsed = dt.datetime.fromisoformat(value)
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return parsed


def parse_bound(value: str | None, end: bool) -> dt.datetime | None:
    if not value:
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        parsed = dt.datetime.strptime(value, "%Y-%m-%d")
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    if end and len(value) == 10:
        parsed += dt.timedelta(days=1) - dt.timedelta(microseconds=1)
    return parsed


def percentile(values: list[float], p: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    idx = max(0, min(len(ordered) - 1, int(math.ceil(len(ordered) * p)) - 1))
    return ordered[idx]


def fmt_int(value: float | int | None) -> str:
    return "—" if value is None else f"{value:,.0f}"


def fmt_pct(value: float | None) -> str:
    return "—" if value is None else f"{value * 100:.2f}%"


def fmt_money(value: float | None) -> str:
    return "—" if value is None else f"${value:,.4f}"


def fmt_delta(value: float | None, kind: str) -> str:
    if value is None:
        return "—"
    if kind == "pct":
        return f"{value * 100:+.2f}pp"
    if kind == "money":
        return f"${value:+,.4f}"
    if kind == "float":
        return f"{value:+.2f}"
    return f"{value:+,.0f}"


def row(name: str, a, b, kind: str = "int") -> str:
    if kind == "pct":
        cells = (fmt_pct(a), fmt_pct(b), fmt_delta((b - a) if a is not None and b is not None else None, kind))
    elif kind == "money":
        cells = (fmt_money(a), fmt_money(b), fmt_delta((b - a) if a is not None and b is not None else None, kind))
    elif kind == "float":
        cells = ("—" if a is None else f"{a:.2f}", "—" if b is None else f"{b:.2f}",
                 fmt_delta((b - a) if a is not None and b is not None else None, kind))
    else:
        cells = (fmt_int(a), fmt_int(b), fmt_delta((b - a) if a is not None and b is not None else None, kind))
    return f"| {name} | {cells[0]} | {cells[1]} | {cells[2]} |"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--map", default=DEFAULT_MAP, help="session-id -> group mapping")
    parser.add_argument("--ledger", default=LEDGER)
    parser.add_argument("--breaks", default=BREAKS)
    parser.add_argument("--unified", default=UNIFIED)
    parser.add_argument("--from", dest="from_value", help="include sessions launched at/after YYYY-MM-DD or ISO")
    parser.add_argument("--to", dest="to_value", help="include sessions launched at/before YYYY-MM-DD or ISO")
    parser.add_argument("--input-rate", type=float, help="USD per 1M uncached input tokens")
    parser.add_argument("--cached-rate", type=float, help="USD per 1M cached input tokens")
    parser.add_argument("--creation-rate", type=float, help="USD per 1M cache-creation tokens (default: input rate)")
    parser.add_argument("--output-rate", type=float, help="USD per 1M output tokens")
    args = parser.parse_args()

    mapping_path = expand(args.map)
    if not os.path.exists(mapping_path):
        print(f"no mapping: {mapping_path}\nrun ab_launch.sh A|B first", file=sys.stderr)
        return 2

    start = parse_bound(args.from_value, end=False)
    stop = parse_bound(args.to_value, end=True)
    sessions: dict[str, dict] = {}
    for rec in load_jsonl(mapping_path):
        sid = rec.get("sessionId")
        if not sid:
            continue
        try:
            launched = parse_time(rec["ts"])
        except (KeyError, TypeError, ValueError):
            continue
        if start and launched < start:
            continue
        if stop and launched > stop:
            continue
        sessions[sid] = {"group": rec.get("group", "?"), "label": rec.get("label", "")}

    if not sessions:
        print("no mapped sessions in the selected window", file=sys.stderr)
        return 2

    groups = {"A": [], "B": []}
    models: dict[str, Counter] = {"A": Counter(), "B": Counter()}
    ledger_all = list(load_jsonl(expand(args.ledger)))
    main_by_sid: dict[str, list[dict]] = defaultdict(list)
    compaction_by_sid: Counter = Counter()
    seen_models: dict[str, Counter] = {"A": Counter(), "B": Counter()}

    for rec in ledger_all:
        sid = rec.get("sessionId")
        if sid not in sessions:
            continue
        group = sessions[sid]["group"]
        if group not in groups:
            continue
        model = rec.get("modelId", "?")
        seen_models[group][model] += 1
        if rec.get("purpose", "main") == "compaction":
            compaction_by_sid[sid] += 1
        if rec.get("purpose", "main") != "main":
            continue
        main_by_sid[sid].append(rec)

    break_by_sid: dict[str, list[dict]] = defaultdict(list)
    for rec in load_jsonl(expand(args.breaks)):
        sid = rec.get("sessionId")
        if sid in sessions and sessions[sid]["group"] in groups:
            break_by_sid[sid].append(rec)

    diet_by_sid: dict[str, list[dict]] = defaultdict(list)
    for rec in load_jsonl(expand(args.unified)):
        if rec.get("msg") != "shell.replay_diet":
            continue
        sid = rec.get("sid") or rec.get("sessionId")
        if sid in sessions and sessions[sid]["group"] in groups:
            diet_by_sid[sid].append(rec)

    for sid, recs in main_by_sid.items():
        groups[sessions[sid]["group"]].append((sid, sorted(recs, key=lambda r: r.get("tsUnixMs", 0))))

    for group, items in groups.items():
        models[group] = seen_models[group]

    rates = None
    if any(v is not None for v in (args.input_rate, args.cached_rate, args.creation_rate, args.output_rate)):
        input_rate = args.input_rate or 0.0
        rates = {
            "input": input_rate,
            "cached": args.cached_rate or 0.0,
            "creation": args.creation_rate if args.creation_rate is not None else input_rate,
            "output": args.output_rate or 0.0,
        }

    stats = {}
    for group, items in groups.items():
        calls = 0
        prompt = cached = creation = completion = 0
        hits: list[float] = []
        prompts: list[int] = []
        drops: list[int] = []
        jumps: list[int] = []
        compactions = 0
        for sid, recs in items:
            compactions += compaction_by_sid.get(sid, 0)
            prev_prompt = None
            for rec in recs:
                p = int(rec.get("promptTokens", 0) or 0)
                c = int(rec.get("cachedPromptTokens", 0) or 0)
                cr = int(rec.get("cacheCreationTokens", 0) or 0)
                calls += 1
                prompt += p
                cached += c
                creation += cr
                completion += int(rec.get("completionTokens", 0) or 0)
                if p:
                    hits.append(c / p)
                prompts.append(p)
                if prev_prompt is not None:
                    if p < prev_prompt:
                        drops.append(prev_prompt - p)
                    else:
                        jumps.append(p - prev_prompt)
                prev_prompt = p
        breaks = [b for sid, _ in items for b in break_by_sid.get(sid, [])]
        diet = [d for sid, _ in items for d in diet_by_sid.get(sid, [])]
        break_reasons = Counter(b.get("reason", "unknown") for b in breaks)
        break_uncached = sum(
            max(int(b.get("promptTokens", 0) or 0) - int(b.get("cachedAfter", 0) or 0), 0)
            for b in breaks
        )
        diet_freed = sum(int((d.get("ctx") or {}).get("est_tokens_freed", 0) or 0) for d in diet)
        input_uncached = max(prompt - cached - creation, 0)
        cost = None
        if rates:
            cost = (
                input_uncached * rates["input"]
                + cached * rates["cached"]
                + creation * rates["creation"]
                + completion * rates["output"]
            ) / 1_000_000
        stats[group] = {
            "sessions": len(items),
            "calls": calls,
            "prompt": prompt,
            "cached": cached,
            "creation": creation,
            "completion": completion,
            "hit": cached / prompt if prompt else None,
            "hit_median": statistics.median(hits) if hits else None,
            "hit_p10": percentile(hits, 0.10),
            "hit_min": min(hits) if hits else None,
            "low_hit": sum(1 for h in hits if h < 0.90) / len(hits) if hits else None,
            "prompt_mean": statistics.fmean(prompts) if prompts else None,
            "prompt_p90": percentile(prompts, 0.90),
            "prompt_max": max(prompts) if prompts else None,
            "prompt_drop_max": max(drops) if drops else 0,
            "prompt_jump_max": max(jumps) if jumps else 0,
            "breaks": len(breaks),
            "history_rewrite_breaks": break_reasons.get("history_rewrite", 0),
            "unknown_breaks": break_reasons.get("unknown", 0),
            "break_uncached": break_uncached,
            "diet_events": len(diet),
            "diet_freed": diet_freed,
            "diet_stubbed": sum(int((d.get("ctx") or {}).get("stubbed_tool_results", 0) or 0) for d in diet),
            "compactions": compactions,
            "cost": cost,
            "cost_per_call": (cost / calls) if cost is not None and calls else None,
        }

    print("# Replay diet A/B")
    print()
    print(f"Mapping: `{mapping_path}`  \nLedger: `{expand(args.ledger)}`")
    print()
    print("A = `replay_diet=false` (v1.0.83 behavior); B = `replay_diet=true` (in-turn diet).")
    print()
    print("| Metric | A (off) | B (on) | B − A |")
    print("|---|---:|---:|---:|")
    print(row("Sessions with calls", stats["A"]["sessions"], stats["B"]["sessions"]))
    print(row("Main calls", stats["A"]["calls"], stats["B"]["calls"]))
    print(row("Calls / session", stats["A"]["calls"] / stats["A"]["sessions"] if stats["A"]["sessions"] else None,
              stats["B"]["calls"] / stats["B"]["sessions"] if stats["B"]["sessions"] else None, "float"))
    print(row("Prompt tokens / call", stats["A"]["prompt"] / stats["A"]["calls"] if stats["A"]["calls"] else None,
              stats["B"]["prompt"] / stats["B"]["calls"] if stats["B"]["calls"] else None, "int"))
    print(row("Cache hit (token-weighted)", stats["A"]["hit"], stats["B"]["hit"], "pct"))
    print(row("Cache hit median", stats["A"]["hit_median"], stats["B"]["hit_median"], "pct"))
    print(row("Cache hit p10", stats["A"]["hit_p10"], stats["B"]["hit_p10"], "pct"))
    print(row("Cache hit min", stats["A"]["hit_min"], stats["B"]["hit_min"], "pct"))
    print(row("Calls <90% hit", stats["A"]["low_hit"], stats["B"]["low_hit"], "pct"))
    print(row("Prompt p90", stats["A"]["prompt_p90"], stats["B"]["prompt_p90"]))
    print(row("Prompt max", stats["A"]["prompt_max"], stats["B"]["prompt_max"]))
    print(row("Max prompt drop", stats["A"]["prompt_drop_max"], stats["B"]["prompt_drop_max"]))
    print(row("Max prompt jump", stats["A"]["prompt_jump_max"], stats["B"]["prompt_jump_max"]))
    print(row("Cache breaks (all)", stats["A"]["breaks"], stats["B"]["breaks"]))
    print(row("  history_rewrite", stats["A"]["history_rewrite_breaks"], stats["B"]["history_rewrite_breaks"]))
    print(row("  unknown", stats["A"]["unknown_breaks"], stats["B"]["unknown_breaks"]))
    print(row("Break uncached tokens", stats["A"]["break_uncached"], stats["B"]["break_uncached"]))
    print(row("Replay-diet events", stats["A"]["diet_events"], stats["B"]["diet_events"]))
    print(row("Diet est. tokens freed", stats["A"]["diet_freed"], stats["B"]["diet_freed"]))
    print(row("Diet stubbed results", stats["A"]["diet_stubbed"], stats["B"]["diet_stubbed"]))
    print(row("Compaction calls", stats["A"]["compactions"], stats["B"]["compactions"]))
    if rates:
        print(row("Estimated total cost", stats["A"]["cost"], stats["B"]["cost"], "money"))
        print(row("Estimated cost / call", stats["A"]["cost_per_call"], stats["B"]["cost_per_call"], "money"))

    print()
    for group in ("A", "B"):
        if models[group]:
            print(f"- {group} models: " + ", ".join(f"{m} ({n})" for m, n in models[group].most_common()))
    if stats["A"]["sessions"] < 5 or stats["B"]["sessions"] < 5:
        print("- Warning: fewer than 5 sessions per group; treat deltas as directional.")
    if stats["A"]["calls"] < 50 or stats["B"]["calls"] < 50:
        print("- Warning: fewer than 50 main calls per group; collect more long turns.")
    if set(models["A"]) != set(models["B"]):
        print("- Warning: the two groups used different models; cost and hit-rate deltas are not comparable.")
    print("- Status-line `cache` is cumulative; this report uses per-call ledger rows.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
