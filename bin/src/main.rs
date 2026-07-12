//! envseal — decrypt SOPS+age secrets and hand them to a child process, in-memory only.
//!
//! Usage:
//!   envseal unlock                     Prompt, then spawn an unlocked subshell.
//!   envseal unlock -- <cmd> [args...]  Prompt, then run <cmd> with the secret env.
//!   envseal -h | --help
//!
//! Design notes:
//!   * Decryption is delegated to the `sops` CLI (which knows how to find the age
//!     key and is a vetted implementation). This binary is a thin, auditable launcher,
//!     not a reimplementation of SOPS/age crypto.
//!   * Plaintext lives only in this process's memory and the child's environment. It is
//!     never written to disk and never placed in a shell variable that could be echoed.
//!   * The parsed secret buffer is zeroized after the child is spawned.
//!   * Values are NEVER printed. Errors reference variable names only.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zeroize::Zeroize;

const SECRETS_FILE: &str = "secrets/secrets.enc.env";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("-h") | Some("--help") | None => {
            print_help();
            std::process::exit(if args.is_empty() { 2 } else { 0 });
        }
        Some("unlock") => {}
        Some(other) => {
            eprintln!("envseal: unknown command '{other}'\n");
            print_help();
            std::process::exit(2);
        }
    }

    // Everything after a literal `--` is the command to run wrapped.
    // `envseal unlock -- npm run build`  → ["npm", "run", "build"]
    // `envseal unlock`                   → spawn an interactive subshell.
    let rest = &args[1..];
    let cmd: Vec<String> = match rest.iter().position(|a| a == "--") {
        Some(i) => rest[i + 1..].to_vec(),
        None => {
            // Allow `envseal unlock npm run build` too (no -- needed) as long as the
            // first token isn't a flag we recognize.
            rest.to_vec()
        }
    };

    let secrets_path = locate_secrets().unwrap_or_else(|| {
        eprintln!(
            "envseal: could not find {SECRETS_FILE} in this directory or any parent."
        );
        std::process::exit(1);
    });

    if !confirm(&secrets_path) {
        eprintln!("envseal: aborted — secrets not unlocked.");
        std::process::exit(1);
    }

    // Decrypt via sops (dotenv output: KEY=value lines).
    let mut plaintext = decrypt(&secrets_path).unwrap_or_else(|e| {
        eprintln!("envseal: decryption failed: {e}");
        std::process::exit(1);
    });

    let vars = parse_dotenv(&plaintext);
    plaintext.zeroize(); // scrub the raw decrypted blob ASAP

    if vars.is_empty() {
        eprintln!("envseal: no variables found in decrypted secrets.");
        std::process::exit(1);
    }

    // Report by NAME only — never values.
    let names: Vec<&str> = vars.iter().map(|(k, _)| k.as_str()).collect();
    eprintln!("🔓 envseal: loaded {} secret(s): {}", names.len(), names.join(", "));

    let code = spawn_with_env(&cmd, vars, &secrets_path);
    std::process::exit(code);
}

/// Walk up from the CWD looking for `secrets/secrets.enc.env`.
fn locate_secrets() -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        let candidate = dir.join(SECRETS_FILE);
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Interactive [y/N] confirmation gate. Returns true only on an explicit yes.
/// If stdin is not a TTY (e.g. CI), refuse rather than silently proceed.
fn confirm(path: &Path) -> bool {
    if !is_stdin_tty() {
        eprintln!(
            "envseal: refusing to unlock non-interactively (stdin is not a TTY). \
             Use `sops exec-env` directly in automation with a dedicated key."
        );
        return false;
    }
    eprint!("Unlock secrets from {}? [y/N] ", path.display());
    let _ = io::stderr().flush();
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }
    matches!(input.trim(), "y" | "Y" | "yes" | "YES")
}

#[cfg(unix)]
fn is_stdin_tty() -> bool {
    // 0 == STDIN_FILENO. isatty is a libc call; avoid the dep by using /dev/stdin heuristics.
    // Simpler: use the `isatty` via std when available (Rust has IsTerminal since 1.70).
    use std::io::IsTerminal;
    io::stdin().is_terminal()
}

#[cfg(windows)]
fn is_stdin_tty() -> bool {
    use std::io::IsTerminal;
    io::stdin().is_terminal()
}

/// Run `sops -d --output-type dotenv <path>` and capture stdout.
fn decrypt(path: &Path) -> Result<String, String> {
    let out = Command::new("sops")
        .arg("-d")
        .arg("--output-type")
        .arg("dotenv")
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit()) // let sops explain key/permission errors
        .output()
        .map_err(|e| format!("could not run sops ({e}). Is it installed and on PATH?"))?;

    if !out.status.success() {
        return Err("sops exited non-zero (see message above)".into());
    }
    String::from_utf8(out.stdout).map_err(|_| "sops output was not valid UTF-8".into())
}

/// Parse KEY=value dotenv lines. Values are kept verbatim (no unquoting beyond a single
/// surrounding pair of matching quotes, which sops's dotenv output may add).
fn parse_dotenv(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in s.lines() {
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        let key = line[..eq].trim();
        if key.is_empty() {
            continue;
        }
        let mut val = line[eq + 1..].to_string();
        // Strip one matching pair of surrounding quotes if present.
        if val.len() >= 2 {
            let b = val.as_bytes();
            if (b[0] == b'"' && b[val.len() - 1] == b'"')
                || (b[0] == b'\'' && b[val.len() - 1] == b'\'')
            {
                val = val[1..val.len() - 1].to_string();
            }
        }
        out.push((key.to_string(), val));
    }
    out
}

/// Spawn either the given command or an interactive subshell, with `vars` in its env.
/// Zeroizes the values after the child has been launched. Returns the child's exit code.
fn spawn_with_env(cmd: &[String], mut vars: Vec<(String, String)>, secrets_path: &Path) -> i32 {
    let (program, args, interactive) = if cmd.is_empty() {
        let (sh, sh_args) = default_shell();
        eprintln!(
            "🔓 envseal: launching unlocked subshell. Type `exit` to lock. \
             (secrets from {})",
            secrets_path.display()
        );
        (sh, sh_args, true)
    } else {
        (OsString::from(&cmd[0]), cmd[1..].iter().map(OsString::from).collect(), false)
    };

    let mut command = Command::new(&program);
    command.args(&args);
    for (k, v) in &vars {
        command.env(k, v);
    }
    // A breadcrumb so a nested shell/prompt can show it's unlocked, without exposing values.
    command.env("ENVSEAL_UNLOCKED", "1");
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let result = command.spawn();

    // The child now has its own copy of the environment; scrub ours.
    for (_, v) in vars.iter_mut() {
        v.zeroize();
    }

    match result {
        Ok(mut child) => match child.wait() {
            Ok(status) => status.code().unwrap_or(if interactive { 0 } else { 1 }),
            Err(e) => {
                eprintln!("envseal: error waiting for child: {e}");
                1
            }
        },
        Err(e) => {
            eprintln!(
                "envseal: failed to launch '{}': {e}",
                program.to_string_lossy()
            );
            127
        }
    }
}

#[cfg(unix)]
fn default_shell() -> (OsString, Vec<OsString>) {
    let sh = env::var_os("SHELL").unwrap_or_else(|| OsString::from("/bin/sh"));
    (sh, vec![OsString::from("-i")])
}

#[cfg(windows)]
fn default_shell() -> (OsString, Vec<OsString>) {
    // Prefer PowerShell if present via COMSPEC-independent name; fall back to cmd.
    if let Some(comspec) = env::var_os("COMSPEC") {
        (comspec, Vec::new())
    } else {
        (OsString::from("cmd.exe"), Vec::new())
    }
}

fn print_help() {
    eprintln!(
        "envseal — unlock SOPS+age secrets into a child process (in-memory only)\n\
         \n\
         USAGE:\n\
         \x20 envseal unlock                     Prompt, then spawn an unlocked subshell.\n\
         \x20 envseal unlock -- <cmd> [args...]  Prompt, then run <cmd> with the secret env.\n\
         \n\
         EXAMPLES:\n\
         \x20 envseal unlock                     # start Claude / your AI in this shell\n\
         \x20 envseal unlock -- npm run build    # run one command, secrets die with it\n\
         \x20 envseal unlock -- fly deploy\n\
         \n\
         Secrets are read from {SECRETS_FILE} (searched upward from the CWD).\n\
         Values are never printed. Decryption is delegated to the `sops` CLI."
    );
}
