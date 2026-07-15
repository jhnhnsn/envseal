# Onboarding to this repo's secrets

This project uses [envstow](./README.md) to share encrypted secrets through git. Getting set
up is three steps. To teach your AI agent (Claude Code, etc.) how to use envstow, install the
skill — see [Hardening your repo for AI agents](#hardening-your-repo-for-ai-agents) below.

## 1. Install envstow (once per machine)

A prebuilt binary — no toolchain required. Copy one line:

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envstow/releases/latest/download/envstow-installer.sh | sh
```
```powershell
# Windows (PowerShell)
powershell -c "irm https://github.com/jhnhnsn/envstow/releases/latest/download/envstow-installer.ps1 | iex"
```

<details>
<summary>Security-conscious install options (inspect the script, verify checksums, custom path)</summary>

The installer always verifies the downloaded binary's SHA-256 and enforces HTTPS/TLS 1.2, so the
*binary* is checksum-protected no matter which option you use. These options additionally let you
vet the *installer script* first, or choose where it installs. **All give the same result.**

**Inspect first** — download, read it, then run it:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envstow/releases/latest/download/envstow-installer.sh -o envstow-installer.sh
less envstow-installer.sh          # inspect — it's plain, readable shell
sh envstow-installer.sh
```

**Via authenticated `gh`, or verify the checksum by hand:**

```bash
gh release download v0.1.1 --repo jhnhnsn/envstow --pattern '*installer.sh' -O envstow-installer.sh
sh envstow-installer.sh
#   …or download your platform's archive + its .sha256 and verify by hand:
gh release download v0.1.1 --repo jhnhnsn/envstow --pattern '*apple-darwin*'
shasum -a 256 -c envstow-*.tar.xz.sha256      # must print "OK"
```

**Custom install location** — set `ENVSTOW_INSTALL_DIR` (e.g. a dir already on your PATH):

```bash
ENVSTOW_INSTALL_DIR="$HOME/bin" curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envstow/releases/latest/download/envstow-installer.sh | sh
```
</details>

Installs to `~/.local/bin` and adds it to your PATH — **for new shells.** Your *current*
terminal won't see it yet, so **open a new terminal** (or run `source ~/.local/bin/env`) before
continuing. Then confirm:

```bash
envstow --version               # e.g. envstow 0.1.1
```

If `envstow: command not found` in the same terminal you installed from, that's expected —
open a fresh terminal. The install didn't fail.

<details>
<summary>Clone and build from source (needs <a href="https://rustup.rs">Rust</a>)</summary>

For contributors, or to install to a directory you choose:

```bash
git clone https://github.com/jhnhnsn/envstow.git
cd envstow

# Option A — install onto your PATH via cargo (→ ~/.cargo/bin/envstow):
cargo install --path bin

# Option B — build, then copy the binary wherever you want:
cargo build --release
cp target/release/envstow ~/bin/          # …or /usr/local/bin, or any dir on your PATH
```

`cargo install` puts it in `~/.cargo/bin` (already on PATH if you have Rust). Option B lets you
pick the exact location.
</details>

## 2. Create your key and share it

> **envstow works per project directory.** Every command operates on the secret store of the
> repo you're currently *inside* — it looks for a `recipients` file in the current directory and
> walks up to find the project root. So always `cd` into the project first. One machine, one
> personal key (in `~/.config/envstow/`), but each repo has its own `recipients` + encrypted
> store. Running a command outside any envstow repo gives you "no `recipients` file found."

From inside the project:

```bash
cd ~/path/to/the-project        # ← be in the repo; envstow acts on THIS repo's store
envstow init                    # generates your private key (once) + your recipients entry here
                                #   also offers [Y/n] to add the Claude Code agent skill to the repo
envstow pubkey                  # prints your PUBLIC key (age1...) — safe to share
```

`init` offers to drop the agent skill into `.claude/skills/envstow/` — say yes, then commit it,
and every teammate who clones gets it (their agent learns to use secrets safely). `--no-skill`
skips. For the full guardrails (denylist + output-guard hook), see
[GUARDRAILS.md](./GUARDRAILS.md).

Send that `age1...` public key to a current member (Slack, email, or — best — open a PR that
adds it to the `recipients` file). Your **private** key stays in `~/.config/envstow/` and is
never shared or committed.

> ⚠️ Running `envstow init` adds your name to `recipients`, but you **cannot decrypt the store
> yet** — an existing member has to add your key and re-encrypt (step 3).

## 3. A current member adds you

An existing member runs:

```bash
envstow add-recipient age1yourkey... your-name
git add recipients secrets/secrets.enc && git commit -m "Add your-name" && git push
```

Then you `git pull`, and you're in:

```bash
envstow list                    # you can now see the stored secret names
```

## Daily use

You never need the plaintext. Run commands that need secrets through envstow — it sets them as
env vars for that one command:

```bash
envstow unlock -- npm run build
envstow unlock -- sh -c 'deploy --token "$FLY_API_TOKEN"'
```

Or start your whole session unlocked:

```bash
envstow unlock                  # subshell with all secrets set; `exit` locks
```

See the [README](./README.md) for the full command list.

## Hardening your repo for AI agents

`envstow init` already offers to add the **agent skill** (Layer 1 — instructions). For the full
defense — a **command denylist** and an **output-guard hook** that mechanically blocks a leaked
value — follow **[GUARDRAILS.md](./GUARDRAILS.md)**, which covers Claude Code, Cursor, and others.

Tip: you can point your agent at that file's URL —
`https://github.com/jhnhnsn/envstow/blob/main/GUARDRAILS.md` — and ask it to apply the
guardrails for whatever editor you use.
