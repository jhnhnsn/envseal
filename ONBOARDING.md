# Onboarding to this repo's secrets

This project uses [envseal](./README.md) to share encrypted secrets through git. Getting set
up is three steps. Your AI agent (Claude Code, etc.) already knows how to use envseal — the
`.claude/skills/envseal/` skill ships with this repo, so it loads automatically.

## 1. Install envseal (once per machine)

A prebuilt binary — no toolchain required. Pick the level of caution you want; **all four give
the same result.** The installer verifies the downloaded binary's SHA-256 before installing, and
enforces HTTPS/TLS 1.2, so the *binary* is checksum-protected either way. The choice below is
about whether you also inspect the *installer script* before running it.

**Quickest** — pipe straight to a shell (trusts the script on first use):

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envseal/releases/latest/download/envseal-installer.sh | sh
```
```powershell
# Windows (PowerShell)
powershell -c "irm https://github.com/jhnhnsn/envseal/releases/latest/download/envseal-installer.ps1 | iex"
```

**Safer** — download, read it, then run it:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envseal/releases/latest/download/envseal-installer.sh -o envseal-installer.sh
less envseal-installer.sh          # inspect — it's plain, readable shell
sh envseal-installer.sh
```

**Most cautious** — use the GitHub CLI (verifies via your authenticated `gh`), or grab the
binary and its checksum yourself:

```bash
gh release download v0.1.0 --repo jhnhnsn/envseal --pattern '*installer.sh' -O envseal-installer.sh
sh envseal-installer.sh
#   …or download your platform's archive + its .sha256 and verify by hand:
gh release download v0.1.0 --repo jhnhnsn/envseal --pattern '*apple-darwin*'
shasum -a 256 -c envseal-*.tar.xz.sha256      # must print "OK"
```

Installs to `~/.local/bin` and adds it to your PATH. Restart your shell (or `source ~/.profile`)
if `envseal` isn't found.

<details>
<summary>Build from source instead (needs <a href="https://rustup.rs">Rust</a>)</summary>

```bash
cargo install --path bin        # → ~/.cargo/bin/envseal
```
</details>

## 2. Create your key and share it

```bash
envseal init                    # generates your private key + creates your recipients entry
envseal pubkey                  # prints your PUBLIC key (age1...) — safe to share
```

Send that `age1...` public key to a current member (Slack, email, or — best — open a PR that
adds it to the `recipients` file). Your **private** key stays in `~/.config/envseal/` and is
never shared or committed.

> ⚠️ Running `envseal init` adds your name to `recipients`, but you **cannot decrypt the store
> yet** — an existing member has to add your key and re-encrypt (step 3).

## 3. A current member adds you

An existing member runs:

```bash
envseal add-recipient age1yourkey... your-name
git add recipients secrets/secrets.enc && git commit -m "Add your-name" && git push
```

Then you `git pull`, and you're in:

```bash
envseal list                    # you can now see the stored secret names
```

## Daily use

You never need the plaintext. Run commands that need secrets through envseal — it sets them as
env vars for that one command:

```bash
envseal unlock -- npm run build
envseal unlock -- sh -c 'deploy --token "$FLY_API_TOKEN"'
```

Or start your whole session unlocked:

```bash
envseal unlock                  # subshell with all secrets set; `exit` locks
```

See the [README](./README.md) for the full command list. Your agent has the same knowledge via
the bundled skill — just tell it what you want to deploy or call, and it will reference secrets
by name.
