#!/usr/bin/env bash
# Launch one replay-diet A/B session and record its group -> session id.
#
#   bash docs-local/replay-diet-ab/ab_launch.sh A
#   bash docs-local/replay-diet-ab/ab_launch.sh B
#
# A = replay_diet off (v1.0.83 in-turn behavior)
# B = replay_diet on  (v1.0.84+ in-turn diet)
#
# Extra arguments are passed to grok2. The wrapper always creates a new
# session so the policy is resolved at spawn time.
set -euo pipefail

group="${1:?usage: ab_launch.sh <A|B> [grok2 args...]}"
shift || true

case "$group" in
  A|a) group=A; diet=0 ;;
  B|b) group=B; diet=1 ;;
  *) echo "ab_launch: group must be A or B" >&2; exit 2 ;;
esac

grok2_bin="${GROK2_BIN:-/d/tool-cli/grok2/grok2.exe}"
if [ ! -x "$grok2_bin" ]; then
  echo "ab_launch: grok2 not executable: $grok2_bin (set GROK2_BIN)" >&2
  exit 2
fi

uuid="$(python -c 'import uuid; print(uuid.uuid4())')"
label="${AB_LABEL:-}"

python -c '
import datetime, json, os, sys
uid, group, label, cwd = sys.argv[1:5]
path = os.path.expanduser("~/.grok/ab/replay-diet/sessions.jsonl")
os.makedirs(os.path.dirname(path), exist_ok=True)
record = {
    "ts": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "group": group,
    "sessionId": uid,
    "label": label,
    "cwd": cwd,
    "grokVersion": "1.0.85",
}
with open(path, "a", encoding="utf-8") as f:
    f.write(json.dumps(record, ensure_ascii=False) + "\n")
' "$uuid" "$group" "$label" "$PWD"

echo "ab_launch: group=$group session=$uuid diet=$diet" >&2
export GROK_REPLAY_DIET="$diet"
exec "$grok2_bin" --session-id "$uuid" "$@"
