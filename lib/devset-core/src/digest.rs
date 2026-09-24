//! Content digests and file fingerprints.

use derive_more::{Display, FromStr};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// Leading bytes git inspects to decide a file is binary.
const BINARY_SNIFF: usize = 8000;

/// UTF-8 byte order mark.
pub(crate) const BOM: &[u8] = b"\xEF\xBB\xBF";

/// A BLAKE3 digest, written and serialized as lowercase hex.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Display,
    derive_more::Debug,
    FromStr,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[debug("{_0}")]
pub struct Digest(blake3::Hash);

impl Digest {
    /// Digest of `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(blake3::hash(bytes))
    }

    /// Wraps a finished hash.
    pub(crate) const fn new(hash: blake3::Hash) -> Self {
        Self(hash)
    }

    /// Raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

/// Exact and canonical digests of one file; see [`Fingerprint::of`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// Digest of the bytes as they are.
    pub exact: Digest,
    /// Digest of the canonical form.
    pub canonical: Digest,
}

impl Fingerprint {
    /// Fingerprints `bytes`, reading them once.
    ///
    /// The canonical form drops a UTF-8 BOM, strips trailing whitespace from every line, drops
    /// trailing blank lines and ends in exactly one `\n`. Binary content, git's test of a NUL in
    /// the first 8000 bytes, is its own canonical form.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        let exact = Digest::of(bytes);
        let canonical = if is_binary(bytes) {
            exact
        } else {
            let mut hasher = blake3::Hasher::new();
            canonical(bytes, |part| {
                hasher.update(part);
            });
            Digest(hasher.finalize())
        };
        Self { exact, canonical }
    }

    /// Whether `self` and `other` differ at most cosmetically.
    #[must_use]
    pub fn same_content(&self, other: &Self) -> bool {
        self.canonical == other.canonical
    }
}

/// Git's binary heuristic.
pub(crate) fn is_binary(bytes: &[u8]) -> bool {
    bytes.get(..BINARY_SNIFF).unwrap_or(bytes).contains(&0)
}

/// Feeds the canonical form of text `bytes` to `emit` as borrowed slices; nothing is copied.
fn canonical<F: FnMut(&[u8])>(bytes: &[u8], mut emit: F) {
    let text = bytes.strip_prefix(BOM).unwrap_or(bytes);
    let mut blank = 0_usize;
    let mut started = false;
    for line in text.split(|&b| b == b'\n').map(<[u8]>::trim_ascii_end) {
        if line.is_empty() {
            blank = blank.saturating_add(1);
            continue;
        }
        if started {
            emit(b"\n");
        }
        for _ in 0..blank {
            emit(b"\n");
        }
        emit(line);
        started = true;
        blank = 0;
    }
    if started {
        emit(b"\n");
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{Fingerprint, canonical};

    fn canon(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        canonical(bytes, |part| out.extend_from_slice(part));
        out
    }

    #[test]
    fn canonical_form() {
        for (input, want) in [
            (&b""[..], &b""[..]),
            (b" \n\t\n", b""),
            (b"a", b"a\n"),
            (b"a\n\n\n", b"a\n"),
            (b"a  \r\nb\t\r\n", b"a\nb\n"),
            (b"\xEF\xBB\xBFa\n", b"a\n"),
            (b"\n\na\n\n b\n", b"\n\na\n\n b\n"),
            (b"a\rb\n", b"a\rb\n"),
        ] {
            assert_eq!(
                canon(input),
                want,
                "input {:?}",
                String::from_utf8_lossy(input)
            );
        }
    }

    #[test]
    fn binary_is_exact() {
        let bytes = b"a \0 b  \r\n";
        let fp = Fingerprint::of(bytes);
        assert_eq!(
            fp.exact, fp.canonical,
            "binary content must not be canonicalized"
        );
    }

    proptest! {
        #[test]
        fn idempotent(text in "[a-z \t\r\n]{0,64}") {
            let once = canon(text.as_bytes());
            prop_assert_eq!(canon(&once), once);
        }

        #[test]
        fn cosmetic_changes_keep_content(text in "[a-z]{1,8}(\n[a-z ]{0,8}){0,8}") {
            let noisy = format!("\u{FEFF}{}  \r\n\n\n", text.replace('\n', " \t\r\n"));
            let (a, b) = (Fingerprint::of(text.as_bytes()), Fingerprint::of(noisy.as_bytes()));
            prop_assert!(a.same_content(&b), "{text:?} vs {noisy:?}");
        }

        #[test]
        fn real_changes_are_seen(
            (text, at) in "[a-z]{1,16}".prop_flat_map(|t| { let n = t.len(); (Just(t), 0..n) })
        ) {
            let mut edited = text.clone().into_bytes();
            edited[at] = b'Z';
            let (a, b) = (Fingerprint::of(text.as_bytes()), Fingerprint::of(&edited));
            prop_assert!(!a.same_content(&b), "{text:?} vs {:?}", String::from_utf8_lossy(&edited));
        }
    }
}
