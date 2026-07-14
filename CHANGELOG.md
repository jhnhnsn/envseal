# Changelog

All notable changes to envseal are documented here. Versions follow [SemVer](https://semver.org).

## 0.1.3

### Changed
- **`get` now masks under any recognized AI agent, not just Claude Code.** Detection was
  broadened to Cursor (`CURSOR_TRACE_ID`/`CURSOR_AGENT`), Aider (`AIDER_*`), Windsurf, and
  generic `AI_AGENT`/`AGENT` markers, alongside the existing `ENVSEAL_AGENT=1` opt-in. Human
  `$(envseal get X)` scripting (no agent markers) still reveals as before.

### Documentation
- Added **[GUARDRAILS.md](GUARDRAILS.md)** — manual setup for the three agent-safety layers
  (instructions, command denylist, output-guard hook), with Claude Code as the worked example
  and the pattern generalized to Cursor, Aider, and Windsurf. A human or an agent can fetch it
  by URL and apply the guardrails for whatever editor is in use.

## 0.1.2

### Added
- **Masked confirmation for `envseal set`.** After storing a value, `set` now prints a masked
  preview — the first 5 characters followed by dots (e.g. `✔ set MY_SECRET (sk-pr••••••••)`) —
  so you can sanity-check a paste without the full value on screen. Values of 5 characters or
  fewer are fully masked, and under an AI agent the preview is fully masked so no characters
  reach the transcript.

### Changed
- **Smoother first install.** The installer now prints a clear next step — open a new terminal
  (or `source ~/.local/bin/env`), then run `envseal --version` — so a "command not found" in the
  same terminal you installed from is no longer mistaken for a failed install. `~/.local/bin` is
  added to PATH for new shells automatically.

### Documentation
- `ONBOARDING.md` leads with a single copy-paste install line; the inspect-the-script,
  verify-checksums, and custom-path (`ENVSEAL_INSTALL_DIR`) options moved into a collapsible
  "security-conscious" section.
- Documented that envseal operates **per project directory** (commands act on the store of the
  repo you're inside), and how to install from a clone to a directory you choose.
- The first `set` example now shows pasting from a password manager (`pbpaste | envseal set …`).
- Fixed a contradiction that said multi-line values were "rejected" — they are supported (pipe
  them in; stored base64-encoded internally).
- Examples use a neutral `MY_SUPER_SECRET_KEY` placeholder.

## 0.1.1

### Added
- **`envseal --version`** (also `-V` / `version`) — prints the installed version.

### Documentation
- Documented safer install options (inspect the installer script, verify SHA-256 by hand).

## 0.1.0

Initial release.

### Features
- Age-encrypted key-value secret store (`secrets/secrets.enc`) committed to your repo, decrypted
  per-user with each collaborator's own age key. All crypto is the `age` crate — no external
  `sops`/`age` CLIs required.
- Commands: `init`, `set` (value via stdin), `edit` (`$EDITOR` round-trip), `get` (masked under
  an AI agent unless `--show`), `list`, `unlock [-- <cmd>]`, `pubkey`, `add-recipient`,
  `remove-recipient`, `reencrypt`.
- **AI-safe by design:** secrets are referenced by name; `get` masks its output under an agent so
  plaintext never enters an agent's context.
- Multi-line secrets (PEM keys, certs, JSON) supported via stdin, base64-encoded internally.
- One-line prebuilt-binary installer (macOS arm64/x86_64, Linux arm64/x86_64, Windows) with
  SHA-256 verification.
- Bundled Claude Code agent skill so an agent knows how to use envseal on clone.
