//! What happened, and where things stand: the apply log, `status`, its JSON, and `diff`.
//!
//! Here, what they share: what a step does to a file, where a path stands, how it compares with
//! its record, and the merge drivers a layer suggests.

mod diff;
mod explain;
mod features;
mod json;
mod list;
mod log;
mod status;

use alloc::collections::BTreeMap;
use core::fmt;
use std::io;

use anstyle::{AnsiColor, Style};
use clap_cargo::style::WARN;
use derive_more::Display;
use devset_core::name::FeatureName;
use devset_core::plan::Action;
use devset_core::profile::MergeSpec;
use devset_core::resolve::{Applied, Layer, Suggestion, Warning};
use devset_core::survey::{Drift, Entry};
use devset_core::{RelPath, Survey};
use serde::Serialize;

pub(crate) use self::diff::diff;
pub(crate) use self::explain::explain;
pub(crate) use self::features::features;
pub(crate) use self::list::list;
pub(crate) use self::log::{Wrote, applied, dropped, rollback, rolled_back};
pub(crate) use self::status::status;
use crate::shell::Shell;
use crate::words::count;

/// What a step does to a file, as users read it; displayed as its verb, for what would happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Display)]
#[serde(rename_all = "lowercase")]
#[display(rename_all = "lowercase")]
pub(crate) enum Change {
    /// Writes a file that was not there.
    Create,
    /// Starts tracking a file that was already there.
    Adopt,
    /// Writes the profile's new version over an untouched file.
    Update,
    /// Writes the profile's version over a local edit or deletion.
    Restore,
    /// Writes the profile's version over a file never tracked.
    Overwrite,
    /// Merges local edits with the profile's new version.
    Merge,
    /// Records the profile's version as the base, the file already matching.
    Record,
    /// Stops tracking a file no layer provides; the file stays, edited or the target's.
    Untrack,
    /// Deletes a file no layer provides, or takes the part out of it, as devset recorded it.
    Remove,
    /// A merge conflicted.
    Conflict,
}

impl Change {
    /// What `action` does to `entry`; `None` for [`Action::Keep`].
    pub(crate) fn of(entry: &Entry, action: Action) -> Option<Self> {
        Some(match action {
            Action::Keep => return None,
            Action::Untrack => Self::Untrack,
            Action::Remove => Self::Remove,
            // A merge without a record adopts a part, writing in the keys it lacked.
            Action::Record | Action::Merge if entry.record.is_none() => Self::Adopt,
            Action::Merge => Self::Merge,
            Action::Conflict => Self::Conflict,
            Action::Record => Self::Record,
            Action::Write => match (entry.drift(), entry.found.is_some()) {
                (None, false) => Self::Create,
                (None, true) => Self::Overwrite,
                (Some(Drift::Edited | Drift::Missing), _) => Self::Restore,
                (Some(Drift::Unchanged | Drift::Cosmetic), _) => Self::Update,
            },
        })
    }

    /// The verb, for what happened.
    const fn past(self) -> &'static str {
        match self {
            Self::Create => "Created",
            Self::Adopt => "Adopted",
            Self::Update => "Updated",
            Self::Restore => "Restored",
            Self::Overwrite => "Overwrote",
            Self::Merge => "Merged",
            Self::Record => "Recorded",
            Self::Untrack => "Untracked",
            Self::Remove => "Removed",
            Self::Conflict => "Conflicted",
        }
    }
}

/// The line that says every one of `files` matches the profile.
pub(crate) fn in_sync(files: usize) -> String {
    match files {
        0 => "The profile provides no files.".to_owned(),
        1 => "1 file matches the profile.".to_owned(),
        n => format!("All {} match the profile.", count(n, "file")),
    }
}

/// Where a path stands, which decides the part of `status` that lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Standing {
    /// A merge awaits resolution in `.devset/conflicts/`.
    Conflict,
    /// Only `apply --force` would change it: an `owned` file edited, deleted or never recorded.
    Drifted(Change),
    /// `apply` would change it.
    Pending(Change),
    /// Edited or deleted, and kept so by its policy.
    Local,
    /// Matches the profile.
    InSync,
}

impl Standing {
    /// Where `entry` stands.
    pub(crate) fn of(entry: &Entry) -> Self {
        if entry.conflict {
            return Self::Conflict;
        }
        let change = |mode| Change::of(entry, entry.action(mode));
        match (change(devset_core::Mode::Apply), change(devset_core::Mode::Force), entry.drift()) {
            (Some(change), ..) => Self::Pending(change),
            (None, Some(change), _) => Self::Drifted(change),
            (None, None, Some(Drift::Edited | Drift::Missing)) => Self::Local,
            (None, None, _) => Self::InSync,
        }
    }
}

/// How a path compares with its record, in a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Display)]
#[serde(rename_all = "lowercase")]
#[display(rename_all = "lowercase")]
pub(crate) enum State {
    /// A merge awaits resolution.
    Conflict,
    /// No layer provides it any more.
    Dropped,
    /// Byte-identical to its record.
    Unchanged,
    /// Differs from its record only in whitespace, line endings or a BOM.
    Cosmetic,
    /// Edited since recorded.
    Edited,
    /// Deleted since recorded.
    Missing,
    /// Neither recorded nor on disk.
    New,
    /// On disk, never recorded.
    Untracked,
}

impl State {
    /// Where `entry` stands against its record.
    pub(crate) fn of(entry: &Entry) -> Self {
        if entry.conflict {
            return Self::Conflict;
        }
        if entry.want.is_none() {
            return Self::Dropped;
        }
        match (entry.drift(), entry.found) {
            (Some(Drift::Unchanged), _) => Self::Unchanged,
            (Some(Drift::Cosmetic), _) => Self::Cosmetic,
            (Some(Drift::Edited), _) => Self::Edited,
            (Some(Drift::Missing), _) => Self::Missing,
            (None, None) => Self::New,
            (None, Some(_)) => Self::Untracked,
        }
    }

    /// Its colour.
    fn style(self) -> Style {
        let color = match self {
            Self::Unchanged => AnsiColor::Green,
            Self::Edited | Self::Missing | Self::Conflict => AnsiColor::Red,
            Self::Cosmetic => AnsiColor::BrightBlack,
            Self::Dropped | Self::New | Self::Untracked => AnsiColor::Yellow,
        };
        Style::new().fg_color(Some(color.into()))
    }
}

/// How many files `survey` manages, a file with parts once.
pub(crate) fn files(survey: &Survey) -> usize {
    let mut paths: Vec<&RelPath> = survey.entries().iter().map(|entry| &entry.path).collect();
    paths.dedup();
    paths.len()
}

/// Tells the user about merge drivers a layer suggests and the target has not configured.
pub(crate) fn suggestions(shell: &Shell, suggestions: &[Suggestion]) -> io::Result<()> {
    for Suggestion { layer, driver } in suggestions {
        let merge = MergeSpec { driver: Some(driver.clone()), ..MergeSpec::default() };
        let snippet =
            toml::to_string(&BTreeMap::from([("merge", merge)])).map_err(io::Error::other)?;
        let text = format!(
            "profile {layer} suggests a merge driver; devset never runs a program a profile names"
        );
        let help = format!("to use it, add to .devset/config.toml:{}", indent(snippet.trim_end()));
        shell.note(&text, Some(&help))?;
    }
    Ok(())
}

/// Tells the user what the graph settled that they may not expect.
pub(crate) fn warnings(shell: &Shell, warnings: &[Warning]) -> io::Result<()> {
    for warning in warnings {
        match warning {
            Warning::DefaultsOn { profile, by } => shell.warn(
                &format!(
                    "profile {by} turns on the default features of {profile}, which its layer \
                     switches off"
                ),
                Some(&format!("`devset features {profile}` shows what is on, and why")),
            )?,
        }
    }
    Ok(())
}

/// A layer's one-line summary: `source/name`, version, where an unnamed source is, commit,
/// features, and whether it is applied.
pub(crate) struct Heading<'a>(pub(crate) &'a Layer);

impl fmt::Display for Heading<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (layer, bold) = (self.0, Style::new().bold());
        write!(f, "{bold}{}{bold:#}", layer.qualified())?;
        if let Some(version) = &layer.meta().version {
            write!(f, " {version}")?;
        }
        if layer.source_name().is_none() {
            write!(f, "  {}", layer.source())?;
        }
        if let Some(rev) = layer.rev() {
            write!(f, "  @{}", rev.short())?;
        }
        if !layer.features().is_empty() {
            let features: Vec<&str> = layer.features().keys().map(FeatureName::as_str).collect();
            write!(f, "  [{}]", features.join(", "))?;
        }
        if !layer.configured()
            && let Some(by) = layer.required_by().first()
        {
            write!(f, "  {WARN}(required by {by}){WARN:#}")?;
        }
        match layer.applied() {
            Applied::Current => {},
            Applied::Changed => write!(f, "  {WARN}(changed since it was applied){WARN:#}")?,
            Applied::Never => write!(f, "  {WARN}(not applied yet){WARN:#}")?,
        }
        Ok(())
    }
}

/// `text` on new lines, each indented.
fn indent(text: &str) -> String {
    text.lines().flat_map(|line| ["\n    ", line]).collect()
}
