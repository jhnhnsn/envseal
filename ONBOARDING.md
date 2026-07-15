# Onboarding to this repo's secrets

This project uses [envstow](./README.md) to share encrypted secrets through git. Getting set
up is three steps. To teach your AI agent (Claude Code, etc.) how to use envstow, install the
skill — see [Hardening your repo for AI agents](#hardening-your-repo-for-ai-agents) below.

## 1. Install envstow (once per machine)

A prebuilt binary — no toolchain required. Copy one line.

**macOS / Linux:**

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envstow/releases/latest/download/envstow-installer.sh | sh
```

**Windows** (PowerShell):

```powershell
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
gh release download --repo jhnhnsn/envstow --pattern '*installer.sh' -O envstow-installer.sh
sh envstow-installer.sh
#   …or download your platform's archive + its .sha256 and verify by hand:
gh release download --repo jhnhnsn/envstow --pattern '*apple-darwin*'
shasum -a 256 -c envstow-*.tar.xz.sha256      # must print "OK"
```

(No tag means the latest release. Only the newest release keeps its built artifacts — older
versions stay rebuildable from their git tags, so pin by building from source if you need one.)

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

Your key is **per machine**, not per project — so make it **outside the project**:

```bash
cd ~                # anywhere but the project
envstow init        # generates your private key (once per machine)
envstow pubkey      # prints your PUBLIC key (age1...) — safe to share
```

Send that `age1...` key to a current member (Slack or email is fine — it's public).
Your **private** key stays in `~/.config/envstow/` and is never shared or committed.

> **Why outside the project?** `recipients` is an **input to encryption, not an access list.**
> Running `init` *inside* the project appends your key to that file — which grants you nothing,
> since the store is still encrypted only to the existing members. You'd then need someone to run
> `envstow reencrypt` instead of the simpler `add-recipient`. Init elsewhere and skip the detour.
> (If you already did it, no harm: envstow will tell you exactly who needs to run what.)

## 3. A current member adds you

An existing member runs:

```bash
envstow add-recipient age1yourkey... your-name
git add .envstow && git commit -m "Add your-name" && git push
```

Then you `git pull`, and you're in:

```bash
cd ~/path/to/the-project        # every command acts on the store of the folder you're in
envstow list                    # you can now see the stored secret names
```

`add-recipient` adds your key **and** re-encrypts the store — that second half is what actually
grants access.

## Daily use

**Every command acts on the folder you're in.** envstow looks for `.envstow/` in the current
directory and walks up to find it, so `cd` into the project first. Outside one, you'll get
"no `.envstow/` found". One personal key per machine; each project has its own store.

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

Run `envstow init` **inside the project** to add the **agent skill** (Layer 1 — instructions) at
`.claude/skills/envstow/`. It prompts `[Y/n]`; commit the result and every teammate who clones
gets it, so their agent learns to use secrets by name. This is safe to run once you're already a
recipient — it's idempotent and won't disturb the store.

For the full defense — a **command denylist** and an **output-guard hook** that mechanically
blocks a leaked value — follow **[GUARDRAILS.md](./GUARDRAILS.md)**, which covers Claude Code,
Cursor, and others.

Tip: you can point your agent at that file's URL —
`https://github.com/jhnhnsn/envstow/blob/main/GUARDRAILS.md` — and ask it to apply the
guardrails for whatever editor you use.
