//! Carrying out a [`Plan`]: every file, blob and record written atomically, state last.

use alloc::collections::{BTreeMap, BTreeSet};
use std::collections::HashSet;
use std::fs::TryLockError;
use std::io::{self, Write};

use atomic_write_file::AtomicWriteFile;
use camino::Utf8Path;
use serde::Serialize;

use crate::digest::Digest;
use crate::errors::{Result, TargetError};
use crate::path::RelPath;
use crate::plan::{Action, Plan, Step};
use crate::resolve::Resolved;
use crate::target::{
    ANSWERS, BASE, CONFIG, CONFLICTS, LOCK, Lock, Locked, PENDING, STATE, State, Target,
    read_optional,
};
use crate::tree::Tree;

/// Held in `.devset/` while committing.
const LOCKFILE: &str = ".lock";

/// `.devset/.gitignore`: local work in progress.
///
/// Anchored, so it matches devset's own entries and nothing else.
const GITIGNORE: &str = "/conflicts/\n/pending.toml\n/.lock\n";

/// `.devset/.gitattributes`: keep blobs out of diffs, and away from end-of-line translation.
const GITATTRIBUTES: &str = "base/** -diff -text\n";

/// Carries out `plan` in `target`, returning its steps.
///
/// Everything is decided before anything is written; then files, blobs and `.devset/` metadata
/// are replaced atomically, with `state.toml` last, so an interruption leaves a target that the
/// next run repairs. A held plan writes only its conflicts, and the lock it waits to apply.
///
/// # Errors
/// - [`TargetError::Busy`] or [`TargetError::Concurrent`], another devset is at work; nothing is
///   written.
/// - [`Error::Io`](crate::Error::Io), a write fails; the next run repairs what it left.
pub fn commit(plan: Plan, target: &Target) -> Result<Vec<Step>> {
    let Plan { resolved, steps, merged, held } = plan;
    let writes = Writes::decide(&resolved, &steps, &merged)?;
    let _guard = lock(target)?;
    writes.perform(target, &steps, held)?;
    Ok(steps)
}

/// Everything a commit writes, decided before any of it is.
struct Writes<'a> {
    /// Target files and their new bytes.
    files: Vec<(&'a RelPath, &'a [u8])>,
    /// Bases to store.
    blobs: Vec<(Digest, &'a [u8])>,
    /// Conflicted merges, for `.devset/conflicts/`.
    sidecars: Vec<(&'a RelPath, &'a [u8])>,
    /// `lock.toml`.
    lock: String,
    /// `state.toml`.
    state: String,
    /// `answers.toml`; `None` when nothing is declared.
    answers: Option<String>,
    /// Bases `state.toml` references.
    live: HashSet<Digest>,
}

impl<'a> Writes<'a> {
    /// What committing `steps` writes.
    fn decide(resolved: &'a Resolved, steps: &'a [Step], merged: &'a Tree) -> Result<Self> {
        let mut records = BTreeMap::new();
        let (mut files, mut blobs, mut sidecars) = (Vec::new(), Vec::new(), Vec::new());
        let mut conflicted = BTreeSet::new();
        for step in steps {
            let path = &step.entry.path;
            match (step.action, step.entry.want) {
                (Action::Release, _) => {},
                (Action::Write | Action::Record | Action::Merge, Some(want)) => {
                    let payload = resolved.payload(&want, path)?;
                    if step.action == Action::Write {
                        files.push((path, payload));
                    }
                    blobs.push((want.fingerprint.exact, payload));
                    records.insert(path.clone(), want.fingerprint);
                },
                (action, _) => {
                    if action == Action::Conflict {
                        conflicted.insert(path);
                    }
                    records.extend(step.entry.record.map(|record| (path.clone(), record)));
                },
            }
        }
        for (path, bytes) in merged.iter() {
            if conflicted.contains(path) {
                sidecars.push((path, bytes));
            } else {
                files.push((path, bytes));
            }
        }
        let locked = resolved.layers().iter().map(|layer| Locked {
            rev: layer.rev().cloned(),
            digest: layer.digest(),
            source: layer.source().clone(),
        });
        let lock = to_toml(&Lock::new(locked.collect()))?;
        let answers = resolved.answers();
        let answers = if answers.is_empty() { None } else { Some(to_toml(answers)?) };
        let live = records.values().map(|record| record.exact).collect();
        let state = to_toml(&State::new(records))?;
        Ok(Self { files, blobs, sidecars, lock, state, answers, live })
    }

    /// Writes everything, `state.toml` last; only conflicts and the pending lock if `held`.
    fn perform(&self, target: &Target, steps: &[Step], held: bool) -> Result<()> {
        let dir = target.dir();
        for &(path, bytes) in &self.sidecars {
            write(&target.sidecar(path), bytes)?;
        }
        write_if_changed(&dir.join(".gitignore"), GITIGNORE.as_bytes())?;
        match &self.answers {
            Some(answers) => write_if_changed(&dir.join(ANSWERS), answers.as_bytes())?,
            None => remove_if_present(&dir.join(ANSWERS))?,
        }
        if held {
            return write(&dir.join(PENDING), self.lock.as_bytes());
        }
        for &(path, bytes) in &self.files {
            write(&path.under(target.root()), bytes)?;
        }
        let base = dir.join(BASE);
        for &(digest, bytes) in &self.blobs {
            let blob = base.join(digest.to_string());
            if !blob.is_file() {
                write(&blob, bytes)?;
            }
        }
        write_if_changed(&dir.join(".gitattributes"), GITATTRIBUTES.as_bytes())?;
        write_if_changed(&dir.join(CONFIG), target.config_text().as_bytes())?;
        write_if_changed(&dir.join(LOCK), self.lock.as_bytes())?;
        write_if_changed(&dir.join(STATE), self.state.as_bytes())?;

        remove_if_present(&dir.join(PENDING))?;
        if self.sidecars.is_empty() {
            remove_if_present(&dir.join(CONFLICTS))?;
        } else {
            for step in steps.iter().filter(|s| s.entry.conflict && s.action != Action::Conflict) {
                remove_if_present(&target.sidecar(&step.entry.path))?;
            }
        }
        for blob in fs_err::read_dir(&base)? {
            let blob = blob?;
            let digest = blob.file_name().to_str().and_then(|name| name.parse::<Digest>().ok());
            if digest.is_some_and(|digest| !self.live.contains(&digest)) {
                fs_err::remove_file(blob.path())?;
            }
        }
        Ok(())
    }
}

/// Takes `target`'s lock, and checks no other devset wrote `state.toml` since it was read.
fn lock(target: &Target) -> Result<fs_err::File> {
    let dir = target.dir();
    fs_err::create_dir_all(dir.join(BASE))?;
    let guard = fs_err::File::create(dir.join(LOCKFILE))?;
    match guard.file().try_lock() {
        Ok(()) => {},
        Err(TryLockError::WouldBlock) => return Err(TargetError::Busy.into()),
        Err(TryLockError::Error(e)) => return Err(e.into()),
    }
    if read_optional(&dir.join(STATE))?.as_deref().map(Digest::of) != target.state_digest() {
        return Err(TargetError::Concurrent.into());
    }
    Ok(guard)
}

/// `value` as TOML.
fn to_toml<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    Ok(toml::to_string(value).map_err(io::Error::other)?)
}

/// Replaces `path` with `bytes` atomically, creating parent directories.
fn write(path: &Utf8Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    let with_path = |e: io::Error| io::Error::new(e.kind(), format!("{path}: {e}"));
    let mut file = AtomicWriteFile::open(path).map_err(with_path)?;
    file.write_all(bytes).map_err(with_path)?;
    file.commit().map_err(with_path)?;
    Ok(())
}

/// [`write`](write()), unless `path` already holds `bytes`.
fn write_if_changed(path: &Utf8Path, bytes: &[u8]) -> Result<()> {
    if read_optional(path)?.as_deref() == Some(bytes) { Ok(()) } else { write(path, bytes) }
}

/// Removes the file or directory tree at `path`, if there is one.
fn remove_if_present(path: &Utf8Path) -> Result<()> {
    let removed =
        if path.is_dir() { fs_err::remove_dir_all(path) } else { fs_err::remove_file(path) };
    match removed {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
        Ok(()) | Err(_) => Ok(()),
    }
}
