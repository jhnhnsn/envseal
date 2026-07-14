# Onboarding to this repo's secrets

This project uses [envseal](./README.md) to share encrypted secrets through git. Getting set
up is three steps. Your AI agent (Claude Code, etc.) already knows how to use envseal — the
`.claude/skills/envseal/` skill ships with this repo, so it loads automatically.

## 1. Install envseal (once per machine)

Needs [Rust](https://rustup.rs). No other tools.

```bash
cargo install --path bin        # → ~/.cargo/bin/envseal (on your PATH)
```

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
