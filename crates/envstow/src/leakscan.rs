//! `envstow scan-leak` — the output-guard detection, moved into the binary.
//!
//! Wired as a Claude Code `PostToolUse` hook, it reads the tool-result payload on stdin and exits
//! **2** (which the agent treats as blocking) if that output contains a live secret value —
//! withholding the result from the agent's context. It exits **0** (allow) otherwise, and never
//! prints a value, only the offending variable name.
//!
//! This replaces the hand-copied `scripts/redact-guard.sh`: the same logic, but shipped in the
//! binary so `envstow upgrade` delivers fixes instead of every copied-out script rotting. Setup is
//! one line in the user's `.claude/settings.json` (see GUARDRAILS.md) — envstow never writes there.
//!
//! Detection (identical to the hardened script):
//!   * which vars are secrets: every name in `ENVSTOW_LOADED` (what `unlock` set — name-agnostic,
//!     so `DATABASE_URL`/DSN/connection strings count), unioned with a `*_KEY`/`*_TOKEN`/… name
//!     heuristic for vars that reached the env some other way;
//!   * a value counts only if it's *distinctive* (see [`distinctive`]) — a length+entropy gate
//!     that catches short random tokens without blocking on `12345678`/`password`;
//!   * matching is exact-substring and multi-line-aware (each line of a PEM/JSON value is a
//!     needle), plus a base64-encoded copy.
//!
//! Out of scope by design (documented in the threat model): values < 5 chars, low-entropy values,
//! and encodings other than raw/base64.

use std::env;
use std::io::{self, IsTerminal, Read};

use base64::Engine;

use crate::error::AppError;

/// Read the tool-result payload on stdin and block (exit 2) if it leaks a live secret value.
pub fn cmd_scan_leak(args: &[String]) -> crate::Cmd {
    if let Some(a) = args.first() {
        return Err(AppError::usage(format!(
            "unexpected argument '{a}' — scan-leak reads its payload on stdin"
        )));
    }

    // Run by hand at a terminal? It would otherwise hang waiting on stdin. Explain and exit clean.
    if io::stdin().is_terminal() {
        eprintln!(
            "envstow scan-leak reads a tool-result payload on stdin and exits non-zero if it\n\
             contains a live secret value. It's meant to be wired as a Claude Code PostToolUse\n\
             hook, not run by hand — see GUARDRAILS.md."
        );
        return Ok(());
    }

    let mut payload = String::new();
    if io::stdin().read_to_string(&mut payload).is_err() {
        // Can't read the payload → nothing to inspect → allow (fail open, matching the script).
        return Ok(());
    }

    match scan(&payload, &env_secret_names(), &EnvLookup) {
        Some((name, how)) => {
            // Printed verbatim (no `envstow:` prefix) so the agent sees the guard's own voice.
            eprintln!(
                "BLOCKED by envstow: command output contained {how} ${name}; result withheld \
                 from context. Reference secrets by variable name only — never echo, print, log, \
                 or encode a value."
            );
            // Exit 2, print nothing further (we already printed the block message).
            Err(AppError::silent(2))
        }
        None => Ok(()),
    }
}

/// How a value was found in the output — used only to phrase the block message.
type How = &'static str;

/// Abstracts "what's this env var's value" so tests can inject values without touching the real
/// process environment (which is global and shared across the test binary).
trait Env {
    fn get(&self, name: &str) -> Option<String>;
}

struct EnvLookup;
impl Env for EnvLookup {
    fn get(&self, name: &str) -> Option<String> {
        env::var(name).ok()
    }
}

/// The names that count as secrets: `ENVSTOW_LOADED` (authoritative, name-agnostic) unioned with
/// the `*_KEY`/`*_TOKEN`/… convention. Sorted + deduped for deterministic iteration.
fn env_secret_names() -> Vec<String> {
    let mut names: Vec<String> = env::var("ENVSTOW_LOADED")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    for (name, _) in env::vars() {
        if is_secret_shaped(&name) {
            names.push(name);
        }
    }
    names.sort();
    names.dedup();
    names
}

/// The name-convention fallback: `*_KEY`, `*_TOKEN`, `*_SECRET`, `*_PASSWORD`, `*_PASSWD`, `API_*`.
fn is_secret_shaped(name: &str) -> bool {
    name.starts_with("API_")
        || ["_KEY", "_TOKEN", "_SECRET", "_PASSWORD", "_PASSWD"]
            .iter()
            .any(|suf| name.ends_with(suf))
}

/// Core scan: for each secret name, is its value (or a line of it, or its base64) present in the
/// output extracted from `payload`? Returns the first hit as `(name, how)`.
fn scan(payload: &str, names: &[String], envs: &impl Env) -> Option<(String, How)> {
    let output = extract_output(payload);
    if output.is_empty() {
        return None;
    }
    for name in names {
        if let Some(how) = leak(&output, name, envs) {
            return Some((name.clone(), how));
        }
    }
    None
}

/// Does `name`'s value appear in `output`?
fn leak(output: &str, name: &str, envs: &impl Env) -> Option<How> {
    let value = envs.get(name).unwrap_or_default();
    if !distinctive(&value) {
        return None;
    }
    // Needles: the whole value (already known distinctive), plus each line of a multi-line value
    // (a PEM/JSON secret can leak one sensitive line at a time, which is not a substring of the
    // whole — each line is re-gated so a boilerplate `-----END-----` alone can't trip it).
    if output.contains(&value) {
        return Some("the live value of");
    }
    for line in value.lines() {
        if distinctive(line) && output.contains(line) {
            return Some("the live value of");
        }
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(value.as_bytes());
    if b64.len() >= 12 && output.contains(&b64) {
        return Some("a base64-encoded copy of");
    }
    None
}

/// Is `s` distinctive enough that finding it in tool output means a real leak, not a chance
/// collision with ordinary text?  Length is a poor gate on its own — `12345678`/`password` are
/// 8+ chars yet common, while a 6-char random token almost never collides. So: `< 5` never, `>=
/// 12` always, and `5..11` only if it mixes ≥2 character classes (lower/upper/digit/symbol).
fn distinctive(s: &str) -> bool {
    let n = s.chars().count();
    if n < 5 {
        return false;
    }
    if n >= 12 {
        return true;
    }
    let classes = usize::from(s.chars().any(|c| c.is_lowercase()))
        + usize::from(s.chars().any(|c| c.is_uppercase()))
        + usize::from(s.chars().any(|c| c.is_ascii_digit()))
        + usize::from(s.chars().any(|c| !c.is_alphanumeric()));
    classes >= 2
}

/// Extract the text that reached the agent — the `stdout`/`stderr`/`output` string fields of the
/// PostToolUse payload — decoding JSON string escapes so multi-line values match.
///
/// A minimal, targeted extractor rather than a full JSON parser or a `serde_json` dependency: for
/// each `"stdout"`/`"stderr"`/`"output"` key it parses the following string value. It fails open —
/// on anything it can't parse it simply yields no text (allow), matching the script's `except`.
fn extract_output(payload: &str) -> String {
    let bytes = payload.as_bytes();
    let mut parts: Vec<String> = Vec::new();
    for key in ["stdout", "stderr", "output"] {
        let needle = format!("\"{key}\"");
        let mut from = 0;
        while let Some(rel) = payload[from..].find(&needle) {
            let mut i = from + rel + needle.len();
            // Expect optional whitespace, then ':', then optional whitespace, then a '"'.
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b':' {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'"' {
                    if let Some(value) = parse_json_string(&payload[i..]) {
                        parts.push(value);
                    }
                }
            }
            from = from + rel + needle.len();
        }
    }
    parts.join("\n")
}

/// Parse the JSON string literal at the start of `s` (which begins with the opening `"`). Returns
/// the decoded contents, or `None` if it never closes. Decodes `\" \\ \/ \n \r \t \b \f` and
/// `\uXXXX` — the escapes that affect substring matching.
fn parse_json_string(s: &str) -> Option<String> {
    let mut chars = s.chars();
    if chars.next() != Some('"') {
        return None;
    }
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'b' => out.push('\u{08}'),
                'f' => out.push('\u{0C}'),
                'u' => {
                    let hex: String = (0..4).map_while(|_| chars.next()).collect();
                    let cp = u32::from_str_radix(&hex, 16).ok()?;
                    out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                }
                other => out.push(other),
            },
            _ => out.push(c),
        }
    }
    None // unterminated
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MapEnv(HashMap<String, String>);
    impl Env for MapEnv {
        fn get(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }
    }
    fn env(pairs: &[(&str, &str)]) -> MapEnv {
        MapEnv(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }
    fn payload(stdout: &str) -> String {
        // Build a real JSON payload with proper escaping via a tiny encoder.
        let esc: String = stdout
            .chars()
            .flat_map(|c| match c {
                '"' => vec!['\\', '"'],
                '\\' => vec!['\\', '\\'],
                '\n' => vec!['\\', 'n'],
                '\r' => vec!['\\', 'r'],
                '\t' => vec!['\\', 't'],
                other => vec![other],
            })
            .collect();
        format!("{{\"tool_response\":{{\"stdout\":\"{esc}\"}}}}")
    }

    #[test]
    fn blocks_a_leaked_value() {
        let p = payload("the token is sk-test-9d4f2a7c here");
        let hit = scan(
            &p,
            &["FAKE_TOKEN".into()],
            &env(&[("FAKE_TOKEN", "sk-test-9d4f2a7c")]),
        );
        assert_eq!(hit.map(|(n, _)| n), Some("FAKE_TOKEN".to_string()));
    }

    #[test]
    fn allows_a_name_reference() {
        let p = payload("using $FAKE_TOKEN to authenticate");
        assert!(scan(
            &p,
            &["FAKE_TOKEN".into()],
            &env(&[("FAKE_TOKEN", "sk-test-9d4f2a7c")])
        )
        .is_none());
    }

    #[test]
    fn blocks_a_base64_copy() {
        let val = "sk-test-9d4f2a7c";
        let b64 = base64::engine::general_purpose::STANDARD.encode(val);
        let p = payload(&format!("blob {b64} end"));
        assert!(scan(&p, &["FAKE_TOKEN".into()], &env(&[("FAKE_TOKEN", val)])).is_some());
    }

    #[test]
    fn non_conventional_name_still_blocks() {
        // DATABASE_URL doesn't match the *_KEY/*_TOKEN convention — caught only because the name
        // is passed in (as ENVSTOW_LOADED would supply it).
        let p = payload("connecting to postgres://admin:hunter2SECRETval@db/main");
        let hit = scan(
            &p,
            &["DATABASE_URL".into()],
            &env(&[("DATABASE_URL", "postgres://admin:hunter2SECRETval@db/main")]),
        );
        assert!(hit.is_some());
    }

    #[test]
    fn blocks_the_middle_line_of_a_multiline_value() {
        let pem = "-----BEGIN-----\nMIISECRETMIDDLExyz0000\n-----END-----";
        let p = payload("exfiltrated: MIISECRETMIDDLExyz0000");
        assert!(scan(&p, &["TLS_KEY".into()], &env(&[("TLS_KEY", pem)])).is_some());
    }

    #[test]
    fn does_not_over_block_low_entropy() {
        // A digit run and a dictionary word are 8 chars but common — must not block.
        let p1 = payload("exit 12345678 lines processed");
        assert!(scan(&p1, &["PIN".into()], &env(&[("PIN", "12345678")])).is_none());
        let p2 = payload("enter your password to continue");
        assert!(scan(&p2, &["WORD".into()], &env(&[("WORD", "password")])).is_none());
    }

    #[test]
    fn blocks_short_but_random_token() {
        let p = payload("leaked: sk-9x2 oops");
        assert!(scan(&p, &["K".into()], &env(&[("K", "sk-9x2")])).is_some());
    }

    #[test]
    fn allows_unrelated_output() {
        let p = payload("build succeeded in 3.2s");
        assert!(scan(&p, &["TOK".into()], &env(&[("TOK", "sk-test-9d4f2a7c")])).is_none());
    }

    #[test]
    fn unparseable_payload_fails_open() {
        assert!(scan(
            "not json at all",
            &["TOK".into()],
            &env(&[("TOK", "sk-test-9d4f2a7c")])
        )
        .is_none());
        assert!(scan("", &["TOK".into()], &env(&[("TOK", "sk-test-9d4f2a7c")])).is_none());
    }

    #[test]
    fn extract_decodes_escaped_newlines() {
        // A value leaked across an escaped newline in JSON must still match.
        let p = "{\"tool_response\":{\"stdout\":\"line1\\nMIISECRETMIDDLExyz0000\\nline3\"}}";
        assert!(extract_output(p).contains("MIISECRETMIDDLExyz0000"));
        assert!(extract_output(p).contains('\n'));
    }

    #[test]
    fn distinctive_gate() {
        for ok in ["sk-9x2", "x9K2mQ", "sk-proj-abcdefgh"] {
            assert!(distinctive(ok), "{ok} should be distinctive");
        }
        for no in ["", "ab12", "hello", "12345678", "password", "secret"] {
            assert!(!distinctive(no), "{no} should not be distinctive");
        }
    }
}
