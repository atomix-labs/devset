//! Comparing a target with its resolved profile, path by path.

use alloc::borrow::Cow;
use alloc::collections::{BTreeMap, BTreeSet};
use core::fmt;
use std::io;

pub use crate::digest::{Digest, Fingerprint};
use crate::errors::{Result, TargetError};
use crate::format::Format;
pub use crate::part::Scope;
use crate::part::{Key, Shape, Slot, decode};
use crate::path::RelPath;
use crate::plan::Mode;
use crate::profile::Policy;
use crate::resolve::Resolved;
use crate::target::{Record, Target, read_optional};

/// One managed file, or one profile's part of it: what the profile wants, what devset recorded,
/// what is on disk.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The file.
    pub path: RelPath,
    /// The part of the file this entry is; `None` for the whole file.
    pub part: Option<Part>,
    /// What the profile provides; `None` once no layer provides it.
    pub want: Option<Want>,
    /// What devset last recorded.
    pub record: Option<Fingerprint>,
    /// The policy it was recorded under; `None` without a record.
    pub recorded: Option<Policy>,
    /// What is on disk: the file, or what it holds of the part.
    pub found: Option<Fingerprint>,
    /// Whether a conflicted merge of the file waits in `.devset/conflicts/`.
    pub conflict: bool,
    /// For keys, every key the part covers: its base's and the profile's.
    pub(crate) keys: BTreeSet<Key>,
}

/// One profile's part of a file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Part {
    /// The profile whose part it is, which names it.
    pub owner: String,
    /// How much of the file it is: [`Scope::Keys`] or [`Scope::Block`].
    pub scope: Scope,
}

/// What a layer provides at one path, or in one part.
#[derive(Clone, Copy, Debug)]
pub struct Want {
    /// Index into the resolved layers.
    pub layer: usize,
    /// How the file, or the part, is managed.
    pub policy: Policy,
    /// How a merge of the file is checked.
    pub validate: Format,
    /// Whether the file is written executable.
    pub executable: bool,
    /// The profile's bytes, or the part's content.
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
    /// How much of the file this entry is.
    #[must_use]
    pub fn scope(&self) -> Scope {
        self.part.as_ref().map_or(Scope::File, |part| part.scope)
    }

    /// Where it lives.
    pub(crate) fn slot(&self) -> Slot {
        Slot {
            path: self.path.clone(),
            part: self.part.as_ref().map(|part| part.owner.clone()),
        }
    }

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

/// The path, and for a part its scope and owner: `Cargo.toml [keys: lints]`.
impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.part {
            Some(Part { owner, scope }) => write!(f, "{} [{}: {owner}]", self.path, scope.as_str()),
            None => write!(f, "{}", self.path),
        }
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

    /// Whether an update is unfinished: conflicts wait in `.devset/conflicts/`.
    #[must_use]
    pub fn unfinished(&self) -> bool {
        self.entries.iter().any(|entry| entry.conflict)
    }

    /// `entry`'s file as the profile would have it: the profile's file, or for a part, the file
    /// in `target` with the part as the profile has it; `None` when no layer provides it.
    ///
    /// # Errors
    /// - [`Error::Parse`](crate::Error::Parse), the part cannot be written into its file.
    /// - [`Error::Io`](crate::Error::Io), the file cannot be read.
    pub fn wanted(&self, entry: &Entry, target: &Target) -> Result<Option<Vec<u8>>> {
        if entry.want.is_none() {
            return Ok(None);
        }
        let payload = self.resolved.payload(entry)?;
        let Some(portion) = self.resolved.portion(entry) else {
            return Ok(Some(payload.to_vec()));
        };
        let file = read(target, &entry.path)?.unwrap_or_default();
        Ok(Some(portion.splice(
            &file,
            payload,
            &entry.keys,
            entry.path.as_str(),
        )?))
    }

    /// Whether `apply --force` would write a file: the drift gate.
    #[must_use]
    pub fn drifted(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.action(Mode::Force).writes())
    }
}

/// Compares `target`'s files with `resolved`.
///
/// A part is read out of its file: the keys its base and the profile define, or its block.
///
/// # Errors
/// - [`TargetError::NotAFile`], a managed path on disk is a symlink or a directory.
/// - [`Error::Parse`](crate::Error::Parse), a file with parts does not parse, or a block's markers
///   do not pair.
/// - [`Error::Io`](crate::Error::Io), a managed file cannot be read.
pub fn survey(resolved: Resolved, target: &Target) -> Result<Survey> {
    let mut wants: BTreeMap<&Slot, (Scope, Want, &[u8])> = resolved
        .provided()
        .map(|(slot, provided, bytes)| {
            let want = Want {
                layer: provided.layer,
                policy: provided.policy,
                validate: provided.validate,
                executable: provided.executable,
                fingerprint: Fingerprint::of(bytes),
            };
            let scope = provided
                .portion
                .as_ref()
                .map_or(Scope::File, |portion| portion.scope);
            (slot, (scope, want, bytes))
        })
        .collect();
    let mut entries = Vec::new();
    for (slot, record) in target.records() {
        let wanted = wants.remove(&slot);
        let now = wanted.map_or(record.scope, |(now, ..)| now);
        // A part the profile now owns in another scope has other content: its record is no base.
        let record = (now == record.scope).then_some(record);
        entries.push(entry(
            slot,
            now,
            wanted.map(|(_, want, bytes)| (want, bytes)),
            record,
        ));
    }
    for (slot, (scope, want, bytes)) in wants {
        entries.push(entry(slot.clone(), scope, Some((want, bytes)), None));
    }
    entries.sort_unstable_by(|a, b| (&a.path, &a.part).cmp(&(&b.path, &b.part)));
    let mut file: Option<(RelPath, Option<Vec<u8>>)> = None;
    for entry in &mut entries {
        if file.as_ref().is_none_or(|(path, _)| *path != entry.path) {
            file = Some((entry.path.clone(), read(target, &entry.path)?));
        }
        let Some((_, bytes)) = &file else { continue };
        entry.conflict = target.sidecar(&entry.path).is_file();
        let Some(bytes) = bytes else { continue };
        let Some(part) = &entry.part else {
            entry.found = Some(Fingerprint::of(bytes));
            continue;
        };
        let shape = match resolved.portion(entry) {
            Some(portion) => Cow::Borrowed(&portion.shape),
            None => match released(target, entry) {
                Some(shape) => Cow::Owned(shape),
                None => continue,
            },
        };
        if part.scope == Scope::Keys
            && let Some(record) = &entry.record
        {
            let base = target.base(record)?.unwrap_or_default();
            entry.keys.extend(shape.keys(&base));
        }
        let view = shape.view(bytes, &entry.keys, entry.path.as_str())?;
        entry.found = view.as_deref().map(Fingerprint::of);
    }
    Ok(Survey { resolved, entries })
}

/// The shape of a part no layer provides any more, as its path tells: its format by name or by
/// the target's override, its markers in the file type's comment syntax. `None` when the path does
/// not tell, as for a block in a comment syntax the profile chose.
pub(crate) fn released(target: &Target, entry: &Entry) -> Option<Shape> {
    let part = entry.part.as_ref()?;
    let format = target
        .config()
        .files
        .get(&entry.path)
        .and_then(|over| over.validate);
    let format = format.unwrap_or_else(|| Format::of(&entry.path));
    Shape::of(part.scope, &entry.path, format, None, &part.owner)
        .ok()
        .flatten()
}

/// The entry at `slot`, in `scope`, before the disk is read.
fn entry(slot: Slot, scope: Scope, want: Option<(Want, &[u8])>, record: Option<Record>) -> Entry {
    let Slot { path, part } = slot;
    let part = part.map(|owner| Part { owner, scope });
    let keys = match (&part, want) {
        (
            Some(Part {
                scope: Scope::Keys, ..
            }),
            Some((_, content)),
        ) => decode(content).map(|leaves| leaves.into_keys().collect()),
        _ => None,
    };
    Entry {
        path,
        part,
        want: want.map(|(want, _)| want),
        record: record.map(|record| record.fingerprint),
        recorded: record.map(|record| record.policy),
        found: None,
        conflict: false,
        keys: keys.unwrap_or_default(),
    }
}

/// The regular file at `path` in `target`; `None` if absent.
fn read(target: &Target, path: &RelPath) -> Result<Option<Vec<u8>>> {
    let file = path.under(target.root());
    match fs_err::symlink_metadata(&file) {
        Ok(meta) if meta.is_file() => read_optional(&file),
        Ok(meta) => {
            let kind = if meta.is_symlink() {
                "a symlink"
            } else if meta.is_dir() {
                "a directory"
            } else {
                "a special file"
            };
            Err(TargetError::NotAFile {
                path: path.clone(),
                kind,
            }
            .into())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
