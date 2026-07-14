# envseal

An **age-encrypted key-value store committed to your repo**, decrypted with each collaborator's
**own age key** and surfaced **by name** — so neither a human nor an AI coding agent (Claude
Code, Cursor, …) has to paste a secret's plaintext onto a command line.

- **Self-contained:** one Rust binary. All crypto is the [`age`](https://crates.io/crates/age)
  crate (X25519 + ChaCha20-Poly1305) compiled in — **no `sops`, no `age` CLI, nothing else to
  install.**
- **Multi-user:** the store is encrypted to every collaborator's age public key. Each decrypts
  with their own private key. Add/remove people by editing a `recipients` file.
- **AI-safe by construction:** agents reference secrets by **name** (`$AI_API_KEY`). A value is
  never printed unless it's safe to (not captured by an agent) or a human explicitly asks.

---

## How it works

```
recipients                        # age PUBLIC keys, committed. Who can decrypt.
secrets/secrets.enc               # age-encrypted KEY=value store, committed.
~/.config/envseal/identity.txt    # YOUR age private key. Never committed. (0600)
                                  #   Windows: %APPDATA%\envseal\identity.txt
```

To *use* a secret you unlock it into a **child process**. The child gets the value in its
environment and does its job; the value never appears in your shell history, an agent's tool
call, or its transcript. You only ever type the variable **name**.

---

## Install

```bash
# macOS / Linux — prebuilt binary, no toolchain needed:
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envseal/releases/latest/download/envseal-installer.sh | sh
```
```powershell
# Windows (PowerShell):
powershell -c "irm https://github.com/jhnhnsn/envseal/releases/latest/download/envseal-installer.ps1 | iex"
```

Installs to `~/.local/bin` — **open a new terminal** (or `source ~/.local/bin/env`) before
running `envseal`, then `envseal --version` to confirm. The installer verifies the binary's
SHA-256 and enforces TLS. To inspect the script first, or verify checksums by hand, see the
install options in [ONBOARDING.md](./ONBOARDING.md#1-install-envseal-once-per-machine). Or build
from source (needs [Rust](https://rustup.rs)): `cargo install --path bin`.

**Joining a team that already uses envseal?** See **[ONBOARDING.md](./ONBOARDING.md)** — install,
share your key, get added. An **AI-agent skill is bundled** in `.claude/skills/envseal/`, so
Claude Code (and compatible agents) automatically know how to use secrets by name in this repo.

---

## Usage scenarios

Secrets are always referenced by **name**; the plaintext only ever lives inside the child
process envseal spawns.

### 1. First-time setup

```bash
envseal init
git add recipients secrets/secrets.enc && git commit -m "Add envseal store"
```

`init` creates your private key (in `~/.config/envseal/`, never committed), the `recipients`
list, and an empty store. Idempotent.

### 2. Add and list secrets

Copy a secret from your password manager, then paste it into `set` — the value comes from
**stdin**, so it never lands on the command line or in your shell history:

```bash
pbpaste | envseal set MY_SUPER_SECRET_KEY                   # macOS: paste from clipboard
# Linux (wayland): wl-paste | envseal set MY_SUPER_SECRET_KEY
# Linux (X11):     xclip -o  | envseal set MY_SUPER_SECRET_KEY
#   → ✔  set MY_SUPER_SECRET_KEY (sk-pr••••••••)   ← masked confirmation of what you stored

envseal set MY_SUPER_SECRET_KEY                             # …or run bare, then paste + Enter
printf 'sk-proj-abc123' | envseal set MY_SUPER_SECRET_KEY   # …or pipe a literal
envseal edit                                           # …or edit them all in $EDITOR
envseal list                                           # names only, never values
```

`set` confirms with a **masked preview** — the first 5 characters then dots (or all dots for
short values) — so you can sanity-check the paste without the full value on screen. Under an AI
agent the preview is fully masked.

The bare interactive prompt reads a **single line** (API keys, tokens, passwords). Multi-line
values (PEM keys, certs, JSON) work too — just **pipe** them rather than typing at the prompt;
see [Multi-line secrets](#multi-line-secrets) below.

### 3. Run something that needs secrets

`envseal unlock -- <cmd>` runs one command with every secret set as an env var:

```bash
envseal unlock -- npm run build
envseal unlock -- flyctl deploy
envseal unlock -- sh -c 'psql "$DATABASE_URL" -f migrate.sql'
```

You typed `$DATABASE_URL` — the shell expands it *inside the child*, so the value reaches `psql`
but never your history or a log.

### 4. Working with an AI agent

Start the agent from an unlocked subshell; every command it runs inherits the secrets:

```bash
envseal unlock     # subshell with all secrets set; `exit` locks
claude             # launched inside it — references $MY_SUPER_SECRET_KEY by name
```

If the agent tries to read a value directly, it can't — `envseal get` masks under an agent:

```bash
envseal get FLY_API_TOKEN    # → ••••••••  (see "Why this is AI-safe")
```

### 5. Read a value yourself

Outside an agent, `envseal get` prints the value when its output is captured; `--show` forces it:

```bash
export GITHUB_TOKEN="$(envseal get GITHUB_TOKEN)"
envseal get DATABASE_URL --show
```

### 6. Onboard a teammate

```bash
# Alice: generate her key and share the public half (safe to paste anywhere).
envseal init && envseal pubkey        # → age1abc…

# You: add her, re-encrypt, commit.
envseal add-recipient age1abc… alice
git add recipients secrets/secrets.enc && git commit -m "Add Alice"
```

Only the **public** key (`age1…`) is shared — it lets you encrypt *to* someone, never decrypt.
The private key (`~/.config/envseal/identity.txt`) is never shared or committed.

### 7. Offboard a teammate

```bash
envseal remove-recipient alice
```

This re-encrypts without Alice, but her key still decrypts old commits. **Rotation is the real
revocation:** regenerate each secret she saw and `envseal set` the new value.

### 8. CI / automation

Point `$ENVSEAL_IDENTITY` at a dedicated CI key (added as a recipient, stored as a CI secret):

```bash
ENVSEAL_IDENTITY=/path/to/ci-key envseal unlock -- npm run deploy
```

### On Windows

Most commands are identical — `envseal init`, `list`, `pubkey`, `add-recipient`, and
`envseal unlock -- <program>` all work as-is. Only a few things differ:

```powershell
# Your identity lives at %APPDATA%\envseal\identity.txt; `edit` opens Notepad.
'sk-proj-abc123' | envseal set MY_SUPER_SECRET_KEY     # PowerShell pipes a value to stdin
envseal unlock -- npm run build                   # runs the program directly — same as POSIX

# The only real difference: no `sh -c`. To reference a value by name in a shell,
# use PowerShell (%VAR% for cmd.exe):
envseal unlock -- powershell -c 'psql $env:DATABASE_URL -f migrate.sql'
envseal unlock -- cmd /c "psql %DATABASE_URL% -f migrate.sql"

# Start an unlocked subshell (cmd.exe by default via %COMSPEC%):
envseal unlock
```

### Multi-line secrets

`set` handles multi-line values (PEM keys, TLS certs, service-account JSON) — **pipe them in**,
since a multi-line value can't be typed at the single-line interactive prompt:

```bash
envseal set TLS_KEY   < privkey.pem
envseal set GCP_CREDS < service-account.json
```

Under the hood, multi-line values are base64-encoded inside the store (so the on-disk dotenv
stays one line per key); `unlock`/`get` decode them transparently, so the env var your program
sees is the exact original. Single-line secrets are stored as-is. Pasting a multi-line value
into the interactive prompt won't work — pipe it or use `envseal edit`.

---

## Command reference

| Command | Purpose |
|---|---|
| `envseal init` | Generate identity, create `recipients` + empty store. Idempotent. |
| `envseal set <NAME>` | Store a value read from **stdin** (keeps it off the command line). |
| `envseal edit` | Decrypt all secrets into `$EDITOR`, re-encrypt on save (temp file shredded). |
| `envseal get <NAME> [--show]` | Resolve one secret by name. **Masked under an agent** unless `--show`. |
| `envseal list` | List secret **names** (never values). |
| `envseal pubkey` | Print your age **public** key, to share so a member can add you. |
| `envseal unlock [-- <cmd>]` | Run a command (or subshell) with every secret set as an env var. |
| `envseal add-recipient <age1…> [label]` | Add a collaborator; re-encrypt. |
| `envseal remove-recipient <key\|label>` | Remove a collaborator; re-encrypt (then **rotate**). |
| `envseal reencrypt` | Re-encrypt the store to the current `recipients` (after hand-editing it). |

**Environment:** `ENVSEAL_IDENTITY` overrides the identity path (default
`~/.config/envseal/identity.txt`). `ENVSEAL_AGENT=1` forces agent-masking for `get` in tools
that aren't auto-detected.

---

## Why this is AI-safe

The environment-variable channel and the AI's context channel are **separate**. You tell the
agent "the token is in `$FLY_API_TOKEN`", and it runs `envseal unlock -- sh -c 'deploy --token
"$FLY_API_TOKEN"'`. The shell expands `$FLY_API_TOKEN` *inside the child envseal spawns* — the
value never appears in the agent's tool call or its output.

`envseal get` reinforces this: **under an agent it masks its output by default** (prints
`••••••••`), because an agent captures stdout and we can't distinguish "used inside `$(…)`"
from "run bare into the transcript". A human who needs the value runs `envseal get NAME --show`.

Three defense layers back this up:
- **`CLAUDE.md`** — instructs the agent to reference by name; never echo/print/log a value.
- **`.claude/settings.json`** — denies `env`, `printenv`, `echo $*`, `set`, …
- **`scripts/redact-guard.sh`** — `PostToolUse` hook; blocks any command output containing a
  live secret value (raw or base64) as accident insurance.

> Defense-in-depth, **not** a vault. It makes accidental exposure very unlikely. A human or
> agent who deliberately runs `--show` will see the value — that's by design (you own the
> secret). What it prevents is *pasting* and *accidental* leakage.

---

## Threat model

**Protects:** secrets readable in the repo/host (encrypted at rest); onboarding/offboarding
without a shared master password; **humans/agents pasting plaintext onto command lines**;
casual/accidental AI exposure of values.

**Does NOT protect:** a compromised dependency reading `process.env` at runtime; a determined
process exfiltrating a live var; plaintext already in git history; retroactive access removal;
a value someone deliberately reveals with `--show` or re-encodes to evade the redact-guard.
For those: rotate, and treat this as strong hygiene, not a vault.

---

## Developing on envseal

```bash
cd bin && cargo test         # unit + integration: crypto round-trip, masking, full CLI lifecycle
scripts/test-redact-guard.sh # proves the hook blocks a leak and allows name references
```

CI (`.github/workflows/ci.yml`) builds + tests + `fmt` + `clippy` on macOS/Linux/Windows, and
runs `shellcheck` + the redact-guard test on Linux.

See `DESIGN.md` for the full design rationale.
