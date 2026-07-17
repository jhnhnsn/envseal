---
name: verify
description: How to build and drive the envstow CLI end-to-end to verify a change at its real surface.
---

# Verifying envstow changes

Build: `cargo build` → binary at `target/debug/envstow`.

Drive it against a **scratch store**, never the repo's own `.envstow/`:

```sh
mkdir -p "$SCRATCH" && cd "$SCRATCH"
export ENVSTOW_IDENTITY="$SCRATCH/identity.txt"   # keeps the real key untouched
target/debug/envstow init --no-skill </dev/null
printf 'dummy-value' | target/debug/envstow set NAME
```

Gotchas:

- **This session IS an agent context** — `CLAUDECODE` is set, so `get`/`env`/previews mask or
  refuse. That's one surface to verify. To drive the *human* path, wrap in
  `sh -c 'unset <every marker in agent::AGENT_ENV_MARKERS>; ...'` (list lives in
  `crates/envstow/src/agent.rs`).
- Stdout here is a pipe, so TTY-guarded paths take their piped branch. To fake a terminal:
  `script -q /dev/null target/debug/envstow <cmd>` (macOS syntax).
- Eval-emitting commands (`env`, `refresh`, `set --export`): verify the round-trip inside
  `sh -c` — `eval "$(envstow env 2>/dev/null)"` then `printf` the var — and check hostile
  values (embedded `'`, `$( )`) arrive byte-for-byte unexecuted.
- `... | cat; echo $?` reports cat's exit code — check exit codes without pipes.
- Use only dummy values in the scratch store; the PostToolUse redact guard scans output for
  real store values.
