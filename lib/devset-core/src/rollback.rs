//! Taking back an unfinished update, from what [`commit`](crate::commit()) saved beforehand.

use alloc::collections::BTreeMap;
use core::fmt;
use std::collections::HashSet;
use std::io;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::commit::{lock, prune, remove_if_present, write};
use crate::digest::Digest;
use crate::errors::{MergeError, Result};
use crate::path::RelPath;
use crate::target::{
    ANSWERS, CONFIG, CONFLICTS, DIR, LOCK, STATE, State, Target, UNDO, UNDO_BLOBS, V1, from_toml,
    label, read_optional,
};

/// devset's own files an update may change, in the order they are restored.
///
/// `state.toml` goes last, so an interrupted rollback still describes the update and runs again.
const RECORDS: [&str; 4] = [CONFIG, ANSWERS, LOCK, STATE];

/// A file an unfinished update changed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Location {
    /// A managed file in the target.
    File(RelPath),
    /// A conflicted merge in `.devset/conflicts/`.
    Sidecar(RelPath),
    /// One of devset's own files in `.devset/`.
    Record(&'static str),
}

impl Location {
    /// Where it is under `target`.
    fn under(&self, target: &Target) -> Utf8PathBuf {
        match self {
            Self::File(path) => path.under(target.root()),
            Self::Sidecar(path) => target.sidecar(path),
            Self::Record(name) => target.dir().join(name),
        }
    }

    /// The location a key of `undo.toml` names, if it names one.
    fn parse(key: &str) -> Option<Self> {
        let Some(inner) = key.strip_prefix(".devset/") else {
            return RelPath::new(key).ok().map(Self::File);
        };
        if let Some(path) = inner.strip_prefix("conflicts/") {
            return RelPath::new(path).ok().map(Self::Sidecar);
        }
        RECORDS
            .into_iter()
            .find(|&name| name == inner)
            .map(Self::Record)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(path) => write!(f, "{path}"),
            Self::Sidecar(path) => write!(f, "{DIR}/{CONFLICTS}/{path}"),
            Self::Record(name) => write!(f, "{DIR}/{name}"),
        }
    }
}

/// One file's change: what it held before the update, and what the update left in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Change {
    /// Digest of the bytes before; `None` when the file did not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    before: Option<Digest>,
    /// Digest of the bytes the update wrote; `None` when it removed the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    after: Option<Digest>,
}

/// `undo.toml`: every file an unfinished update changed, the bytes it replaced saved beside it.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Undo {
    /// Format version.
    version: V1,
    /// Each change, keyed by the file's path relative to the target root.
    #[serde(default)]
    files: BTreeMap<String, Change>,
}

impl Undo {
    /// The undo `target` holds, or an empty one.
    pub(crate) fn load(target: &Target) -> Result<Self> {
        let file = target.unfinished().join(UNDO);
        let label = label(&format!("{CONFLICTS}/{DIR}/{UNDO}"));
        read_optional(&file)?.map_or_else(|| Ok(Self::default()), |raw| from_toml(&raw, &label))
    }

    /// Notes that `location` is about to hold `bytes`, or be removed.
    ///
    /// The first time, saves what it holds now.
    pub(crate) fn record(
        &mut self,
        target: &Target,
        location: &Location,
        bytes: Option<&[u8]>,
    ) -> Result<()> {
        let after = bytes.map(Digest::of);
        let key = location.to_string();
        if let Some(change) = self.files.get_mut(&key) {
            change.after = after;
            if change.before == after {
                self.files.remove(&key);
            }
            return Ok(());
        }
        let current = read_optional(&location.under(target))?;
        let before = current.as_deref().map(Digest::of);
        if before == after {
            return Ok(());
        }
        if let (Some(current), Some(digest)) = (&current, before) {
            let blob = target
                .unfinished()
                .join(UNDO_BLOBS)
                .join(digest.to_string());
            if !blob.is_file() {
                write(&blob, current)?;
            }
        }
        self.files.insert(key, Change { before, after });
        Ok(())
    }

    /// Writes `undo.toml`.
    pub(crate) fn save(&self, target: &Target) -> Result<()> {
        let text = toml::to_string(self).map_err(io::Error::other)?;
        write(&target.unfinished().join(UNDO), text.as_bytes())
    }

    /// Each change, by where it is.
    fn changes(&self) -> impl Iterator<Item = (Location, Change)> + '_ {
        self.files
            .iter()
            .filter_map(|(key, &change)| Some((Location::parse(key)?, change)))
    }
}

/// What taking a target file back does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Revert {
    /// Puts back the bytes it held before the update.
    Restore(RelPath),
    /// Removes it: the update created it.
    Remove(RelPath),
}

/// An unfinished update, and what taking it back does.
///
/// Nothing is written until [`apply`](Self::apply).
/// An update is unfinished while conflicts wait in `.devset/conflicts/`. Taking it back returns
/// every file it wrote, and `.devset/`'s records, to what they held before it, and discards its
/// conflicts, as though it had not run.
#[derive(Debug)]
pub struct Rollback {
    /// What the update changed.
    undo: Undo,
    /// What taking it back does to target files, in path order.
    reverts: Vec<Revert>,
    /// Files changed since the update, which taking it back would discard.
    changed: Vec<String>,
}

impl Rollback {
    /// The unfinished update in `target`.
    ///
    /// # Errors
    /// - [`MergeError::NothingToAbort`], no update is unfinished.
    /// - [`Error::Parse`](crate::Error::Parse), what the update saved does not parse.
    pub fn of(target: &Target) -> Result<Self> {
        if !target.dir().join(CONFLICTS).is_dir() {
            return Err(MergeError::NothingToAbort.into());
        }
        let undo = Undo::load(target)?;
        let (mut reverts, mut changed) = (Vec::new(), Vec::new());
        for (location, change) in undo.changes() {
            let now = read_optional(&location.under(target))?
                .as_deref()
                .map(Digest::of);
            // Already as before: never written, or restored by an interrupted rollback.
            if now == change.before {
                continue;
            }
            if now != change.after {
                changed.push(location.to_string());
            }
            if let Location::File(path) = location {
                reverts.push(match change.before {
                    Some(_) => Revert::Restore(path),
                    None => Revert::Remove(path),
                });
            }
        }
        Ok(Self {
            undo,
            reverts,
            changed,
        })
    }

    /// What taking the update back does to target files, in path order.
    #[must_use]
    pub fn reverts(&self) -> &[Revert] {
        &self.reverts
    }

    /// Files changed since the update, which taking it back discards, from the target root.
    #[must_use]
    pub fn changed(&self) -> &[String] {
        &self.changed
    }

    /// Takes the update back in `target`, returning what it did to target files.
    ///
    /// Target files and sidecars go first, then `.devset/`'s records with `state.toml` last, then
    /// `.devset/conflicts/`; an interruption leaves a rollback that runs again.
    ///
    /// # Errors
    /// - [`MergeError::ChangedSince`], files changed since the update and `force` is off; nothing
    ///   is written.
    /// - [`TargetError::Busy`](crate::TargetError::Busy) or
    ///   [`TargetError::Concurrent`](crate::TargetError::Concurrent), another devset is at work.
    /// - [`MergeError::CorruptUndo`], a saved copy is missing or damaged; what was restored before
    ///   it stays restored.
    pub fn apply(self, target: &Target, force: bool) -> Result<Vec<Revert>> {
        if !force && !self.changed.is_empty() {
            return Err(MergeError::ChangedSince {
                paths: self.changed,
            }
            .into());
        }
        let _guard = lock(target)?;
        let mut changes: Vec<(Location, Change)> = self.undo.changes().collect();
        // `Location`'s order puts files, then sidecars, then records; records go in `RECORDS`
        // order.
        changes.sort_by_key(|(location, _)| match location {
            Location::Record(name) => RECORDS.iter().position(|record| record == name),
            Location::File(_) | Location::Sidecar(_) => None,
        });
        let blobs = target.unfinished().join(UNDO_BLOBS);
        for (location, change) in &changes {
            let file = location.under(target);
            match change.before {
                None => remove_if_present(&file)?,
                Some(digest) => {
                    let saved = read_optional(&blobs.join(digest.to_string()))?
                        .filter(|bytes| Digest::of(bytes) == digest)
                        .ok_or_else(|| MergeError::CorruptUndo {
                            path: location.to_string(),
                        })?;
                    write(&file, &saved)?;
                }
            }
        }
        let state = read_optional(&target.dir().join(STATE))?;
        let state: State = state.map_or_else(
            || Ok(State::default()),
            |raw| from_toml(&raw, &label(STATE)),
        )?;
        let live: HashSet<Digest> = state.bases().collect();
        prune(target, &live)?;
        remove_if_present(&target.dir().join(CONFLICTS))?;
        Ok(self.reverts)
    }
}

#[cfg(test)]
mod tests {
    use super::Location;
    use crate::path::RelPath;

    #[test]
    fn locations_round_trip_through_their_keys() {
        let path = || RelPath::new(".github/ci.yml").expect("a valid path");
        for location in [
            Location::File(path()),
            Location::Sidecar(path()),
            Location::Record("lock.toml"),
        ] {
            assert_eq!(
                Location::parse(&location.to_string()),
                Some(location),
                "as written"
            );
        }
        for key in [
            ".devset/base/x",
            ".devset/conflicts/.devset/undo.toml",
            "../escape",
        ] {
            assert_eq!(Location::parse(key), None, "{key} names no location");
        }
    }
}
