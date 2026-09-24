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
use crate::rollback::{Location, Undo};
use crate::target::{
    ANSWERS, BASE, CONFIG, CONFLICTS, LOCK, Lock, Locked, PENDING, Record, STATE, State, Target,
    read_optional,
};
use crate::tree::Tree;

/// Held in `.devset/` while committing.
const LOCKFILE: &str = ".lock";

/// `.devset/.gitignore`: local work in progress.
///
/// Anchored, so it matches devset's own entries and nothing else.
const GITIGNORE: &str = "/conflicts/\n/.lock\n";

/// `.devset/.gitattributes`: keep blobs out of diffs, and away from end-of-line translation.
const GITATTRIBUTES: &str = "base/** -diff -text\n";

/// Carries out `plan` in `target`, returning its steps.
///
/// Everything is decided before anything is written; then files, blobs and `.devset/` metadata
/// are replaced atomically, with `state.toml` last, so an interruption leaves a target that the
/// next run repairs. A held plan writes only its conflicts, and the lock it waits to apply.
///
/// A commit that leaves conflicts first saves what it is about to change, and keeps every base,
/// so the unfinished update can be taken back with [`Rollback`](crate::Rollback).
///
/// # Errors
/// - [`TargetError::Busy`] or [`TargetError::Concurrent`], another devset is at work; nothing is
///   written.
/// - [`Error::Io`](crate::Error::Io), a write fails; the next run repairs what it left.
pub fn commit(plan: Plan, target: &Target) -> Result<Vec<Step>> {
    let Plan {
        resolved,
        steps,
        merged,
        gone,
        held,
    } = plan;
    let writes = Writes::decide(&resolved, &steps, &merged, &gone)?;
    let _guard = lock(target)?;
    writes.perform(target, &steps, held)?;
    Ok(steps)
}

/// Everything a commit writes, decided before any of it is.
struct Writes<'a> {
    /// Target files and their new bytes.
    files: Vec<(&'a RelPath, &'a [u8])>,
    /// Target files to delete.
    removals: Vec<&'a RelPath>,
    /// Target files written executable.
    executable: BTreeSet<&'a RelPath>,
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
    fn decide(
        resolved: &'a Resolved,
        steps: &'a [Step],
        merged: &'a Tree,
        gone: &'a BTreeSet<RelPath>,
    ) -> Result<Self> {
        let mut records = BTreeMap::new();
        let (mut files, mut blobs, mut sidecars) = (Vec::new(), Vec::new(), Vec::new());
        let mut conflicted = BTreeSet::new();
        for step in steps {
            let (entry, path) = (&step.entry, &step.entry.path);
            let scope = entry.scope();
            match (step.action, entry.want) {
                (Action::Untrack | Action::Remove, _) => {}
                (Action::Write | Action::Record | Action::Merge, Some(want)) => {
                    let payload = resolved.payload(entry)?;
                    // A written part is in its composed file, among the merged.
                    if step.action == Action::Write && entry.part.is_none() {
                        files.push((path, payload));
                    }
                    blobs.push((want.fingerprint.exact, payload));
                    let (fingerprint, policy) = (want.fingerprint, want.policy);
                    records.insert(
                        entry.slot(),
                        Record {
                            scope,
                            fingerprint,
                            policy,
                        },
                    );
                }
                (action, _) => {
                    if action == Action::Conflict {
                        conflicted.insert(path);
                    }
                    let policy = entry.want.map(|want| want.policy).or(entry.recorded);
                    let record = entry.record.map(|fingerprint| Record {
                        scope,
                        fingerprint,
                        policy: policy.unwrap_or_default(),
                    });
                    records.extend(record.map(|record| (entry.slot(), record)));
                }
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
            name: Some(layer.meta().name.clone()),
            rev: layer.rev().cloned(),
            digest: layer.digest(),
            source: layer.source().clone(),
        });
        let lock = to_toml(&Lock::new(locked.collect()))?;
        let answers = resolved.answers();
        let answers = if answers.is_empty() {
            None
        } else {
            Some(to_toml(answers)?)
        };
        let live = records
            .values()
            .map(|record| record.fingerprint.exact)
            .collect();
        let state = to_toml(&State::new(records))?;
        let removals = gone.iter().collect();
        let executable = steps
            .iter()
            .filter(|step| step.entry.want.is_some_and(|want| want.executable))
            .map(|step| &step.entry.path)
            .collect();
        Ok(Self {
            files,
            removals,
            executable,
            blobs,
            sidecars,
            lock,
            state,
            answers,
            live,
        })
    }

    /// Writes everything, `state.toml` last; only conflicts and the pending lock if `held`.
    fn perform(&self, target: &Target, steps: &[Step], held: bool) -> Result<()> {
        let dir = target.dir();
        let unfinished = !self.sidecars.is_empty();
        if unfinished {
            self.remember(target, steps, held)?;
        }
        for &(path, bytes) in &self.sidecars {
            write(&target.sidecar(path), bytes)?;
        }
        write_if_changed(&dir.join(".gitignore"), GITIGNORE.as_bytes())?;
        match &self.answers {
            Some(answers) => write_if_changed(&dir.join(ANSWERS), answers.as_bytes())?,
            None => remove_if_present(&dir.join(ANSWERS))?,
        }
        if held {
            return write(&target.unfinished().join(PENDING), self.lock.as_bytes());
        }
        for &(path, bytes) in &self.files {
            let file = path.under(target.root());
            write(&file, bytes)?;
            if self.executable.contains(path) {
                make_executable(&file)?;
            }
        }
        for &path in &self.removals {
            remove_with_empty_parents(target.root(), path)?;
        }
        for &(digest, bytes) in &self.blobs {
            let blob = dir.join(BASE).join(digest.to_string());
            if !blob.is_file() {
                write(&blob, bytes)?;
            }
        }
        write_if_changed(&dir.join(".gitattributes"), GITATTRIBUTES.as_bytes())?;
        write_if_changed(&dir.join(CONFIG), target.config_text().as_bytes())?;
        write_if_changed(&dir.join(LOCK), self.lock.as_bytes())?;
        write_if_changed(&dir.join(STATE), self.state.as_bytes())?;

        if unfinished {
            remove_if_present(&target.unfinished().join(PENDING))?;
            for step in steps
                .iter()
                .filter(|s| s.entry.conflict && s.action != Action::Conflict)
            {
                remove_if_present(&target.sidecar(&step.entry.path))?;
            }
            Ok(())
        } else {
            remove_if_present(&dir.join(CONFLICTS))?;
            prune(target, &self.live)
        }
    }

    /// Saves what `perform` changes before it writes, so a conflicted commit can be taken back.
    fn remember(&self, target: &Target, steps: &[Step], held: bool) -> Result<()> {
        let mut undo = Undo::load(target)?;
        for &(path, bytes) in &self.sidecars {
            undo.record(target, &Location::Sidecar(path.clone()), Some(bytes))?;
        }
        for step in steps
            .iter()
            .filter(|s| s.entry.conflict && s.action != Action::Conflict)
        {
            undo.record(target, &Location::Sidecar(step.entry.path.clone()), None)?;
        }
        let answers = self.answers.as_deref().map(str::as_bytes);
        undo.record(target, &Location::Record(ANSWERS), answers)?;
        if !held {
            for &(path, bytes) in &self.files {
                undo.record(target, &Location::File(path.clone()), Some(bytes))?;
            }
            for &path in &self.removals {
                undo.record(target, &Location::File(path.clone()), None)?;
            }
            let config = Some(target.config_text().as_bytes());
            undo.record(target, &Location::Record(CONFIG), config)?;
            undo.record(target, &Location::Record(LOCK), Some(self.lock.as_bytes()))?;
            undo.record(
                target,
                &Location::Record(STATE),
                Some(self.state.as_bytes()),
            )?;
        }
        undo.save(target)
    }
}

/// Removes every base in `.devset/base/` but those in `live`.
pub(crate) fn prune(target: &Target, live: &HashSet<Digest>) -> Result<()> {
    let base = target.dir().join(BASE);
    if !base.is_dir() {
        return Ok(());
    }
    for blob in fs_err::read_dir(&base)? {
        let blob = blob?;
        let digest = blob
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<Digest>().ok());
        if digest.is_some_and(|digest| !live.contains(&digest)) {
            fs_err::remove_file(blob.path())?;
        }
    }
    Ok(())
}

/// Takes `target`'s lock, and checks no other devset wrote `state.toml` since it was read.
pub(crate) fn lock(target: &Target) -> Result<fs_err::File> {
    let dir = target.dir();
    fs_err::create_dir_all(dir.join(BASE))?;
    let guard = fs_err::File::create(dir.join(LOCKFILE))?;
    match guard.file().try_lock() {
        Ok(()) => {}
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
pub(crate) fn write(path: &Utf8Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs_err::create_dir_all(parent)?;
    }
    let with_path = |e: io::Error| io::Error::new(e.kind(), format!("{path}: {e}"));
    let mut file = AtomicWriteFile::open(path).map_err(with_path)?;
    file.write_all(bytes).map_err(with_path)?;
    file.commit().map_err(with_path)?;
    Ok(())
}

/// Gives the file at `path` mode 755.
#[cfg(unix)]
fn make_executable(path: &Utf8Path) -> Result<()> {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt as _;
    Ok(fs_err::set_permissions(
        path,
        Permissions::from_mode(0o755),
    )?)
}

/// Files have no mode here.
#[cfg(not(unix))]
fn make_executable(_: &Utf8Path) -> Result<()> {
    Ok(())
}

/// [`write`](write()), unless `path` already holds `bytes`.
fn write_if_changed(path: &Utf8Path, bytes: &[u8]) -> Result<()> {
    if read_optional(path)?.as_deref() == Some(bytes) {
        Ok(())
    } else {
        write(path, bytes)
    }
}

/// Removes the file at `path` under `root`, then each directory it leaves empty, up to `root`.
fn remove_with_empty_parents(root: &Utf8Path, path: &RelPath) -> Result<()> {
    let file = path.under(root);
    remove_if_present(&file)?;
    let mut dir = file.parent();
    while let Some(parent) = dir.filter(|parent| *parent != root && parent.starts_with(root)) {
        if fs_err::read_dir(parent).map_or(true, |mut entries| entries.next().is_some()) {
            break;
        }
        fs_err::remove_dir(parent)?;
        dir = parent.parent();
    }
    Ok(())
}

/// Removes the file or directory tree at `path`, if there is one.
pub(crate) fn remove_if_present(path: &Utf8Path) -> Result<()> {
    let removed = if path.is_dir() {
        fs_err::remove_dir_all(path)
    } else {
        fs_err::remove_file(path)
    };
    match removed {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
        Ok(()) | Err(_) => Ok(()),
    }
}
