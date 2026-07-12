# envseal

A single **encrypted secrets file, checked into the repo**, unlocked per-user via
**age keypairs**, decrypted through **SOPS**, loaded into your shell via **direnv with an
explicit confirmation gate**, and hardened so an AI coding tool (Claude Code, Cursor, …)
can *use* secrets by name **without their plaintext ever entering its context**.

- **Encryption:** SOPS (values-only, diffable) + age (per-user keypairs)
- **Unlock:** direnv `.envrc` with a confirmation gate — no silent auto-decrypt
- **AI safety:** name-reference-only + command denylist + output-scanning hook
- **Cross-platform:** macOS, Linux, Windows — Node / shell / Make / CI / mobile / cloud deploys

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
brew install age sops direnv
eval "$(direnv hook zsh)"        # add to ~/.zshrc

# 2. Generate your age key (once per machine)
mkdir -p ~/.config/sops/age
age-keygen -o ~/.config/sops/age/keys.txt
#   -> copy the printed "Public key: age1..." into .sops.yaml

# 3. Put your public key in .sops.yaml (replace the REPLACE_WITH_... line)

# 4. Create the encrypted secrets file
sops secrets/secrets.enc.env    # editor opens; add KEY=value lines; saves encrypted

# 5. Unlock for a session, then start your AI tool IN THIS SHELL
direnv allow
export SECRETS_UNLOCK=1 && direnv reload   # 🔓 secrets loaded
claude                                      # inherits the env; uses secrets by name
```

Leaving the directory (`cd ..`) auto-unloads the vars.

---

## 1. Tooling install

Native binaries on all platforms — no WSL needed.

| Tool | macOS | Linux | Windows |
|------|-------|-------|---------|
| age | `brew install age` | pkg mgr / release | `scoop install age` / `winget install FiloSottile.age` |
| sops | `brew install sops` | release binary | `scoop install sops` / `winget install getsops.sops` |
| direnv | `brew install direnv` | pkg mgr | `scoop install direnv` |

direnv shell hook (once): bash `eval "$(direnv hook bash)"` · zsh `eval "$(direnv hook zsh)"`
· fish `direnv hook fish | source` · pwsh `Invoke-Expression "$(direnv hook pwsh)"`.

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

## 4. The direnv unlock gate

See `.envrc`. Two safety layers: direnv won't run `.envrc` until `direnv allow` (so a
malicious PR edit can't auto-execute), and the gate requires `SECRETS_UNLOCK=1` so merely
`cd`-ing in never silently decrypts. Windows alternative: `. .\scripts\unlock.ps1`.

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
