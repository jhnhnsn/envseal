//! Self-update: `envstow upgrade` and its `--check`. Deliberately dependency-free — the version
//! check follows the `/releases/latest` redirect via `curl` (no HTTP crate, no JSON parsing) and
//! the install re-runs the published installer. See the module functions for the reasoning.

use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Command;

use crate::error::AppError;
use crate::layout;

/// The published shell installer, i.e. the command the README tells you to run. `upgrade` re-runs
/// it so you don't have to remember it — that IS the feature.
///
/// POSIX-only: Windows installs via the PowerShell installer and takes a different branch in
/// `cmd_upgrade`, so this would be dead code there (and `-D warnings` in CI rightly fails on it).
#[cfg(not(windows))]
const INSTALLER_URL: &str =
    "https://github.com/jhnhnsn/envstow/releases/latest/download/envstow-installer.sh";

/// `/releases/latest` 302s to `/releases/tag/vX.Y.Z`, so the redirect target names the newest
/// version. That's the whole version check: no JSON to parse (no serde), no API token, and it
/// isn't subject to the API's unauthenticated rate limit.
const LATEST_URL: &str = "https://github.com/jhnhnsn/envstow/releases/latest";

/// Ask GitHub for the latest released version by following the `/releases/latest` redirect and
/// reading the tag off the final URL. Shells out to `curl` rather than linking an HTTP stack:
/// envstow is a secrets tool with three dependencies on purpose, and a self-updater is
/// convenience, not function — not worth tripling the code running beside your decrypted keys.
/// `curl` is already how the README says to install, so it's a dependency we already require.
pub fn latest_version() -> Result<String, String> {
    let out = Command::new("curl")
        .args([
            "-sSL",
            "--proto",
            "=https",
            "--tlsv1.2",
            "--max-time",
            "15",
            "-o",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
            "-w",
            "%{url_effective}",
            LATEST_URL,
        ])
        .output()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => "curl not found — install it, or update manually:\n\
                 \x20  curl --proto '=https' --tlsv1.2 -LsSf <installer> | sh"
                .to_string(),
            _ => format!("could not run curl: {e}"),
        })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            "could not reach GitHub to check for updates".to_string()
        } else {
            format!("could not check for updates: {err}")
        });
    }
    let url = String::from_utf8_lossy(&out.stdout);
    // …/releases/tag/v0.1.11 → 0.1.11
    let tag = url
        .rsplit('/')
        .next()
        .filter(|t| !t.is_empty() && *t != "latest")
        .ok_or_else(|| format!("unexpected release URL: {url}"))?;
    Ok(tag.trim_start_matches('v').to_string())
}

/// Compare dotted numeric versions (0.1.9 < 0.1.11 — string compare would get this backwards).
/// Non-numeric or extra components fall back to comparing what parses; unknown shapes sort equal
/// so we never claim an update exists on a version we can't read.
pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split(['.', '-', '+'])
            .map_while(|p| p.parse::<u64>().ok())
            .collect()
    };
    let (c, u) = (parse(candidate), parse(current));
    if c.is_empty() || u.is_empty() {
        return false;
    }
    for i in 0..c.len().max(u.len()) {
        let (a, b) = (
            c.get(i).copied().unwrap_or(0),
            u.get(i).copied().unwrap_or(0),
        );
        if a != b {
            return a > b;
        }
    }
    false
}

/// How this envstow was installed, from the cargo-dist receipt beside the identity config.
/// `None` means no receipt — a package manager, `cargo install`, or a hand-placed binary.
fn install_receipt() -> Option<String> {
    let path = layout::identity_path()
        .parent()?
        .join("envstow-receipt.json");
    let text = std::fs::read_to_string(path).ok()?;
    // Deliberately not parsing JSON — that would mean a serde dependency for one field. We only
    // need to know whether OUR installer wrote this, which this substring answers.
    if text.contains("\"source\": \"cargo-dist\"") || text.contains("\"source\":\"cargo-dist\"") {
        Some("cargo-dist".to_string())
    } else {
        Some("unknown".to_string())
    }
}

/// `envstow upgrade [--check]` — check for a newer release, and install it by re-running the
/// published installer.
///
/// Refuses to self-update an install we didn't perform: overwriting a Homebrew/AUR-managed binary
/// desynchronizes it from the package manager's database (`brew doctor` complains; pacman
/// considers it hostile), or drops a second envstow on PATH that may shadow the managed one.
/// When there's no cargo-dist receipt, we say who should do the updating instead.
pub fn cmd_upgrade(args: &[String]) -> crate::Cmd {
    let mut check_only = false;
    let mut yes = false;
    for a in args {
        match a.as_str() {
            "--check" => check_only = true,
            "--yes" | "-y" => yes = true,
            s => {
                return Err(AppError::usage(format!(
                    "unknown argument '{s}'\nusage: envstow upgrade [--check] [--yes]"
                )));
            }
        }
    }

    let current = env!("CARGO_PKG_VERSION");
    let latest = latest_version()?;

    if !version_is_newer(&latest, current) {
        eprintln!("envstow {current} is up to date (latest: {latest}).");
        return Ok(());
    }
    eprintln!("⬆️  envstow {latest} is available (you have {current}).");
    eprintln!("   {}/releases/tag/v{latest}", layout::REPO_URL);

    if check_only {
        return Ok(());
    }

    // Only self-update an install we own.
    if install_receipt().as_deref() != Some("cargo-dist") {
        return Err(AppError::msg(format!(
            "this copy wasn't installed by the envstow installer, so `update` won't touch it\n\
             \x20  (no cargo-dist receipt at {}).\n\
             \x20  Update it with whatever installed it — e.g. `brew upgrade envstow`, your \
             distro's\n\
             \x20  package manager, or `cargo install --path crates/envstow` from a fresh checkout.",
            layout::identity_path()
                .parent()
                .unwrap_or(Path::new("."))
                .join("envstow-receipt.json")
                .display()
        )));
    }

    // Confirm before replacing the binary. Unlike `init`'s skill prompt, a non-interactive run
    // does NOT proceed by default: this downloads and executes a remote script over the running
    // executable, and a CI job that silently swapped its own envstow out from under itself would
    // be a nasty surprise. Non-TTY callers must opt in with --yes.
    if !yes {
        if !io::stdin().is_terminal() {
            return Err(AppError::msg(
                "refusing to update non-interactively — pass `--yes` to confirm:\n\
                 \x20  envstow upgrade --yes",
            ));
        }
        eprint!("Download and install envstow {latest}? [Y/n] ");
        let _ = io::stderr().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let ans = input.trim().to_ascii_lowercase();
            if ans == "n" || ans == "no" {
                eprintln!("   skipped.");
                return Ok(());
            }
        }
    }

    // Windows installs via the PowerShell installer; there's no `sh` to pipe through, so we print
    // the command instead of running it.
    #[cfg(windows)]
    {
        eprintln!(
            "\nenvstow: run the PowerShell installer to upgrade:\n\
             \x20  powershell -c \"irm https://github.com/jhnhnsn/envstow/releases/latest/download/envstow-installer.ps1 | iex\""
        );
        Ok(())
    }

    #[cfg(not(windows))]
    {
        eprintln!("   running the official installer…");
        // Exactly the pipeline the README documents — same URL, same TLS pinning. We're only
        // saving you from having to remember it.
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "curl --proto '=https' --tlsv1.2 -LsSf {INSTALLER_URL} | sh"
            ))
            .status()
            .map_err(|e| AppError::msg(format!("could not run the installer: {e}")))?;
        if status.success() {
            eprintln!("updated to envstow {latest}. Open a new shell (or `hash -r`) to use it.");
            Ok(())
        } else {
            Err(AppError::msg(format!(
                "the installer exited with {}. Try it by hand:\n\
                 \x20  curl --proto '=https' --tlsv1.2 -LsSf {INSTALLER_URL} | sh",
                status.code().unwrap_or(-1)
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare_is_numeric_not_lexical() {
        // The bug this exists to prevent: "0.1.9" > "0.1.11" as strings, so a lexical compare
        // would tell everyone on 0.1.11 to "update" to 0.1.9, forever.
        assert!(version_is_newer("0.1.11", "0.1.9"), "0.1.11 > 0.1.9");
        assert!(!version_is_newer("0.1.9", "0.1.11"), "0.1.9 is not newer");

        assert!(version_is_newer("0.2.0", "0.1.11"));
        assert!(version_is_newer("1.0.0", "0.99.99"));
        assert!(!version_is_newer("0.1.11", "0.1.11"), "equal is not newer");
        assert!(!version_is_newer("0.1.10", "0.1.11"));

        // Differing component counts: missing parts are zero.
        assert!(version_is_newer("0.2", "0.1.11"));
        assert!(!version_is_newer("0.1", "0.1.0"), "0.1 == 0.1.0");
        assert!(version_is_newer("0.1.1", "0.1"));

        // Pre-release / build suffixes: compare the numeric lead, don't panic.
        assert!(version_is_newer("0.2.0-beta.1", "0.1.11"));

        // Unparseable input must never claim an update — better silent than wrong.
        assert!(!version_is_newer("garbage", "0.1.11"));
        assert!(!version_is_newer("0.1.12", "garbage"));
        assert!(!version_is_newer("", "0.1.11"));
    }
}
