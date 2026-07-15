#!/usr/bin/env bash
# Behavioral test for redact-guard.sh.
#
# Proves the two claims that matter:
#   1. A tool result containing a LIVE secret value is BLOCKED (exit 2).
#   2. A tool result that only references a secret by NAME is ALLOWED (exit 0).
#   3. A base64-encoded copy of the secret is also BLOCKED (evasion attempt).
#
# The guard compares against the live environment, so we export a fake secret
# here and let the child hook inherit it. The fake value never touches a real
# secret store; it is a random-looking literal defined below.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
guard="$here/redact-guard.sh"

# A fake, obviously-not-real secret value used only for this test.
export FAKE_API_TOKEN="sk-test-9d4f2a7c1e8b0000dEADbeef"

fail=0
run_case() {
  local desc="$1" expected="$2" payload="$3"
  local got=0
  printf '%s' "$payload" | "$guard" >/dev/null 2>&1 || got=$?
  if [ "$got" -eq "$expected" ]; then
    echo "ok   - $desc (exit $got)"
  else
    echo "FAIL - $desc (expected $expected, got $got)"
    fail=1
  fi
}

# 1. Leak: the literal value appears in stdout -> blocked (2).
run_case "blocks a leaked secret value" 2 \
  "$(printf '{"tool_response":{"stdout":"the token is %s here"}}' "$FAKE_API_TOKEN")"

# 2. Name-reference only -> allowed (0). The literal '$FAKE_API_TOKEN' is intentional here —
# we are asserting that a NAME reference (not the value) passes the guard, so it must NOT expand.
# shellcheck disable=SC2016
run_case "allows a name reference" 0 \
  '{"tool_response":{"stdout":"using $FAKE_API_TOKEN to authenticate"}}'

# 3. Base64-encoded value -> blocked (2). Guards the common encoding-evasion path.
b64="$(printf '%s' "$FAKE_API_TOKEN" | base64)"
run_case "blocks a base64-encoded secret" 2 \
  "$(printf '{"tool_response":{"stdout":"blob %s"}}' "$b64")"

# 4. Empty / unrelated output -> allowed (0).
run_case "allows unrelated output" 0 \
  '{"tool_response":{"stdout":"build succeeded in 3.2s"}}'

# 5. A secret whose NAME does NOT match the convention, but IS listed in
#    ENVSTOW_LOADED, must still be blocked. This is the DATABASE_URL gap: the old
#    name-only guard let it leak. ENVSTOW_LOADED makes the match name-agnostic.
export DATABASE_URL="postgres://admin:hunter2SUPERSECRETvalue@db.prod/main"
export ENVSTOW_LOADED="DATABASE_URL"
run_case "blocks a non-conventionally-named secret via ENVSTOW_LOADED" 2 \
  "$(printf '{"tool_response":{"stdout":"connecting to %s now"}}' "$DATABASE_URL")"
unset ENVSTOW_LOADED

# 6. A MULTI-LINE secret must be caught even when only its middle (sensitive)
#    line leaks — the old line-oriented grep matched only the first line.
export TLS_KEY=$'-----BEGIN-----\nMIISECRETMIDDLELINExyz0000\n-----END-----'
export ENVSTOW_LOADED="TLS_KEY"
run_case "blocks the middle line of a multi-line secret" 2 \
  '{"tool_response":{"stdout":"exfiltrated: MIISECRETMIDDLELINExyz0000"}}'
unset ENVSTOW_LOADED TLS_KEY DATABASE_URL

# 7. A SHORT but distinctive token (7 chars, mixes letters+digit+symbol) must be
#    blocked — the old fixed 8-char floor missed these.
export SHORT_KEY="sk-9x2"
export ENVSTOW_LOADED="SHORT_KEY"
run_case "blocks a short but high-entropy token" 2 \
  '{"tool_response":{"stdout":"leaked token: sk-9x2 oops"}}'
unset ENVSTOW_LOADED SHORT_KEY

# 8. A low-entropy value must NOT over-block, even at 8+ chars: a run of digits
#    appears in innocent output all the time. (Old floor blocked this.)
export PIN_TOKEN="12345678"
export ENVSTOW_LOADED="PIN_TOKEN"
run_case "does not over-block a low-entropy digit run" 0 \
  '{"tool_response":{"stdout":"build finished, exit 12345678 lines processed"}}'
unset ENVSTOW_LOADED PIN_TOKEN

# 9. A dictionary-word value must NOT over-block: blocking every mention of a
#    common word would be worse than the rare short-secret miss.
export WORD_SECRET="password"
export ENVSTOW_LOADED="WORD_SECRET"
run_case "does not over-block a common dictionary word" 0 \
  '{"tool_response":{"stdout":"enter your password to continue"}}'
unset ENVSTOW_LOADED WORD_SECRET

exit "$fail"
