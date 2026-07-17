//! The in-memory secret store: an ordered list of `(name, value)` pairs, decrypted from the age
//! ciphertext for the lifetime of one command.
//!
//! The whole point of this type is that **plaintext values are zeroized on drop**. Before it,
//! every command hand-wrote `for (_, v) in vars.iter_mut() { v.zeroize() }` at each early return —
//! a security-critical invariant enforced by copy-paste, and exactly the kind of thing that rots.
//! Here it's automatic: however a `Secrets` goes out of scope (normal return, `?`, panic unwind),
//! its values are scrubbed. Commands get named operations (`get`/`upsert`/`remove`) instead of
//! open-coded vec surgery, and never touch zeroize directly.

use zeroize::Zeroize;

/// An ordered set of secrets (name → value) held in memory. Order is preserved so the on-disk
/// dotenv store stays stable across writes. Values are zeroized when the `Secrets` drops.
#[derive(Default)]
pub struct Secrets {
    vars: Vec<(String, String)>,
}

impl Secrets {
    /// Wrap already-decrypted `(name, value)` pairs (from `crypto::parse_dotenv` + decode).
    pub fn from_pairs(vars: Vec<(String, String)>) -> Self {
        Self { vars }
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// The secret names, in store order. Names are not secret — `list` prints them.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.vars.iter().map(|(k, _)| k.as_str())
    }

    /// `(name, value)` pairs, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.vars.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// The raw pairs, for rendering to dotenv. Callers must not copy a value out unscrubbed.
    pub fn pairs(&self) -> &[(String, String)] {
        &self.vars
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.vars
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.vars.iter().any(|(k, _)| k == name)
    }

    /// Insert `name`, or replace its value if it already exists. A replaced value is zeroized
    /// before being overwritten, so no stale plaintext lingers.
    pub fn upsert(&mut self, name: &str, value: String) {
        match self.vars.iter_mut().find(|(k, _)| k == name) {
            Some((_, v)) => {
                v.zeroize();
                *v = value;
            }
            None => self.vars.push((name.to_string(), value)),
        }
    }

    /// Keep only the named secrets (store order preserved), zeroizing every value dropped.
    /// Callers validate `names` against the store BEFORE scoping — this just applies the scope.
    pub fn retain_only(&mut self, names: &[String]) {
        self.vars.retain_mut(|(k, v)| {
            if names.iter().any(|n| n == k) {
                true
            } else {
                v.zeroize();
                false
            }
        });
    }

    /// Remove `name`, zeroizing its value as it leaves. Returns whether it was present.
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(i) = self.vars.iter().position(|(k, _)| k == name) {
            let (_, mut value) = self.vars.remove(i);
            value.zeroize();
            true
        } else {
            false
        }
    }
}

impl Drop for Secrets {
    fn drop(&mut self) {
        for (_, v) in &mut self.vars {
            v.zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_inserts_then_replaces() {
        let mut s = Secrets::default();
        s.upsert("A", "1".into());
        s.upsert("B", "2".into());
        assert_eq!(s.get("A"), Some("1"));
        s.upsert("A", "9".into());
        assert_eq!(s.get("A"), Some("9"));
        assert_eq!(s.names().count(), 2, "replace must not add a second entry");
        // Order preserved: A stays first.
        assert_eq!(s.names().collect::<Vec<_>>(), vec!["A", "B"]);
    }

    #[test]
    fn retain_only_scopes_in_store_order() {
        let mut s = Secrets::from_pairs(vec![
            ("A".into(), "1".into()),
            ("B".into(), "2".into()),
            ("C".into(), "3".into()),
        ]);
        // Request order must not matter; store order is what survives.
        s.retain_only(&["C".into(), "A".into()]);
        assert_eq!(s.names().collect::<Vec<_>>(), vec!["A", "C"]);
        assert!(!s.contains("B"));
    }

    #[test]
    fn remove_reports_presence() {
        let mut s = Secrets::from_pairs(vec![("A".into(), "1".into())]);
        assert!(s.remove("A"));
        assert!(!s.remove("A"), "second remove: already gone");
        assert!(!s.contains("A"));
        assert!(s.is_empty());
    }
}
