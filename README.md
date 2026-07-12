# envseal

A single **encrypted secrets file, checked into the repo**, unlocked per-user via
**age keypairs**, decrypted through **SOPS**, and handed to a child process by the
**`envseal` launcher** (a small Rust binary with an explicit confirmation prompt) — so an
AI coding tool (Claude Code, Cursor, …) can *use* secrets by name **without their
plaintext ever entering its context**.

- **Encryption:** SOPS (values-only, diffable) + age (per-user keypairs)
- **Unlock:** `envseal` binary — interactive `[y/N]` prompt, in-memory only, no disk writes.
  A direnv `.envrc` gate is included as an optional convenience (§4b).
- **AI safety:** name-reference-only + command denylist + output-scanning hook
- **Cross-platform:** macOS, Linux, Windows — one binary, no shell-quoting or direnv-on-Windows issues

---

## Why this is AI-safe

The environment-variable channel and the AI's context channel are **separate**. A child
process inherits `MY_API_KEY` and *uses* it; the AI that *launched* it never reads the
value. You tell the AI "the token is in `$FLY_API_TOKEN`, use it" — it runs `fly deploy`,
which reads the token from the env. The value never enters the transcript.

This holds **only** if the AI never runs a command that prints a secret. That's enforced
here by three layers: `CLAUDE.md` rules, a permission denylist, and a `PostToolUse` hook
that blocks any output containing a live secret value (§5).

> Defense-in-depth, **not** a vault. It makes accidental exposure very unlikely; it does
> not make leakage cryptographically impossible.

---

## Quickstart

```bash
# 1. Install tooling (macOS shown; Linux/Windows in §1)
brew install age sops

# 2. Build the envseal launcher (needs Rust: https://rustup.rs)
cargo build --release --manifest-path bin/Cargo.toml
#   -> binary at bin/target/release/envseal  (copy it onto your PATH if you like)

# 3. Generate your age key (once per machine)
mkdir -p ~/.config/sops/age
age-keygen -o ~/.config/sops/age/keys.txt
#   -> copy the printed "Public key: age1..." into .sops.yaml

# 4. Put your public key in .sops.yaml (replace the REPLACE_WITH_... line)

# 5. Create the encrypted secrets file
sops secrets/secrets.enc.env    # editor opens; add KEY=value lines; saves encrypted

# 6a. Start your AI tool in an unlocked subshell:
bin/target/release/envseal unlock          # prompts [y/N], then spawns a subshell
claude                                       # inherits the env; uses secrets by name

# 6b. …or run a single command with secrets, which die when it exits:
bin/target/release/envseal unlock -- npm run build
bin/target/release/envseal unlock -- fly deploy
```

The launcher decrypts in-memory, hands the env to the child process only, and zeroizes
its own copy after spawning. Values are never printed; on unlock it lists variable
**names** only. It refuses to run non-interactively (no TTY → no unlock), so CI must use a
dedicated key (§6).

---

## 1. Tooling install

Native binaries on all platforms — no WSL needed.

| Tool | macOS | Linux | Windows |
|------|-------|-------|---------|
| age | `brew install age` | pkg mgr / release | `scoop install age` / `winget install FiloSottile.age` |
| sops | `brew install sops` | release binary | `scoop install sops` / `winget install getsops.sops` |
| Rust (to build `envseal`) | `rustup` | `rustup` | `rustup` |
| direnv (optional, §4b) | `brew install direnv` | pkg mgr | not native — WSL only |

Build the launcher once: `cargo build --release --manifest-path bin/Cargo.toml`. Copy
`bin/target/release/envseal` (`.exe` on Windows) somewhere on your PATH, or call it by
path. It shells out to `sops`, so `sops` must be installed and on PATH.

## 2. Per-user age keys

```bash
# macOS/Linux
mkdir -p ~/.config/sops/age && age-keygen -o ~/.config/sops/age/keys.txt
```
```powershell
# Windows
mkdir "$env:APPDATA\sops\age" -Force; age-keygen -o "$env:APPDATA\sops\age\keys.txt"
```
Private key stays in `keys.txt` (never committed/shared/pasted to an AI). Public key
(`age1…`) goes to the maintainer for `.sops.yaml`.

## 3. Editing secrets

```bash
sops secrets/secrets.enc.env            # edit (encrypts on save)
sops updatekeys secrets/secrets.enc.env # after changing recipients in .sops.yaml
```
SOPS encrypts **values**, leaving keys visible so `git diff` shows *which* secret changed.

## 4. Unlocking

### 4a. The `envseal` launcher (primary, all platforms)

```bash
envseal unlock                     # prompt [y/N] → spawn an unlocked subshell
envseal unlock -- <cmd> [args...]  # prompt [y/N] → run one command with the secret env
```

- Decrypts via `sops` **in memory** — never writes plaintext to disk or a shell variable.
- Hands the env to the child process only; zeroizes its own copy after spawn.
- Prints variable **names** on unlock, never values.
- **Refuses to run without a TTY** — so it can't be silently driven in a pipe or CI. This
  is a deliberate part of the confirmation gate. For automation, use `sops exec-env` with a
  dedicated key instead (§6).

Searches upward from the CWD for `secrets/secrets.enc.env`, so it works from any
subdirectory of the repo.

### 4b. direnv gate (optional convenience, Unix / WSL)

`.envrc` offers a `cd`-based unlock **if you already use direnv** (macOS/Linux/WSL — direnv
has no native Windows build). Two safety layers: direnv won't run `.envrc` until
`direnv allow`, and the gate requires `SECRETS_UNLOCK=1` so entering the directory never
silently decrypts. On Windows without WSL, use the `envseal` launcher (§4a) — that's the
recommended path everywhere anyway.

## 5. Hardened AI guardrails

- **`CLAUDE.md`** — reference secrets by name only; never echo/print/log.
- **`.claude/settings.json`** — denies `env`, `printenv`, `echo $*`, `set`, `sops -d`, …
- **`scripts/redact-guard.sh`** — `PostToolUse` hook; blocks any command output that
  contains a live secret value (compares against the real env, not just patterns).

## 6. Per-consumer notes

- **Node / shell / Make:** vars are in the env; `npm run build` / `make deploy` inherit them.
  One-off outside the unlocked shell: `sops exec-env secrets/secrets.enc.env '<cmd>'`.
- **CI/CD:** give CI a **dedicated** age key as a masked secret (`SOPS_AGE_KEY`), or add
  AWS KMS as a recipient so runners decrypt via IAM. Never reuse a human's key.
- **Android (Gradle):** `sops exec-env secrets/secrets.enc.env './gradlew assembleRelease'`;
  read via `System.getenv(...)`.
- **iOS (Xcode):** generate a gitignored `.xcconfig` from decrypted values in a build phase,
  or inject via Fastlane. Keep signing certs in Fastlane **match**, not SOPS.
- **AWS:** add a `kms:` recipient; deploy roles decrypt via IAM, no stored key.
- **Fly / Cloudflare:** SOPS is the source of truth — *push* into their native stores with
  `scripts/sync-secrets.sh fly` / `scripts/sync-secrets.sh cloudflare`.

> **Mobile caveat:** decrypted signing/API material gets baked into the built app. SOPS
> protects it in-repo and at rest; nothing once compiled in. High-value signing keys belong
> in Play/App Store Connect API keys or Fastlane match.

## 7. Rotation & offboarding

- **Add:** teammate sends public key → add to `.sops.yaml` → `sops updatekeys …` → commit.
- **Remove:** delete key from `.sops.yaml` → `sops updatekeys …` → **rotate every secret they
  saw** → commit. (Revocation is not retroactive.)
- **Leak:** rotate at the source immediately. Git history keeps any plaintext ever committed —
  rotation is the only real fix.

## 8. Threat model

**Protects:** secrets readable in the repo/host (encrypted at rest); onboarding/offboarding
without a shared master password; casual/accidental AI exposure; silent decrypt on `cd`;
unreviewed `.envrc` edits.

**Does NOT protect:** a compromised dependency reading `process.env` at runtime; a determined
adversarial process exfiltrating a live var; plaintext already in git history; retroactive
access removal. For those: rotate, and treat this as strong hygiene, not a vault.
