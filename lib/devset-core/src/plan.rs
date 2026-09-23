//! Deciding what committing does to each path, merges included.

use crate::errors::{MergeError, Result};
use crate::merge::{Merged, has_markers};
use crate::path::RelPath;
use crate::profile::{OnConflict, Policy};
use crate::resolve::Resolved;
use crate::survey::{Entry, Survey};
use crate::target::Target;
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
    /// Forget the record; leave the file.
    Release,
    /// Write the merge of local edits and the profile's new bytes, and record the latter.
    Merge,
    /// Leave the file; the merge, with markers, waits in `.devset/conflicts/`.
    Conflict,
}

impl Action {
    /// Whether the file on disk would change.
    #[must_use]
    pub const fn writes(self) -> bool {
        matches!(self, Self::Write | Self::Merge)
    }
}

impl Entry {
    /// What committing does to this path under `mode`.
    ///
    /// A needed merge is [`Action::Merge`] here; [`plan`] runs it, and may find a conflict.
    #[must_use]
    pub fn action(&self, mode: Mode) -> Action {
        let Some(want) = self.want else { return Action::Release };
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
/// # Errors
/// - [`MergeError::Unresolved`], conflicts wait under [`Mode::Apply`] or [`Mode::Force`].
/// - [`MergeError::NothingToContinue`], none waits under [`Mode::Continue`].
/// - [`MergeError::Unmerged`] or [`MergeError::Invalid`], a resolution still has markers, or does
///   not pass its file's check.
/// - [`MergeError::Run`], a merge driver does not run to completion.
pub fn plan(survey: Survey, mode: Mode, target: &Target) -> Result<Plan> {
    let Survey { resolved, entries } = survey;
    let unresolved: Vec<RelPath> =
        entries.iter().filter(|e| e.conflict).map(|e| e.path.clone()).collect();
    match (mode, unresolved.is_empty()) {
        (Mode::Continue, true) => return Err(MergeError::NothingToContinue.into()),
        (Mode::Apply | Mode::Force, false) => {
            return Err(MergeError::Unresolved { paths: unresolved }.into());
        },
        _ => {},
    }
    let mut merged = Tree::default();
    let mut steps = Vec::with_capacity(entries.len());
    for entry in entries {
        let (action, note) = match (entry.action(mode), entry.want, entry.record) {
            (_, Some(want), _) if entry.conflict => {
                let bytes = fs_err::read(target.sidecar(&entry.path))?;
                if has_markers(&bytes) {
                    return Err(MergeError::Unmerged { path: entry.path }.into());
                }
                want.validate
                    .check(&bytes)
                    .map_err(|reason| MergeError::Invalid { path: entry.path.clone(), reason })?;
                merged.put(entry.path.clone(), &bytes);
                (Action::Merge, None)
            },
            (Action::Merge, Some(want), Some(record)) => {
                let base = target.base(&entry.path, &record)?;
                let ours = fs_err::read(entry.path.under(target.root()))?;
                let theirs = resolved.payload(&want, &entry.path)?;
                let driver = &resolved.settings().driver;
                let Merged { bytes, conflict } =
                    driver.merge(target.root(), &entry.path, &base, &ours, theirs)?;
                let invalid = || {
                    want.validate
                        .check(&bytes)
                        .err()
                        .map(|why| format!("merged, but invalid: {why}"))
                };
                let conflict = conflict.or_else(invalid);
                merged.put(entry.path.clone(), &bytes);
                (if conflict.is_some() { Action::Conflict } else { Action::Merge }, conflict)
            },
            (action, ..) => (action, None),
        };
        steps.push(Step { entry, action, note });
    }
    let conflicted = steps.iter().any(|step| step.action == Action::Conflict);
    let held = conflicted && resolved.settings().on_conflict == OnConflict::ApplyNone;
    Ok(Plan { resolved, steps, merged, held })
}

#[cfg(test)]
mod tests {
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
            want: Some(Want { layer: 0, policy, validate: Format::None, fingerprint: fp("p") }),
            record: record.map(fp),
            found: found.map(fp),
            conflict: false,
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
    fn vanished_is_released() {
        let entry = Entry { want: None, ..entry(Policy::Owned, Some("p"), Some("p")) };
        assert_eq!(entry.action(Mode::Force), Action::Release, "no provider means release");
    }
}
