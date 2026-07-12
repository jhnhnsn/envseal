#!/usr/bin/env bash
# envseal PostToolUse guard.
#
# Reads the Claude Code PostToolUse payload on stdin and blocks the tool result
# from reaching the model's context if it contains any CURRENT secret value.
# It compares against live environment variables that look like secrets, so it
# catches ACTUAL leaks — not just patterns — and never prints the secret itself.
#
# Exit codes: 0 = allow, 2 = block (Claude Code treats non-zero as blocking).
set -euo pipefail

payload="$(cat)"

# Pull stdout + stderr from the tool_response, tolerating shape differences.
output="$(
  printf '%s' "$payload" | python3 -c '
import sys, json
try:
    d = json.load(sys.stdin)
except Exception:
    print(""); sys.exit(0)
r = d.get("tool_response", {}) or {}
parts = []
for k in ("stdout", "stderr", "output"):
    v = r.get(k)
    if isinstance(v, str):
        parts.append(v)
print("\n".join(parts))
' 2>/dev/null || true
)"

[ -z "$output" ] && exit 0

leaked=0
# Walk the environment; flag any secret-shaped var whose VALUE appears in output.
while IFS='=' read -r name value; do
  case "$name" in
    *_KEY|*_TOKEN|*_SECRET|*_PASSWORD|*_PASSWD|API_*|*_API_KEY|*_PRIVATE_KEY)
      # Skip trivially short / empty values to avoid false positives.
      if [ "${#value}" -ge 8 ] && printf '%s' "$output" | grep -qF -- "$value"; then
        leaked=1
      fi
      ;;
  esac
done < <(env)

if [ "$leaked" -eq 1 ]; then
  echo "BLOCKED by envseal: command output contained a live secret value; result withheld from context. Do not echo, print, or log secrets — reference them by variable name only." >&2
  exit 2
fi

exit 0
