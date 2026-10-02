#!/usr/bin/env python3
"""i18n 扫尾静态扫描器（零编译，仿 win_scan.py 先例，T3 方法第 1 步）。

扫描 xai-grok-pager 的 views/ slash/ app/ 下未被 tr()/tr_str() 包裹的英文
字符串字面量，并按宏上下文预分类：

  plain  — 非日志宏上下文的裸英文字面量（人工分类：译 / 口径豁免）
  fmt    — format!/write!/writeln! 模板（整键进表，{} 保留）
  cmp    — 比较/匹配键（==, starts_with, contains, match 臂 =>, get(...)）——不译
  log    — log 宏 / println / eprintln / panic / assert——不译（口径：日志）
  test   — #[cfg(test)] 模块内——跳过
  wrapped— 已被 tr()/tr_str() 包裹——跳过

结构扫描（括号配对/宏区间）在「注释剥离 + 字符串掩码」上进行：字符串字面量
内部的花括号/圆括号（如 "}"、\\u{2026}、format 模板）不参与结构计数。

用法：python scripts-local/i18n_sweep.py [输出文件]
不传输出文件则写 docs-local/i18n-sweep-classified-<date>.txt 并打印 plain+fmt 摘要。
"""

import re
import sys
from bisect import bisect_right
from collections import defaultdict
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "crates" / "codegen" / "xai-grok-pager" / "src"
TARGET_DIRS = [SRC / "views", SRC / "slash", SRC / "app"]
EXCLUDE_FILES = {"i18n.rs"}  # 翻译表本身：满表英文键，排除
SKIP_FILE_SUFFIX = "_tests.rs"

CHAR_LIT = re.compile(
    r"'(?:\\(?:u\{[0-9A-Fa-f_]+\}|x[0-9A-Fa-f]{2}|.)|[^'\\])'"
)


def strip_comments(text):
    """把注释替换为空（保留换行），字符串原样保留。返回 code 文本（与原文本不等长）。"""
    out = []
    i, n = 0, len(text)
    state = "code"
    while i < n:
        c = text[i]
        if state == "code":
            if c == '"':
                state = "str"
                out.append(c)
            elif c == "r" and i + 1 < n and text[i + 1] in "\"#":
                j = i + 1
                hashes = 0
                while j < n and text[j] == "#":
                    hashes += 1
                    j += 1
                if j < n and text[j] == '"':
                    state = "raw"
                    raw_hashes = hashes
                    out.append(text[i : j + 1])
                    i = j + 1
                    continue
                out.append(c)
            elif c == "'":
                m = CHAR_LIT.match(text, i)
                if m:
                    out.append(m.group(0))
                    i = m.end()
                    continue
                out.append(c)  # lifetime，按普通代码处理
            elif c == "/" and i + 1 < n and text[i + 1] == "/":
                state = "line_comment"
                i += 2
                continue
            elif c == "/" and i + 1 < n and text[i + 1] == "*":
                state = "block_comment"
                depth = 1
                i += 2
                continue
            else:
                out.append(c)
        elif state == "str":
            out.append(c)
            if c == "\\":
                if i + 1 < n:
                    out.append(text[i + 1])
                i += 2
                continue
            if c == '"':
                state = "code"
        elif state == "raw":
            out.append(c)
            if c == '"' and text[i : i + raw_hashes + 2] == '"' + "#" * raw_hashes:
                out.append("#" * raw_hashes)
                i += raw_hashes + 1
                state = "code"
                continue
        elif state == "line_comment":
            if c == "\n":
                state = "code"
                out.append(c)
        elif state == "block_comment":
            if c == "*" and i + 1 < n and text[i + 1] == "/":
                depth -= 1
                i += 2
                if depth == 0:
                    state = "code"
                continue
            if c == "/" and i + 1 < n and text[i + 1] == "*":
                depth += 1
                i += 2
                continue
            if c == "\n":
                out.append(c)
        i += 1
    return "".join(out)


STR_LIT = re.compile(r'"(?:[^"\\\n]|\\.)*"')


def extract_literals(kept):
    """提取全部字符串字面量（含原始字符串），返回 [(start, end, text)]，外层优先。"""
    cands = []
    for m in STR_LIT.finditer(kept):
        cands.append((m.start(), m.end(), m.group(0)))
    for m in re.finditer(r'r(#+)"', kept):
        hashes = m.group(1)
        closer = '"' + hashes
        end = kept.find(closer, m.end())
        if end == -1:
            continue
        cands.append((m.start(), end + len(closer), kept[m.start() : end + len(closer)]))
    cands.sort()
    out = []
    for start, end, text in cands:
        # 跳过被上一个区间完整覆盖的（raw 字符串内部的 "..." 会被 STR_LIT 命中）
        if out and start < out[-1][1]:
            continue
        out.append((start, end, text))
    return out


MACRO_RE = re.compile(r"\b([A-Za-z_][A-Za-z0-9_]*)\s*!\s*\(")
# log 桶 = 日志/断言/panic/运行时数据宏（json!/info_span!/include_str! 等协议与数据构造，
# 口径一律不译；报告只详列 plain/fmt，本桶仅计数）
LOG_MACROS = {
    "debug", "trace", "info", "warn", "error", "println", "print", "eprintln",
    "eprint", "panic", "unreachable", "todo", "unimplemented", "assert",
    "assert_eq", "assert_ne", "expect",
    "debug_assert", "debug_assert_eq", "debug_assert_ne",
    "bail", "anyhow", "json", "info_span", "span", "include_str", "concat",
}
FMT_MACROS = {"format", "write", "writeln"}
TR_PRECEDERS = ("tr(", "tr_str(")
CMP_PRECEDERS = (
    "==", "!=", ".starts_with(", ".contains(", ".ends_with(", ".eq(", ".ne(",
    ".find(", ".rfind(", ".replace(", ".get(", ".split(", ".split_once(",
    ".trim_start_matches(", ".trim_end_matches(", ".trim_matches(",
    ".strip_prefix(", ".strip_suffix(", "matches!(",
    ".any(", ".all(", ".position(", ".then_some(",
)


def test_mod_line_ranges(kept, mask):
    """返回 #[cfg(test)] 模块覆盖的 (start_line, end_line) 行区间（1-based，含端点）。

    括号配对跳过字符串字面量内部（mask==1 的偏移）。
    """
    ranges = []
    for m in re.finditer(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]", kept):
        if mask[m.start()]:
            continue
        line = kept.count("\n", 0, m.start()) + 1
        mod = re.compile(r"\bmod\s+\w+\s*\{").search(kept, m.end())
        if not mod or mask[mod.end() - 1]:
            continue
        depth = 0
        i = mod.end() - 1
        n = len(kept)
        while i < n:
            if mask[i]:
                i += 1
                continue
            c = kept[i]
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    ranges.append((line, kept.count("\n", 0, i) + 1))
                    break
            i += 1
    return ranges


def classify_file(path):
    raw = path.read_text(encoding="utf-8", errors="replace")
    kept = strip_comments(raw)
    lits = extract_literals(kept)
    mask = bytearray(len(kept))
    for start, end, _ in lits:
        mask[start:end] = b"\x01" * (end - start)

    starts = [s for s, _, _ in lits]
    ends = [e for _, e, _ in lits]

    def in_string(off):
        i = bisect_right(starts, off) - 1
        return i >= 0 and off < ends[i]

    test_ranges = test_mod_line_ranges(kept, mask)

    def in_test(off):
        line = kept.count("\n", 0, off) + 1
        return any(a <= line <= b for a, b in test_ranges)

    # 宏上下文：宏区间 = 宏名起点到其调用括号闭合；字符串内部字符不参与计数。
    pending = [(m.start(), m.group(1)) for m in MACRO_RE.finditer(kept) if not mask[m.start()]]
    pending_map = dict(pending)
    spans = []
    open_stack = []  # (entry_depth, name, start_off)
    depth = 0
    i = 0
    n = len(kept)
    while i < n:
        if mask[i]:
            i += 1
            continue
        name = pending_map.get(i)
        if name is not None:
            j = kept.index("(", i)
            open_stack.append((depth, name, i))
            depth += 1  # 进入宏的 '('
            i = j + 1
            continue
        c = kept[i]
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if open_stack and depth == open_stack[-1][0]:
                _, nm, st = open_stack.pop()
                spans.append((st, i + 1, nm))
        i += 1

    def macro_at(off):
        found = None
        for st, en, nm in spans:
            if st <= off < en:
                found = nm  # 后命中者起点更靠后 = 更内层
        return found

    findings = []
    for start, end, text in lits:
        line = kept.count("\n", 0, start) + 1
        inner = text[text.index('"') + 1 : text.rindex('"')]
        # 英文过滤：含 3 连字母，或含空格且有 2 连字母
        if not (
            re.search(r"[A-Za-z]{3,}", inner)
            or (re.search(r"[A-Za-z]{2,}", inner) and " " in inner)
        ):
            continue
        before = kept[:start].rstrip()
        after = kept[end:].lstrip()
        mac = macro_at(start) or ""
        # 标识符形态：kebab/snake/路径式小写键（协议/状态/配置键，渲染侧另行翻译）→ id 桶
        is_id = (
            re.fullmatch(r"[a-z0-9][a-z0-9_.:/\-]*", inner) is not None
            and re.search(r"[-_./]", inner) is not None
        )
        if in_test(start):
            tag = "test"
        elif before.endswith(TR_PRECEDERS):
            tag = "wrapped"
        elif mac in LOG_MACROS:
            tag = "log"
        elif after.startswith("=>"):
            tag = "cmp"
        elif any(before.endswith(p) for p in CMP_PRECEDERS):
            tag = "cmp"
        elif is_id:
            tag = "id"
        elif mac in FMT_MACROS:
            tag = "fmt"
        else:
            tag = "plain"
        findings.append((line, tag, inner, mac))
    return findings


def main():
    out_path = (
        sys.argv[1]
        if len(sys.argv) > 1
        else str(ROOT / "docs-local" / f"i18n-sweep-classified-{date.today().isoformat()}.txt")
    )
    per_file = defaultdict(lambda: defaultdict(list))
    total = defaultdict(int)
    files = []
    for d in TARGET_DIRS:
        files.extend(sorted(d.rglob("*.rs")))
    for f in files:
        rel = f.relative_to(SRC).as_posix()
        if (
            f.name in EXCLUDE_FILES
            or f.name.endswith(SKIP_FILE_SUFFIX)
            or f.name == "tests.rs"
            or "/tests/" in f"/{rel}"
            or rel.startswith("tests/")
        ):
            continue
        try:
            findings = classify_file(f)
        except Exception as e:  # noqa: BLE001
            print(f"[error] {rel}: {e}", file=sys.stderr)
            continue
        for line, tag, inner, mac in findings:
            per_file[rel][tag].append((line, inner, mac))
            total[tag] += 1

    with open(out_path, "w", encoding="utf-8") as w:
        for rel in sorted(per_file):
            w.write(f"## {rel}\n")
            for tag in ("plain", "fmt", "id", "cmp", "log", "wrapped", "test"):
                items = per_file[rel].get(tag)
                if not items:
                    continue
                w.write(f"  [{tag}] x{len(items)}\n")
                if tag in ("plain", "fmt"):
                    for line, inner, mac in items:
                        text = inner if len(inner) <= 110 else inner[:107] + "..."
                        w.write(f"    {line}: ({mac}) {text!r}\n")
            w.write("\n")
    print(f"files scanned: {len(files)}")
    print(f"totals: {dict(total)}")
    print(f"report: {out_path}")
    print("\nplain/fmt per file:")
    for rel in sorted(per_file):
        p = len(per_file[rel].get("plain", []))
        fm = len(per_file[rel].get("fmt", []))
        if p or fm:
            print(f"  {rel}: plain={p} fmt={fm}")


if __name__ == "__main__":
    main()
