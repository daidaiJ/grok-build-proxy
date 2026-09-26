#!/usr/bin/env python3
"""Reverse-infer provider cache TTL and RPM/TPM limits from limit-probe records.

Reads the ndjson written by `xai-grok-sampler/src/limit_probe.rs`
(default `%USERPROFILE%/.grok/limit-probe/records.ndjson`, lines of kind
`req` / `err` / `usage`) and prints:

  1. Cache TTL survival table: for consecutive calls in the same session with
     an unchanged tools block and a growing message prefix, P(cache hit) per
     inter-call gap bucket. The cliff edge is the observed effective TTL.
  2. RPM/TPM estimate from 429s: accepted request count and accepted
     prompt+completion tokens in the 60s window before each 429 bracket the
     limits from below; provider `*ratelimit*` headers are printed verbatim
     when present (exact limits).

Passive only — never sends requests. See docs-local/limit-inference.md.

Usage:
  python scripts-local/limit-inference.py [--file PATH] [--window 60] [--min-n 5]
"""

from __future__ import annotations

import argparse
import bisect
import json
import os
import statistics
from collections import defaultdict
from datetime import datetime
from pathlib import Path

GAP_EDGES_S = [15, 30, 60, 120, 300, 600, 1200, 1800, 3600, 7200, 21600, 43200, 86400]


def fmt_secs(secs: float) -> str:
    if secs < 60:
        return f"{int(secs)}s"
    if secs < 3600:
        return f"{secs / 60:g}m"
    if secs < 86400:
        return f"{secs / 3600:g}h"
    return f"{secs / 86400:g}d"


def fmt_bucket(edge: int | None) -> str:
    return "(>24h)" if edge is None else f"≤{fmt_secs(edge)}"


def load_records(path: Path) -> list[dict]:
    records = []
    with path.open("r", encoding="utf-8") as fh:
        for line_no, line in enumerate(fh, 1):
            line = line.strip()
            if not line:
                continue
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError as err:
                print(f"  [warn] line {line_no}: bad json ({err})")
    return records


# ---------------------------------------------------------------------------
# 1. Cache TTL survival analysis
# ---------------------------------------------------------------------------

def ttl_survival(usage_by_model: dict[str, list[dict]]) -> dict[str, list[tuple[str, int, int]]]:
    """Per model: [(bucket_label, n, hits)] over consecutive-call gaps."""
    by_model: dict[str, list[tuple[str, int, int]]] = {}
    for model, events in usage_by_model.items():
        buckets = {(edge, None): [0, 0] for edge in GAP_EDGES_S}
        buckets[(None, None)] = [0, 0]  # beyond last edge
        considered = skipped = 0
        for prev, cur in zip(events, events[1:]):
            # Prefix continuity guards: tools block changed -> miss is not TTL
            # evidence; prefix shrank (compaction/edit) -> identity changed.
            if not cur.get("msgs_count") or not prev.get("msgs_count"):
                skipped += 1
                continue
            if cur.get("tools_hash") != prev.get("tools_hash"):
                skipped += 1
                continue
            if cur["msgs_count"] < prev["msgs_count"]:
                skipped += 1
                continue
            gap = (cur["ts_ms"] - prev["ts_ms"]) / 1000.0
            if gap < 0:
                skipped += 1
                continue
            hit = cur.get("cached_prompt_tokens", 0) > 0
            considered += 1
            edge = next((e for e in GAP_EDGES_S if gap <= e), None)
            cell = buckets[(edge, None)]
            cell[0] += 1
            cell[1] += int(hit)
        rows = []
        for edge in [*GAP_EDGES_S, None]:
            n, hits = buckets[(edge, None)]
            rows.append((fmt_bucket(edge), n, hits))
        if considered or skipped:
            print(f"\n  {model}: {considered} gap samples ({skipped} skipped: tools changed / prefix shrank)")
            print(f"    {'gap':<10} {'n':>4} {'hits':>5} {'hit_rate':>9}")
            for label, n, hits in rows:
                if n:
                    print(f"    {label:<10} {n:>4} {hits:>5} {hits / n:>8.0%}")
        by_model[model] = rows
    return by_model


def estimate_ttl(rows: list[tuple[str, int, int]], min_n: int) -> str:
    """First-crossing rule: last gap bucket with hit_rate >= 50% before the drop."""
    upper = None
    for label, n, hits in rows:
        if n < min_n:
            continue
        if hits / n >= 0.5:
            upper = label
        else:
            break
    return upper or "insufficient data"


# ---------------------------------------------------------------------------
# 2. RPM / TPM from 429s
# ---------------------------------------------------------------------------

def rate_limit_estimates(
    reqs: list[dict],
    usage_by_req: dict[str, dict],
    window_s: float,
    min_n: int,
) -> None:
    # Authoritative headers first: any *ratelimit* header not remaining/reset
    # is a documented limit value.
    header_values: dict[str, set[str]] = defaultdict(set)
    retry_afters = []
    four29s = []
    for rec in reqs:
        if rec.get("retry_after") is not None and rec.get("status") == 429:
            retry_afters.append(rec["retry_after"])
        for name, value in (rec.get("rl") or {}).items():
            if "remaining" in name or "reset" in name:
                continue
            header_values[name].add(str(value))
        if rec.get("status") == 429:
            four29s.append(rec)

    if header_values:
        print("\n  provider ratelimit headers (exact, when present):")
        for name in sorted(header_values):
            print(f"    {name} = {', '.join(sorted(header_values[name]))}")
    if retry_afters:
        print(
            f"\n  429 Retry-After: n={len(retry_afters)} "
            f"median={statistics.median(retry_afters)}s max={max(retry_afters)}s"
        )

    if not four29s:
        print("\n  no 429 events recorded yet — RPM/TPM brackets appear once the provider throttles")
        return

    four29s.sort(key=lambda r: r["ts_ms"])
    accepted = sorted(
        (r for r in reqs if 200 <= r.get("status", 0) < 300), key=lambda r: r["ts_ms"]
    )
    usage_events = sorted(usage_by_req.values(), key=lambda r: r["ts_ms"])
    accepted_ts = [r["ts_ms"] for r in accepted]
    usage_ts = [r["ts_ms"] for r in usage_events]

    print(f"\n  429 sliding-window brackets ({int(window_s)}s before each 429, lower bounds):")
    print(f"    {'model':<28} {'events':>6} {'req/window min–med':>20} {'tokens/window min–med':>22}")
    by_model_429: dict[str, list[dict]] = defaultdict(list)
    for ev in four29s:
        by_model_429[ev.get("model", "?")].append(ev)
    for model in sorted(by_model_429):
        events = by_model_429[model]
        rpm_seen, tpm_seen = [], []
        for ev in events:
            t0 = ev["ts_ms"] - window_s * 1000
            lo = bisect.bisect_left(accepted_ts, t0)
            hi = bisect.bisect_right(accepted_ts, ev["ts_ms"])
            rpm_seen.append(hi - lo)
            ulo = bisect.bisect_left(usage_ts, t0)
            uhi = bisect.bisect_right(usage_ts, ev["ts_ms"])
            tpm_seen.append(
                sum(
                    u.get("prompt_tokens", 0) + u.get("completion_tokens", 0)
                    for u in usage_events[ulo:uhi]
                )
            )
        rpm_lo, rpm_med = min(rpm_seen), statistics.median(rpm_seen)
        tpm_lo, tpm_med = min(tpm_seen), statistics.median(tpm_seen)
        print(
            f"    {model:<28} {len(events):>6} "
            f"{rpm_lo:>9}–{rpm_med:<9.0f} {tpm_lo:>10}–{tpm_med:<11.0f}"
        )
    print(
        "    true limits are >= each bracket (traffic was still being accepted before the 429);\n"
        f"    with more 429 events the min converges toward the limit. n<={min_n} samples are indicative only."
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    default_path = Path(os.environ.get("GROK_LIMIT_PROBE_FILE", "")) if os.environ.get("GROK_LIMIT_PROBE_FILE") else (
        Path(os.environ.get("USERPROFILE", os.environ.get("HOME", ""))) / ".grok" / "limit-probe" / "records.ndjson"
    )
    parser.add_argument("--file", type=Path, default=default_path)
    parser.add_argument("--window", type=float, default=60.0, help="sliding window seconds for 429 brackets")
    parser.add_argument("--min-n", type=int, default=5, help="min samples per gap bucket for TTL estimate")
    args = parser.parse_args()

    if not args.file.exists():
        raise SystemExit(f"no probe log at {args.file} — run grok2 with the limit-probe build first")
    records = load_records(args.file)
    reqs = [r for r in records if r.get("kind") == "req"]
    usages = [r for r in records if r.get("kind") == "usage"]
    errs = [r for r in records if r.get("kind") == "err"]

    # Dedup usage per req_id (streaming may repeat the usage chunk).
    usage_by_req: dict[str, dict] = {}
    for rec in usages:
        usage_by_req[rec.get("req_id", "")] = rec

    ts_all = [r["ts_ms"] for r in records if r.get("ts_ms")]
    print(f"file: {args.file}")
    print(f"lines: {len(records)} (req={len(reqs)} usage={len(usages)} err={len(errs)})", end="")
    if ts_all:
        span = datetime.fromtimestamp(max(ts_all) / 1000) - datetime.fromtimestamp(min(ts_all) / 1000)
        print(f" over {span}")
    else:
        print()

    # TTL: usage grouped by (session, model), chronological.
    by_session_model: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for rec in usages:
        by_session_model[(rec.get("session_id", "?"), rec.get("model", "?"))].append(rec)
    usage_by_model: dict[str, list[dict]] = defaultdict(list)
    for (_session, model), events in by_session_model.items():
        events.sort(key=lambda r: r["ts_ms"])
        usage_by_model[model].extend(events)
    for events in usage_by_model.values():
        events.sort(key=lambda r: r["ts_ms"])

    rows_by_model = ttl_survival(usage_by_model)
    print("\n  effective TTL (first gap bucket where hit rate drops below 50%):")
    for model, rows in rows_by_model.items():
        print(f"    {model}: {estimate_ttl(rows, args.min_n)}")

    rate_limit_estimates(reqs, usage_by_req, args.window, args.min_n)


if __name__ == "__main__":
    main()
