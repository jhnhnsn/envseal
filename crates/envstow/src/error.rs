//! The one error type commands return, and the single place exit codes are decided.
//!
//! Before this, every `cmd_*` returned `i32` and hand-rolled the same dance at each failure:
//! `match … { Ok(v) => v, Err(e) => { eprintln!("envstow: {e}"); return 1 } }` — 49 such blocks,
//! 80 bare `return 1/2`s. Now commands return `Result<(), AppError>`, propagate with `?`, and
//! `main` maps the error to a message and exit code in exactly one spot.
//!
//! Exit-code convention (unchanged): **2** for a usage/argument error, **1** for any other
//! failure, **0** for success.

use std::fmt;

use crate::crypto::CryptoError;
use crate::layout::LayoutError;

/// A command failure: the human-facing message and the process exit code to use. `main` prints
/// `envstow: {message}` to stderr and exits with `code`.
#[derive(Debug)]
pub struct AppError {
    message: String,
    code: i32,
}

impl AppError {
    /// A runtime failure (exit 1) — the common case.
    pub fn msg(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 1,
        }
    }

    /// A usage / bad-argument error (exit 2).
    pub fn usage(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code: 2,
        }
    }

    /// Exit with `code` and print nothing. Used to propagate a child process's own exit code out
    /// of `unlock` — the child already produced whatever output it has, so envstow stays silent.
    pub fn silent(code: i32) -> Self {
        Self {
            message: String::new(),
            code,
        }
    }

    /// Override the exit code (e.g. 127 for "command not found"), keeping the message.
    pub fn with_code(mut self, code: i32) -> Self {
        self.code = code;
        self
    }

    pub fn code(&self) -> i32 {
        self.code
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AppError {}

// `?` across module boundaries: layout/crypto errors become runtime failures with their own
// message. A bare `String` (used by a few in-module helpers) does too.
impl From<LayoutError> for AppError {
    fn from(e: LayoutError) -> Self {
        Self::msg(e.to_string())
    }
}

impl From<CryptoError> for AppError {
    fn from(e: CryptoError) -> Self {
        Self::msg(e.to_string())
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::msg(message)
    }
}
