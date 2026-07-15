//! Command-line argument parsing: profile resolution and the shared `[flags] <NAME>` parser
//! used by get/set/delete. Kept dependency-free (no `clap`) to hold envstow to three crates.

use std::env;

use crate::error::AppError;
use crate::layout;

/// Resolve which profile to use and return `(profile, remaining_args)` with any `--profile
/// <name>` (or `--profile=<name>`) removed from the args. Precedence: `--profile` flag >
/// `ENVSTOW_PROFILE` env var > `default`. Returns an error string on a bad/missing name.
pub fn resolve_profile(args: &[String]) -> Result<(String, Vec<String>), AppError> {
    let mut profile: Option<String> = None;
    let mut rest = Vec::with_capacity(args.len());
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--profile" {
            let Some(name) = args.get(i + 1) else {
                return Err(AppError::usage("--profile requires a name"));
            };
            profile = Some(name.clone());
            i += 2;
        } else if let Some(name) = a.strip_prefix("--profile=") {
            profile = Some(name.to_string());
            i += 1;
        } else {
            rest.push(a.clone());
            i += 1;
        }
    }
    let profile = profile
        .or_else(|| env::var("ENVSTOW_PROFILE").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| layout::DEFAULT_PROFILE.to_string());
    if !layout::valid_profile_name(&profile) {
        return Err(AppError::usage(format!(
            "invalid profile name '{profile}' (use letters, digits, - or _)"
        )));
    }
    Ok((profile, rest))
}

/// A parsed `[flags] [<NAME>]` command line, shared by `get`/`set`/`delete` — the three commands
/// with the same shape.
pub struct ParsedArgs<'a> {
    /// Canonical names of the boolean flags that were present.
    pub flags: Vec<&'static str>,
    /// The single positional argument (a secret NAME), if given.
    pub positional: Option<&'a str>,
}

impl ParsedArgs<'_> {
    pub fn has(&self, flag: &'static str) -> bool {
        self.flags.contains(&flag)
    }
}

/// Parse `[flags] [<NAME>]`. `known` maps each accepted flag spelling to a canonical name (so
/// aliases like `-c`/`--clipboard` collapse to one). An unknown `-flag`, or more than one
/// positional, is a usage error naming the offender.
pub fn parse_simple<'a>(
    args: &'a [String],
    known: &[(&str, &'static str)],
) -> Result<ParsedArgs<'a>, AppError> {
    let mut flags = Vec::new();
    let mut positional = None;
    for a in args {
        let s = a.as_str();
        if let Some((_, canon)) = known.iter().find(|(spelling, _)| *spelling == s) {
            if !flags.contains(canon) {
                flags.push(*canon);
            }
        } else if s.starts_with('-') {
            return Err(AppError::usage(format!("unknown flag '{s}'")));
        } else if positional.is_some() {
            return Err(AppError::usage("expected a single NAME"));
        } else {
            positional = Some(s);
        }
    }
    Ok(ParsedArgs { flags, positional })
}
