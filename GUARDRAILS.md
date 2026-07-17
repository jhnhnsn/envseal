# envstow — AI agent guardrails setup

> **Canonical source:** https://github.com/jhnhnsn/envstow/blob/main/GUARDRAILS.md
> A human *or an AI agent* can fetch this page and follow it to harden a repo. If you're an
> agent asked to "set up envstow guardrails," read this file and apply the steps for the
> agent/editor in use.

envstow keeps secrets encrypted and lets programs use them **by name**. That already keeps a
value out of your transcript when you run `envstow run -- <cmd>`. These guardrails add
*defense in depth* so an agent is steered away from — and mechanically blocked from —
accidentally exposing a secret value.

There are **three layers**. Set up as many as your agent supports. Claude Code supports all
three; other agents support layers 1 and (via their own hook system) 3.

---

## Layer 1 — Instructions (works with every agent)

Tell the agent the rules: reference secrets by name, never print/echo/log a value, use
`envstow run -- <cmd>`. Where these live depends on the agent:

| Agent | Put the rules in |
|---|---|
| **Claude Code** | `CLAUDE.md`, and/or the envstow skill (see below) |
| **Cursor** | `.cursorrules` (or `.cursor/rules/*.mdc`) |
| **Aider** | `CONVENTIONS.md` (referenced in `.aider.conf.yml`) |
| **Windsurf / others** | their project-rules file |
| **Cross-tool** | `AGENTS.md` at the repo root (read by a growing set of agents) |

Minimum text to include (adapt wording to the file):

```markdown
## Secret handling (envstow)
- Secrets are managed by envstow and referenced BY NAME (e.g. $MY_SUPER_SECRET_KEY).
- Never print, echo, log, or paste a secret VALUE. Never run `env`, `printenv`,
  `echo $SOME_SECRET`, or `cat` a file that holds one.
- To use a secret in a command, run it through envstow so the value stays in the child
  process: `envstow run -- sh -c 'deploy --token "$MY_SUPER_SECRET_KEY"'`.
- `envstow get <NAME>` masks its output under an agent — that is intentional; do not try to
  defeat it. If a human needs a value, they run `envstow get <NAME> --show` themselves.
- If you think you need a plaintext value, STOP and ask the human.
```

For Claude Code, **`envstow init` already offers to install the skill** into this repo's
`.claude/skills/envstow/` — commit it and it travels to teammates on clone. To (re)install it
manually, or install it globally so it loads in every repo you work in:

```bash
# Per-project (what `init` does; travels to teammates):
mkdir -p .claude/skills/envstow
curl -fsSL https://raw.githubusercontent.com/jhnhnsn/envstow/main/agent/envstow-skill.md \
  -o .claude/skills/envstow/SKILL.md

# …or global (available in all YOUR repos; doesn't travel to teammates):
mkdir -p ~/.claude/skills/envstow
curl -fsSL https://raw.githubusercontent.com/jhnhnsn/envstow/main/agent/envstow-skill.md \
  -o ~/.claude/skills/envstow/SKILL.md
```

Restart Claude Code afterward — skills load at startup.

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

**The guard is built into the binary: `envstow scan-leak`.** It reads the tool-result payload on
stdin and exits non-zero (blocking) if the output contains a live secret value. Because it ships
in the binary, `envstow upgrade` keeps the detection current — there is no script to copy or keep
in sync. envstow never writes to your agent's config; you add one hook line yourself (below).

### Claude Code

Add the hook to `.claude/settings.json` (merge with any existing config) — no file to copy:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "envstow scan-leak" }
        ]
      }
    ]
  }
}
```

That's the whole setup. `scan-leak` reads the payload on stdin, exits `0` to allow or `2` to block,
and on a block prints a one-line reason to stderr — naming the offending variable, never its value.

### Cursor

Wire `envstow scan-leak` into an `afterShellExecution` hook in your Cursor hooks config (Cursor
also offers `beforeShellExecution` to block *before* a risky command runs). The contract is
identical: read the tool result on stdin, exit non-zero to block.

### Windsurf / Aider / other agents

Any agent with a post-command / post-tool hook can run `envstow scan-leak` — the mechanism is the
same everywhere (*"intercept tool output, block on policy"*). If an agent has no hook system, you
rely on Layers 1–2 plus the model's own judgment.

### Legacy: `scripts/redact-guard.sh`

Earlier versions shipped this as a hand-copied bash script. It still works and its behavior is
identical, but it's **deprecated** — it doesn't auto-update, needs `python3`, and must be copied
into each repo. Prefer `envstow scan-leak`. If you have the old hook wired, just point it at
`envstow scan-leak` and delete the script.

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
