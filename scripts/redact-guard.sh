#!/usr/bin/env bash
# envstow PostToolUse guard.
#
# Reads the Claude Code PostToolUse payload on stdin and blocks the tool result
# from reaching the model's context if it contains any CURRENT secret value.
# It compares against live secret values in the environment, so it catches
# ACTUAL leaks — not just patterns — and never prints the secret itself.
#
# Which env vars count as secrets, most-reliable first:
#   1. Every name listed in $ENVSTOW_LOADED — the exact set `envstow unlock`
#      injected (names only; envstow sets this). This is authoritative: it does
#      not depend on a value's NAME matching a convention, so it catches
#      DATABASE_URL, DSN, CONNECTION_STRING, and anything else the store holds.
#   2. …unioned with a name-convention heuristic (*_KEY, *_TOKEN, …) as a
#      fallback for secret-shaped vars that reached the env some other way.
#
# Matching is exact-substring and multi-line-safe (done in Python, not a
# line-oriented grep), so a leaked middle line of a PEM/JSON value is caught.
# Still best-effort: values <8 chars are skipped to avoid false positives, and
# encodings other than raw/base64 (hex, gzip, url-encoding) can evade it — see
# the README threat model.
#
# Exit codes: 0 = allow, 2 = block (Claude Code treats non-zero as blocking).
set -euo pipefail

payload="$(cat)"

# Parse the payload, enumerate secret values from the environment, and check for
# leaks — all in one Python pass. Secrets are read from os.environ (never argv),
# and the value itself is never printed; only the offending NAME is named.
printf '%s' "$payload" | python3 -c '
import sys, os, json, base64, re

try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(0)  # unparseable payload -> nothing to inspect, allow

r = d.get("tool_response", {}) or {}
parts = [v for k in ("stdout", "stderr", "output")
         for v in [r.get(k)] if isinstance(v, str)]
output = "\n".join(parts)
if not output:
    sys.exit(0)

# 1) exact names envstow injected (authoritative, name-agnostic)
names = set()
loaded = os.environ.get("ENVSTOW_LOADED", "")
names.update(n for n in loaded.split(",") if n)

# 2) fallback: secret-shaped names that got into the env some other way
conv = re.compile(r"(_KEY|_TOKEN|_SECRET|_PASSWORD|_PASSWD)$|^API_")
names.update(n for n in os.environ if conv.search(n))

MIN = 8  # skip trivially short values -> avoids false positives on noise

def leak(name):
    value = os.environ.get(name, "")
    if len(value) < MIN:
        return None
    # Needles: the whole value, PLUS each individual line of a multi-line value
    # (a PEM/JSON secret can leak one sensitive line at a time, which is not a
    # substring of the whole). Over-blocking on a boilerplate line is the
    # fail-safe direction — key material in tool output is suspicious regardless.
    needles = {value}
    needles.update(ln for ln in value.splitlines() if len(ln) >= MIN)
    for n in needles:
        if len(n) >= MIN and n in output:
            return "the live value of"
    b64 = base64.b64encode(value.encode()).decode()
    if len(b64) >= MIN and b64 in output:
        return "a base64-encoded copy of"
    return None

for name in names:
    how = leak(name)
    if how:
        sys.stderr.write(
            f"BLOCKED by envstow: command output contained {how} ${name}; "
            "result withheld from context. Reference secrets by variable name "
            "only — never echo, print, log, or encode a value.\n")
        sys.exit(2)

sys.exit(0)
'
