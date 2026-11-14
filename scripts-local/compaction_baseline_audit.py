#!/usr/bin/env python3
"""压缩后基线归因 + 零调用工具统计（只读）。

用法:
    python scripts-local/compaction_baseline_audit.py [--days N]

数据源:
    ~/.grok/sessions/<cwd>/<session>/compaction_checkpoints/*.json
    ~/.grok/sessions/<cwd>/<session>/tool_definitions.json
    ~/.grok/sessions/<cwd>/<session>/chat_history.jsonl

口径:
    - 检查点的 compacted_history 固定为 system / user_info / user_query /
      summary / 技能清单 五条消息；逐条用 o200k_base 计 token。
    - 工具声明对 tool_definitions.json 的全量 JSON 计 token。
    - 估算基线 = 五条消息 + 工具声明；实测首轮从 model-usage.jsonl 取
      检查点写入后的第一条 main 采样（存在时）。
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import sys
import time
from datetime import datetime

try:
    import tiktoken

    _ENC = tiktoken.get_encoding("o200k_base")

    def count_tokens(text: str) -> int:
        return len(_ENC.encode(text))

except Exception:  # tiktoken 不可用时退化为 4 字符/token
    def count_tokens(text: str) -> int:
        return max(1, len(text) // 4)


SESSIONS = os.path.expanduser("~/.grok/sessions")
LEDGER = os.path.expanduser("~/.grok/cache/model-usage.jsonl")


def message_text(message: dict) -> str:
    content = message.get("content")
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(
            part.get("text", "") for part in content if isinstance(part, dict)
        )
    return ""


def load_ledger() -> list[dict]:
    rows: list[dict] = []
    try:
        with open(LEDGER, encoding="utf-8") as handle:
            for line in handle:
                line = line.strip()
                if line:
                    rows.append(json.loads(line))
    except OSError:
        pass
    return rows


def first_main_after(ledger: list[dict], session_id: str, ts_ms: float) -> int | None:
    hits = [
        row
        for row in ledger
        if row.get("sessionId") == session_id
        and row.get("purpose") in (None, "main")
        and row.get("tsUnixMs", 0) >= ts_ms
    ]
    return hits[0]["promptTokens"] if hits else None


def audit_checkpoints(ledger: list[dict]) -> None:
    rows = []
    for path in glob.glob(f"{SESSIONS}/*/*/compaction_checkpoints/*.json"):
        try:
            checkpoint = json.load(open(path, encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        created = checkpoint.get("created_at", "")
        try:
            ts_ms = datetime.fromisoformat(
                created.replace(".<SSN>", ".123456")
            ).timestamp() * 1000
        except ValueError:
            ts_ms = 0
        session_id = path.split(os.sep)[-3]
        messages = checkpoint.get("compacted_history") or []
        tokens = [count_tokens(message_text(message)) for message in messages]
        tool_path = os.path.join(os.path.dirname(os.path.dirname(path)), "tool_definitions.json")
        tool_tokens = 0
        try:
            tools = json.load(open(tool_path, encoding="utf-8"))
            tool_tokens = count_tokens(json.dumps(tools, ensure_ascii=False))
        except (OSError, json.JSONDecodeError):
            pass
        rows.append(
            {
                "created": created[:19],
                "session": session_id[:8],
                "components": tokens,
                "text": sum(tokens),
                "tools": tool_tokens,
                "estimate": sum(tokens) + tool_tokens,
                "actual": first_main_after(ledger, session_id, ts_ms),
            }
        )
    rows.sort(key=lambda row: row["created"])
    header = (
        f"{'created':19}  {'session':8}  {'system':>6} {'user_info':>9} "
        f"{'summary':>8} {'skills':>7} {'text':>7} {'tools':>6} {'estimate':>8} {'actual':>8}"
    )
    print(header)
    print("-" * len(header))
    for row in rows:
        components = row["components"] + [0] * (5 - len(row["components"]))
        system, user_info, _query, summary, skills = components[:5]
        print(
            f"{row['created']:19}  {row['session']:8}  {system:6d} {user_info:9d} "
            f"{summary:8d} {skills:7d} {row['text']:7d} {row['tools']:6d} "
            f"{row['estimate']:8d} {row['actual'] if row['actual'] is not None else '-':>8}"
        )


def audit_tools(days: int) -> None:
    cutoff = time.time() - days * 86400
    counts: dict[str, int] = {}
    sessions = 0
    for history in glob.glob(f"{SESSIONS}/*/*/chat_history.jsonl"):
        try:
            if os.path.getmtime(history) < cutoff:
                continue
        except OSError:
            continue
        sessions += 1
        with open(history, encoding="utf-8", errors="ignore") as handle:
            for line in handle:
                try:
                    record = json.loads(line)
                except json.JSONDecodeError:
                    continue
                for call in record.get("tool_calls") or []:
                    name = call.get("name")
                    if name:
                        counts[name] = counts.get(name, 0) + 1

    latest = max(
        glob.glob(f"{SESSIONS}/*/*/tool_definitions.json"),
        key=os.path.getmtime,
        default=None,
    )
    declared: dict[str, int] = {}
    if latest:
        tools = json.load(open(latest, encoding="utf-8"))
        for tool in tools:
            function = tool.get("function") or tool
            declared[function.get("name", "?")] = count_tokens(
                json.dumps(tool, ensure_ascii=False)
            )

    print(f"\n工具使用：最近 {days} 天、{sessions} 个会话、{sum(counts.values())} 次调用")
    print(f"{'calls':>7}  {'decl_tokens':>11}  tool")
    print("-" * 40)
    for name, tokens in sorted(declared.items(), key=lambda item: -item[1]):
        print(f"{counts.get(name, 0):7d}  {tokens:11d}  {name}")
    zero = sum(tokens for name, tokens in declared.items() if not counts.get(name))
    print(f"\n零调用工具声明合计: {zero} / {sum(declared.values())} token")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--days", type=int, default=30)
    args = parser.parse_args()
    if not os.path.isdir(SESSIONS):
        print(f"未找到会话目录: {SESSIONS}", file=sys.stderr)
        return 1
    audit_checkpoints(load_ledger())
    audit_tools(args.days)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
