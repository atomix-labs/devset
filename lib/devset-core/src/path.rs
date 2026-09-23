//! Managed paths, checked to stay inside their root on every supported filesystem.

use alloc::borrow::Cow;
use core::fmt;

use camino::{Utf8Path, Utf8PathBuf};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::errors::PathError;

/// Components no managed path may contain, compared by [`RelPath::fold`].
///
/// Repository internals, their NTFS short name, and devset's own state.
const RESERVED: [&str; 3] = [".git", "git~1", ".devset"];

/// Characters some supported filesystem cannot store.
const UNPORTABLE: [char; 8] = ['\\', ':', '*', '?', '"', '<', '>', '|'];

/// Code points HFS+ ignores in names, so `.g\u{200C}it` is `.git` there. Git's list.
const IGNORABLE: [char; 15] = [
    '\u{200C}', '\u{200D}', '\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}',
    '\u{202E}', '\u{206A}', '\u{206B}', '\u{206C}', '\u{206D}', '\u{206E}', '\u{FEFF}',
];

/// A path a profile manages, relative to the target root.
///
/// `/`-separated; no empty, `.` or `..` components; no component ending in `.` or a space; only
/// characters every supported filesystem can store; and no component that folds to `.git` or
/// `.devset`. Anything built from a `RelPath` stays inside its root.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(Box<str>);

impl RelPath {
    /// `path`, checked against the rules above.
    ///
    /// # Errors
    /// [`PathError`], `path` breaks one of them.
    pub fn new(path: &str) -> Result<Self, PathError> {
        Self::try_from(String::from(path))
    }

    /// The path as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// This path resolved under `root`.
    #[must_use]
    pub fn under(&self, root: &Utf8Path) -> Utf8PathBuf {
        let mut path = root.to_path_buf();
        path.extend(self.0.split('/'));
        path
    }

    /// The key two paths share when a case- or normalization-insensitive filesystem merges them.
    #[must_use]
    pub fn fold(&self) -> String {
        fold(&self.0)
    }
}

/// NFC, lowercase, HFS+ ignorables removed.
fn fold(s: &str) -> String {
    s.nfc().flat_map(char::to_lowercase).filter(|c| !IGNORABLE.contains(c)).collect()
}

/// The first rule `path` breaks.
fn check(path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("empty");
    }
    if path.starts_with('/') {
        return Err("absolute");
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err("empty, `.` or `..` component");
        }
        if part.ends_with(['.', ' ']) {
            return Err("component ends in `.` or a space");
        }
        if part.chars().any(|c| c.is_control() || UNPORTABLE.contains(&c)) {
            return Err("unportable character");
        }
        if RESERVED.contains(&fold(part).as_str()) {
            return Err("reserved component");
        }
    }
    Ok(())
}

impl TryFrom<String> for RelPath {
    type Error = PathError;

    fn try_from(path: String) -> Result<Self, PathError> {
        match check(&path) {
            Ok(()) => Ok(Self(path.into_boxed_str())),
            Err(rule) => Err(PathError { path, rule }),
        }
    }
}

impl AsRef<str> for RelPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<RelPath> for String {
    fn from(path: RelPath) -> Self {
        path.0.into_string()
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl JsonSchema for RelPath {
    fn schema_name() -> Cow<'static, str> {
        "RelPath".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        String::json_schema(generator)
    }

    fn inline_schema() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::RelPath;

    #[test]
    fn accepts() {
        for path in ["a", ".github/workflows/ci.yml", ".gitignore", "a.b/c-d_e", "devset/x"] {
            assert!(RelPath::new(path).is_ok(), "{path} should be accepted");
        }
    }

    #[test]
    fn rejects() {
        for path in [
            "",
            "/a",
            "a//b",
            "a/",
            "./a",
            "a/../b",
            "a.",
            "a ",
            "a\\b",
            "c:x",
            "a\nb",
            ".git",
            "x/.GIT/config",
            "x/.g\u{200C}it",
            "GIT~1/x",
            ".devset/state.toml",
            "x/.DevSet",
        ] {
            assert!(RelPath::new(path).is_err(), "{path:?} should be rejected");
        }
    }

    #[test]
    fn fold_collides_case_and_normalization() {
        let fold = |s: &str| RelPath::new(s).map(|p| p.fold()).ok();
        assert_eq!(fold("README.md"), fold("readme.md"), "case must fold");
        assert_eq!(fold("caf\u{E9}"), fold("cafe\u{301}"), "NFC and NFD must fold together");
    }
}
