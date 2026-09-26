//! The apply log: what an apply, or taking an update back, did or would do, and the notes after it.

use std::io;

use clap_cargo::style::{ERROR, GOOD, WARN};
use devset_core::plan::Step;
use devset_core::profile::VarName;
use devset_core::{RelPath, Revert, Rollback};

use super::Change;
use crate::shell::Shell;
use crate::words::{count, list};

/// What an apply wrote, which decides how its log reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wrote {
    /// Everything it planned.
    All,
    /// Only its conflicts: `on-conflict = "apply-none"` withheld the rest.
    Conflicts,
    /// Nothing, being a dry run of a plan that would write all, or only its conflicts if `held`.
    Nothing {
        /// Whether `on-conflict = "apply-none"` would withhold the rest.
        held: bool,
    },
}

impl Wrote {
    /// What an apply of a plan that is `held` or not writes, unless `dry_run`.
    pub(crate) const fn of(dry_run: bool, held: bool) -> Self {
        match (dry_run, held) {
            (true, held) => Self::Nothing { held },
            (false, true) => Self::Conflicts,
            (false, false) => Self::All,
        }
    }

    /// Whether `on-conflict = "apply-none"` holds back everything but the conflicts.
    const fn held(self) -> bool {
        matches!(self, Self::Conflicts | Self::Nothing { held: true })
    }
}

/// Prints the log of `steps`, which wrote as `wrote` says; returns whether any conflicted.
pub(crate) fn applied(shell: &Shell, steps: &[Step], wrote: Wrote) -> io::Result<bool> {
    let (mut changes, mut conflicts) = (0_usize, 0_usize);
    for step in steps {
        let Some(change) = Change::of(&step.entry, step.action) else {
            continue;
        };
        let path = &step.entry;
        if change == Change::Conflict {
            conflicts = conflicts.saturating_add(1);
            let why = step.note.as_deref().unwrap_or("conflicting changes");
            let (status, message) = match wrote {
                Wrote::Nothing { .. } => ("Would", format!("conflict {path}  {why}")),
                Wrote::All | Wrote::Conflicts => ("Conflicted", format!("{path}  {why}")),
            };
            shell.always(status, ERROR, message)?;
            continue;
        }
        changes = changes.saturating_add(1);
        // What its gates turned off says why it goes.
        let path = step
            .entry
            .gate
            .as_ref()
            .map_or_else(|| path.to_string(), |why| format!("{path}  ({why})"));
        let (status, style, message) = match wrote {
            Wrote::All => (change.past(), GOOD, path),
            Wrote::Conflicts => ("Withheld", WARN, format!("{path}  would {change}")),
            Wrote::Nothing { held: false } => ("Would", GOOD, format!("{change} {path}")),
            Wrote::Nothing { held: true } => ("Would", WARN, format!("withhold {path}  {change}")),
        };
        shell.status(status, style, message)?;
    }
    let outcome = match (changes, conflicts) {
        (0, 0) => "up to date".to_owned(),
        (n, c) if wrote.held() && n > 0 => format!(
            "{}; {} withheld by `on-conflict = \"apply-none\"`",
            count(c, "conflict"),
            count(n, "change")
        ),
        (n, 0) => count(n, "change"),
        (0, c) => count(c, "conflict"),
        (n, c) => format!("{}, {}", count(n, "change"), count(c, "conflict")),
    };
    let dry_run = matches!(wrote, Wrote::Nothing { .. });
    let summary = if dry_run && changes.saturating_add(conflicts) > 0 {
        format!("dry run: {outcome}, nothing written")
    } else {
        outcome
    };
    shell.status("Finished", GOOD, summary)?;
    if conflicts > 0 {
        shell.help(if dry_run {
            "run the command without `--dry-run`, then resolve the conflicts in .devset/conflicts/"
        } else {
            "resolve the files in .devset/conflicts/, then run `devset update --continue`;\n\
             or take the update back with `devset update --abort`"
        })?;
    }
    Ok(conflicts > 0)
}

/// Prints what taking an update back would do, and the changes since it that it would discard.
pub(crate) fn rollback(shell: &Shell, rollback: &Rollback) -> io::Result<()> {
    for revert in rollback.reverts() {
        let (verb, _, path) = revert_parts(revert);
        shell.status("Would", GOOD, format_args!("{verb} {path}"))?;
    }
    let summary = reverted(rollback.reverts(), ("to restore", "to remove"));
    shell.status("Finished", GOOD, format!("dry run: {summary}, nothing written"))?;
    if rollback.changed().is_empty() {
        return Ok(());
    }
    let text =
        format!("this discards changes made since the update to {}", list(rollback.changed()));
    shell.note(&text, Some("`devset update --abort` refuses them; add `--force` to discard them"))
}

/// Prints what taking an update back did.
pub(crate) fn rolled_back(shell: &Shell, reverts: &[Revert]) -> io::Result<()> {
    for revert in reverts {
        let (_, past, path) = revert_parts(revert);
        shell.status(past, GOOD, path)?;
    }
    let summary = reverted(reverts, ("restored", "removed"));
    shell.status("Finished", GOOD, format!("update aborted: {summary}"))
}

/// A revert's verb, for what would happen and what happened, and its path.
const fn revert_parts(revert: &Revert) -> (&'static str, &'static str, &RelPath) {
    match revert {
        Revert::Restore(path) => ("restore", "Restored", path),
        Revert::Remove(path) => ("remove", "Removed", path),
    }
}

/// How many files `reverts` restores and removes, with `words` for each.
fn reverted(reverts: &[Revert], (restore, remove): (&str, &str)) -> String {
    let restored = reverts.iter().filter(|r| matches!(r, Revert::Restore(_))).count();
    let removed = reverts.len().saturating_sub(restored);
    match (restored, removed) {
        (0, 0) => "no file to take back".to_owned(),
        (n, 0) => format!("{} {restore}", count(n, "file")),
        (0, m) => format!("{} {remove}", count(m, "file")),
        (n, m) => format!("{} {restore}, {m} {remove}", count(n, "file")),
    }
}

/// Tells the user which answers leave `.devset/answers.toml`, or would in a `dry_run`.
///
/// An answer leaves when no layer declares its variable any more.
pub(crate) fn dropped(shell: &Shell, dropped: &[VarName], dry_run: bool) -> io::Result<()> {
    if dropped.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = dropped.iter().map(|name| format!("`{name}`")).collect();
    let verb = if dry_run { "would drop" } else { "dropped" };
    let answers = if dropped.len() == 1 { "the answer" } else { "the answers" };
    let text = format!("{verb} {answers} to {}, which no layer declares now", list(&names));
    shell.note(&text, None)
}
