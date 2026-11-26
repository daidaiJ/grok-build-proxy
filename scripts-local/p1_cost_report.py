#!/usr/bin/env python3
"""P1 压缩策略阶段报告（只读）。

用法:
    python scripts-local/p1_cost_report.py [--post-end 2026-11-27]

数据源:
    ~/.grok/cache/model-usage.jsonl       逐调用账本
    ~/.grok/cache/cache-breaks.jsonl      缓存断崖
    ~/.grok/logs/unified.jsonl            replay_diet / relay_compact / idle_wait
    ~/.grok/sessions/<cwd>/<session>/usage.json   回合级汇总

口径:
    - 只统计 relay 主模型 deepseek-flash；价格按实际账单
      input $0.15/M、cache read $0.03/M、output $0.60/M。
    - 变更前近端 = 2026-11-20T14:00Z..2026-11-21T12:08Z（v1.0.84 部署前）；
      变更前 6 天 = 2026-11-14T00:00Z..2026-11-20T11:00Z；
      变更后 = 2026-11-22T00:55Z（首个 replay_diet）..post_end。
    - 排除 2026-11-20T11:00Z..14:00Z 的压测流量，以及 01a1fd23 接力基准。
    - 长回合按 usage.json 的 modelCalls 分桶；成本按窗口内逐调用账本聚合。
"""

from __future__ import annotations

import argparse
import glob
import json
import os
from collections import Counter, defaultdict
from datetime import datetime, timezone

HOME = os.path.expanduser("~")
LEDGER = os.path.join(HOME, ".grok", "cache", "model-usage.jsonl")
BREAKS = os.path.join(HOME, ".grok", "cache", "cache-breaks.jsonl")
LOG = os.path.join(HOME, ".grok", "logs", "unified.jsonl")
SESSIONS = os.path.join(HOME, ".grok", "sessions")

INPUT_PER_M = 0.15
CACHE_PER_M = 0.03
OUTPUT_PER_M = 0.60

PRE_BROAD = ("2026-11-14T00:00:00", "2026-11-20T11:00:00")
PRE_NEAR = ("2026-11-20T00:00:00", "2026-11-21T12:08:00")
POST_START = "2026-11-22T00:55:00"
BENCH = [
    ("2026-11-20T11:00:00", "2026-11-20T14:00:00"),  # 压测流量
    ("2026-11-21T09:17:00", "2026-11-21T12:14:00"),  # 01a1fd23 接力基准
]
BENCH_SESSION = "01a1fd23"


def dt(value: str) -> datetime:
    text = str(value)
    if len(text) == 10:
        text += "T00:00:00"
    return datetime.fromisoformat(text[:19] + "+00:00")


def cost(prompt: int, cached: int, output: int) -> float:
    return (
        (prompt - cached) * INPUT_PER_M / 1e6
        + cached * CACHE_PER_M / 1e6
        + output * OUTPUT_PER_M / 1e6
    )


def load_ledger() -> list[tuple[datetime, dict]]:
    rows: list[tuple[datetime, dict]] = []
    try:
        with open(LEDGER, encoding="utf-8") as handle:
            for line in handle:
                try:
                    row = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if row.get("modelId") != "deepseek-flash":
                    continue
                ts = datetime.fromtimestamp(row["tsUnixMs"] / 1000, timezone.utc)
                if any(dt(start) <= ts < dt(end) for start, end in BENCH):
                    continue
                rows.append((ts, row))
    except OSError:
        pass
    return rows


def load_log() -> tuple[list[dict], list[dict], list[dict], list[dict], dict[str, list[dict]]]:
    diets: list[dict] = []
    relays: list[dict] = []
    idle: list[dict] = []
    breaks: list[dict] = []
    calls: dict[str, list[dict]] = defaultdict(list)
    try:
        with open(LOG, encoding="utf-8", errors="replace") as handle:
            for line in handle:
                try:
                    row = json.loads(line)
                except json.JSONDecodeError:
                    continue
                sid = row.get("sid")
                if not sid:
                    continue
                msg = row.get("msg")
                if msg == "shell.replay_diet":
                    diets.append(row)
                elif msg == "shell.relay_compact":
                    relays.append(row)
                elif msg and (
                    msg.startswith("shell.idle_wait_compact")
                    or msg == "shell.idle_compact.aborted"
                ):
                    idle.append(row)
                elif msg == "session.cache_break":
                    breaks.append(row)
                elif msg == "shell.turn.inference_done":
                    calls[sid].append(row)
    except OSError:
        pass
    return diets, relays, idle, breaks, calls


def load_turns() -> list[dict]:
    rows: list[dict] = []
    for path in glob.glob(os.path.join(SESSIONS, "*", "*", "usage.json")):
        if os.path.getsize(path) == 0:
            continue
        try:
            usage = json.load(open(path, encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        sid = (usage.get("sessionId") or "")[:8]
        if sid == BENCH_SESSION:
            continue
        summary_path = os.path.join(os.path.dirname(path), "summary.json")
        try:
            summary = json.load(open(summary_path, encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            summary = {}
        title = summary.get("session_summary") or ""
        if "基准" in title:
            continue
        for turn in usage.get("turns") or []:
            def number(key: str) -> int | None:
                value = turn.get(key)
                return value if isinstance(value, (int, float)) else None

            calls = number("modelCalls")
            prompt = number("inputTokens")
            cached = number("cachedReadTokens")
            output = number("outputTokens")
            if None in (calls, prompt, cached, output) or calls <= 0:
                continue
            rows.append(
                {
                    "sid": sid,
                    "ts": dt(turn.get("endedAt") or ""),
                    "calls": calls,
                    "prompt": prompt,
                    "cached": cached,
                    "output": output,
                }
            )
    return rows


def aggregate_ledger(rows: list[tuple[datetime, dict]], start: str, end: str) -> dict:
    a, b = dt(start), dt(end)
    z = [row for ts, row in rows if a <= ts < b]
    prompt = sum(row.get("promptTokens", 0) for row in z)
    cached = sum(row.get("cachedPromptTokens", 0) for row in z)
    output = sum(row.get("completionTokens", 0) for row in z)
    reasoning = sum(row.get("reasoningReplayTokens", 0) for row in z)
    tools = sum(row.get("toolResultTokens", 0) for row in z)
    total = cost(prompt, cached, output)
    calls = len(z)
    return {
        "calls": calls,
        "prompt": prompt,
        "cached": cached,
        "output": output,
        "reasoning": reasoning,
        "tools": tools,
        "cost": total,
        "hit": cached / prompt if prompt else 0,
        "prompt_call": prompt / calls if calls else 0,
        "cost_call": total / calls if calls else 0,
    }


def aggregate_turns(rows: list[dict], start: str, end: str, low: int, high: int) -> dict:
    a, b = dt(start), dt(end)
    z = [row for row in rows if a <= row["ts"] < b and low <= row["calls"] <= high]
    calls = sum(row["calls"] for row in z)
    prompt = sum(row["prompt"] for row in z)
    cached = sum(row["cached"] for row in z)
    output = sum(row["output"] for row in z)
    total = sum(cost(row["prompt"], row["cached"], row["output"]) for row in z)
    return {
        "turns": len(z),
        "calls": calls,
        "prompt": prompt,
        "cached": cached,
        "output": output,
        "cost": total,
        "hit": cached / prompt if prompt else 0,
        "prompt_call": prompt / calls if calls else 0,
        "output_call": output / calls if calls else 0,
        "cost_call": total / calls if calls else 0,
    }


def print_ledger_table(title: str, rows: list[tuple[datetime, dict]], windows: list[tuple[str, str, str]]) -> None:
    print(f"\n## {title}")
    print("| 窗口 | calls | prompt | hit | output | $/call | avg prompt |")
    print("|---|---:|---:|---:|---:|---:|---:|")
    for label, start, end in windows:
        agg = aggregate_ledger(rows, start, end)
        print(
            f"| {label} | {agg['calls']} | {agg['prompt']/1e6:.1f}M | "
            f"{agg['hit']*100:.2f}% | {agg['output']/1e3:.0f}k | "
            f"${agg['cost_call']:.5f} | {agg['prompt_call']/1000:.1f}k |"
        )


def print_turn_table(title: str, rows: list[dict], windows: list[tuple[str, str, str]]) -> None:
    print(f"\n## {title}")
    print("| 窗口 | 桶 | turns | calls | avg prompt | hit | out/call | $/call |")
    print("|---|---|---:|---:|---:|---:|---:|---:|")
    for label, start, end in windows:
        for low, high, name in ((1, 19, "1-19"), (20, 10**9, "20+"), (50, 10**9, "50+")):
            agg = aggregate_turns(rows, start, end, low, high)
            if not agg["calls"]:
                continue
            print(
                f"| {label} | {name} | {agg['turns']} | {agg['calls']} | "
                f"{agg['prompt_call']/1000:.1f}k | {agg['hit']*100:.1f}% | "
                f"{agg['output_call']:.0f} | ${agg['cost_call']:.5f} |"
            )


def print_diet_table(diets: list[dict], calls: dict[str, list[dict]]) -> None:
    print("\n## replay diet 事件")
    print("| 时间(UTC) | 会话 | before | after | 释放 tokens | reasoning | stubbed | 下一调用 |")
    print("|---|---|---:|---:|---:|---:|---:|---:|")
    for event in sorted(diets, key=lambda row: row["ts"]):
        ctx = event.get("ctx") or {}
        sid = event["sid"]
        after = [row for row in calls.get(sid, []) if row["ts"] > event["ts"]]
        next_prompt = after[0]["ctx"].get("prompt_tokens") if after else None
        next_text = f"{next_prompt/1000:.0f}k" if next_prompt else "-"
        print(
            f"| {event['ts'][:19]} | {sid[:8]} | {ctx.get('prompt_tokens_before',0)/1000:.0f}k | "
            f"{ctx.get('prompt_tokens_after',0)/1000:.0f}k | {ctx.get('est_tokens_freed',0)/1000:.0f}k | "
            f"{ctx.get('removed_reasoning',0)} | {ctx.get('stubbed_tool_results',0)} | {next_text} |"
        )


def print_break_table(breaks: list[dict], calls: dict[str, list[dict]]) -> None:
    post = [row for row in breaks if row["ts"] >= POST_START]
    print(f"\n## 缓存断崖（变更后 {len(post)} 次）")
    print("| 时间(UTC) | 会话 | 原因 | before hit | after hit | 恢复调用数 |")
    print("|---|---|---|---:|---:|---:|")
    for event in sorted(post, key=lambda row: row["ts"]):
        sid = event["sid"]
        after = sorted(
            [row for row in calls.get(sid, []) if row["ts"] > event["ts"]],
            key=lambda row: row["ts"],
        )
        recovered = "-"
        for index, row in enumerate(after, 1):
            ctx = row["ctx"]
            hit = ctx.get("cached_prompt_tokens", 0) / max(ctx.get("prompt_tokens", 1), 1)
            if hit >= 0.95:
                recovered = str(index)
                break
        ctx = event.get("ctx") or {}
        print(
            f"| {event['ts'][:19]} | {sid[:8]} | {ctx.get('reason','?')} | "
            f"{ctx.get('hit_rate_before',0)*100:.1f}% | {ctx.get('hit_rate_after',0)*100:.1f}% | {recovered} |"
        )


def print_session_table(sessions: list[dict], diets: list[dict], relays: list[dict]) -> None:
    diet_count = Counter(row["sid"][:8] for row in diets)
    relay_count = Counter(row["sid"][:8] for row in relays)
    print("\n## 变更后会话")
    print("| 会话 | 项目 | 调用 | prompt | hit | 成本 | $/call | diet | full |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    for row in sorted(sessions, key=lambda item: item["ts"]):
        print(
            f"| {row['sid']} | {row['project'][:18]} | {row['calls']} | {row['prompt']/1e6:.1f}M | "
            f"{row['hit']*100:.1f}% | ${row['cost']:.2f} | ${row['cost_call']:.5f} | "
            f"{diet_count[row['sid']]} | {relay_count[row['sid']]} |"
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--post-end", default="2026-11-27")
    parser.add_argument("--daily", action="store_true", help="附每日账本")
    args = parser.parse_args()

    ledger = load_ledger()
    diets, relays, idle, breaks, calls = load_log()
    turns = load_turns()

    windows = [
        ("变更前 6 天", PRE_BROAD[0], PRE_BROAD[1]),
        ("变更前近端", PRE_NEAR[0], PRE_NEAR[1]),
        ("变更后", POST_START, args.post_end),
    ]

    print(f"# P1 阶段报告（截至 {args.post_end}）")
    print_ledger_table("逐调用账本", ledger, windows)
    print_turn_table("回合桶（排除基准会话）", turns, windows)

    sessions = []
    seen = set()
    for path in glob.glob(os.path.join(SESSIONS, "*", "*", "usage.json")):
        if os.path.getsize(path) == 0:
            continue
        try:
            usage = json.load(open(path, encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        sid = (usage.get("sessionId") or "")[:8]
        if sid in seen or sid == BENCH_SESSION:
            continue
        summary_path = os.path.join(os.path.dirname(path), "summary.json")
        try:
            summary = json.load(open(summary_path, encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            summary = {}
        updated = summary.get("updated_at") or ""
        if updated[:19] < POST_START:
            continue
        session = usage.get("session") or {}
        prompt = session.get("inputTokens")
        cached = session.get("cachedReadTokens")
        output = session.get("outputTokens")
        count = session.get("modelCalls")
        if not all(isinstance(value, (int, float)) for value in (prompt, cached, output, count)):
            continue
        seen.add(sid)
        project = (summary.get("info", {}).get("cwd") or "").replace("\\", "/").split("/")[-1]
        total = cost(prompt, cached, output)
        sessions.append(
            {
                "sid": sid,
                "project": project,
                "ts": dt(updated),
                "calls": count,
                "prompt": prompt,
                "hit": cached / prompt if prompt else 0,
                "cost": total,
                "cost_call": total / count if count else 0,
            }
        )
    print_session_table(sessions, diets, relays)

    print_diet_table(diets, calls)
    print_break_table(breaks, calls)

    print("\n## relay / idle 事件")
    print(f"- relay_compact: {len(relays)} 次")
    for event in relays:
        ctx = event.get("ctx") or {}
        print(
            f"  - {event['ts'][:19]} {event['sid'][:8]} generation={ctx.get('relay_generation')} "
            f"messages={ctx.get('relay_user_messages')} hits={ctx.get('relay_summary_verbatim_hits')} "
            f"missing={ctx.get('relay_summary_verbatim_missing')} summary_count={ctx.get('summary_count')}"
        )
    fired = [row for row in idle if row.get("msg", "").endswith("idle_wait_compact.fired")]
    skipped = [row for row in idle if row.get("msg", "").endswith("idle_wait_compact.skipped")]
    aborted = [row for row in idle if row.get("msg", "").endswith(".aborted")]
    print(f"- idle_wait_compact: fired={len(fired)} skipped={len(skipped)} aborted={len(aborted)}")
    for label, rows in (("skipped", skipped), ("aborted", aborted)):
        reasons = Counter((row.get("ctx") or {}).get("reason", "?") for row in rows)
        if reasons:
            print(f"  - {label}: {dict(reasons)}")

    if args.daily:
        print("\n## 每日账本")
        print("| 日期 | calls | prompt | hit | output | $ | $/call |")
        print("|---|---:|---:|---:|---:|---:|---:|")
        by_day: dict[str, list[tuple[datetime, dict]]] = defaultdict(list)
        for ts, row in ledger:
            if dt("2026-11-14") <= ts < dt(args.post_end):
                by_day[ts.date().isoformat()].append((ts, row))
        for day in sorted(by_day):
            day_rows = by_day[day]
            prompt = sum(row.get("promptTokens", 0) for _, row in day_rows)
            cached = sum(row.get("cachedPromptTokens", 0) for _, row in day_rows)
            output = sum(row.get("completionTokens", 0) for _, row in day_rows)
            total = cost(prompt, cached, output)
            print(
                f"| {day} | {len(day_rows)} | {prompt/1e6:.1f}M | {cached/prompt*100:.1f}% | "
                f"{output/1e3:.0f}k | ${total:.2f} | ${total/len(day_rows):.5f} |"
            )


if __name__ == "__main__":
    main()
