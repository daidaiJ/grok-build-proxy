#!/usr/bin/env bash
# ctest — Windows 本机测试入口门控（策略见 docs-local/WIN-TEST-GATE.md）
#
# 用法: ctest [cargo test 参数...]     例: ctest -p xai-chat-state --lib
#
# 行为（L0+L1+L2 三层门控一次到位）:
#   L0 入口白名单: 只放行 docs-local/win-whitelist.txt 里的 crate；workspace 级
#      （不带 -p）与非白名单 crate 直接拒绝。上游套件（~90% Linux 耦合）根本进不来。
#   L1 环境修正: 强制 TMP/TEMP=D:\cargo-tmp、RUST_MIN_STACK=32MB（溢出族零成本消解）。
#   L2 分族 skip: docs-local/win-skip.txt 每行一个 --skip 子串，自动拼到命令尾部。
#
# 逃生门（用于分诊/统计，不用于日常）:
#   GATE_FORCE=1   放行非白名单 crate（仍注入 L1/L2）
#   GATE_SKIP=0    不拼 win-skip.txt
#   GATE_ENV=0     不注入 L1 环境变量
#   GATE_DRYRUN=1  只打印最终命令与 env，不执行（验证门控本身用）
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WHITELIST_FILE="$REPO/docs-local/win-whitelist.txt"
SKIPFILE="$REPO/docs-local/win-skip.txt"

# 白名单读取：文件优先；缺失时用内嵌兜底（本地自有主题 crate）
WHITELIST="$(cat "$WHITELIST_FILE" 2>/dev/null | sed 's/#.*//' | tr -d ' \r' | grep -v '^$' | paste -sd: -)"
WHITELIST="${WHITELIST:-xai-chat-state:xai-grok-sampler:xai-grok-sampling-types:xai-compaction-transcript:xai-grok-extra-ca:xai-grok-status-line:xai-grok-pager}"

# ---- L0: 解析 -p 包名，做白名单裁决 ----
args=("$@")
pkgs=()
i=0
while [ $i -lt ${#args[@]} ]; do
  case "${args[$i]}" in
    -p) [ $((i+1)) -lt ${#args[@]} ] && pkgs+=("${args[$((i+1))]}") && i=$((i+2)) || { echo "ctest: -p 缺包名" >&2; exit 2; };;
    -p=*) pkgs+=("${args[$i]#-p=}"); i=$((i+1));;
    *) i=$((i+1));;
  esac
done

deny() {
  echo "ctest: 已拒绝 —— $1" >&2
  echo "  本机默认只跑白名单 crate（上游套件 ~90% Linux 耦合，逐个排查是无底洞）。" >&2
  echo "  白名单/晋升规则见 docs-local/WIN-TEST-GATE.md；确要绕过: GATE_FORCE=1 ctest ..." >&2
  exit 3
}

if [ ${#pkgs[@]} -eq 0 ]; then
  deny "未指定 -p，workspace 级测试会整编译整跑上游套件"
fi
for p in "${pkgs[@]}"; do
  case ":$WHITELIST:" in
    *":$p:"*) ;;
    *) [ "${GATE_FORCE:-0}" = 1 ] || deny "crate '$p' 不在白名单";;
  esac
done

# ---- L1: 环境修正 ----
pre_env=()
if [ "${GATE_ENV:-1}" = 1 ]; then
  mkdir -p /d/cargo-tmp 2>/dev/null
  export TMP='D:\cargo-tmp' TEMP='D:\cargo-tmp'
  export RUST_MIN_STACK=33554432   # 32MB：Windows 测试线程默认 1MiB，深结构测试成族溢出 0xc00000fd
fi

# ---- L2: skip 清单拼装 ----
skip_args=()
if [ "${GATE_SKIP:-1}" = 1 ] && [ -f "$SKIPFILE" ]; then
  while IFS= read -r raw; do
    line="${raw%%#*}"
    line="${line#"${line%%[![:space:]]*}"}"; line="${line%"${line##*[![:space:]]}"}"
    [ -n "$line" ] && skip_args+=(--skip "$line")
  done < "$SKIPFILE"
fi

final_cmd=(cargo test "${args[@]}")

# libtest 参数必须位于 cargo 的 -- 之后；用户没给 -- 时补一个
has_ddash=0
for a in "${args[@]}"; do [ "$a" = "--" ] && has_ddash=1; done
if [ ${#skip_args[@]} -gt 0 ] && [ $has_ddash = 0 ]; then
  final_cmd+=(--)
fi

final_cmd=("${final_cmd[@]}" "${skip_args[@]}")

if [ "${GATE_DRYRUN:-0}" = 1 ]; then
  echo "DRY-RUN env: TMP=$TMP TEMP=$TEMP RUST_MIN_STACK=${RUST_MIN_STACK:-unset}"
  echo "DRY-RUN cmd: ${final_cmd[*]}"
  echo "DRY-RUN skip 条数: $(( ${#skip_args[@]} / 2 ))"
  exit 0
fi

exec "${final_cmd[@]}"
