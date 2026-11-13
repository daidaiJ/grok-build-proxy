#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""quota_audit.py — 统一池 pct 跳变跨设备对账取证脚本（配套 docs-local/quota-pct-jump-audit.md §5.1）

只读本地文件，零网络请求：
  - ~/.grok/logs/unified.jsonl 的 "billing: fetched credits config" 采样 → pct 轨迹 + 跳变窗口
  - ~/.grok/cache/model-usage.jsonl → 分模型 token 账本，按 config.toml 的 base_url 分类
    XAI-DIRECT（唯一动 pct 的流量）与第三方网关
  - ~/.grok/sessions 的文件 mtime → 窗口内本地活动

用法：
  python quota_audit.py                                        # 全量轨迹 + 跳变检测 + 账本
  python quota_audit.py --window "2026-09-25 00:00,2026-09-25 19:14"   # 只看某窗口（本地时间）
"""
import argparse
import json
import os
import sys
import time
from collections import defaultdict
from datetime import datetime, timedelta

GROK_DIR = os.environ.get("GROK_HOME") or os.path.expanduser("~/.grok")
UNIFIED = os.path.join(GROK_DIR, "logs", "unified.jsonl")
LEDGER = os.path.join(GROK_DIR, "cache", "model-usage.jsonl")
CONFIG = os.path.join(GROK_DIR, "config.toml")
SESSIONS = os.path.join(GROK_DIR, "sessions")
JUMP_THRESHOLD = 5  # pct 点


def ts_to_local(ms):
    return datetime.fromtimestamp(ms / 1000.0).strftime("%m-%d %H:%M:%S")


def iso_to_local(s):
    try:
        return datetime.fromisoformat(s.replace("Z", "+00:00")).astimezone()
    except Exception:
        return None


def parse_window(spec):
    """'YYYY-MM-DD HH:MM,YYYY-MM-DD HH:MM'（本地时间）→ (start, end)"""
    a, b = spec.split(",")
    fmt = "%Y-%m-%d %H:%M"
    return datetime.strptime(a.strip(), fmt), datetime.strptime(b.strip(), fmt)


def read_config_models():
    """config.toml → {model_key: base_url}。简易解析，只认 [model.X] 段内的 base_url。"""
    models = {}
    cur = None
    try:
        with open(CONFIG, "r", encoding="utf-8", errors="replace") as f:
            for line in f:
                line = line.strip()
                if line.startswith("[") and line.endswith("]"):
                    name = line[1:-1].strip().strip('"').strip("'")
                    cur = name if name.startswith("model.") else None
                elif cur and "=" in line:
                    k, _, v = line.partition("=")
                    if k.strip() == "base_url":
                        models[cur[len("model."):]] = v.strip().strip('"').strip("'")
    except FileNotFoundError:
        pass
    return models


def classify(model_id, cfg_models):
    """XAI-DIRECT = config 无该模型的第三方 base_url（默认后端 = api.x.ai）。
    匹配两轮：先精确（id 或网关前缀路径），再去 vendor 前缀（cc-/ark-）模糊兜底。"""
    mid = model_id.lower()
    for key, url in cfg_models.items():
        k = key.lower()
        if mid == k or mid.endswith("/" + k):
            return ("third-party", url) if url else ("XAI-DIRECT", url)
    for key, url in cfg_models.items():
        k = key.lower()
        if "-" in k:
            k = k.split("-", 1)[1]  # 去 vendor 前缀
        if k and k in mid:
            return ("third-party", url) if url else ("XAI-DIRECT", url)
    return "XAI-DIRECT", "(no [model.*] entry / default backend)"


def load_billing_samples():
    samples = []
    try:
        with open(UNIFIED, "r", encoding="utf-8", errors="replace") as f:
            for line in f:
                if '"billing: fetched credits config"' not in line:
                    continue
                try:
                    rec = json.loads(line)
                except Exception:
                    continue
                cfg = (rec.get("ctx") or {}).get("config") or {}
                pct = cfg.get("creditUsagePercent")
                period = cfg.get("currentPeriod") or {}
                ts = iso_to_local(rec.get("ts", ""))
                if pct is None or ts is None:
                    continue
                samples.append({
                    "ts": ts,
                    "ts_utc": rec.get("ts", ""),
                    "pct": float(pct),
                    "period_start": period.get("start", ""),
                    "tier": (rec.get("ctx") or {}).get("subscriptionTier", ""),
                })
    except FileNotFoundError:
        print(f"[!] 未找到 {UNIFIED}，跳过计费采样")
    samples.sort(key=lambda s: s["ts"])
    return samples


def load_ledger():
    rows = []
    try:
        with open(LEDGER, "r", encoding="utf-8", errors="replace") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    rows.append(json.loads(line))
                except Exception:
                    continue
    except FileNotFoundError:
        print(f"[!] 未找到 {LEDGER}，跳过账本")
    return rows


def session_mtime_activity(win_start, win_end):
    """sessions 目录下窗口内有 mtime 变动的文件（会话在写的迹象）。"""
    hits = []
    if not os.path.isdir(SESSIONS):
        return hits
    for root, _dirs, files in os.walk(SESSIONS):
        for fn in files:
            p = os.path.join(root, fn)
            try:
                m = datetime.fromtimestamp(os.path.getmtime(p)).astimezone()
            except OSError:
                continue
            if win_start <= m <= win_end:
                hits.append((m, os.path.relpath(p, GROK_DIR)))
    hits.sort()
    return hits


def main():
    ap = argparse.ArgumentParser(description="统一池 pct 跳变对账取证（只读本地）")
    ap.add_argument("--window", help='只看该窗口，本地时间 "YYYY-MM-DD HH:MM,YYYY-MM-DD HH:MM"')
    ap.add_argument("--intervals-since", dest="intervals_since",
                    help='逐区间对账表：该本地时间起的每个计费采样区间，Δpct vs 本机 xAI 直连 tokens')
    ap.add_argument("--jump", type=float, default=JUMP_THRESHOLD, help="跳变判定阈值（pct 点，默认 5）")
    args = ap.parse_args()
    win = parse_window(args.window) if args.window else None

    cfg_models = read_config_models()
    samples = load_billing_samples()
    ledger = load_ledger()

    # ---- 0. 逐区间对账表（--intervals-since）----
    if args.intervals_since:
        since = datetime.strptime(args.intervals_since, "%Y-%m-%d %H:%M").astimezone()
        pts = [s for s in samples if s["ts"] >= since]
        if len(pts) < 2:
            print("[0] --intervals-since：采样不足（本地 unified.jsonl 只有 %d 个点 ≥ %s）" % (len(pts), since))
        else:
            print("[0] 逐区间对账表（%s 起，共 %d 区间）" % (since.strftime("%m-%d %H:%M"), len(pts) - 1))
            print("  %-17s %-17s %6s %16s %10s %16s" % (
                "区间起", "区间止", "Δpct", "本机xAI tokens", "本机调用", "本机三方tokens"))
            tot_d = tot_x = tot_n = tot_tp = 0
            for a, b in zip(pts, pts[1:]):
                dp = b["pct"] - a["pct"]
                nx = tx = ntp = ttp = 0
                for r in ledger:
                    ts = r.get("tsUnixMs")
                    if not ts:
                        continue
                    t = datetime.fromtimestamp(ts / 1000.0).astimezone()
                    if not (a["ts"] < t <= b["ts"]):
                        continue
                    tk = (r.get("promptTokens", 0) or 0) + (r.get("completionTokens", 0) or 0)
                    cat, _u = classify(r.get("modelId", "?"), cfg_models)
                    if cat == "XAI-DIRECT":
                        nx += 1
                        tx += tk
                    else:
                        ntp += 1
                        ttp += tk
                print("  %-17s %-17s %+5.0f%% %16d %7d+%3d %16d" % (
                    a["ts"].strftime("%m-%d %H:%M:%S"), b["ts"].strftime("%m-%d %H:%M:%S"),
                    dp, tx, nx, ntp, ttp))
                tot_d += dp
                tot_x += tx
                tot_n += nx
                tot_tp += ttp
            print("  合计 Δpct=%+.0f 点 | 本机 xAI 直连 %d 次 / %d tokens | 本机三方 %d 次 / %d tokens"
                  % (tot_d, tot_n, tot_x, ntp, tot_tp))
            print("  （%s 之前的采样已随 unified.jsonl 滚动丢失，那段见 audit 文档 §2 表格）"
                  % pts[0]["ts"].strftime("%m-%d %H:%M"))
        if not args.window:
            return 0

    print("=" * 78)
    print("统一池对账取证  %s  (本机时间)" % datetime.now().strftime("%Y-%m-%d %H:%M"))
    print("config [model.*] base_url: %d 条第三方；其余模型 id 归类 XAI-DIRECT" % len(cfg_models))
    for k, v in cfg_models.items():
        print("    %-24s -> %s" % (k, v))

    # ---- 1. pct 轨迹，按计费周期分组 ----
    print("=" * 78)
    periods = defaultdict(list)
    for s in samples:
        periods[s["period_start"]].append(s)
    print("\n[1] billing pct 轨迹（%d 个采样点，%d 个周期）" % (len(samples), len(periods)))
    jumps = []
    for pstart in sorted(periods, key=lambda x: x or ""):
        pts = periods[pstart]
        p0 = iso_to_local(pstart) if pstart else None
        pend = p0 + timedelta(days=7) if p0 else None
        label = "%s → %s" % (p0.strftime("%m-%d %H:%M") if p0 else "?",
                             pend.strftime("%m-%d %H:%M") if pend else "?")
        print("\n  周期 %s  （%d 采样，%s，%s → %s%%）" % (
            pstart[:19] or "?", len(pts), label,
            ts_to_local(pts[0]["ts"].timestamp() * 1000) if pts else "-",
            pts[-1]["pct"] if pts else "-"))
        prev = None
        for s in pts:
            mark = ""
            if prev is not None and s["pct"] - prev["pct"] >= args.jump:
                mark = "   <<<< JUMP +%d 点" % (s["pct"] - prev["pct"])
                jumps.append((prev, s))
                print("  %s  %5.0f%%" % (prev["ts"].strftime("%m-%d %H:%M:%S"), prev["pct"]))
            if mark or prev is None or abs(s["pct"] - prev["pct"]) >= 1:
                print("  %s  %5.0f%%%s" % (s["ts"].strftime("%m-%d %H:%M:%S"), s["pct"], mark))
            prev = s

    if not jumps:
        print("\n  无 >=%d 点的跳变。" % args.jump)

    # ---- 2. 跳变窗口内的本地活动 ----
    for (a, b) in jumps:
        print("\n" + "-" * 78)
        print("[2] 跳变窗口  %s (%.0f%%) → %s (%.0f%%)   本地活动核查" % (
            a["ts"].strftime("%m-%d %H:%M:%S"), a["pct"],
            b["ts"].strftime("%m-%d %H:%M:%S"), b["pct"]))
        # unified.jsonl 小时分布
        hrs = defaultdict(int)
        n_all = 0
        try:
            with open(UNIFIED, "r", encoding="utf-8", errors="replace") as f:
                for line in f:
                    try:
                        rec = json.loads(line)
                    except Exception:
                        continue
                    t = iso_to_local(rec.get("ts", ""))
                    if t and a["ts"] <= t <= b["ts"]:
                        hrs[t.strftime("%m-%d %H:")] += 1
                        n_all += 1
        except FileNotFoundError:
            pass
        print("  unified.jsonl 窗口内 %d 条；按小时: %s" % (
            n_all, (", ".join("%s%d" % (h, c) for h, c in sorted(hrs.items())) or "0 条（静默）")))
        # 账本窗口内 xAI-direct tokens
        agg = defaultdict(lambda: [0, 0, 0, 0, 0])  # calls, prompt, completion, cached, reasoning
        for r in ledger:
            ts = r.get("tsUnixMs")
            if not ts:
                continue
            t = datetime.fromtimestamp(ts / 1000.0).astimezone()
            if not (a["ts"] <= t <= b["ts"]):
                continue
            cat, _url = classify(r.get("modelId", "?"), cfg_models)
            v = agg[(cat, r.get("modelId", "?"))]
            v[0] += 1
            v[1] += r.get("promptTokens", 0) or 0
            v[2] += r.get("completionTokens", 0) or 0
            v[3] += r.get("cachedPromptTokens", 0) or 0
            v[4] += r.get("reasoningTokens", 0) or 0
        if not agg:
            print("  账本（model-usage.jsonl）窗口内 0 次模型调用")
        else:
            print("  %-12s %-34s %6s %14s %12s %12s %12s" % (
                "类别", "模型", "调用", "prompt", "completion", "cached", "reasoning"))
            for (cat, mid), v in sorted(agg.items(), key=lambda kv: -sum(kv[1][1:3])):
                print("  %-12s %-34s %6d %14d %12d %12d %12d" % (cat, mid, *v))
            for cat in ("XAI-DIRECT", "third-party"):
                tot = sum(v[1] + v[2] for (c, _m), v in agg.items() if c == cat)
                if tot:
                    print("  >> %s 窗口内 tokens 合计: %d" % (cat, tot))
        # sessions mtime
        hits = session_mtime_activity(a["ts"], b["ts"])
        print("  sessions/ 窗口内 mtime 变动文件: %d 个%s" % (
            len(hits), ("（前 5: " + "; ".join(p for _m, p in hits[:5]) + "）") if hits else ""))

    # ---- 3. 全程分模型账本（默认=全周期；给了窗口则过滤） ----
    print("\n" + "=" * 78)
    if win:
        print("[3] 指定窗口账本 %s → %s（本地时间）" % (win[0], win[1]))
        def keep(t):
            return win[0] <= datetime.fromtimestamp(t / 1000.0) <= win[1]
    else:
        print("[3] 全程分模型账本（账本只记成功推理，失败请求系统性低估）")
        def keep(_t):
            return True
    agg = defaultdict(lambda: [0, 0, 0, 0, 0])
    first_ts = last_ts = None
    for r in ledger:
        ts = r.get("tsUnixMs")
        if not ts or not keep(ts):
            continue
        if first_ts is None or ts < first_ts:
            first_ts = ts
        if last_ts is None or ts > last_ts:
            last_ts = ts
        cat, url = classify(r.get("modelId", "?"), cfg_models)
        v = agg[(cat, r.get("modelId", "?"))]
        v[0] += 1
        v[1] += r.get("promptTokens", 0) or 0
        v[2] += r.get("completionTokens", 0) or 0
        v[3] += r.get("cachedPromptTokens", 0) or 0
        v[4] += r.get("reasoningTokens", 0) or 0
    if not agg:
        print("  （无数据）")
    else:
        print("  范围 %s → %s" % (ts_to_local(first_ts), ts_to_local(last_ts)))
        print("  %-12s %-34s %6s %14s %12s %12s %12s" % (
            "类别", "模型", "调用", "prompt", "completion", "cached", "reasoning"))
        for (cat, mid), v in sorted(agg.items(), key=lambda kv: -sum(kv[1][1:3])):
            print("  %-12s %-34s %6d %14d %12d %12d %12d" % (cat, mid, *v))
        for cat in ("XAI-DIRECT", "third-party"):
            n = sum(v[0] for (c, _m), v in agg.items() if c == cat)
            tot = sum(v[1] + v[2] for (c, _m), v in agg.items() if c == cat)
            print("  >> %s: %d 次调用 / %d tokens" % (cat, n, tot))
    # ---- 4. 真正的 xAI 直连流量明细（模型 id 含 grok-）----
    print("\n[4] xAI 直连流量明细（modelId 含 'grok-'；账本只记成功推理）")
    n_grok = 0
    for r in ledger:
        mid = r.get("modelId", "")
        if "grok-" not in mid.lower():
            continue
        n_grok += 1
        ts = r.get("tsUnixMs", 0)
        print("  %s  %-16s prompt=%-8d completion=%-6d cached=%-8d" % (
            ts_to_local(ts), mid, r.get("promptTokens", 0), r.get("completionTokens", 0),
            r.get("cachedPromptTokens", 0)))
    if not n_grok:
        print("  0 次调用")
    unknown = sorted({r.get("modelId", "?") for r in ledger
                      if classify(r.get("modelId", "?"), cfg_models)[0] == "XAI-DIRECT"
                      and "grok-" not in r.get("modelId", "").lower()})
    if unknown:
        print("  ⚠ 无 config 条目而按默认后端归 XAI-DIRECT 的模型 id（需人工甄别真实出口）: %s"
              % ", ".join(unknown))

    print("\n对账公式：某设备贡献点数 ≈ 窗口内 XAI-DIRECT tokens ÷ Q × 100（Q 服务端不下发，先比量级）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
