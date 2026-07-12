# envseal — project instructions

This repo stores encrypted secrets (SOPS + age) that are decrypted into the shell
environment via a human-gated direnv unlock. AI coding tools operate on secrets **by
reference**, never by value.

## Secret handling — MANDATORY

- Secrets are loaded as environment variables (names matching `*_KEY`, `*_TOKEN`,
  `*_SECRET`, `*_PASSWORD`, `API_*`). Their **values must NEVER** appear in your output,
  tool-call arguments, or any file you write.
- Reference secrets by variable **name** only (e.g. use `$FLY_API_TOKEN`). Never expand,
  echo, print, `cat`, or log them.
- **Never run:** `env`, `printenv`, `echo $VAR`, `set`, `export -p`, `sops -d`,
  `sops --decrypt`, or any command whose purpose is to reveal secret values. These are
  denied in `.claude/settings.json`; do not try to work around the denylist.
- When a command needs a secret, it is already in the environment — pass it via the env,
  do not inline the literal value.
- A `PostToolUse` hook (`scripts/redact-guard.sh`) blocks any command output that contains
  a live secret value. If you see a "BLOCKED by envseal" message, that is working as
  intended — do not retry in a way that surfaces the value.
- If you believe you genuinely need to see a secret's value, **STOP and ask the human.**

## Unlocking (how secrets reach the environment)

- The human unlocks with the `envseal` launcher: `envseal unlock` (spawns an unlocked
  subshell) or `envseal unlock -- <cmd>` (runs one command with the secret env). It prompts
  `[y/N]` and refuses without a TTY — you cannot drive it non-interactively, and you should
  not try to. Decryption is a human-gated step.
- Once the human has unlocked, you are running inside a shell that already has the vars.
  Just use them by name — do not attempt to unlock, decrypt, or re-run `envseal` yourself.

## Working in this repo

- Edit encrypted secrets with `sops secrets/secrets.enc.env` (opens an editor; re-encrypts
  on save). This is interactive and human-driven.
- To add a recipient: add their age public key to `.sops.yaml`, then
  `sops updatekeys secrets/secrets.enc.env`.
- Never commit plaintext. `.gitignore` blocks `.env`, `*.dec.env`, and `keys.txt`.
