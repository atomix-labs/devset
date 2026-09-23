//! Comparing a target with its resolved profile, path by path.

use alloc::collections::BTreeMap;
use std::io::{self, Read};

use camino::Utf8Path;

pub use crate::digest::{Digest, Fingerprint};
use crate::errors::{Result, TargetError};
use crate::format::Format;
use crate::path::RelPath;
use crate::plan::Mode;
use crate::profile::Policy;
use crate::resolve::{Provided, Resolved};
use crate::target::Target;

/// One managed path: what the profile wants, what devset recorded, what is on disk.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The path.
    pub path: RelPath,
    /// What the profile provides; `None` once no layer provides the path.
    pub want: Option<Want>,
    /// What devset last recorded.
    pub record: Option<Fingerprint>,
    /// What is on disk.
    pub found: Option<Fingerprint>,
    /// Whether a conflicted merge waits in `.devset/conflicts/`.
    pub conflict: bool,
}

/// What a layer provides at one path.
#[derive(Clone, Copy, Debug)]
pub struct Want {
    /// Index into the resolved layers.
    pub layer: usize,
    /// How the file is managed.
    pub policy: Policy,
    /// How a merge of it is checked.
    pub validate: Format,
    /// The profile's bytes.
    pub fingerprint: Fingerprint,
}

/// How a file on disk compares with its record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drift {
    /// Byte-identical.
    Unchanged,
    /// Differs only in whitespace, line endings or a BOM.
    Cosmetic,
    /// Edited.
    Edited,
    /// Deleted.
    Missing,
}

impl Entry {
    /// How the file compares with its record; `None` when there is no record.
    #[must_use]
    pub fn drift(&self) -> Option<Drift> {
        let record = self.record?;
        Some(match self.found {
            None => Drift::Missing,
            Some(found) if found.exact == record.exact => Drift::Unchanged,
            Some(found) if found.same_content(&record) => Drift::Cosmetic,
            Some(_) => Drift::Edited,
        })
    }
}

/// A target's managed paths against its resolved profile, in path order.
#[derive(Debug)]
pub struct Survey {
    /// The profile surveyed against.
    pub(crate) resolved: Resolved,
    /// Every provided or recorded path.
    pub(crate) entries: Vec<Entry>,
}

impl Survey {
    /// The profile surveyed against.
    #[must_use]
    pub const fn resolved(&self) -> &Resolved {
        &self.resolved
    }

    /// Every provided or recorded path, in order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Whether `apply --force` would write a file: the drift gate.
    #[must_use]
    pub fn drifted(&self) -> bool {
        self.entries.iter().any(|entry| entry.action(Mode::Force).writes())
    }
}

/// Compares `target`'s files with `resolved`.
///
/// # Errors
/// - [`TargetError::NotAFile`], a managed path on disk is a symlink or a directory.
/// - [`Error::Io`](crate::Error::Io), a managed file cannot be read.
pub fn survey(resolved: Resolved, target: &Target) -> Result<Survey> {
    let mut wants: BTreeMap<&RelPath, Want> = resolved
        .provided()
        .map(|(path, provided, bytes)| {
            let Provided { layer, policy, validate } = provided;
            (path, Want { layer, policy, validate, fingerprint: Fingerprint::of(bytes) })
        })
        .collect();
    let mut entries: Vec<Entry> = target
        .records()
        .map(|(path, record)| Entry {
            path: path.clone(),
            want: wants.remove(path),
            record: Some(*record),
            found: None,
            conflict: false,
        })
        .collect();
    entries.extend(wants.into_iter().map(|(path, want)| Entry {
        path: path.clone(),
        want: Some(want),
        record: None,
        found: None,
        conflict: false,
    }));
    entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    let mut buf = Vec::new();
    for entry in &mut entries {
        entry.found = fingerprint(&entry.path.under(target.root()), &mut buf)?;
        entry.conflict = target.sidecar(&entry.path).is_file();
    }
    Ok(Survey { resolved, entries })
}

/// Fingerprints the regular file at `file`, reading it into `buf`; `None` if absent.
fn fingerprint(file: &Utf8Path, buf: &mut Vec<u8>) -> Result<Option<Fingerprint>> {
    match fs_err::symlink_metadata(file) {
        Ok(meta) if meta.is_file() => {
            buf.clear();
            fs_err::File::open(file)?.read_to_end(buf)?;
            Ok(Some(Fingerprint::of(buf)))
        },
        Ok(_) => Err(TargetError::NotAFile { path: file.to_owned() }.into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
