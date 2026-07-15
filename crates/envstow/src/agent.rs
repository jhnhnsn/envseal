//! Agent detection and value masking — the AI-safety guard rails baked into the binary.
//!
//! When envstow runs under an AI coding agent that captures stdout into its context, `get` masks
//! its output so a plaintext value can't land in a transcript. Detection is a best-effort
//! allowlist of known agent env markers plus a generic opt-in.

use std::env;

/// Environment markers set by AI coding agents that capture command output into their context.
/// If any is present, `get` masks its value so plaintext can't land in the agent's transcript.
/// This is a best-effort allowlist across known tools plus a generic opt-in — an agent that
/// sets none of these is still expected to use `unlock -- <cmd>` (secrets by name), which never
/// exposes a value regardless of detection.
pub const AGENT_ENV_MARKERS: &[&str] = &[
    // Claude Code
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    // Cursor
    "CURSOR_TRACE_ID",
    "CURSOR_AGENT",
    // Aider
    "AIDER_MODEL",
    "AIDER_CHAT",
    // Windsurf
    "WINDSURF",
    "WINDSURF_AGENT",
    // Generic / cross-tool conventions + explicit opt-in
    "AI_AGENT",
    "AGENT",
    "ENVSTOW_AGENT",
];

/// Are we very likely running under an agent that captures our stdout into its context?
pub fn under_agent() -> bool {
    AGENT_ENV_MARKERS.iter().any(|m| env::var_os(m).is_some())
}

/// A fixed-width mask that hides both a value and its length.
pub fn mask(value: &str) -> String {
    let _ = value;
    "••••••••".to_string()
}

/// A masked preview for confirming a freshly-set value: the first few characters followed by a
/// fixed run of dots — enough to recognize a paste, without showing the secret or its length.
/// Short values (≤5 chars) are fully masked so a whole short secret is never revealed.
pub fn masked_preview(value: &str) -> String {
    const SHOWN: usize = 5;
    const DOTS: &str = "••••••••";
    // Count by chars (not bytes) so multibyte values aren't split mid-codepoint.
    let char_count = value.chars().count();
    if char_count <= SHOWN {
        return DOTS.to_string();
    }
    let head: String = value.chars().take(SHOWN).collect();
    format!("{head}{DOTS}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_hides_value_and_length() {
        assert_eq!(mask("short"), mask("a-much-longer-secret-value"));
        assert!(!mask("sk-abc123").contains("sk-"));
    }

    #[test]
    fn masked_preview_shows_first_five_then_dots() {
        let p = masked_preview("sk-proj-abc123def456");
        assert!(p.starts_with("sk-pr"), "should show first 5 chars: {p}");
        assert!(!p.contains("abc123"), "must not reveal the rest: {p}");
        assert!(p.contains('•'), "should be masked after the prefix");
    }

    #[test]
    fn masked_preview_fully_masks_short_values() {
        for v in ["", "a", "abcd", "exact"] {
            assert!(
                !masked_preview(v).chars().any(|c| c != '•'),
                "short value {v:?} should be all dots, got {}",
                masked_preview(v)
            );
        }
    }

    #[test]
    fn masked_preview_counts_chars_not_bytes() {
        let p = masked_preview("café☕secret-tail");
        assert!(p.starts_with("café☕"), "5 chars incl. multibyte: {p}");
        assert!(!p.contains("secret"), "rest hidden: {p}");
    }

    #[test]
    fn under_agent_detects_every_known_marker() {
        // env::set_var is process-global, so snapshot the full set to avoid disturbing other tests.
        let saved: Vec<(String, Option<std::ffi::OsString>)> = AGENT_ENV_MARKERS
            .iter()
            .map(|m| (m.to_string(), env::var_os(m)))
            .collect();
        for (k, _) in &saved {
            env::remove_var(k);
        }

        assert!(!under_agent(), "no markers → not under agent");
        for marker in AGENT_ENV_MARKERS {
            env::set_var(marker, "1");
            assert!(under_agent(), "{marker} should be detected as an agent");
            env::remove_var(marker);
        }

        for (k, v) in saved {
            match v {
                Some(v) => env::set_var(&k, v),
                None => env::remove_var(&k),
            }
        }
    }
}
