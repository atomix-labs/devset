//! Deciding what committing does to each path, merges included.

use alloc::collections::BTreeSet;

use serde_json::Value;

use crate::digest::{Fingerprint, is_binary};
use crate::errors::{MergeError, Result, list};
use crate::format::Format;
use crate::merge::{Driver, Merged, has_markers};
use crate::part::{Key, Leaves, Shape, decode, display, encode, merge, overlap};
use crate::path::RelPath;
use crate::profile::{OnConflict, Policy};
use crate::resolve::{Portion, Resolved};
use crate::survey::{Entry, Survey, released};
use crate::target::{Target, read_optional};
use crate::tree::Tree;

/// How far a plan may go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Never destroy bytes.
    Apply,
    /// Also restore `owned` files that were edited, deleted, or never recorded.
    Force,
    /// Install resolved conflicts from `.devset/conflicts/`; otherwise as [`Mode::Apply`].
    Continue,
}

/// What committing does to one path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Nothing.
    Keep,
    /// Write the profile's bytes and record them.
    Write,
    /// Record the profile's bytes as the base; leave the file.
    Record,
    /// Stop tracking it, and leave the file: no layer provides it any more, and it is not as
    /// devset recorded it, or it became the target's when written.
    Untrack,
    /// Delete the file, or take the part out of it: no layer provides it any more, and it is as
    /// devset recorded it.
    Remove,
    /// Write the merge of local edits and the profile's new bytes, and record the latter.
    Merge,
    /// Leave the file; the merge, with markers, waits in `.devset/conflicts/`.
    Conflict,
}

impl Action {
    /// Whether the file on disk would change.
    #[must_use]
    pub const fn writes(self) -> bool {
        matches!(self, Self::Write | Self::Merge | Self::Remove)
    }
}

impl Entry {
    /// What committing does to this path under `mode`.
    ///
    /// A needed merge is [`Action::Merge`] here; [`plan`] runs it, and may find a conflict.
    #[must_use]
    pub fn action(&self, mode: Mode) -> Action {
        let Some(want) = self.want else {
            return self.released();
        };
        let incoming = want.fingerprint;
        let restore = mode == Mode::Force && want.policy == Policy::Owned;
        match (self.found, self.record) {
            (None, None) => Action::Write,
            (None, Some(_)) => {
                if restore {
                    Action::Write
                } else {
                    Action::Keep
                }
            },
            // Already the profile's content, perhaps reformatted: never rewritten.
            (Some(found), record) if found.same_content(&incoming) => {
                if record == Some(incoming) {
                    Action::Keep
                } else {
                    Action::Record
                }
            },
            // Present but never recorded: adopted, so the profile becomes its base.
            (Some(_), None) => {
                if restore {
                    Action::Write
                } else {
                    Action::Record
                }
            },
            // Untouched since recorded, and the profile has moved on.
            (Some(found), Some(record)) if found.same_content(&record) => {
                if want.policy == Policy::Once { Action::Keep } else { Action::Write }
            },
            (Some(_), Some(record)) => {
                if restore {
                    Action::Write
                } else if want.policy == Policy::Merge && !record.same_content(&incoming) {
                    Action::Merge
                } else {
                    Action::Keep
                }
            },
        }
    }
}

impl Entry {
    /// What becomes of a path no layer provides: removed when it is as devset recorded it, unless
    /// it became the target's when written (`once`); otherwise untracked, and left as it is.
    fn released(&self) -> Action {
        let unchanged = matches!(
            (self.found, self.record),
            (Some(found), Some(record)) if found.same_content(&record)
        );
        if unchanged && self.recorded != Some(Policy::Once) {
            Action::Remove
        } else {
            Action::Untrack
        }
    }
}

/// One path and what committing does to it.
#[derive(Clone, Debug)]
pub struct Step {
    /// The path, in all three forms.
    pub entry: Entry,
    /// What happens to it.
    pub action: Action,
    /// Why a merge conflicted.
    pub note: Option<String>,
}

/// Everything committing will do; nothing is written until [`commit`](crate::commit()).
#[derive(Debug)]
pub struct Plan {
    /// Where the profile's bytes come from.
    pub(crate) resolved: Resolved,
    /// Every path, in order.
    pub(crate) steps: Vec<Step>,
    /// Merge results, clean or conflicted, by path.
    pub(crate) merged: Tree,
    /// Files to delete: removed whole, or left empty by the parts removed from them.
    pub(crate) gone: BTreeSet<RelPath>,
    /// Whether `on-conflict = "apply-none"` withholds everything but the conflicts.
    pub(crate) held: bool,
}

impl Plan {
    /// The profile planned against.
    #[must_use]
    pub const fn resolved(&self) -> &Resolved {
        &self.resolved
    }

    /// Every path and its action, in path order.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Whether conflicts withhold the rest of the update.
    #[must_use]
    pub const fn held(&self) -> bool {
        self.held
    }
}

/// Decides every path of `survey` under `mode`, running the merges it needs.
///
/// A file with parts is composed: each part spliced into it, merged where it needs merging, and
/// the whole checked. A conflict in one part holds the whole file back.
///
/// # Errors
/// - [`MergeError::Unresolved`], conflicts wait under [`Mode::Apply`] or [`Mode::Force`].
/// - [`MergeError::NothingToContinue`], none waits under [`Mode::Continue`].
/// - [`MergeError::Unmerged`] or [`MergeError::Invalid`], a resolution still has markers, or does
///   not pass its file's check.
/// - [`MergeError::Run`], a merge driver does not run to completion.
/// - [`Error::Parse`](crate::Error::Parse), a part cannot be written into its file: a key it sets
///   lies under one the file holds as a value.
pub fn plan(survey: Survey, mode: Mode, target: &Target) -> Result<Plan> {
    let Survey { resolved, entries } = survey;
    let mut unresolved: Vec<RelPath> =
        entries.iter().filter(|e| e.conflict).map(|e| e.path.clone()).collect();
    unresolved.dedup();
    match (mode, unresolved.is_empty()) {
        (Mode::Continue, true) => return Err(MergeError::NothingToContinue.into()),
        (Mode::Apply | Mode::Force, false) => {
            return Err(MergeError::Unresolved { paths: unresolved }.into());
        },
        _ => {},
    }
    let mut planner = Planner {
        resolved: &resolved,
        target,
        mode,
        merged: Tree::default(),
        gone: BTreeSet::new(),
    };
    let mut steps = Vec::with_capacity(entries.len());
    let mut entries = entries.into_iter().peekable();
    while let Some(first) = entries.next() {
        let path = first.path.clone();
        let mut file = vec![first];
        while let Some(next) = entries.next_if(|next| next.path == path) {
            file.push(next);
        }
        steps.extend(planner.file(file)?);
    }
    let Planner { merged, gone, .. } = planner;
    let conflicted = steps.iter().any(|step| step.action == Action::Conflict);
    let held = conflicted && resolved.settings().on_conflict == OnConflict::ApplyNone;
    Ok(Plan { resolved, steps, merged, gone, held })
}

/// Plans a file at a time, gathering the bytes merges and parts write.
struct Planner<'a> {
    /// The profile.
    resolved: &'a Resolved,
    /// Where the files are.
    target: &'a Target,
    /// How far the plan may go.
    mode: Mode,
    /// Merged and composed files, conflicted or not, by path.
    merged: Tree,
    /// Files to delete.
    gone: BTreeSet<RelPath>,
}

/// What one provided part of a file becomes.
struct Decision<'a> {
    /// What committing does to it.
    action: Action,
    /// The content it puts in the file, when it changes.
    content: Option<Vec<u8>>,
    /// Why it conflicts.
    note: Option<String>,
    /// The keys both sides changed.
    clash: Option<Clash<'a>>,
}

impl Decision<'_> {
    /// A decision with no clash.
    const fn of(action: Action, content: Option<Vec<u8>>, note: Option<String>) -> Self {
        Self { action, content, note, clash: None }
    }
}

/// Keys both sides changed in one part, and their values on each side but ours.
struct Clash<'a> {
    /// The part.
    portion: &'a Portion,
    /// The keys.
    keys: BTreeSet<Key>,
    /// Their values in the base.
    base: Leaves,
    /// Their values in the profile.
    theirs: Leaves,
}

impl<'a> Planner<'a> {
    /// Steps for every entry of one file: the whole file, its parts, or its resolution.
    fn file(&mut self, entries: Vec<Entry>) -> Result<Vec<Step>> {
        if entries.first().is_some_and(|entry| entry.conflict) {
            return self.resolution(entries);
        }
        let mut steps = Vec::with_capacity(entries.len());
        let mut parts = Vec::new();
        // A layer that owns the whole file decides all of it: a dropped part there is left.
        let whole = entries.iter().any(|entry| entry.part.is_none() && entry.want.is_some());
        for entry in entries {
            match (&entry.part, entry.want) {
                (None, _) => steps.push(self.whole(entry)?),
                (Some(_), None) if whole => {
                    steps.push(Step { entry, action: Action::Untrack, note: None });
                },
                (Some(_), _) => parts.push(entry),
            }
        }
        if !parts.is_empty() {
            steps.extend(self.parts(parts)?);
            steps.sort_by(|a, b| a.entry.part.cmp(&b.entry.part));
        }
        Ok(steps)
    }

    /// The step for a whole file, merged if it needs merging.
    fn whole(&mut self, entry: Entry) -> Result<Step> {
        let (action, note) = match (entry.action(self.mode), entry.want, entry.record) {
            (Action::Merge, Some(want), Some(record)) => {
                let base = self.target.base(&record)?;
                let ours = fs_err::read(entry.path.under(self.target.root()))?;
                let theirs = self.resolved.payload(&entry)?;
                let Merged { bytes, conflict } =
                    self.merge(&entry.path, base.as_deref(), &ours, theirs)?;
                let conflict = conflict.or_else(|| {
                    want.validate
                        .check(&bytes)
                        .err()
                        .map(|why| format!("merged, but invalid: {why}"))
                });
                self.merged.put(entry.path.clone(), &bytes);
                (if conflict.is_some() { Action::Conflict } else { Action::Merge }, conflict)
            },
            (Action::Remove, ..) => {
                self.gone.insert(entry.path.clone());
                (Action::Remove, None)
            },
            (action, ..) => (action, None),
        };
        Ok(Step { entry, action, note })
    }

    /// Merges `ours` and `theirs` from `base` for `path` with the configured driver.
    fn merge(
        &self, path: &RelPath, base: Option<&[u8]>, ours: &[u8], theirs: &[u8],
    ) -> Result<Merged> {
        let driver = &self.resolved.settings().driver;
        let root = self.target.root();
        let Merged { bytes, conflict } =
            driver.merge(root, path, base.unwrap_or_default(), ours, theirs)?;
        // Without its base a merge cannot tell a local edit from a profile change, so every
        // difference waits for the user; a binary file keeps its own note.
        let lost = base.is_none() && !is_binary(ours) && !is_binary(theirs);
        let conflict = if lost {
            Some("its recorded base is missing, so it merged without one".to_owned())
        } else {
            conflict
        };
        Ok(Merged { bytes, conflict })
    }

    /// Steps for the parts of one file: each spliced in, merged where it needs merging, then
    /// the whole file checked, and each part read back.
    fn parts(&mut self, entries: Vec<Entry>) -> Result<Vec<Step>> {
        let resolved = self.resolved;
        let Some(path) = entries.first().map(|entry| entry.path.clone()) else {
            return Ok(Vec::new());
        };
        let label = path.as_str();
        let disk = read_optional(&path.under(self.target.root()))?.unwrap_or_default();
        let mut file = disk.clone();
        let mut steps = Vec::with_capacity(entries.len());
        let mut written: Vec<(usize, &Shape, Vec<u8>)> = Vec::new();
        let mut removed: Vec<(usize, Shape, BTreeSet<Key>)> = Vec::new();
        let mut clashes = Vec::new();
        // Keys a part still provided covers, which no dropped part takes with it.
        let claimed: Vec<Key> = entries
            .iter()
            .filter(|entry| entry.want.is_some())
            .flat_map(|entry| entry.keys.iter().cloned())
            .collect();
        for entry in entries {
            let Some(portion) = resolved.portion(&entry) else {
                let taken = self.take_out(&mut file, &entry, &claimed, label)?;
                let action = if taken.is_some() { Action::Remove } else { Action::Untrack };
                removed.extend(taken.map(|(shape, keys)| (steps.len(), shape, keys)));
                steps.push(Step { entry, action, note: None });
                continue;
            };
            let Decision { action, content, note, clash } = self.decide(&entry, portion, &disk)?;
            clashes.extend(clash);
            if let Some(content) = content {
                file = portion.splice(&file, &content, &entry.keys, label)?;
                written.push((steps.len(), &portion.shape, content));
            }
            steps.push(Step { entry, action, note });
        }
        if written.is_empty() && removed.is_empty() {
            return Ok(steps);
        }
        // Markers never parse: a conflicted file is checked once resolved, by `--continue`.
        let marked = steps.iter().any(|step| step.action == Action::Conflict);
        let trouble = if marked { None } else { check(&file, &steps, &written, &removed, label)? };
        let conflicted = marked || trouble.is_some();
        if !conflicted {
            // A file that held nothing but the parts removed from it goes with them.
            if !removed.is_empty() && file.trim_ascii().is_empty() {
                self.gone.insert(path);
            } else {
                self.merged.put(path, &file);
            }
            return Ok(steps);
        }
        for step in &mut steps {
            if step.action.writes() {
                step.action = Action::Conflict;
                step.note = trouble.clone().or_else(|| {
                    Some("waits for the conflict in another part of the file".to_owned())
                });
            }
        }
        let sidecar = self.mark(&path, &file, &clashes)?;
        self.merged.put(path, &sidecar);
        Ok(steps)
    }

    /// What `entry`, a part `portion` provides, becomes against its file as it is on `disk`.
    fn decide(&self, entry: &Entry, portion: &'a Portion, disk: &[u8]) -> Result<Decision<'a>> {
        let (shape, theirs) = (&portion.shape, self.resolved.payload(entry)?);
        let label = entry.path.as_str();
        Ok(match (entry.action(self.mode), entry.record, shape) {
            (Action::Write, ..) => Decision::of(Action::Write, Some(theirs.to_vec()), None),
            (Action::Merge, Some(record), Shape::Keys(_)) => {
                let base = self.target.base(&record)?;
                let base = base.as_deref().and_then(decode).unwrap_or_default();
                let ours = shape.view(disk, &entry.keys, label)?;
                let ours = ours.as_deref().and_then(decode).unwrap_or_default();
                let theirs = decode(theirs).unwrap_or_default();
                let merged = merge(&base, &ours, &theirs, &entry.keys);
                let content = Some(encode(&merged.leaves));
                if merged.conflicts.is_empty() {
                    return Ok(Decision::of(Action::Merge, content, None));
                }
                let note = format!("both changed {}", keys(&merged.conflicts));
                let at = |side: &Leaves| {
                    let conflicts = &merged.conflicts;
                    side.iter().filter(|(k, _)| conflicts.contains(*k)).map(clone).collect()
                };
                let (base, theirs) = (at(&base), at(&theirs));
                let clash = Clash { portion, keys: merged.conflicts, base, theirs };
                Decision { action: Action::Conflict, content, note: Some(note), clash: Some(clash) }
            },
            (Action::Merge, Some(record), Shape::Block(_)) => {
                let base = self.target.base(&record)?;
                let ours = shape.view(disk, &entry.keys, label)?.unwrap_or_default();
                let Merged { bytes, conflict } =
                    self.merge(&entry.path, base.as_deref(), &ours, theirs)?;
                let action = if conflict.is_some() { Action::Conflict } else { Action::Merge };
                Decision::of(action, Some(bytes), conflict)
            },
            // Adopted leaf by leaf: a key the file holds keeps its value, one it lacks is
            // written, as a whole file already there is kept and one missing is written.
            (Action::Record, None, Shape::Keys(_)) => {
                let ours = shape.view(disk, &entry.keys, label)?;
                let ours = ours.as_deref().and_then(decode).unwrap_or_default();
                let mut filled = decode(theirs).unwrap_or_default();
                filled.extend(ours.iter().map(clone));
                if filled == ours {
                    Decision::of(Action::Record, None, None)
                } else {
                    Decision::of(Action::Merge, Some(encode(&filled)), None)
                }
            },
            (action, ..) => Decision::of(action, None, None),
        })
    }

    /// Takes a part no layer provides out of `file`, as its path reads it, when it is as devset
    /// recorded it; a key a part still provided covers stays. The part's shape and the keys it
    /// took, or `None` when it stays, untracked.
    fn take_out(
        &self, file: &mut Vec<u8>, entry: &Entry, claimed: &[Key], label: &str,
    ) -> Result<Option<(Shape, BTreeSet<Key>)>> {
        if entry.action(self.mode) != Action::Remove {
            return Ok(None);
        }
        let Some(shape) = released(self.target, entry) else {
            return Ok(None);
        };
        let mut keys = entry.keys.clone();
        keys.retain(|key| !claimed.iter().any(|kept| overlap(key, kept)));
        *file = shape.remove(file, &keys, label)?;
        Ok(Some((shape, keys)))
    }

    /// `file` with conflict markers around the keys in `clashes`: ours, as `file` holds them,
    /// against the profile's.
    fn mark(&self, path: &RelPath, file: &[u8], clashes: &[Clash<'_>]) -> Result<Vec<u8>> {
        if clashes.is_empty() {
            return Ok(file.to_vec());
        }
        let label = path.as_str();
        let (mut base, mut theirs) = (file.to_vec(), file.to_vec());
        for clash in clashes {
            base = clash.portion.splice(&base, &encode(&clash.base), &clash.keys, label)?;
            theirs = clash.portion.splice(&theirs, &encode(&clash.theirs), &clash.keys, label)?;
        }
        let root = self.target.root();
        Ok(Driver::Builtin.merge(root, path, &base, file, &theirs)?.bytes)
    }

    /// Installs the resolution of a file that waits in `.devset/conflicts/`.
    ///
    /// The whole file is the resolution, so a part the update changed is recorded from the
    /// profile; one it left alone keeps its record.
    fn resolution(&mut self, entries: Vec<Entry>) -> Result<Vec<Step>> {
        let path = entries.iter().find(|entry| entry.want.is_some()).map(|e| e.path.clone());
        if let Some(path) = path {
            let bytes = fs_err::read(self.target.sidecar(&path))?;
            if has_markers(&bytes) {
                return Err(MergeError::Unmerged { path }.into());
            }
            for want in entries.iter().filter_map(|entry| entry.want) {
                want.validate
                    .check(&bytes)
                    .map_err(|reason| MergeError::Invalid { path: path.clone(), reason })?;
            }
            self.merged.put(path, &bytes);
        }
        let mode = self.mode;
        let step = |entry: Entry| {
            let action = match (entry.want, &entry.part) {
                (None, _) => Action::Untrack,
                (Some(_), None) => Action::Merge,
                (Some(_), Some(_)) => match entry.action(mode) {
                    action if action.writes() => Action::Merge,
                    action => action,
                },
            };
            Step { entry, action, note: None }
        };
        Ok(entries.into_iter().map(step).collect())
    }
}

/// Why the composed `file` cannot be written: it does not pass a part's check, a part reads back
/// other than as `written`, or one `removed` is still there.
fn check(
    file: &[u8], steps: &[Step], written: &[(usize, &Shape, Vec<u8>)],
    removed: &[(usize, Shape, BTreeSet<Key>)], label: &str,
) -> Result<Option<String>> {
    let mut formats: Vec<Format> =
        steps.iter().filter_map(|s| s.entry.want).map(|w| w.validate).collect();
    formats.dedup();
    for format in formats {
        if let Err(why) = format.check(file) {
            return Ok(Some(format!("with it, the file is invalid: {why}")));
        }
    }
    for (index, shape, content) in written {
        let Some(step) = steps.get(*index) else {
            continue;
        };
        let back = shape.view(file, &step.entry.keys, label)?.unwrap_or_default();
        if !Fingerprint::of(&back).same_content(&Fingerprint::of(content)) {
            return Ok(Some("it reads back other than devset wrote it".to_owned()));
        }
    }
    for (_, shape, keys) in removed {
        let left = shape.view(file, keys, label)?;
        if left.is_some_and(|left| decode(&left).is_none_or(|leaves| !leaves.is_empty())) {
            return Ok(Some("a part devset removed is still in it".to_owned()));
        }
    }
    Ok(None)
}

/// A leaf, owned.
fn clone((key, value): (&Key, &Value)) -> (Key, Value) {
    (key.clone(), value.clone())
}

/// `keys` as people write them, the first few by name.
fn keys(keys: &BTreeSet<Key>) -> String {
    const NAMED: usize = 3;
    let named = keys.iter().take(NAMED).map(|key| format!("`{}`", display(key)));
    let rest = keys.len().saturating_sub(NAMED);
    list(named.chain((rest > 0).then(|| format!("{rest} more"))))
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeSet;

    use super::{Action, Mode};
    use crate::digest::Fingerprint;
    use crate::format::Format;
    use crate::path::RelPath;
    use crate::profile::Policy;
    use crate::survey::{Entry, Want};

    /// On-disk and recorded contents, by name.
    ///
    /// `"p"` is the profile's bytes, `"p~"` the same reformatted, `"o"` older profile bytes,
    /// `"x"` a local edit.
    fn fp(name: &str) -> Fingerprint {
        Fingerprint::of(match name {
            "p" => b"profile\n",
            "p~" => b"profile  \r\n\n",
            "o" => b"old\n",
            _ => b"edited\n",
        })
    }

    fn entry(policy: Policy, found: Option<&str>, record: Option<&str>) -> Entry {
        Entry {
            path: RelPath::new("f").unwrap(),
            part: None,
            want: Some(Want {
                layer: 0,
                policy,
                validate: Format::None,
                executable: false,
                fingerprint: fp("p"),
            }),
            record: record.map(fp),
            recorded: record.map(|_| policy),
            found: found.map(fp),
            conflict: false,
            keys: BTreeSet::new(),
        }
    }

    #[test]
    fn decision_table() {
        use Action::{Keep, Merge, Record, Write};
        use Policy::{Merge as M, Once, Owned};
        #[rustfmt::skip]
        let table = [
            // policy found       record      apply   force
            (Owned, None,       None,       Write,  Write),  // new
            (Owned, None,       Some("p"),  Keep,   Write),  // deleted
            (Owned, Some("p"),  Some("p"),  Keep,   Keep),   // in sync
            (Owned, Some("p~"), Some("p"),  Keep,   Keep),   // cosmetic
            (Owned, Some("p~"), Some("o"),  Record, Record), // already the new content
            (Owned, Some("x"),  None,       Record, Write),  // untracked: adopt
            (Owned, Some("o"),  Some("o"),  Write,  Write),  // profile moved on
            (Owned, Some("x"),  Some("p"),  Keep,   Write),  // edited
            (Once,  None,       None,       Write,  Write),
            (Once,  None,       Some("p"),  Keep,   Keep),
            (Once,  Some("x"),  None,       Record, Record),
            (Once,  Some("o"),  Some("o"),  Keep,   Keep),
            (Once,  Some("x"),  Some("p"),  Keep,   Keep),
            (M,     Some("x"),  Some("o"),  Merge,  Merge),
            (M,     Some("x"),  Some("p"),  Keep,   Keep),
            (M,     Some("o"),  Some("o"),  Write,  Write),
        ];
        for (policy, found, record, apply, force) in table {
            let entry = entry(policy, found, record);
            let case = format!("{policy:?} found={found:?} record={record:?}");
            assert_eq!(entry.action(Mode::Apply), apply, "apply: {case}");
            assert_eq!(entry.action(Mode::Force), force, "force: {case}");
        }
    }

    #[test]
    fn a_dropped_file_goes_unless_edited_or_the_targets() {
        use Action::{Remove, Untrack};
        use Policy::{Merge as M, Once, Owned};
        #[rustfmt::skip]
        let table = [
            // policy found       record      both modes
            (Owned, Some("p"),  Some("p"),  Remove),  // as recorded
            (Owned, Some("p~"), Some("p"),  Remove),  // reformatted only
            (M,     Some("p"),  Some("p"),  Remove),
            (Owned, Some("x"),  Some("p"),  Untrack), // edited: the edit stays
            (Owned, None,       Some("p"),  Untrack), // already deleted
            (Once,  Some("p"),  Some("p"),  Untrack), // the target's since written
        ];
        for (policy, found, record, action) in table {
            let entry = Entry { want: None, ..entry(policy, found, record) };
            let case = format!("{policy:?} found={found:?} record={record:?}");
            assert_eq!(entry.action(Mode::Apply), action, "apply: {case}");
            assert_eq!(entry.action(Mode::Force), action, "force: {case}");
        }
    }
}
