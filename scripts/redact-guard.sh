#!/usr/bin/env bash
# envstow PostToolUse guard — DEPRECATED.
#
# This hand-copied script is superseded by the built-in `envstow scan-leak`, which has identical
# behavior but ships in the binary (so `envstow upgrade` keeps it current) and needs no python3.
# Point your PostToolUse hook at `envstow scan-leak` instead — see GUARDRAILS.md. Kept working so
# existing setups don't break.
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
# The Python body is single-quoted on purpose — `$name` etc. are Python, not
# shell, so they must NOT be expanded by bash.
# shellcheck disable=SC2016
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

def distinctive(s):
    """Is `s` distinctive enough that finding it in tool output means a real
    leak, not a chance collision with ordinary text?

    Length alone is a poor gate: `12345678` and `password` are 8+ chars yet
    appear in innocent output constantly (false positives), while a 6-char
    random token like `x9K2mQ` almost never does. So we gate on both length and
    character-class diversity — a proxy for entropy:

      * < 5 chars           -> never (too short to match safely, e.g. a PIN)
      * >= 12 chars         -> yes (accidental collision is negligible)
      * 5..11 chars         -> only if it mixes >=2 classes
                               (lower/upper/digit/symbol) — i.e. looks random,
                               not like a dictionary word or a run of digits

    This catches short-but-random secrets the old fixed floor of 8 missed, AND
    stops the old floor from blocking on common 8+ char strings.
    """
    n = len(s)
    if n < 5:
        return False
    if n >= 12:
        return True
    classes = (
        any(c.islower() for c in s)
        + any(c.isupper() for c in s)
        + any(c.isdigit() for c in s)
        + any(not c.isalnum() for c in s)
    )
    return classes >= 2

def leak(name):
    value = os.environ.get(name, "")
    if not distinctive(value):
        return None
    # Needles: the whole value, PLUS each individual line of a multi-line value
    # (a PEM/JSON secret can leak one sensitive line at a time, which is not a
    # substring of the whole). Over-blocking on a boilerplate line is the
    # fail-safe direction — key material in tool output is suspicious regardless.
    needles = {value}
    needles.update(value.splitlines())
    for n in needles:
        if distinctive(n) and n in output:
            return "the live value of"
    # base64 output is always high-entropy; a length gate is enough to avoid
    # matching a stray short blob.
    b64 = base64.b64encode(value.encode()).decode()
    if len(b64) >= 12 and b64 in output:
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
