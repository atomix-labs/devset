//! Gates: whether an entry applies.
//!
//! An entry's `when` holds when every condition does. `features`, `profiles` and `vars` are
//! settled by [`resolve`](crate::resolve()), which knows the graph and the answers. Scaffolds and
//! `exists` are settled by [`survey`](crate::survey()), which knows the disk:
//!
//! - A scaffold is decided once: written when none of its sentinels was there, found otherwise, and
//!   recorded. A found group's files are off.
//! - `exists` holds when a path will exist once the run is done: on disk and not removed by it, or
//!   written by it, whole or as a part. It only ever turns entries on, so it settles as a least
//!   fixpoint: every entry it gates starts off, and each round turns on those whose paths will
//!   exist, until none turns on. An entry is never its own evidence, and the order of the layers
//!   never matters.
//!
//! An entry left off with a record is released as its profile dropping it would release it; one
//! without is not there at all.

use alloc::collections::{BTreeMap, BTreeSet};
use core::cell::OnceCell;
use core::fmt;

use camino::Utf8Path;
use globset::{GlobBuilder, GlobMatcher};
use ignore::WalkBuilder;

use crate::errors::{Result, TargetError};
use crate::graph::Enabler;
use crate::name::{FeatureName, ProfileName, ScaffoldId};
use crate::part::Slot;
use crate::path::RelPath;
use crate::plan::{Action, Mode};
use crate::profile::When;
use crate::resolve::{Layer, Resolved};
use crate::survey::Entry;
use crate::target::{DIR, Scaffold, Scaffolded, Target};
use crate::vars::VarName;

/// A path in the target, or a glob over its paths.
#[derive(Clone, Debug)]
pub(crate) enum Pattern {
    /// One path.
    Path(RelPath),
    /// A glob: `*` within a component, `**` across them.
    Glob {
        /// As written, variables answered.
        written: String,
        /// Its matcher.
        glob: GlobMatcher,
    },
}

impl Pattern {
    /// `written` as a path, or as a glob when it has any of `*?[{`.
    ///
    /// # Errors
    /// Why it is neither.
    pub(crate) fn parse(written: &str) -> Result<Self, String> {
        if !written.contains(['*', '?', '[', '{']) {
            return RelPath::new(written).map(Self::Path).map_err(|e| e.to_string());
        }
        let glob = GlobBuilder::new(written)
            .literal_separator(true)
            .build()
            .map_err(|e| e.kind().to_string())?;
        Ok(Self::Glob { written: written.to_owned(), glob: glob.compile_matcher() })
    }

    /// As written.
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Path(path) => path.as_str(),
            Self::Glob { written, .. } => written,
        }
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why an entry does not apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reason {
    /// A feature of its profile is off.
    Feature(FeatureName),
    /// A profile is not active in the target.
    Profile(ProfileName),
    /// A variable's answer is none of the values `when` names.
    Var {
        /// The variable.
        name: VarName,
        /// Its answer.
        answer: String,
    },
    /// Its scaffold found the target's own files.
    Found(ScaffoldId),
    /// Its scaffold wrote it elsewhere, before an answer moved its path; it stays there.
    Moved {
        /// The scaffold.
        scaffold: ScaffoldId,
        /// Where it wrote it.
        path: RelPath,
    },
    /// A path, or a glob, will not exist when the run is done.
    Absent(String),
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Feature(feature) => write!(f, "feature {feature} is off"),
            Self::Profile(profile) => write!(f, "profile {profile} is not active"),
            Self::Var { name, answer } => write!(f, "{name} is {answer:?}"),
            Self::Found(scaffold) => write!(f, "scaffold {scaffold} found the target's own"),
            Self::Moved { scaffold, path } => write!(f, "scaffold {scaffold} wrote it at {path}"),
            Self::Absent(pattern) => write!(f, "{pattern} will not exist"),
        }
    }
}

/// Why `when` does not hold for a layer with `features` on, among `active` profiles and
/// `answers`, as far as those tell; `None` when it does.
pub(crate) fn check(
    when: &When, features: &BTreeMap<FeatureName, BTreeSet<Enabler>>,
    active: &BTreeSet<&ProfileName>, answers: &BTreeMap<VarName, String>,
) -> Option<Reason> {
    if let Some(feature) = when.features.iter().find(|f| !features.contains_key(*f)) {
        return Some(Reason::Feature(feature.clone()));
    }
    if let Some(profile) = when.profiles.iter().find(|p| !active.contains(p)) {
        return Some(Reason::Profile(profile.clone()));
    }
    when.vars.iter().find_map(|(name, values)| {
        let answer = answers.get(name).map_or("", String::as_str);
        (!values.iter().any(|value| value == answer))
            .then(|| Reason::Var { name: name.clone(), answer: answer.to_owned() })
    })
}

/// Turns off every file of `layers` whose `features`, `profiles` or `vars` do not hold.
pub(crate) fn statics(layers: &mut [Layer], answers: &BTreeMap<VarName, String>) {
    let active: BTreeSet<ProfileName> = layers.iter().map(|layer| layer.name().clone()).collect();
    let active: BTreeSet<&ProfileName> = active.iter().collect();
    for layer in layers {
        let features = layer.features().clone();
        for file in layer.files_mut().values_mut() {
            file.gated = check(&file.spec.when, &features, &active, answers);
        }
    }
}

/// What [`settle`] decided: each scaffold, each slot its gates turned off with why, and how each
/// `exists` came out.
#[derive(Debug, Default)]
pub(crate) struct Settled {
    /// Every decided scaffold, and where it wrote its files.
    pub scaffolds: BTreeMap<ScaffoldId, Scaffold>,
    /// Every slot its gates turned off, whether or not it was recorded.
    pub gated: BTreeMap<Slot, Reason>,
    /// Each path and glob of each gated slot's `when.exists`, and whether it will exist.
    pub exists: BTreeMap<Slot, Vec<(String, bool)>>,
}

/// Settles scaffolds and `exists` for `entries`, as surveyed in `target` against `resolved`:
/// an entry left off with a record is released, one without is dropped.
///
/// # Errors
/// [`TargetError::NoSuchScaffold`], a scaffold to write again is no active profile's.
pub(crate) fn settle(
    entries: &mut Vec<Entry>, resolved: &Resolved, target: &Target,
) -> Result<Settled> {
    let disk = Disk::new(target.root());
    let mut scaffolds = decide(resolved, target, &disk)?;
    let mut off: Vec<Option<Reason>> = vec![None; entries.len()];
    let mut gated: Vec<(usize, &[Pattern])> = Vec::new();
    // Each scaffold file that applies, by entry: its scaffold, and its path as written.
    let mut scaffolded: Vec<(usize, ScaffoldId, &RelPath)> = Vec::new();
    for (at, entry) in entries.iter_mut().enumerate() {
        let (Some(file), Some(profile)) = (resolved.file(entry), resolved.provider(entry)) else {
            continue;
        };
        if let Some(group) = &file.spec.scaffold {
            let id = ScaffoldId { profile: profile.clone(), group: group.clone() };
            let reason = match scaffolds.get(&id) {
                Some(Scaffold { decision: Scaffolded::Found, .. }) => {
                    Some(Reason::Found(id.clone()))
                },
                Some(Scaffold { files, .. }) => files
                    .get(&file.written)
                    .filter(|path| **path != entry.path)
                    .map(|path| Reason::Moved { scaffold: id.clone(), path: path.clone() }),
                None => None,
            };
            if let Some(reason) = reason {
                if let Some(slot) = off.get_mut(at) {
                    *slot = Some(reason);
                }
                continue;
            }
            if target.rescaffolds().contains(&id) {
                entry.record = None;
                entry.recorded = None;
            }
            scaffolded.push((at, id, &file.written));
        }
        if !file.exists.is_empty() {
            gated.push((at, &file.exists));
        }
    }
    let evidence = Evidence::new(entries, &disk);
    let mut on: Vec<bool> = off.iter().map(Option::is_none).collect();
    for &(at, _) in &gated {
        if let Some(slot) = on.get_mut(at) {
            *slot = false;
        }
    }
    loop {
        let turned: Vec<usize> = gated
            .iter()
            .filter(|&&(at, _)| !on.get(at).copied().unwrap_or(true))
            .filter(|&&(at, patterns)| {
                patterns.iter().all(|pattern| evidence.holds(pattern, &on, at))
            })
            .map(|&(at, _)| at)
            .collect();
        if turned.is_empty() {
            break;
        }
        for at in turned {
            if let Some(slot) = on.get_mut(at) {
                *slot = true;
            }
        }
    }
    let mut exists = BTreeMap::new();
    for &(at, patterns) in &gated {
        let outcomes: Vec<(String, bool)> = patterns
            .iter()
            .map(|pattern| (pattern.to_string(), evidence.holds(pattern, &on, at)))
            .collect();
        let absent = outcomes.iter().find(|(_, holds)| !holds).map(|(pattern, _)| pattern.clone());
        if let (Some(slot), Some(absent), false) =
            (off.get_mut(at), absent, on.get(at).copied().unwrap_or(true))
        {
            *slot = Some(Reason::Absent(absent));
        }
        if let Some(entry) = entries.get(at) {
            exists.insert(entry.slot(), outcomes);
        }
    }
    for (at, id, written) in scaffolded {
        let (Some(true), Some(entry), Some(scaffold)) =
            (on.get(at).copied(), entries.get(at), scaffolds.get_mut(&id))
        else {
            continue;
        };
        scaffold.files.insert(written.clone(), entry.path.clone());
    }
    let mut settled = Settled { scaffolds, gated: BTreeMap::new(), exists };
    let mut kept = Vec::with_capacity(entries.len());
    for (entry, reason) in entries.drain(..).zip(off) {
        let Some(reason) = reason else {
            kept.push(entry);
            continue;
        };
        settled.gated.insert(entry.slot(), reason.clone());
        if entry.record.is_some() {
            kept.push(Entry { want: None, gate: Some(reason), ..entry });
        }
    }
    *entries = kept;
    Ok(settled)
}

/// Every scaffold's decision: recorded, asked to be written again, or decided now against the
/// disk. A group none of whose files applies yet is left undecided, so a sentinel that appears
/// before one does still counts.
fn decide(
    resolved: &Resolved, target: &Target, disk: &Disk<'_>,
) -> Result<BTreeMap<ScaffoldId, Scaffold>> {
    let mut decided = BTreeMap::new();
    let mut known = Vec::new();
    for layer in resolved.layers() {
        for (group, unless) in layer.sentinels() {
            let id = ScaffoldId { profile: layer.name().clone(), group: group.clone() };
            known.push(id.to_string());
            let applies = layer
                .files()
                .values()
                .any(|file| file.gated.is_none() && file.spec.scaffold.as_ref() == Some(group));
            let rescaffold = target.rescaffolds().contains(&id);
            let decision = if rescaffold {
                Scaffold { decision: Scaffolded::Written, files: BTreeMap::new() }
            } else if let Some(recorded) = target.scaffolds().get(&id) {
                recorded.clone()
            } else if !applies {
                continue;
            } else {
                let found = unless.iter().any(|pattern| disk.has(pattern));
                let decision = if found { Scaffolded::Found } else { Scaffolded::Written };
                Scaffold { decision, files: BTreeMap::new() }
            };
            decided.insert(id, decision);
        }
    }
    if let Some(unknown) = target.rescaffolds().iter().find(|id| !decided.contains_key(*id)) {
        return Err(
            TargetError::NoSuchScaffold { name: unknown.to_string(), scaffolds: known }.into()
        );
    }
    Ok(decided)
}

/// The target's files as found, walked once when a glob needs them.
struct Disk<'a> {
    /// The target root.
    root: &'a Utf8Path,
    /// Every file no `.gitignore` excludes, `.git` and `.devset` left out.
    files: OnceCell<Vec<RelPath>>,
}

impl<'a> Disk<'a> {
    /// The target at `root`.
    const fn new(root: &'a Utf8Path) -> Self {
        Self { root, files: OnceCell::new() }
    }

    /// Whether `path` is there: a file, or a directory.
    fn exists(&self, path: &RelPath) -> bool {
        fs_err::symlink_metadata(path.under(self.root)).is_ok()
    }

    /// Whether anything is there that `pattern` names.
    fn has(&self, pattern: &Pattern) -> bool {
        match pattern {
            Pattern::Path(path) => self.exists(path),
            Pattern::Glob { glob, .. } => {
                self.files().iter().any(|file| glob.is_match(file.as_str()))
            },
        }
    }

    /// Every file no `.gitignore` excludes, in a git repository or not.
    fn files(&self) -> &[RelPath] {
        self.files.get_or_init(|| {
            WalkBuilder::new(self.root)
                .hidden(false)
                .require_git(false)
                .git_global(false)
                .filter_entry(|entry| !matches!(entry.file_name().to_str(), Some(".git" | DIR)))
                .build()
                .flatten()
                .filter(|entry| entry.file_type().is_some_and(|kind| !kind.is_dir()))
                .filter_map(|entry| {
                    let inside = entry.path().strip_prefix(self.root).ok()?;
                    let inside = Utf8Path::from_path(inside)?;
                    let joined: Vec<&str> = inside.components().map(|part| part.as_str()).collect();
                    RelPath::new(&joined.join("/")).ok()
                })
                .collect()
        })
    }
}

/// What will exist once the run is done, for an assignment of the gated entries.
struct Evidence<'a> {
    /// Every entry, as surveyed.
    entries: &'a [Entry],
    /// Each managed path's entries.
    by_path: BTreeMap<&'a RelPath, Vec<usize>>,
    /// The disk, for paths no entry manages.
    disk: &'a Disk<'a>,
}

impl<'a> Evidence<'a> {
    /// The evidence of `entries` and `disk`.
    fn new(entries: &'a [Entry], disk: &'a Disk<'a>) -> Self {
        let mut by_path: BTreeMap<&RelPath, Vec<usize>> = BTreeMap::new();
        for (at, entry) in entries.iter().enumerate() {
            by_path.entry(&entry.path).or_default().push(at);
        }
        Self { entries, by_path, disk }
    }

    /// Whether `pattern` names a path that will exist, each entry applying as `on` says; the
    /// entry at `own` asks, so its own path is judged without it.
    fn holds(&self, pattern: &Pattern, on: &[bool], own: usize) -> bool {
        match pattern {
            Pattern::Path(path) => self.stays(path, on, own),
            Pattern::Glob { glob, .. } => {
                let managed = self.by_path.keys().copied();
                let found =
                    self.disk.files().iter().filter(|file| !self.by_path.contains_key(file));
                managed
                    .chain(found)
                    .any(|path| glob.is_match(path.as_str()) && self.stays(path, on, own))
            },
        }
    }

    /// Whether `path` will exist, each entry but `own` applying as `on` says: a file an entry
    /// keeps or writes, a directory one writes under, or else what is on disk.
    fn stays(&self, path: &RelPath, on: &[bool], own: usize) -> bool {
        let at: Vec<usize> = self
            .by_path
            .get(path)
            .map_or_default(|at| at.iter().copied().filter(|&at| at != own).collect());
        if at.is_empty() {
            let under = format!("{path}/");
            let written =
                self.by_path.keys().filter(|managed| managed.as_str().starts_with(&under));
            return self.disk.exists(path) || written.into_iter().any(|p| self.stays(p, on, own));
        }
        at.iter().any(|&at| {
            let Some(entry) = self.entries.get(at) else { return false };
            let applies = on.get(at).copied().unwrap_or(false);
            let action = if applies {
                entry.action(Mode::Apply)
            } else {
                Entry { want: None, ..entry.clone() }.action(Mode::Apply)
            };
            match action {
                Action::Write | Action::Merge | Action::Record => true,
                // A part taken out of a file leaves the rest of it.
                Action::Remove => entry.part.is_some() && entry.present,
                Action::Keep | Action::Conflict | Action::Untrack => entry.present,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Pattern;

    #[test]
    fn patterns() {
        assert!(matches!(Pattern::parse("docs/book.toml"), Ok(Pattern::Path(_))), "a path");
        assert!(matches!(Pattern::parse("**/*.proto"), Ok(Pattern::Glob { .. })), "a glob");
        let Ok(Pattern::Glob { glob, .. }) = Pattern::parse("src/*.rs") else { panic!("a glob") };
        assert!(
            glob.is_match("src/lib.rs") && !glob.is_match("src/a/lib.rs"),
            "`*` stays in a component"
        );
        for bad in ["", "../x", "/abs", "a/[b"] {
            assert!(Pattern::parse(bad).is_err(), "{bad:?} is neither");
        }
    }
}
