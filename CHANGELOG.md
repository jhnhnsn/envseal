# Changelog

All notable changes to envstow are documented here. Versions follow [SemVer](https://semver.org).

## 0.1.5

### Changed
- **Renamed the project from `envseal` to `envstow`.** The binary, config directory
  (`~/.config/envstow/`), environment variables (`ENVSTOW_IDENTITY`, `ENVSTOW_AGENT`,
  `ENVSTOW_UNLOCKED`, `ENVSTOW_INSTALL_DIR`), and repo are all renamed. This is a clean break:
  the new binary does **not** read the old `ENVSEAL_*` variables. Re-run `envstow init` to set
  up (a fresh identity/store under the new name).

### Added
- **Terminal-title indicator.** An `envstow unlock` subshell sets the terminal window/tab title
  to `[envstow:unlocked]` so it's obvious the window holds unlocked secrets; the title clears on
  `exit`. Plain ASCII (no emoji) for terminal compatibility; best-effort (some prompt frameworks
  re-set the title per command).

## 0.1.4

### Added
- **`envstow init` offers to install the Claude Code agent skill** into the current repo's
  `.claude/skills/envstow/` (prompts `[Y/n]`, default yes; `--no-skill` to skip). Committing it
  means every teammate who clones the repo gets it — their agent learns to use secrets by name
  and never print a value. The skill is embedded in the binary, so no separate download is
  needed. Non-interactive runs (CI) install it without prompting.

## 0.1.3

### Changed
- **`get` now masks under any recognized AI agent, not just Claude Code.** Detection was
  broadened to Cursor (`CURSOR_TRACE_ID`/`CURSOR_AGENT`), Aider (`AIDER_*`), Windsurf, and
  generic `AI_AGENT`/`AGENT` markers, alongside the existing `ENVSTOW_AGENT=1` opt-in. Human
  `$(envstow get X)` scripting (no agent markers) still reveals as before.

### Documentation
- Added **[GUARDRAILS.md](GUARDRAILS.md)** — manual setup for the three agent-safety layers
  (instructions, command denylist, output-guard hook), with Claude Code as the worked example
  and the pattern generalized to Cursor, Aider, and Windsurf. A human or an agent can fetch it
  by URL and apply the guardrails for whatever editor is in use.

## 0.1.2

### Added
- **Masked confirmation for `envstow set`.** After storing a value, `set` now prints a masked
  preview — the first 5 characters followed by dots (e.g. `✔ set MY_SECRET (sk-pr••••••••)`) —
  so you can sanity-check a paste without the full value on screen. Values of 5 characters or
  fewer are fully masked, and under an AI agent the preview is fully masked so no characters
  reach the transcript.

### Changed
- **Smoother first install.** The installer now prints a clear next step — open a new terminal
  (or `source ~/.local/bin/env`), then run `envstow --version` — so a "command not found" in the
  same terminal you installed from is no longer mistaken for a failed install. `~/.local/bin` is
  added to PATH for new shells automatically.

### Documentation
- `ONBOARDING.md` leads with a single copy-paste install line; the inspect-the-script,
  verify-checksums, and custom-path (`ENVSTOW_INSTALL_DIR`) options moved into a collapsible
  "security-conscious" section.
- Documented that envstow operates **per project directory** (commands act on the store of the
  repo you're inside), and how to install from a clone to a directory you choose.
- The first `set` example now shows pasting from a password manager (`pbpaste | envstow set …`).
- Fixed a contradiction that said multi-line values were "rejected" — they are supported (pipe
  them in; stored base64-encoded internally).
- Examples use a neutral `MY_SUPER_SECRET_KEY` placeholder.

## 0.1.1

### Added
- **`envstow --version`** (also `-V` / `version`) — prints the installed version.

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
- Bundled Claude Code agent skill so an agent knows how to use envstow on clone.
