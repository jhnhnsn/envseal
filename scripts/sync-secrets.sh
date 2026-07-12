#!/usr/bin/env bash
# envseal — push SOPS-managed secrets INTO a platform's native secret store.
#
# SOPS is the versioned source of truth; runtime secrets live in the platform.
# This decrypts in-memory and pipes selected vars into the target store. It never
# writes plaintext to disk.
#
# Usage:
#   scripts/sync-secrets.sh fly            # -> flyctl secrets import
#   scripts/sync-secrets.sh cloudflare     # -> wrangler secret bulk
#
# Run this yourself (human) — it decrypts, so it is intentionally NOT something the
# AI should invoke.
set -euo pipefail

SECRETS_FILE="secrets/secrets.enc.env"
target="${1:-}"

if ! command -v sops >/dev/null 2>&1; then
  echo "sops not installed" >&2; exit 1
fi
if [ ! -f "$SECRETS_FILE" ]; then
  echo "$SECRETS_FILE not found" >&2; exit 1
fi

case "$target" in
  fly)
    command -v flyctl >/dev/null 2>&1 || { echo "flyctl not installed" >&2; exit 1; }
    # flyctl secrets import reads KEY=VALUE lines from stdin.
    sops -d --output-type dotenv "$SECRETS_FILE" | flyctl secrets import
    echo "✅ synced secrets to Fly"
    ;;
  cloudflare|cf)
    command -v wrangler >/dev/null 2>&1 || { echo "wrangler not installed" >&2; exit 1; }
    # wrangler secret bulk reads a JSON object {"KEY":"VALUE",...} from stdin.
    sops -d --output-type json "$SECRETS_FILE" | wrangler secret bulk
    echo "✅ synced secrets to Cloudflare"
    ;;
  *)
    echo "Usage: $0 {fly|cloudflare}" >&2
    exit 2
    ;;
esac
