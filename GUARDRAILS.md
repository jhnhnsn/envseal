# envseal — AI agent guardrails setup

> **Canonical source:** https://github.com/jhnhnsn/envseal/blob/main/GUARDRAILS.md
> A human *or an AI agent* can fetch this page and follow it to harden a repo. If you're an
> agent asked to "set up envseal guardrails," read this file and apply the steps for the
> agent/editor in use.

envseal keeps secrets encrypted and lets programs use them **by name**. That already keeps a
value out of your transcript when you run `envseal unlock -- <cmd>`. These guardrails add
*defense in depth* so an agent is steered away from — and mechanically blocked from —
accidentally exposing a secret value.

There are **three layers**. Set up as many as your agent supports. Claude Code supports all
three; other agents support layers 1 and (via their own hook system) 3.

---

## Layer 1 — Instructions (works with every agent)

Tell the agent the rules: reference secrets by name, never print/echo/log a value, use
`envseal unlock -- <cmd>`. Where these live depends on the agent:

| Agent | Put the rules in |
|---|---|
| **Claude Code** | `CLAUDE.md`, and/or a skill at `.claude/skills/envseal/SKILL.md` |
| **Cursor** | `.cursorrules` (or `.cursor/rules/*.mdc`) |
| **Aider** | `CONVENTIONS.md` (referenced in `.aider.conf.yml`) |
| **Windsurf / others** | their project-rules file |
| **Cross-tool** | `AGENTS.md` at the repo root (read by a growing set of agents) |

Minimum text to include (adapt wording to the file):

```markdown
## Secret handling (envseal)
- Secrets are managed by envseal and referenced BY NAME (e.g. $MY_SUPER_SECRET_KEY).
- Never print, echo, log, or paste a secret VALUE. Never run `env`, `printenv`,
  `echo $SOME_SECRET`, or `cat` a file that holds one.
- To use a secret in a command, run it through envseal so the value stays in the child
  process: `envseal unlock -- sh -c 'deploy --token "$MY_SUPER_SECRET_KEY"'`.
- `envseal get <NAME>` masks its output under an agent — that is intentional; do not try to
  defeat it. If a human needs a value, they run `envseal get <NAME> --show` themselves.
- If you think you need a plaintext value, STOP and ask the human.
```

For Claude Code specifically, the ready-made skill is in this repo at
[`.claude/skills/envseal/SKILL.md`](./.claude/skills/envseal/SKILL.md) — copy that directory
into your repo's `.claude/skills/`.

---

## Layer 2 — Command denylist (Claude Code; adapt for others)

Block the commands whose only purpose is to reveal a value. In Claude Code, add to your repo's
`.claude/settings.json`:

```json
{
  "permissions": {
    "deny": [
      "Bash(env)",
      "Bash(env:*)",
      "Bash(printenv)",
      "Bash(printenv:*)",
      "Bash(echo $*)",
      "Bash(printf $*)",
      "Bash(set)",
      "Bash(export -p)",
      "Bash(cat *identity.txt*)",
      "Bash(cat *.dec.env*)"
    ]
  }
}
```

Cursor/Windsurf: use their `beforeShellExecution` hook to reject the same command patterns.
This layer stops the *obvious* leak commands; Layer 3 catches everything else.

---

## Layer 3 — Output guard hook (the mechanical backstop)

This is the layer that does not depend on the agent's judgment. After every command, a hook
scans the command's output for any **live secret value** and blocks the result from reaching the
agent if one is found. It compares against the actual values in the environment (and their
base64 form), so it catches real leaks — not just patterns.

The guard script is in this repo at
[`scripts/redact-guard.sh`](./scripts/redact-guard.sh). It reads the tool-result payload on
stdin and exits non-zero (blocking) if the output contains a live secret value.

### Claude Code

1. Copy `scripts/redact-guard.sh` into your repo (e.g. `scripts/redact-guard.sh`), keep it
   executable (`chmod +x`).
2. Add the hook to `.claude/settings.json` (merge with any existing config):

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "$CLAUDE_PROJECT_DIR/scripts/redact-guard.sh" }
        ]
      }
    ]
  }
}
```

### Cursor

Wire the same script into an `afterShellExecution` hook in your Cursor hooks config (Cursor
also offers `beforeShellExecution` to block *before* a risky command runs). The script's
contract is identical: read the tool result on stdin, exit non-zero to block.

### Windsurf / Aider / other agents

Any agent with a post-command / post-tool hook can run the same script — the mechanism is the
same everywhere (*"intercept tool output, block on policy"*). Point that agent's hook at
`redact-guard.sh`. If an agent has no hook system, you rely on Layers 1–2 plus the model's own
judgment.

> **Known limits (be honest):** the guard catches a value verbatim or base64-encoded. Other
> encodings (hex, gzip, url-encoding) or a value split across commands can still evade it. It is
> defense-in-depth against *accidental* exposure, not a wall against a determined adversary.
> Name your secrets with `_KEY`/`_TOKEN`/`_SECRET`/`_PASSWORD` suffixes (or starting `API_`) so
> the guard recognizes them.

---

## Quick verification

After setup, in an agent session, ask the agent to *print* a secret value (e.g.
`echo $MY_SECRET`). A correctly guarded repo will either refuse (Layer 1/2) or block the output
(Layer 3) — the value should never appear in the agent's context.
