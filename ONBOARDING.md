# Onboarding to this repo's secrets

This project uses [envseal](./README.md) to share encrypted secrets through git. Getting set
up is three steps. Your AI agent (Claude Code, etc.) already knows how to use envseal — the
`.claude/skills/envseal/` skill ships with this repo, so it loads automatically.

## 1. Install envseal (once per machine)

A prebuilt binary — no toolchain required. Copy one line:

```bash
# macOS / Linux
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envseal/releases/latest/download/envseal-installer.sh | sh
```
```powershell
# Windows (PowerShell)
powershell -c "irm https://github.com/jhnhnsn/envseal/releases/latest/download/envseal-installer.ps1 | iex"
```

<details>
<summary>Security-conscious install options (inspect the script, verify checksums, custom path)</summary>

The installer always verifies the downloaded binary's SHA-256 and enforces HTTPS/TLS 1.2, so the
*binary* is checksum-protected no matter which option you use. These options additionally let you
vet the *installer script* first, or choose where it installs. **All give the same result.**

**Inspect first** — download, read it, then run it:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envseal/releases/latest/download/envseal-installer.sh -o envseal-installer.sh
less envseal-installer.sh          # inspect — it's plain, readable shell
sh envseal-installer.sh
```

**Via authenticated `gh`, or verify the checksum by hand:**

```bash
gh release download v0.1.1 --repo jhnhnsn/envseal --pattern '*installer.sh' -O envseal-installer.sh
sh envseal-installer.sh
#   …or download your platform's archive + its .sha256 and verify by hand:
gh release download v0.1.1 --repo jhnhnsn/envseal --pattern '*apple-darwin*'
shasum -a 256 -c envseal-*.tar.xz.sha256      # must print "OK"
```

**Custom install location** — set `ENVSEAL_INSTALL_DIR` (e.g. a dir already on your PATH):

```bash
ENVSEAL_INSTALL_DIR="$HOME/bin" curl --proto '=https' --tlsv1.2 -LsSf https://github.com/jhnhnsn/envseal/releases/latest/download/envseal-installer.sh | sh
```
</details>

Installs to `~/.local/bin` and adds it to your PATH — **for new shells.** Your *current*
terminal won't see it yet, so **open a new terminal** (or run `source ~/.local/bin/env`) before
continuing. Then confirm:

```bash
envseal --version               # e.g. envseal 0.1.1
```

If `envseal: command not found` in the same terminal you installed from, that's expected —
open a fresh terminal. The install didn't fail.

<details>
<summary>Clone and build from source (needs <a href="https://rustup.rs">Rust</a>)</summary>

For contributors, or to install to a directory you choose:

```bash
git clone https://github.com/jhnhnsn/envseal.git
cd envseal

# Option A — install onto your PATH via cargo (→ ~/.cargo/bin/envseal):
cargo install --path bin

# Option B — build, then copy the binary wherever you want:
cargo build --release
cp target/release/envseal ~/bin/          # …or /usr/local/bin, or any dir on your PATH
```

`cargo install` puts it in `~/.cargo/bin` (already on PATH if you have Rust). Option B lets you
pick the exact location.
</details>

## 2. Create your key and share it

> **envseal works per project directory.** Every command operates on the secret store of the
> repo you're currently *inside* — it looks for a `recipients` file in the current directory and
> walks up to find the project root. So always `cd` into the project first. One machine, one
> personal key (in `~/.config/envseal/`), but each repo has its own `recipients` + encrypted
> store. Running a command outside any envseal repo gives you "no `recipients` file found."

From inside the project:

```bash
cd ~/path/to/the-project        # ← be in the repo; envseal acts on THIS repo's store
envseal init                    # generates your private key (once) + your recipients entry here
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
