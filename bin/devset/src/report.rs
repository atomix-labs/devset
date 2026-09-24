//! What happened, and where things stand: the apply log, `status`, and its JSON.

use alloc::collections::BTreeMap;
use core::{fmt, str};
use std::io::{self, Write};

use anstyle::{AnsiColor, Style};
use clap_cargo::style::{ERROR, GOOD, LITERAL, WARN};
use derive_more::Display;
use devset_core::plan::{Action, Step};
use devset_core::profile::{MergeSpec, OnConflict, Policy, VarName};
use devset_core::resolve::{Applied, Layer, Suggestion};
use devset_core::source::{Oid, Source};
use devset_core::survey::{Digest, Drift, Entry, Scope};
use devset_core::{Error, RelPath, Revert, Rollback, Survey, Target};
use diffy::{DiffOptions, PatchFormatter};
use serde::Serialize;

use crate::shell::Shell;
use crate::words::{count, list};

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
        match (
            change(devset_core::Mode::Apply),
            change(devset_core::Mode::Force),
            entry.drift(),
        ) {
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

/// `text` styled as a command, backticks kept so it stands out without colour too.
fn literal(text: &str) -> String {
    format!("{LITERAL}`{text}`{LITERAL:#}")
}

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
        let (status, style, message) = match wrote {
            Wrote::All => (change.past(), GOOD, path.to_string()),
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
    shell.status(
        "Finished",
        GOOD,
        format!("dry run: {summary}, nothing written"),
    )?;
    if rollback.changed().is_empty() {
        return Ok(());
    }
    let text = format!(
        "this discards changes made since the update to {}",
        list(rollback.changed())
    );
    shell.note(
        &text,
        Some("`devset update --abort` refuses them; add `--force` to discard them"),
    )
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
    let restored = reverts
        .iter()
        .filter(|r| matches!(r, Revert::Restore(_)))
        .count();
    let removed = reverts.len().saturating_sub(restored);
    match (restored, removed) {
        (0, 0) => "no file to take back".to_owned(),
        (n, 0) => format!("{} {restore}", count(n, "file")),
        (0, m) => format!("{} {remove}", count(m, "file")),
        (n, m) => format!("{} {restore}, {m} {remove}", count(n, "file")),
    }
}

/// Prints how each managed file, or each of `paths`, differs from the profile's version.
///
/// A unified diff from the profile's lines to yours; a file that matches prints nothing, even
/// when its whitespace differs. A part is shown as its whole file, the part as the profile has
/// it, so the diff is the part's alone.
pub(crate) fn diff(survey: &Survey, target: &Target, paths: &[RelPath]) -> Result<(), Error> {
    let resolved = survey.resolved();
    let formatter = PatchFormatter::new().with_color();
    let mut out = anstream::stdout().lock();
    for entry in survey.entries() {
        if !paths.is_empty() && !paths.contains(&entry.path) {
            continue;
        }
        let Some(want) = entry.want else { continue };
        if entry
            .found
            .is_some_and(|found| found.same_content(&want.fingerprint))
        {
            continue;
        }
        let Some(profile) = survey.wanted(entry, target)? else {
            continue;
        };
        let disk = match fs_err::read(entry.path.under(target.root())) {
            Ok(disk) => Some(disk),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let yours = match disk {
            Some(_) => format!("{}  (yours)", entry.path),
            None => "/dev/null".to_owned(),
        };
        let disk = disk.unwrap_or_default();
        let (Ok(profile), Ok(disk)) = (text(&profile), text(&disk)) else {
            writeln!(out, "Binary files differ: {entry}")?;
            continue;
        };
        let layer = resolved.provider(entry).unwrap_or_default();
        let patch = DiffOptions::new()
            .set_original_filename(format!("{}  (profile {layer})", entry.path))
            .set_modified_filename(yours)
            .create_patch(profile, disk);
        write!(out, "{}", formatter.fmt_patch(&patch))?;
    }
    Ok(())
}

/// `bytes` as text, unless they are binary by git's test or not UTF-8.
fn text(bytes: &[u8]) -> Result<&str, ()> {
    if bytes.get(..8000).unwrap_or(bytes).contains(&0) {
        return Err(());
    }
    str::from_utf8(bytes).map_err(drop)
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
    let answers = if dropped.len() == 1 {
        "the answer"
    } else {
        "the answers"
    };
    let text = format!(
        "{verb} {answers} to {}, which no layer declares now",
        list(&names)
    );
    shell.note(&text, None)
}

/// Tells the user about merge drivers a layer suggests and the target has not configured.
pub(crate) fn suggestions(shell: &Shell, suggestions: &[Suggestion]) -> io::Result<()> {
    for Suggestion { layer, driver } in suggestions {
        let merge = MergeSpec {
            driver: Some(driver.clone()),
            ..MergeSpec::default()
        };
        let snippet =
            toml::to_string(&BTreeMap::from([("merge", merge)])).map_err(io::Error::other)?;
        let text = format!(
            "profile {layer} suggests a merge driver; devset never runs a program a profile names"
        );
        let help = format!(
            "to use it, add to .devset/config.toml:{}",
            indent(snippet.trim_end())
        );
        shell.note(&text, Some(&help))?;
    }
    Ok(())
}

/// `text` on new lines, each indented.
fn indent(text: &str) -> String {
    text.lines().flat_map(|line| ["\n    ", line]).collect()
}

/// A `status` section: its heading, that heading while an update is unfinished, its standings.
type Section = (&'static str, &'static str, fn(Standing) -> bool);

/// The sections of `status`, in order.
const SECTIONS: [Section; 5] = [
    (
        "Conflicts — resolve in .devset/conflicts/, then run `devset update --continue`:",
        "Conflicts — resolve in .devset/conflicts/, then run `devset update --continue`:",
        |s| s == Standing::Conflict,
    ),
    (
        "Drifted — `devset apply --force` restores:",
        "Drifted — once the update is finished, `devset apply --force` restores:",
        |s| matches!(s, Standing::Drifted(_)),
    ),
    (
        "Pending — `devset apply` will:",
        "Pending — `devset update --continue` will:",
        |s| matches!(s, Standing::Pending(_)),
    ),
    ("Local changes, kept:", "Local changes, kept:", |s| {
        s == Standing::Local
    }),
    ("In sync:", "In sync:", |s| s == Standing::InSync),
];

/// Prints `survey`: what needs doing, grouped by the command that does it.
pub(crate) fn status(shell: &Shell, survey: &Survey, json: bool, verbose: bool) -> io::Result<()> {
    let mut out = anstream::stdout().lock();
    if json {
        serde_json::to_writer_pretty(&mut out, &StatusJson::of(survey))?;
        return writeln!(out);
    }
    let resolved = survey.resolved();
    headings(&mut out, resolved.layers(), verbose)?;
    if verbose {
        writeln!(out, "  merge driver: {}", resolved.settings().driver)?;
        for (name, answer) in resolved.answers() {
            writeln!(out, "  {name} = {answer:?}")?;
        }
    }
    let rows: Vec<(&Entry, Standing)> = survey
        .entries()
        .iter()
        .map(|e| (e, Standing::of(e)))
        .collect();
    let listed = |standing: Standing| verbose || standing != Standing::InSync;
    let width = rows
        .iter()
        .filter(|(_, s)| listed(*s))
        .map(|(e, _)| e.to_string().len())
        .max()
        .unwrap_or(0);
    let layered = resolved.layers().len() > 1;
    let unfinished = survey.unfinished();
    for (heading, while_unfinished, belongs) in SECTIONS {
        let heading = if unfinished {
            while_unfinished
        } else {
            heading
        };
        let members: Vec<_> = rows
            .iter()
            .filter(|(_, s)| belongs(*s) && listed(*s))
            .collect();
        if members.is_empty() {
            continue;
        }
        writeln!(out, "\n{}", Emphasis(heading))?;
        for &&(entry, standing) in &members {
            let state = State::of(entry);
            let (style, path, name) = (state.style(), entry.path.as_str(), entry.to_string());
            // As strings, since a derived `Display` ignores the width the columns need.
            let state = state.to_string();
            let policy = entry
                .want
                .map_or_else(String::new, |want| want.policy.to_string());
            let action = match standing {
                Standing::Pending(change) | Standing::Drifted(change) => format!("  {change}"),
                Standing::Conflict => format!("  → .devset/conflicts/{path}"),
                Standing::Local | Standing::InSync => String::new(),
            };
            // A part is named after its layer already.
            let layer = resolved
                .provider(entry)
                .filter(|_| layered && entry.part.is_none())
                .map(|layer| format!("  ({layer})"));
            let layer = layer.unwrap_or_default();
            let row = format!(
                "    {style}{state:<9}{style:#}  {name:<width$}  {policy:<5}{action}{layer}"
            );
            writeln!(out, "{}", row.trim_end())?;
        }
    }
    if rows.iter().all(|(_, s)| *s == Standing::InSync) {
        writeln!(out, "\n{GOOD}{}{GOOD:#}", in_sync(files(survey)))?;
    }
    drop(out);
    suggestions(shell, resolved.suggestions())
}

/// How many files `survey` manages, a file with parts once.
pub(crate) fn files(survey: &Survey) -> usize {
    let mut paths: Vec<&RelPath> = survey.entries().iter().map(|entry| &entry.path).collect();
    paths.dedup();
    paths.len()
}

/// Writes each layer's heading. Unless `verbose`, a required layer applied as it is goes on a
/// `requires` line under the configured layer that brings it in.
fn headings(out: &mut impl Write, layers: &[Layer], verbose: bool) -> io::Result<()> {
    let folded = |layer: &Layer| {
        !verbose && layer.required_by().is_some() && layer.applied() == Applied::Current
    };
    // The configured layer a required one comes from, however deep.
    let root = |layer: &Layer| {
        let mut at = layer;
        for _ in layers {
            let Some(by) = at.required_by() else { break };
            let Some(parent) = layers.iter().find(|l| l.meta().name == by) else {
                break;
            };
            at = parent;
        }
        at.meta().name.clone()
    };
    for layer in layers.iter().filter(|layer| !folded(layer)) {
        writeln!(out, "{}", Heading(layer))?;
        if layer.required_by().is_some() {
            continue;
        }
        let name = &layer.meta().name;
        let required: Vec<&str> = layers
            .iter()
            .filter(|l| folded(l) && root(l) == *name)
            .map(|l| l.meta().name.as_str())
            .collect();
        if !required.is_empty() {
            writeln!(out, "{}", wrapped("  requires ", &required))?;
        }
    }
    Ok(())
}

/// `lead` and then `names`, comma-separated and wrapped under the first.
fn wrapped(lead: &str, names: &[&str]) -> String {
    let indent = " ".repeat(lead.len());
    let options = textwrap::Options::new(100)
        .initial_indent(lead)
        .subsequent_indent(&indent)
        .break_words(false);
    textwrap::fill(&names.join(", "), options)
}

/// A section heading, commands in backticks styled as literals.
struct Emphasis<'a>(&'a str);

impl fmt::Display for Emphasis<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bold = Style::new().bold();
        for (i, part) in self.0.split('`').enumerate() {
            if i % 2 == 1 {
                f.write_str(&literal(part))?;
            } else {
                write!(f, "{bold}{part}{bold:#}")?;
            }
        }
        Ok(())
    }
}

/// A layer's one-line summary: name, version, source, commit, and whether it is applied.
struct Heading<'a>(&'a Layer);

impl fmt::Display for Heading<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (layer, bold) = (self.0, Style::new().bold());
        write!(f, "{bold}{}{bold:#}", layer.meta().name)?;
        if let Some(version) = &layer.meta().version {
            write!(f, " {version}")?;
        }
        write!(f, "  {}", layer.source())?;
        if let Some(rev) = layer.rev() {
            write!(f, "  @{}", rev.short())?;
        }
        if let Some(by) = layer.required_by() {
            write!(f, "  {WARN}(required by {by}){WARN:#}")?;
        }
        match layer.applied() {
            Applied::Current => {}
            Applied::Changed => write!(f, "  {WARN}(changed since it was applied){WARN:#}")?,
            Applied::Never => write!(f, "  {WARN}(not applied yet){WARN:#}")?,
        }
        Ok(())
    }
}

/// `status --json`.
#[derive(Serialize)]
struct StatusJson<'a> {
    /// The layers, in order.
    layers: Vec<LayerJson<'a>>,
    /// Every managed path.
    files: Vec<FileJson<'a>>,
    /// The settings in force.
    settings: SettingsJson,
    /// The answer to every declared variable.
    answers: &'a BTreeMap<VarName, String>,
    /// Merge drivers the layers suggest that the target has not configured.
    suggestions: Vec<SuggestionJson<'a>>,
    /// Whether `apply --force` would write a file.
    drifted: bool,
    /// Whether an update is unfinished: conflicts wait in `.devset/conflicts/`.
    unfinished: bool,
}

/// A layer in `status --json`.
#[derive(Serialize)]
struct LayerJson<'a> {
    /// Profile name.
    name: &'a str,
    /// Profile version.
    version: Option<&'a str>,
    /// Where it came from, as configured.
    source: &'a Source,
    /// Commit, for a versioned source.
    rev: Option<&'a Oid>,
    /// Content digest.
    digest: Digest,
    /// Whether this content is what the lock recorded.
    applied: Applied,
}

/// A file, or a part of one, in `status --json`.
#[derive(Serialize)]
struct FileJson<'a> {
    /// The path.
    path: &'a RelPath,
    /// How much of the file this is.
    scope: Scope,
    /// The profile whose part of the file this is; `None` for the whole file.
    part: Option<&'a str>,
    /// The layer providing it.
    layer: Option<&'a str>,
    /// Its policy, while a layer provides it.
    policy: Option<Policy>,
    /// How it compares with its record.
    state: State,
    /// What `apply` would do.
    apply: Option<Change>,
    /// What `apply --force` would do.
    force: Option<Change>,
}

/// Settings in `status --json`.
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct SettingsJson {
    /// What an update does when a file conflicts.
    on_conflict: OnConflict,
    /// The merge driver: `builtin`, or its command line.
    driver: String,
}

/// A suggested driver in `status --json`.
#[derive(Serialize)]
struct SuggestionJson<'a> {
    /// The suggesting layer.
    layer: &'a str,
    /// Its driver line.
    driver: String,
}

impl<'a> StatusJson<'a> {
    /// The report for `survey`.
    fn of(survey: &'a Survey) -> Self {
        let resolved = survey.resolved();
        let change = |entry: &Entry, mode| Change::of(entry, entry.action(mode));
        Self {
            layers: resolved
                .layers()
                .iter()
                .map(|layer| LayerJson {
                    name: &layer.meta().name,
                    version: layer.meta().version.as_deref(),
                    source: layer.source(),
                    rev: layer.rev(),
                    digest: layer.digest(),
                    applied: layer.applied(),
                })
                .collect(),
            files: survey
                .entries()
                .iter()
                .map(|entry| FileJson {
                    path: &entry.path,
                    scope: entry.scope(),
                    part: entry.part.as_ref().map(|part| part.owner.as_str()),
                    layer: resolved.provider(entry),
                    policy: entry.want.map(|want| want.policy),
                    state: State::of(entry),
                    apply: change(entry, devset_core::Mode::Apply),
                    force: change(entry, devset_core::Mode::Force),
                })
                .collect(),
            settings: SettingsJson {
                on_conflict: resolved.settings().on_conflict,
                driver: resolved.settings().driver.to_string(),
            },
            answers: resolved.answers(),
            suggestions: resolved
                .suggestions()
                .iter()
                .map(|s| SuggestionJson {
                    layer: &s.layer,
                    driver: s.driver.to_string(),
                })
                .collect(),
            drifted: survey.drifted(),
            unfinished: survey.unfinished(),
        }
    }
}
