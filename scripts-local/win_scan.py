#!/usr/bin/env python3
"""win_scan.py — 阶段 0：Windows 不兼容测试的廉价静态发现（零编译）。

扫描 crates/ 下所有含测试的 .rs 文件，按特征签名分类：
  COMPILE-BREAK  未门控的 unix-only import（std::os::unix / nix / libc / signal_hook / termios）
                 —— 该 crate 的测试目标在 Windows 上编译都过不了，测试目标整体不可跑
  RUNTIME-RISK   能编译但行为 Linux 耦合（POSIX 路径 / sh 执行 / 权限位 / symlink / 信号 /
                 HOME env / 终端探测 / uid-gid）—— 抽样与 skip 候选的来源
  SELF-GATED     紧邻 #[cfg(unix)] 等门控内的命中 —— Windows 构建里根本不存在，零成本

输出：docs-local/win-compat-scan.md（重生成覆盖）+ stdout 摘要。
用法：python scripts-local/win_scan.py [--crates xai-foo,xai-bar]
"""
import argparse
import datetime
import re
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
OUT = REPO / "docs-local" / "win-compat-scan.md"

# (家族, 级别, 正则)。级别: compile=编译即断, runtime=运行期风险
SIGNATURES = [
    ("unix-import", "compile", r"^\s*use\s+(std::os::unix|nix|libc|signal_hook|termios)\b"),
    ("posix-path", "runtime", r"""['\"]/(?:usr|tmp|etc|var|home|proc|dev|opt|bin)/"""),
    ("sh-exec", "runtime", r"""Command::new\("(?:sh|bash|dash|zsh)"\)|\bsh\s+-c\b"""),
    ("perm-mode", "runtime", r"PermissionsExt|set_permissions|\.mode\(0o|\b0o[0-7]{3,4}\b"),
    ("symlink", "runtime", r"\bsymlink|hard_link\b"),
    ("signal", "runtime", r"\bSIG(?:INT|TERM|KILL|HUP|USR1|USR2|PIPE|CONT|STOP)\b|signal_hook"),
    ("pty-fork", "runtime", r"\bopenpty\b|\bfork\(|/dev/pts"),
    ("env-home", "runtime", r'"HOME"'),
    ("term-detect", "runtime", r"\bTERM\b|isatty|IsTty|enable_raw_mode|terminal_size"),
    ("uid-gid", "runtime", r"\bgetuid\b|\bgetgid\b|\bsetuid\b|getpwuid|getgrnam|\bchown\b"),
]

# 紧邻几行内的自门控标记（Windows 构建下代码不存在，不计风险）
# 宽松匹配：cfg(...) 内出现 unix / target_os=linux|macos / not(windows / target_family 任一即视为已门控
SELF_GATE = re.compile(r"cfg\([^)]*(?:\bunix\b|not\(windows|target_os\s*=\s*\"(?:linux|macos)\"|target_family)")
# feature 门控：默认 cargo test 不编译这些测试 —— 单独成桶，不计入风险也不算抽样证据
FEATURE_GATE = re.compile(r"""cfg\(\s*feature\s*=\s*"([^"]+)\"""")
TEST_ATTR = re.compile(r"#\[\s*(?:tokio::|async_std::)?test\b")
UNIQUE = "scan_line_unique"


def classify_line(lines, idx):
    """返回 (family_hits, gate) —— gate: None | 'unix' | ('feature', 名字)。

    自门控判定：回看 120 行内最近的门控标记（就近原则，最近的属性修饰所在 item），
    且两者之间没有顶层 `}` 边界（顶层 `}` 结束被门控的 item；
    不数花括号，format! 字符串会干扰计数）。
    """
    hits = []
    for fam, level, pat in SIGNATURES:
        if re.search(pat, lines[idx]):
            hits.append((fam, level))
    gate = None
    for j in range(idx - 1, max(-1, idx - 121), -1):
        if lines[j].startswith("}"):
            break
        m = FEATURE_GATE.search(lines[j])
        if m:
            gate = ("feature", m.group(1))
            break
        if SELF_GATE.search(lines[j]):
            gate = "unix"
            break
    return hits, gate


def scan_file(path):
    """返回 dict: tests, risk(家族->[(行号,行)]), unix_gated, feat_gated(特征->家族->n),
    compile(bool), features(本文件 cfg(feature) 全集)"""
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    lines = text.splitlines()
    if not TEST_ATTR.search(text):
        return None
    info = {"tests": len(TEST_ATTR.findall(text)), "risk": defaultdict(list),
            "unix_gated": 0, "feat_gated": defaultdict(int), "compile": False,
            "features": sorted(set(re.findall(r"""cfg\(\s*feature\s*=\s*"([^"]+)\"""", text)))}
    for i, _ in enumerate(lines):
        hits, gate = classify_line(lines, i)
        if not hits:
            continue
        if gate == "unix":
            info["unix_gated"] += 1
            continue
        if isinstance(gate, tuple) and gate[0] == "feature":
            for fam, _level in hits:
                info["feat_gated"][gate[1]] += 1
            continue
        for fam, level in hits:
            info["risk"][fam].append((i + 1, lines[i].strip()[:160]))
            if level == "compile":
                info["compile"] = True
    return info


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--crates", default="", help="逗号分隔只扫这些 crate（默认全部）")
    args = ap.parse_args()
    only = {c.strip() for c in args.crates.split(",") if c.strip()} or None

    # 扫描根：crates/（三层分类）+ prod/ 与 third_party/（workspace 成员在 crates 之外）
    def crate_name(rel):
        parts = rel.parts
        if parts[0] == "crates" and len(parts) >= 3:
            return parts[2]
        if parts[0] == "prod" and len(parts) >= 3 and parts[1] == "mc":
            return parts[2]
        if parts[0] in ("prod", "third_party") and len(parts) >= 2:
            return parts[1]
        return None

    results = {}  # crate -> {file -> info}
    for root in ("crates", "prod", "third_party"):
        base = REPO / root
        if not base.exists():
            continue
        for rs in sorted(base.rglob("*.rs")):
            rel = rs.relative_to(REPO)
            cname = crate_name(rel)
            if not cname:
                continue
            if only and cname not in only:
                continue
            info = scan_file(rs)
            if info:
                results.setdefault(cname, {})[str(rel)] = info

    # ---- 汇总 ----
    def crate_verdict(files):
        if any(f["compile"] for f in files.values()):
            return "COMPILE-BREAK"
        n_risk = sum(1 for f in files.values() if f["risk"])
        tests = sum(f["tests"] for f in files.values())
        if n_risk == 0:
            return "CLEAN"
        # 风险文件占比 < 10% 视为轻风险
        return "RISK" if n_risk / max(len(files), 1) > 0.1 else "RISK-LIGHT"

    now = datetime.date.today().isoformat()
    out = [f"# Windows 兼容性静态扫描报告（生成于 {now}，工具 scripts-local/win_scan.py，勿手改）",
           "",
           "verdict 含义：COMPILE-BREAK=未门控 unix import，测试目标在 Windows 编译不过（本机永不跑）；",
           "RISK=风险文件占比>10%，RISK-LIGHT=少量；CLEAN=无特征命中（仍可能有动态耦合，靠抽样兜底）。",
           "feature 门控的测试默认构建不编译，不计入 verdict，单列「feature 名单」供抽样时核对。",
           "",
           "## 汇总",
           "",
           "| crate | 测试文件数 | #test | verdict | 高危家族 | feature 名单 |",
           "|---|---|---|---|---|---|"]
    summary = {}
    for cname in sorted(results):
        files = results[cname]
        tests = sum(f["tests"] for f in files.values())
        verdict = crate_verdict(files)
        fams = defaultdict(int)
        for f in files.values():
            for fam, hits in f["risk"].items():
                fams[fam] += len(hits)
        top = ", ".join(k for k, _ in sorted(fams.items(), key=lambda x: -x[1])[:3]) or "-"
        feats = sorted({x for f in files.values() for x in f["features"]})
        fcell = ", ".join(feats) if feats else "-"
        summary[cname] = verdict
        out.append(f"| {cname} | {len(files)} | {tests} | {verdict} | {top} | {fcell} |")

    for cname in sorted(results):
        files = results[cname]
        verdict = summary[cname]
        feats = sorted({x for f in files.values() for x in f["features"]})
        out += ["", f"## {cname} — {verdict}", ""]
        if feats:
            out.append(f"feature 门控测试（默认构建不跑，抽样不含）：{', '.join(feats)}")
            out.append("")
        for fpath in sorted(files):
            f = files[fpath]
            if not f["risk"] and not f["feat_gated"]:
                continue
            gated_note = f", feature 门控命中 {sum(f['feat_gated'].values())}" if f["feat_gated"] else ""
            out.append(f"### `{fpath}`（{f['tests']} tests, unix 自门控命中 {f['unix_gated']}{gated_note}）")
            for fam, hits in sorted(f["risk"].items(), key=lambda x: -len(x[1])):
                shown = hits[:5]
                more = f"（共 {len(hits)} 处，仅列 {len(shown)}）" if len(hits) > len(shown) else ""
                out.append(f"- **{fam}** ×{len(hits)}{more}")
                for ln, line in shown:
                    out.append(f"  - L{ln}: `{line}`")
            out.append("")

    OUT.write_text("\n".join(out) + "\n", encoding="utf-8")

    # stdout 摘要
    by_v = defaultdict(list)
    for c, v in summary.items():
        by_v[v].append(c)
    print(f"报告已写入 {OUT}\n")
    for v in ("COMPILE-BREAK", "RISK", "RISK-LIGHT", "CLEAN"):
        lst = by_v.get(v, [])
        print(f"{v} ({len(lst)}): {', '.join(lst)}")


if __name__ == "__main__":
    main()
