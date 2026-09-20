#!/usr/bin/env python3
"""win_sample.py — 阶段 2：对静态扫描后幸存的 crate 做抽样测试（docs-local/WIN-TEST-GATE.md）。

流程：--list 枚举 → 过滤 win-skip.txt → 确定性抽样（每 N 个取 1，排序稳定）→
逐个 --exact 运行（单测试挂起按超时记 HANG）→ 结果追加台账 win-compat-ledger.md。

结果语义：抽样通过的模块只是「未见异常」，不等于 win 兼容（还要过阶段 3 速读）；
抽样失败的模块进入阶段 2.5 深排。台账里每个失败模块给一行 --skip 建议。

用法：python scripts-local/win_sample.py -p xai-foo -p xai-bar [--every 10] [--timeout 120] [--list-only]
"""
import argparse
import datetime
import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
LEDGER = REPO / "docs-local" / "win-compat-ledger.md"
SKIPFILE = REPO / "docs-local" / "win-skip.txt"


def gate_env():
    env = dict(os.environ)
    env["TMP"] = env["TEMP"] = "D:\\cargo-tmp"
    env["RUST_MIN_STACK"] = "33554432"  # 32MB，溢出族零成本消解
    return env


def load_skips():
    if not SKIPFILE.exists():
        return []
    out = []
    for raw in SKIPFILE.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            out.append(line)
    return out


def list_tests(pkg, target_args, timeout):
    """返回 (names, err_tail)。err_tail 非 None 表示编译失败。"""
    cmd = ["cargo", "test", "-p", pkg, *target_args, "--", "--list", "--format", "terse"]
    try:
        r = subprocess.run(cmd, cwd=REPO, env=gate_env(), capture_output=True,
                           text=True, errors="replace", timeout=timeout * 6)
    except subprocess.TimeoutExpired:
        return None, "--list 超时（编译过慢，先手动构建一轮再抽样）"
    if r.returncode != 0:
        tail = "\n".join((r.stdout + r.stderr).splitlines()[-15:])
        return None, tail
    names = []
    for line in r.stdout.splitlines():
        m = re.match(r"^(.*): test$", line.strip())
        if m:
            names.append(m.group(1))
    return sorted(set(names)), None


def run_one(pkg, target_args, name, timeout):
    cmd = ["cargo", "test", "-p", pkg, *target_args, "--", "--exact", name]
    try:
        r = subprocess.run(cmd, cwd=REPO, env=gate_env(), capture_output=True,
                           text=True, errors="replace", timeout=timeout)
        if r.returncode == 0:
            return "PASS", ""
        tail = "\n".join((r.stdout + r.stderr).splitlines()[-8:])
        return "FAIL", tail
    except subprocess.TimeoutExpired:
        return "HANG", f"> {timeout}s 无返回，按挂起处理"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-p", dest="pkgs", action="append", required=True)
    ap.add_argument("--every", type=int, default=10, help="抽样密度：每 N 个取 1（默认 10）")
    ap.add_argument("--timeout", type=int, default=120, help="单测试超时秒数（默认 120）")
    ap.add_argument("--target", default="--lib", help="测试目标参数，默认 --lib；可给 --tests 或 --test <name>")
    ap.add_argument("--list-only", action="store_true", help="只枚举统计不运行")
    args = ap.parse_args()
    target_args = args.target.split()

    skips = load_skips()
    now = datetime.datetime.now().strftime("%Y-%m-%d %H:%M")
    entries = []

    for pkg in args.pkgs:
        names, err = list_tests(pkg, target_args, args.timeout)
        if err:
            entries.append(f"\n## {pkg} — {now}\n\n- **LIST-FAIL**（{target_args}）：```\n{err}\n```")
            print(f"[{pkg}] LIST-FAIL")
            continue
        skipset = [s for s in skips if any(s in n for n in names)]
        survivors = [n for n in names if not any(s in n for s in skips)]
        sample = survivors[::max(args.every, 1)]
        head = (f"\n## {pkg} — {now}（{args.target}, every={args.every}, timeout={args.timeout}s）\n\n"
                f"- 枚举 {len(names)}，skip 清单截走 {len(skipset)}，幸存 {len(survivors)}，抽样 {len(sample)}")
        if args.list_only:
            entries.append(head)
            print(f"[{pkg}] 枚举 {len(names)}，skip 截走 {len(skipset)}，幸存 {len(survivors)}（list-only，未运行）")
            continue

        results = defaultdict(list)
        details = []
        for k, name in enumerate(sample):
            status, tail = run_one(pkg, target_args, name, args.timeout)
            results[status].append(name)
            if status != "PASS":
                details.append((name, status, tail))
                print(f"  [{pkg}] {status}: {name}")
            sys.stdout.write(f"\r[{pkg}] {k + 1}/{len(sample)}")
            sys.stdout.flush()
        print()

        mod_of = lambda n: n.rsplit("::", 1)[0]
        failed_mods = defaultdict(list)
        for status_names in (results["FAIL"], results["HANG"]):
            for n in status_names:
                failed_mods[mod_of(n)].append(n)
        skip_suggest = sorted({f"--skip {m}::" for m in failed_mods})

        body = head + (
            f"\n- PASS {len(results['PASS'])} / FAIL {len(results['FAIL'])} / HANG {len(results['HANG'])}"
        )
        if failed_mods:
            body += "\n- 失败模块（进深排）："
            for m, ns in sorted(failed_mods.items()):
                body += f"\n  - `{m}`（{len(ns)} 个）"
            body += "\n- 建议 skip（族级，确认根因后登记 win-skip.txt）："
            for s in skip_suggest:
                body += f"\n  - `{s}`"
        if details:
            body += "\n- 失败输出尾部："
            for name, status, tail in details:
                body += f"\n\n**{status}** `{name}`\n```\n{tail}\n```"
        entries.append(body)
        print(f"[{pkg}] PASS {len(results['PASS'])} / FAIL {len(results['FAIL'])} / HANG {len(results['HANG'])}"
              f"（幸存 {len(survivors)} 中抽 {len(sample)}）")

    with LEDGER.open("a", encoding="utf-8") as f:
        for e in entries:
            f.write(e + "\n")
    print(f"\n台账已追加：{LEDGER}")


if __name__ == "__main__":
    main()
