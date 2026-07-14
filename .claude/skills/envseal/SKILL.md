---
name: envseal
description: Use envseal to access encrypted secrets in this repo — reference secrets by name, run commands that need them, and onboard to the shared store. Load this whenever a task needs an API key, token, password, database URL, or any secret (e.g. deploy, call an authed API, run migrations, set an env var), when `envseal` commands fail, or when a teammate needs to be added to the secret store.
---

# Using envseal

This repo stores secrets in an **age-encrypted key-value store** (`secrets/secrets.enc`).
`envseal` is a single self-contained binary — no `sops`/`age` CLIs needed. Secrets are used
**by name**; their plaintext must never enter your output, a tool-call argument, or a file.

## The one rule

**Never print, echo, log, or paste a secret's value.** Reference it by variable name (e.g.
`$FLY_API_TOKEN`). If you need a secret in a command, use `envseal unlock -- <cmd>` (below) so
the value only ever lives in the child process — never in your transcript.

## Using a secret in a command (the main pattern)

`envseal unlock -- <cmd>` runs one command with **every** secret set as an env var. Reference
the secret by name; the value is expanded inside the child, not by you:

```bash
envseal unlock -- npm run build
envseal unlock -- flyctl deploy
# When a tool needs the value as an argument, reference it by name inside a shell:
envseal unlock -- sh -c 'psql "$DATABASE_URL" -f migrate.sql'
envseal unlock -- sh -c 'curl -H "Authorization: Bearer $OPENAI_API_KEY" https://api.example.com'
```

You write the literal string `$DATABASE_URL` — six inert characters. Never substitute the
actual value yourself.

## Discovering what's available

```bash
envseal list          # prints the NAMES of stored secrets (never values) — safe
```

Use this to learn which names exist before referencing them. If you're unsure a secret exists,
`list` first.

## Reading a value

Prefer **not** to. If you genuinely must resolve a value (rare), `envseal get <NAME>` — but
under an agent it prints a **mask** (`••••••••`) by default. **That masking is intentional; do
not try to defeat it.** If a human needs to see the value, tell them to run
`envseal get <NAME> --show` themselves. Do not run `--show` on the human's behalf unless they
explicitly ask.

## Adding / changing a secret

```bash
printf 'the-value' | envseal set SOME_TOKEN      # value via stdin, never on the command line
envseal set TLS_KEY < key.pem                    # multi-line value (PEM, cert, JSON): pipe it
```

Never put the literal value as a command-line argument. After changing secrets, remind the
human to `git add secrets/secrets.enc && git commit`.

## Common failures and what they mean

- **`no 'recipients' file found ... (run envseal init first)`** — you are not inside an
  envseal repo. `cd` into the project root (the dir containing `recipients`) and retry. Do NOT
  run `envseal init` in a repo that already has a store elsewhere.
- **`decryption failed: No matching keys found`** — the current identity isn't a recipient of
  this store. The human needs to be added (see Onboarding) and the store re-encrypted.
- **`envseal set` seems to hang** — it's waiting on stdin. Pipe the value
  (`printf 'v' | envseal set NAME`) instead of running it bare.
- **`command not found: envseal`** — the binary isn't installed. Point the human to the
  one-line installer in `ONBOARDING.md` (a `curl … | sh` that needs no toolchain), or
  `cargo install --path bin` if they have Rust.

## Onboarding a teammate to the shared store

Adding a person is a two-sided key exchange. Walk the human through it:

1. **New teammate** (on their machine): `envseal init`, then `envseal pubkey` — this prints
   their age **public** key (`age1...`). It is safe to share (Slack, email, a PR); it only lets
   others encrypt *to* them, never decrypt. Their **private** key never leaves their machine.
2. **An existing member** adds them and re-encrypts:
   ```bash
   envseal add-recipient age1theirkey... alice
   git add recipients secrets/secrets.enc && git commit -m "Add alice" && git push
   ```
3. The new teammate pulls; they can now decrypt with their own key.

Prefer having the teammate add their own key line via a **pull request** — the key is in the
diff, tied to their identity, and recorded in history.

## Removing a teammate

```bash
envseal remove-recipient alice
```

This re-encrypts without them, but their key still decrypts old commits. **Rotation is the real
revocation:** for every secret they could see, regenerate it at its source and
`printf 'new' | envseal set NAME`. Remind the human of this — the command prints the warning
too.

## What you must never do

- Never run `env`, `printenv`, `echo $SECRET`, `set`, or anything that dumps a value. These are
  denied by the repo's settings and a `PostToolUse` hook will block leaked output.
- Never write a secret's value into a file, a commit, or your reply.
- Never run `envseal get ... --show` on the human's behalf unless they explicitly ask to see it.
- If you think you truly need a plaintext value, **stop and ask the human.**
