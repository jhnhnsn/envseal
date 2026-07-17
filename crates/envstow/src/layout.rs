//! envstow file & key layout — where the identity, recipients, and encrypted store live,
//! and how they are located, read, and written.
//!
//! Locations (all repo files live under `.envstow/` at the repo root)
//! ---------
//!   * Identity (PRIVATE key): `$ENVSTOW_IDENTITY`, else `~/.config/envstow/identity.txt`
//!     (`%APPDATA%\envstow\identity.txt` on Windows). Contains one `AGE-SECRET-KEY-...` line.
//!     Never committed; created mode 0600 on Unix.
//!   * Recipients (PUBLIC keys): `.envstow/recipients`. Committed. One `age1...` per line;
//!     `#` comments and optional trailing `# Name` allowed. Shared across all profiles.
//!   * Encrypted stores: `.envstow/<profile>.enc`, one per profile. Committed. The default
//!     profile is `.envstow/default.enc`. Each file is an `envstow-format: <n>` header line
//!     followed by the age payload; the decrypted plaintext is dotenv. The header is checked
//!     before decryption so a store from a newer envstow reports that plainly instead of
//!     failing as a decryption error — see [`FORMAT_VERSION`].
//!
//! The repo root is whatever directory (walking up from the CWD) contains a `.envstow/recipients`
//! file — that anchors the stores and any relative operations.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// All envstow files for a repo live in this directory at the repo root.
pub const ENVSTOW_DIR: &str = ".envstow";
/// The recipients file, relative to the repo root (inside `.envstow/`).
pub const RECIPIENTS_FILE: &str = ".envstow/recipients";
/// The default profile's store, relative to the repo root. Named `default.enc` so the file
/// tells you which profile it is.
pub const STORE_FILE: &str = ".envstow/default.enc";
/// The name of the default (unnamed) profile.
pub const DEFAULT_PROFILE: &str = "default";

/// Where to send someone whose envstow is too old to read a store.
pub const REPO_URL: &str = "https://github.com/jhnhnsn/envstow";

/// The on-disk store format this binary reads and writes.
///
/// This versions the *file layout*, not the tool — bump it only when the bytes change shape in a
/// way an older binary would misread (a new envelope, a different payload encoding, a header
/// field). Ordinary releases leave it alone: 0.1.6 → 0.1.7 added a command and did NOT touch the
/// format. Bumping it on every release would cry wolf and train people to ignore the warning.
///
/// When you DO bump it, both guards below start firing for anyone on an older binary — a read
/// gets [`LayoutError::FormatTooNew`], a write gets [`LayoutError::FormatWouldDowngrade`] — each
/// naming the version and pointing at [`REPO_URL`]. Add a note to CHANGELOG.md saying the format
/// moved and that everyone sharing a store must update.
///
/// History:
///   * 1 — headerless: the file is a bare age payload. Everything envstow wrote before 0.1.9.
///   * 2 — the `envstow-format:` header, added in 0.1.9. This bump is the one break the scheme
///     couldn't avoid: a binary with no header code (≤ 0.1.8) sees the header as a corrupt age
///     envelope and reports "decryption failed: Header is invalid". That's precisely the
///     cryptic failure the header exists to prevent — but it can only be prevented for versions
///     that already know to look for it. From 2 onward, an old binary gets a real explanation.
pub const FORMAT_VERSION: u32 = 2;

/// The header line prefixed to every store: `envstow-format: <n>\n`, before the age payload.
///
/// It lives OUTSIDE the ciphertext deliberately. A version sealed inside the encrypted payload is
/// unreadable until after decryption — useless for catching an envelope change, which is exactly
/// the case that would otherwise surface as the maximally-confusing "No matching keys found"
/// (indistinguishable from "you were removed as a recipient"). age itself does the same thing
/// with its own `age-encryption.org/v1` line. The version is public metadata, not a secret.
const FORMAT_PREFIX: &str = "envstow-format: ";

/// Split a store file's bytes into `(format, ciphertext)`.
///
/// A store with no header is format 1: every store written before 0.1.9 starts directly with
/// age's own `age-encryption.org/v1` line. Reading those still works — this binary upgrades them
/// to format 2 the next time anything writes. (The reverse isn't true: a ≤0.1.8 binary can't read
/// what we write. See [`FORMAT_VERSION`].)
fn split_format_header(bytes: &[u8]) -> Result<(u32, &[u8]), LayoutError> {
    let Some(rest) = bytes.strip_prefix(FORMAT_PREFIX.as_bytes()) else {
        return Ok((1, bytes));
    };
    let Some(nl) = rest.iter().position(|b| *b == b'\n') else {
        return Err(LayoutError::BadFormatHeader);
    };
    let digits = std::str::from_utf8(&rest[..nl])
        .map_err(|_| LayoutError::BadFormatHeader)?
        .trim();
    let version: u32 = digits.parse().map_err(|_| LayoutError::BadFormatHeader)?;
    Ok((version, &rest[nl + 1..]))
}

/// The store filename for a given profile, relative to the repo root: `.envstow/<profile>.enc`.
pub fn store_file_for(profile: &str) -> String {
    format!("{ENVSTOW_DIR}/{profile}.enc")
}

/// Validate a profile name: non-empty, and only chars safe as a filename component (so it can't
/// escape the `.envstow/` dir or collide with the `.enc` suffix). `recipients` is reserved.
pub fn valid_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name != "recipients"
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// A parsed recipient entry: the `age1...` key plus an optional human label from a trailing
/// `# Name` comment. The label is cosmetic — matching/removal can use either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipient {
    pub key: String,
    pub label: Option<String>,
}

#[derive(Debug)]
pub enum LayoutError {
    NoRecipientsFile,
    NoStore,
    Io(String),
    NoIdentity(PathBuf),
    Empty(&'static str),
    /// The store is a newer format than this binary can read.
    FormatTooNew {
        found: u32,
    },
    /// The store is a newer format than this binary writes; writing would downgrade it.
    FormatWouldDowngrade {
        found: u32,
    },
    /// The header is present but unparseable — a truncated or corrupted file.
    BadFormatHeader,
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::NoRecipientsFile => write!(
                f,
                "no `{RECIPIENTS_FILE}` file found in this directory or any parent \
                 (run `envstow init` first)"
            ),
            LayoutError::NoStore => {
                write!(f, "no `{STORE_FILE}` found next to `{RECIPIENTS_FILE}`")
            }
            LayoutError::Io(e) => write!(f, "{e}"),
            LayoutError::NoIdentity(p) => write!(
                f,
                "no identity (private key) at {} — run `envstow init` or set $ENVSTOW_IDENTITY",
                p.display()
            ),
            LayoutError::Empty(what) => write!(f, "{what} is empty"),
            LayoutError::FormatTooNew { found } => write!(
                f,
                "this store uses format {found}, but your envstow only understands format \
                 {FORMAT_VERSION}.\n\
                 A teammate wrote it with a newer envstow. Update yours to read it:\n\
                 \x20  {REPO_URL}"
            ),
            LayoutError::FormatWouldDowngrade { found } => write!(
                f,
                "refusing to write — this store is format {found} and your envstow writes format \
                 {FORMAT_VERSION}.\n\
                 Writing would downgrade it and break it for teammates on a newer envstow. \
                 Update yours first:\n\
                 \x20  {REPO_URL}"
            ),
            LayoutError::BadFormatHeader => write!(
                f,
                "the store's `{}` header is malformed — the file looks truncated or corrupted. \
                 Restore it from git history (`git checkout -- .envstow/`).",
                FORMAT_PREFIX.trim_end()
            ),
        }
    }
}

impl std::error::Error for LayoutError {}

/// Resolved paths for a repo: the recipients file and the encrypted store beside it.
pub struct Paths {
    pub recipients: PathBuf,
    pub store: PathBuf,
}

/// Walk up from the CWD to find the `recipients` file that anchors the repo; derive the store
/// path for `profile` beside it. Does not require the store to exist yet (init creates it).
/// All profiles share the one `recipients` file.
pub fn locate(profile: &str) -> Result<Paths, LayoutError> {
    let mut dir = env::current_dir().map_err(|e| LayoutError::Io(e.to_string()))?;
    loop {
        let cand = dir.join(RECIPIENTS_FILE);
        if cand.is_file() {
            return Ok(Paths {
                store: dir.join(store_file_for(profile)),
                recipients: cand,
            });
        }
        if !dir.pop() {
            return Err(LayoutError::NoRecipientsFile);
        }
    }
}

/// The repo root (dir containing `recipients`), for enumerating profiles.
pub fn repo_root() -> Result<PathBuf, LayoutError> {
    let mut dir = env::current_dir().map_err(|e| LayoutError::Io(e.to_string()))?;
    loop {
        if dir.join(RECIPIENTS_FILE).is_file() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err(LayoutError::NoRecipientsFile);
        }
    }
}

/// List the profile names present in a repo (from `.envstow/*.enc`). Each `<name>.enc` is the
/// profile `<name>` (so `default.enc` → `default`). Returns a sorted, de-duplicated list.
pub fn list_profiles(root: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root.join(ENVSTOW_DIR)) {
        for e in entries.flatten() {
            let fname = e.file_name();
            let fname = fname.to_string_lossy();
            if let Some(stem) = fname.strip_suffix(".enc") {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

/// Path to the identity (private key) file: `$ENVSTOW_IDENTITY` or the per-user config path.
pub fn identity_path() -> PathBuf {
    if let Some(p) = env::var_os("ENVSTOW_IDENTITY") {
        return PathBuf::from(p);
    }
    let base = if cfg!(windows) {
        env::var_os("APPDATA").map(PathBuf::from)
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from("."))
        .join("envstow")
        .join("identity.txt")
}

/// Warn (once per invocation, to stderr) if the identity private key is readable by group or
/// other. envstow creates it `0600`, but permissions drift — a copy, a restore from backup, or a
/// loose umask can leave the key world-readable, and anyone who can read it decrypts every store
/// you can. We warn rather than refuse (unlike `ssh`) so a permission slip can't lock you out of
/// your own secrets; the message says exactly how to fix it. Never prints key contents.
#[cfg(unix)]
fn warn_if_identity_perms_loose(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(path) {
        let mode = meta.permissions().mode();
        // Any group/other bit set (0o077) means someone besides the owner can read it.
        if mode & 0o077 != 0 {
            eprintln!(
                "⚠️  envstow: your identity key is readable by others (mode {:o}) — {}\n\
                 \x20  Anyone who can read it can decrypt every store you have access to. Fix it:\n\
                 \x20    chmod 600 {}",
                mode & 0o777,
                path.display(),
                path.display()
            );
        }
    }
}

#[cfg(not(unix))]
fn warn_if_identity_perms_loose(_path: &Path) {
    // Windows: the key lives under %APPDATA%, which is already per-user; no POSIX mode to check.
}

/// Read the identity secret string (`AGE-SECRET-KEY-...`) from the identity file.
pub fn read_identity_secret() -> Result<String, LayoutError> {
    let path = identity_path();
    warn_if_identity_perms_loose(&path);
    let raw = fs::read_to_string(&path).map_err(|_| LayoutError::NoIdentity(path.clone()))?;
    // The file may be an age-keygen-style file with `# ` comment lines; take the first
    // AGE-SECRET-KEY line, else the first non-comment non-blank line.
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("AGE-SECRET-KEY-") {
            return Ok(t.to_string());
        }
    }
    for line in raw.lines() {
        let t = line.trim();
        if !t.is_empty() && !t.starts_with('#') {
            return Ok(t.to_string());
        }
    }
    Err(LayoutError::Empty("identity file"))
}

/// Write a new identity file with the given secret string, creating parent dirs. On Unix the
/// file is created mode 0600. Refuses to overwrite an existing identity.
pub fn write_new_identity(secret: &str) -> Result<PathBuf, LayoutError> {
    let path = identity_path();
    if path.exists() {
        return Err(LayoutError::Io(format!(
            "identity already exists at {} — refusing to overwrite",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| LayoutError::Io(e.to_string()))?;
    }
    let contents = format!("# envstow age identity — PRIVATE. Never commit or share.\n{secret}\n");
    fs::write(&path, contents).map_err(|e| LayoutError::Io(e.to_string()))?;
    set_owner_only(&path)?;
    Ok(path)
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<(), LayoutError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| LayoutError::Io(e.to_string()))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<(), LayoutError> {
    // Windows ACLs are not adjusted here; APPDATA is already per-user.
    Ok(())
}

/// Parse the recipients file text into ordered [`Recipient`] entries.
///
/// Format: one recipient per line, `age1...` optionally followed by `# Label`. Blank lines and
/// full-line `#` comments are ignored. Any line whose first token isn't `age1...` is skipped
/// (keeps the file forgiving of stray notes).
pub fn parse_recipients(text: &str) -> Vec<Recipient> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        // Split off an inline `# label` comment.
        let (keypart, labelpart) = match t.split_once('#') {
            Some((k, l)) => (k.trim(), Some(l.trim().to_string())),
            None => (t, None),
        };
        let key = keypart.split_whitespace().next().unwrap_or("");
        if !key.starts_with("age1") {
            continue;
        }
        out.push(Recipient {
            key: key.to_string(),
            label: labelpart.filter(|s| !s.is_empty()),
        });
    }
    out
}

/// Render recipients back to file text, preserving labels as trailing `# Label` comments.
pub fn render_recipients(recipients: &[Recipient]) -> String {
    let mut s = String::from(
        "# envstow recipients — age PUBLIC keys that can decrypt the store.\n\
         # One `age1...` per line; add a `# Name` label if you like.\n\
         # After editing, run `envstow reencrypt` (or add/remove-recipient) to re-key the store.\n",
    );
    for r in recipients {
        match &r.label {
            Some(l) => s.push_str(&format!("{}  # {}\n", r.key, l)),
            None => s.push_str(&format!("{}\n", r.key)),
        }
    }
    s
}

/// Read + parse the recipients file at `path`.
pub fn read_recipients(path: &Path) -> Result<Vec<Recipient>, LayoutError> {
    let text = fs::read_to_string(path).map_err(|e| LayoutError::Io(e.to_string()))?;
    Ok(parse_recipients(&text))
}

/// Read the encrypted store, verifying the format header and stripping it.
///
/// Returns the age ciphertext alone, so callers hand `crypto::decrypt` exactly what it expects.
/// The format check runs BEFORE any crypto, so a store from a newer envstow fails with a clear
/// "update your envstow" rather than a decryption error that reads like a permissions problem.
pub fn read_store(path: &Path) -> Result<Vec<u8>, LayoutError> {
    if !path.is_file() {
        return Err(LayoutError::NoStore);
    }
    let bytes = fs::read(path).map_err(|e| LayoutError::Io(e.to_string()))?;
    let (version, ciphertext) = split_format_header(&bytes)?;
    if version > FORMAT_VERSION {
        return Err(LayoutError::FormatTooNew { found: version });
    }
    Ok(ciphertext.to_vec())
}

/// Read just the format version of an existing store, without reading it as a store. Used by the
/// write guard, which must inspect a file it may be about to refuse. A store that doesn't exist
/// yet (init, `profile create`) has no format to conflict with.
fn store_format(path: &Path) -> Result<Option<u32>, LayoutError> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|e| LayoutError::Io(e.to_string()))?;
    let (version, _) = split_format_header(&bytes)?;
    Ok(Some(version))
}

/// Write the encrypted store with this binary's format header, creating `.envstow/` if needed.
///
/// Refuses to overwrite a store written in a NEWER format: an old binary re-encrypting a newer
/// store would silently downgrade it and break every teammate who has already updated. The read
/// guard alone can't catch this — by the time anyone reads it, the damage is committed.
pub fn write_store(path: &Path, ciphertext: &[u8]) -> Result<(), LayoutError> {
    if let Some(found) = store_format(path)? {
        if found > FORMAT_VERSION {
            return Err(LayoutError::FormatWouldDowngrade { found });
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| LayoutError::Io(e.to_string()))?;
    }
    let mut out = format!("{FORMAT_PREFIX}{FORMAT_VERSION}\n").into_bytes();
    out.extend_from_slice(ciphertext);
    fs::write(path, out).map_err(|e| LayoutError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_labeled_recipients() {
        let text = "# header comment\n\
                    age1aaa   # Alice\n\
                    age1bbb\n\
                    \n\
                    age1ccc # CI runner\n\
                    not-a-key should be skipped\n";
        let r = parse_recipients(text);
        assert_eq!(
            r,
            vec![
                Recipient {
                    key: "age1aaa".into(),
                    label: Some("Alice".into())
                },
                Recipient {
                    key: "age1bbb".into(),
                    label: None
                },
                Recipient {
                    key: "age1ccc".into(),
                    label: Some("CI runner".into())
                },
            ]
        );
    }

    #[test]
    fn render_then_parse_roundtrips() {
        let recips = vec![
            Recipient {
                key: "age1aaa".into(),
                label: Some("Alice".into()),
            },
            Recipient {
                key: "age1bbb".into(),
                label: None,
            },
        ];
        let text = render_recipients(&recips);
        assert_eq!(parse_recipients(&text), recips);
    }

    #[test]
    fn identity_path_respects_env_override() {
        // Save/restore so we don't disturb other tests' environment assumptions.
        let prev = env::var_os("ENVSTOW_IDENTITY");
        env::set_var("ENVSTOW_IDENTITY", "/tmp/custom-identity.txt");
        assert_eq!(identity_path(), PathBuf::from("/tmp/custom-identity.txt"));
        match prev {
            Some(v) => env::set_var("ENVSTOW_IDENTITY", v),
            None => env::remove_var("ENVSTOW_IDENTITY"),
        }
    }

    #[test]
    fn skips_blank_and_comment_lines() {
        assert!(parse_recipients("\n\n#only comments\n#age1notreal\n").is_empty());
    }

    #[test]
    fn headerless_store_is_format_1() {
        // Every store written before the header existed begins with age's own line. These must
        // keep working untouched — that's what makes the header a silent, migration-free rollout.
        let legacy = b"age-encryption.org/v1\n-----> X25519 abc\npayload";
        let (version, ciphertext) = split_format_header(legacy).unwrap();
        assert_eq!(version, 1, "absent header means format 1");
        assert_eq!(
            ciphertext, legacy,
            "ciphertext must be passed through whole"
        );
    }

    #[test]
    fn header_is_split_from_the_ciphertext() {
        let stored = b"envstow-format: 1\nage-encryption.org/v1\npayload";
        let (version, ciphertext) = split_format_header(stored).unwrap();
        assert_eq!(version, 1);
        assert_eq!(
            ciphertext, b"age-encryption.org/v1\npayload",
            "the age payload must come back byte-exact, header removed"
        );
    }

    #[test]
    fn a_newer_format_is_reported_not_guessed_at() {
        let future = b"envstow-format: 7\nage-encryption.org/v1\npayload";
        let (version, _) = split_format_header(future).unwrap();
        assert_eq!(
            version, 7,
            "parse must report the real version, not clamp it"
        );
        assert!(version > FORMAT_VERSION, "7 is newer than we understand");
    }

    #[test]
    fn malformed_headers_are_rejected() {
        // Truncated (no newline) and non-numeric versions are corruption, not a format we can
        // reason about — better to say so than to guess.
        for bad in [
            &b"envstow-format: 1"[..],
            &b"envstow-format: abc\npayload"[..],
            &b"envstow-format: \npayload"[..],
        ] {
            assert!(
                matches!(split_format_header(bad), Err(LayoutError::BadFormatHeader)),
                "should reject malformed header: {:?}",
                String::from_utf8_lossy(bad)
            );
        }
    }

    #[test]
    fn write_store_refuses_to_downgrade_a_newer_store() {
        // No CLI path reaches this today — set/delete both decrypt first, so the READ guard
        // fires before this one. It's a backstop: it makes downgrade-safety a property of the
        // layout layer, so a future command that writes without reading first can't silently
        // break a newer teammate's store. Tested here because only a unit test can reach it.
        let dir = env::temp_dir().join(format!("envstow-fmt-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let store = dir.join("future.enc");
        fs::write(&store, b"envstow-format: 42\nage-encryption.org/v1\n").unwrap();

        let err = write_store(&store, b"age-encryption.org/v1\nnew").unwrap_err();
        assert!(
            matches!(err, LayoutError::FormatWouldDowngrade { found: 42 }),
            "should refuse, got {err:?}"
        );
        assert_eq!(
            fs::read(&store).unwrap(),
            b"envstow-format: 42\nage-encryption.org/v1\n",
            "the refused write must leave the file untouched"
        );

        // A store at our own format is fine to overwrite, and gets the header back.
        let ours = dir.join("ours.enc");
        fs::write(&ours, format!("{FORMAT_PREFIX}{FORMAT_VERSION}\nold")).unwrap();
        write_store(&ours, b"age-encryption.org/v1\nnew").unwrap();
        assert_eq!(
            fs::read(&ours).unwrap(),
            format!("{FORMAT_PREFIX}{FORMAT_VERSION}\nage-encryption.org/v1\nnew").into_bytes()
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_errors_name_the_version_and_the_repo() {
        // The message IS the feature: it must say what to do, not just what went wrong.
        let too_new = LayoutError::FormatTooNew { found: 2 }.to_string();
        assert!(too_new.contains("format 2"), "names the found version");
        assert!(too_new.contains(REPO_URL), "points at the repo: {too_new}");

        let downgrade = LayoutError::FormatWouldDowngrade { found: 2 }.to_string();
        assert!(
            downgrade.contains("refusing to write"),
            "leads with refusal"
        );
        assert!(downgrade.contains(REPO_URL), "points at the repo");
    }
}
