//! The `unlock`/`refresh` session: spawning a child with secrets in its env, the env markers
//! that let `refresh` scrub what it set, the nested-unlock shadow warning, and the stale-shell
//! nudge. All the "secrets are live in a shell" concerns live here.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::{Command, Stdio};

use crate::cli::resolve_profile;
use crate::error::AppError;
use crate::secrets::Secrets;
use crate::store::load_secrets;

/// `envstow unlock [-- <cmd>...]` — decrypt the whole store and set every value as an env var
/// for a spawned child (an interactive subshell, or the given command). Values never printed;
/// only variable NAMES are listed.
pub fn cmd_unlock(args: &[String]) -> crate::Cmd {
    let (profile, args) = resolve_profile(args)?;
    // Everything after `--` (or all args) is the command to run; empty → interactive subshell.
    let cmd: Vec<String> = match args.iter().position(|a| a == "--") {
        Some(i) => args[i + 1..].to_vec(),
        None => args.to_vec(),
    };

    let secrets = load_secrets(&profile)?;
    if secrets.is_empty() {
        return Err(AppError::msg("store decrypted but contains no variables."));
    }

    let names: Vec<&str> = secrets.names().collect();
    eprintln!(
        "🔓 envstow: loaded {} secret(s) from {}: {}",
        names.len(),
        profile,
        names.join(", ")
    );
    warn_on_shadowed(&secrets);

    spawn_with_env(&cmd, secrets)
}

/// Warn about secrets whose names are ALREADY set in our environment with a different value —
/// the child will see ours, shadowing whatever was there.
///
/// This is the nested-unlock case: unlock in FolderA, cd to FolderB, unlock again. The child gets
/// the UNION of both (env vars are inherited and `Command::env` only adds), with the inner store
/// winning on any shared name. That layering is usually what you want — a subproject adding its
/// own vars on top of shared ones — so this warns rather than blocks.
///
/// Deliberately vague about the source: all we can see is that the name was already set. It might
/// be an outer envstow, your shell rc, or CI. Saying "was already set" is the honest limit of
/// what we know, and it's why identical values are skipped — re-unlocking the same store would
/// otherwise warn about every name, which is noise, not signal.
///
/// Never prints either value, and never reveals which is which — only that they differ.
fn warn_on_shadowed(secrets: &Secrets) {
    let shadowed: Vec<&str> = secrets
        .iter()
        .filter(|(k, v)| {
            // Compare against the inherited value, if any. Only a DIFFERENT value is a real
            // shadow worth reporting.
            env::var_os(k).is_some_and(|existing| existing.to_string_lossy() != *v)
        })
        .map(|(k, _)| k)
        .collect();
    if shadowed.is_empty() {
        return;
    }
    let (count, verb) = if shadowed.len() == 1 {
        ("1 name".to_string(), "was")
    } else {
        (format!("{} names", shadowed.len()), "were")
    };
    eprintln!(
        "⚠️  envstow: {count} {verb} already set with a different value — this store's value wins \
         inside:\n\
         \x20  {}",
        shadowed.join(", ")
    );
}

/// After a `set`/`delete`/`edit` that changed the store, nudge the user IF they ran it from
/// inside an `envstow unlock` shell — that shell holds a copy of the OLD values (a running
/// process's environment can't be changed from outside), so it's now stale. The fix is uniform
/// for every kind of change: exit and unlock again. stderr only; never alters stdout or the exit
/// code. Silent outside an unlocked shell, where there's no stale state to warn about.
pub fn nudge_if_unlocked_shell() {
    if env::var_os("ENVSTOW_UNLOCKED").is_none() {
        return;
    }
    eprintln!(
        "\nℹ️  envstow: you're in an unlocked shell — it still holds the previous values.\n\
         \x20  Run `exit` then `envstow unlock` to pick up this change."
    );
}

/// Env var listing the NAMES envstow set in this environment, comma-separated. Names only —
/// never values. Lets `refresh` unset exactly what envstow owns and nothing else.
const LOADED_MARKER: &str = "ENVSTOW_LOADED";

/// Is `name` a plain shell identifier — `[A-Za-z_][A-Za-z0-9_]*`? Anything else is unsafe to
/// interpolate into shell code that will be `eval`ed.
fn is_shell_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Build the `ENVSTOW_LOADED` value for a child: the names we're about to set, unioned with any
/// an outer unlock already recorded (nested unlocks stack, so the outer names are still live).
fn loaded_marker(secrets: &Secrets) -> String {
    let mut names: Vec<String> = env::var(LOADED_MARKER)
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    for k in secrets.names() {
        if !names.iter().any(|n| n == k) {
            names.push(k.to_string());
        }
    }
    names.join(",")
}

/// The names envstow recorded setting in this environment, per `ENVSTOW_LOADED`.
fn loaded_names() -> Vec<String> {
    env::var(LOADED_MARKER)
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// `envstow status` — report whether this shell is unlocked, which profile it holds, and the
/// secret NAMES that are live in it.
///
/// It reads only the env markers `unlock` set (`ENVSTOW_UNLOCKED`, `ENVSTOW_PROFILE`,
/// `ENVSTOW_LOADED`) — no store is decrypted, no identity is touched, and only names are printed,
/// never values. So it's safe to run anywhere, including under an agent. It reports exactly what
/// envstow put in *this* shell; it can't see shell nesting depth (that's a shell fact, not ours).
pub fn cmd_status(args: &[String]) -> crate::Cmd {
    if let Some(a) = args.first() {
        return Err(AppError::usage(format!("unexpected argument '{a}'")));
    }

    if env::var_os("ENVSTOW_UNLOCKED").is_none() {
        println!("🔒 locked — not inside an `envstow unlock` shell.");
        return Ok(());
    }

    let profile = env::var("ENVSTOW_PROFILE")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::layout::DEFAULT_PROFILE.to_string());
    let names = loaded_names();
    println!("🔓 unlocked — profile: {profile}");
    if names.is_empty() {
        println!("   secrets loaded: (none)");
    } else {
        println!("   secrets loaded ({}): {}", names.len(), names.join(", "));
    }
    Ok(())
}

/// `envstow refresh` — emit shell code to unset secrets this environment has but the store no
/// longer does. Meant to be evaluated by your shell: `eval "$(envstow refresh)"`.
///
/// Why this exists: a child process cannot modify its parent's environment, so nothing envstow
/// runs can clear a stale variable from your shell. `eval` sidesteps that by having YOUR shell
/// execute what we print. The classic form of this trick (ssh-agent, direnv) prints `export
/// NAME=value` — which for envstow would mean dumping every secret in plaintext to stdout, the
/// one thing this tool exists to prevent. So we print ONLY `unset` lines.
///
/// That makes this deliberately one-directional:
///   * a DELETED secret is unset here — nothing about a value is revealed by unsetting its name;
///   * a CHANGED or ADDED secret is NOT updated — that would require printing the new value.
///
/// For those, exit and unlock again. `refresh` reports them so you know.
///
/// Only names in `ENVSTOW_LOADED` are considered, so a `DATABASE_URL` from your shell rc is never
/// touched — envstow only unsets what it set.
pub fn cmd_refresh(args: &[String]) -> crate::Cmd {
    let (profile, args) = resolve_profile(args)?;
    if let Some(a) = args.first() {
        return Err(AppError::usage(format!("unexpected argument '{a}'")));
    }
    if env::var_os("ENVSTOW_UNLOCKED").is_none() {
        return Err(AppError::msg(
            "not inside an `envstow unlock` shell — nothing to refresh.\n\
             \x20  (refresh clears secrets this shell still holds after they left the store.)",
        ));
    }

    let secrets = load_secrets(&profile)?;

    // Stale = envstow set it here, and the store no longer has it. Note we compare against the
    // names WE recorded, not the whole environment, so we never unset someone else's var.
    let in_store: Vec<&str> = secrets.names().collect();
    let stale: Vec<String> = loaded_names()
        .into_iter()
        .filter(|n| !in_store.contains(&n.as_str()) && env::var_os(n).is_some())
        .collect();

    // Changed = still in the store, but this shell holds a different value. We can't fix these
    // without printing the new value, so we only report the count.
    let changed = secrets
        .iter()
        .filter(|(k, v)| env::var_os(k).is_some_and(|existing| existing.to_string_lossy() != *v))
        .count();

    // `secrets` scrubs its values on drop at the end of the function.

    // stdout is the eval payload — shell code ONLY, so a stray word can't be executed.
    //
    // Every name here is interpolated into code the user's shell will EVALUATE, so it must be a
    // plain identifier. A store is trusted input, but "trusted" is not a property to bet a shell
    // injection on: a name like `FOO; rm -rf ~` would otherwise run. Anything that isn't
    // [A-Za-z_][A-Za-z0-9_]* is skipped and reported, never emitted.
    let (safe, unsafe_): (Vec<&String>, Vec<&String>) =
        stale.iter().partition(|n| is_shell_identifier(n));
    let mut out = io::stdout().lock();
    for name in &safe {
        let _ = writeln!(out, "unset {name}");
    }
    let _ = out.flush();
    if !unsafe_.is_empty() {
        eprintln!(
            "envstow: refusing to emit {} name(s) that aren't plain identifiers (would be unsafe \
             to eval). Run `exit` then `envstow unlock` instead.",
            unsafe_.len()
        );
    }

    // Everything human-facing goes to stderr, where `eval "$(...)"` won't swallow or run it.
    if safe.is_empty() {
        eprintln!("envstow: nothing to unset — no secret in this shell has left the store.");
    } else {
        eprintln!(
            "🔄 envstow: unset {} secret(s) no longer in the store: {}",
            safe.len(),
            safe.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if changed > 0 {
        eprintln!(
            "⚠️  envstow: {changed} secret(s) in this shell have a different value in the store. \
             refresh can't update them without printing values — run `exit` then `envstow unlock`."
        );
    }
    Ok(())
}

/// Spawn either the given command or an interactive subshell, with the secrets in its env.
/// `secrets` scrubs its values on drop, after the child has its own copy. Returns the exit code.
fn spawn_with_env(cmd: &[String], secrets: Secrets) -> crate::Cmd {
    let (program, args, interactive) = if cmd.is_empty() {
        let (sh, sh_args) = default_shell();
        eprintln!("🔓 envstow: launching unlocked subshell. Type `exit` to lock.");
        (sh, sh_args, true)
    } else {
        (
            OsString::from(&cmd[0]),
            cmd[1..].iter().map(OsString::from).collect(),
            false,
        )
    };

    let mut command = Command::new(&program);
    command.args(&args);
    for (k, v) in secrets.iter() {
        command.env(k, v);
    }
    command.env("ENVSTOW_UNLOCKED", "1");
    // Record WHICH names we set, so `refresh` can tell an envstow secret from a same-named var
    // that came from your shell rc or CI — and only ever unset the ones we own. Names only; a
    // name is not a secret (`list` prints them). Nested unlocks union with the outer set, so an
    // inner refresh still knows about the outer store's names.
    command.env("ENVSTOW_LOADED", loaded_marker(&secrets));
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // The child inherits a copy of our env at spawn; our `secrets` scrubs on drop (function end).
    match command.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => {
                // Propagate the child's own exit code as ours, silently — it already printed
                // whatever it printed. A child killed by a signal (no code) is 0 for an
                // interactive subshell (you `exit`ed), 1 otherwise.
                let code = status.code().unwrap_or(if interactive { 0 } else { 1 });
                if code == 0 {
                    Ok(())
                } else {
                    Err(AppError::silent(code))
                }
            }
            Err(e) => Err(AppError::msg(format!("error waiting for child: {e}"))),
        },
        Err(e) => Err(AppError::msg(format!(
            "failed to launch '{}': {e}",
            program.to_string_lossy()
        ))
        .with_code(127)),
    }
}

#[cfg(unix)]
fn default_shell() -> (OsString, Vec<OsString>) {
    let sh = env::var_os("SHELL").unwrap_or_else(|| OsString::from("/bin/sh"));
    (sh, vec![OsString::from("-i")])
}

#[cfg(windows)]
fn default_shell() -> (OsString, Vec<OsString>) {
    if let Some(comspec) = env::var_os("COMSPEC") {
        (comspec, Vec::new())
    } else {
        (OsString::from("cmd.exe"), Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_identifiers_gate_what_can_be_evaled() {
        // These are interpolated into code the user's shell will eval. Anything that could break
        // out of `unset <name>` must be rejected — a store is trusted input, but not THAT trusted.
        for ok in ["FOO", "_x", "A1", "DATABASE_URL", "a_b_c9"] {
            assert!(is_shell_identifier(ok), "{ok} should be a valid identifier");
        }
        for bad in [
            "",
            "1FOO",          // leading digit
            "FOO; rm -rf ~", // command injection
            "FOO BAR",
            "FOO$(id)",
            "FOO`id`",
            "FOO&&id",
            "FOO\nid",
            "FOO'",
            "FÖO", // non-ASCII
        ] {
            assert!(
                !is_shell_identifier(bad),
                "{bad:?} must NOT be treated as a safe identifier"
            );
        }
    }

    #[test]
    fn loaded_marker_unions_with_an_outer_unlock() {
        let prev = env::var_os(LOADED_MARKER);
        // Nested unlock: the outer store's names are still live in the environment, so the inner
        // marker must keep them — otherwise a refresh inside the inner shell would forget them.
        env::set_var(LOADED_MARKER, "OUTER_A,SHARED");
        let inner = Secrets::from_pairs(vec![
            ("SHARED".to_string(), "v".to_string()),
            ("INNER_B".to_string(), "v".to_string()),
        ]);
        let marker = loaded_marker(&inner);
        let names: Vec<&str> = marker.split(',').collect();
        assert!(names.contains(&"OUTER_A"), "keeps outer names: {marker}");
        assert!(names.contains(&"INNER_B"), "adds inner names: {marker}");
        assert_eq!(
            names.iter().filter(|n| **n == "SHARED").count(),
            1,
            "no duplicate for a name in both: {marker}"
        );

        env::remove_var(LOADED_MARKER);
        assert_eq!(
            loaded_marker(&inner),
            "SHARED,INNER_B",
            "with no outer marker, just our own names"
        );

        match prev {
            Some(v) => env::set_var(LOADED_MARKER, v),
            None => env::remove_var(LOADED_MARKER),
        }
    }
}
