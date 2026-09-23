//! What happened, and where things stand: the apply log, `status`, and its JSON.

use alloc::collections::BTreeMap;
use core::fmt;
use std::io::{self, Write};

use anstyle::{AnsiColor, Style};
use clap_cargo::style::{ERROR, GOOD, LITERAL, WARN};
use devset_core::plan::{Action, Step};
use devset_core::profile::{MergeSpec, OnConflict, Policy, VarName};
use devset_core::resolve::{Applied, Layer, Suggestion};
use devset_core::source::{Oid, Source};
use devset_core::survey::{Digest, Drift, Entry};
use devset_core::{RelPath, Survey};
use serde::Serialize;

use crate::shell::Shell;

/// What a step does to a file, as users read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
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
    /// Stops tracking a file no layer provides.
    Release,
    /// A merge conflicted.
    Conflict,
}

impl Change {
    /// What `action` does to `entry`; `None` for [`Action::Keep`].
    pub(crate) fn of(entry: &Entry, action: Action) -> Option<Self> {
        Some(match action {
            Action::Keep => return None,
            Action::Release => Self::Release,
            Action::Merge => Self::Merge,
            Action::Conflict => Self::Conflict,
            Action::Record if entry.record.is_none() => Self::Adopt,
            Action::Record => Self::Record,
            Action::Write => match (entry.drift(), entry.found.is_some()) {
                (None, false) => Self::Create,
                (None, true) => Self::Overwrite,
                (Some(Drift::Edited | Drift::Missing), _) => Self::Restore,
                (Some(Drift::Unchanged | Drift::Cosmetic), _) => Self::Update,
            },
        })
    }

    /// The verb, for what would happen.
    pub(crate) const fn verb(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Adopt => "adopt",
            Self::Update => "update",
            Self::Restore => "restore",
            Self::Overwrite => "overwrite",
            Self::Merge => "merge",
            Self::Record => "record",
            Self::Release => "release",
            Self::Conflict => "conflict",
        }
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
            Self::Release => "Released",
            Self::Conflict => "Conflicted",
        }
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
pub(crate) fn state(entry: &Entry) -> &'static str {
    if entry.conflict {
        return "conflict";
    }
    if entry.want.is_none() {
        return "vanished";
    }
    match (entry.drift(), entry.found) {
        (Some(Drift::Unchanged), _) => "unchanged",
        (Some(Drift::Cosmetic), _) => "cosmetic",
        (Some(Drift::Edited), _) => "edited",
        (Some(Drift::Missing), _) => "missing",
        (None, None) => "new",
        (None, Some(_)) => "untracked",
    }
}

/// `text` styled as a command, backticks kept so it stands out without colour too.
fn literal(text: &str) -> String {
    format!("{LITERAL}`{text}`{LITERAL:#}")
}

/// Prints the log of `steps`; returns whether any conflicted.
pub(crate) fn applied(
    shell: &Shell, steps: &[Step], dry_run: bool, held: bool,
) -> io::Result<bool> {
    let (mut changes, mut conflicts) = (0_usize, 0_usize);
    for step in steps {
        let Some(change) = Change::of(&step.entry, step.action) else { continue };
        let path = &step.entry.path;
        if change == Change::Conflict {
            conflicts = conflicts.saturating_add(1);
            let why = step.note.as_deref().unwrap_or("conflicting changes");
            shell.always("Conflicted", ERROR, format_args!("{path}  {why}"))?;
        } else if dry_run {
            changes = changes.saturating_add(1);
            shell.status(&format!("Would {}", change.verb()), GOOD, path)?;
        } else if held {
            changes = changes.saturating_add(1);
            shell.status("Withheld", WARN, format_args!("{path}  would {}", change.verb()))?;
        } else {
            changes = changes.saturating_add(1);
            shell.status(change.past(), GOOD, path)?;
        }
    }
    let summary = match (changes, conflicts, dry_run, held) {
        (0, 0, ..) => "up to date".to_owned(),
        (n, _, true, _) => format!("dry run: {}, nothing written", count(n, "change")),
        (n, c, false, true) => {
            format!(
                "{}; {} withheld by `on-conflict = \"apply-none\"`",
                count(c, "conflict"),
                count(n, "change")
            )
        },
        (0, c, false, false) => count(c, "conflict"),
        (n, 0, false, false) => count(n, "change"),
        (n, c, false, false) => format!("{}, {}", count(n, "change"), count(c, "conflict")),
    };
    shell.status("Finished", GOOD, summary)?;
    if conflicts > 0 {
        shell
            .help("resolve the files in .devset/conflicts/, then run `devset update --continue`")?;
    }
    Ok(conflicts > 0)
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

/// `text` on new lines, each indented.
fn indent(text: &str) -> String {
    text.lines().flat_map(|line| ["\n    ", line]).collect()
}

/// `n` and `noun`, pluralised.
fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

/// A `status` section: its heading, and which standings it lists.
type Section = (&'static str, fn(Standing) -> bool);

/// The sections of `status`, in order.
const SECTIONS: [Section; 5] = [
    ("Conflicts — resolve in .devset/conflicts/, then run `devset update --continue`:", |s| {
        s == Standing::Conflict
    }),
    ("Drifted — `devset apply --force` restores:", |s| matches!(s, Standing::Drifted(_))),
    ("Pending — `devset apply` will:", |s| matches!(s, Standing::Pending(_))),
    ("Local changes, kept:", |s| s == Standing::Local),
    ("In sync:", |s| s == Standing::InSync),
];

/// Prints `survey`: what needs doing, grouped by the command that does it.
pub(crate) fn status(shell: &Shell, survey: &Survey, json: bool, verbose: bool) -> io::Result<()> {
    let mut out = anstream::stdout().lock();
    if json {
        serde_json::to_writer_pretty(&mut out, &StatusJson::of(survey))?;
        return writeln!(out);
    }
    let resolved = survey.resolved();
    for layer in resolved.layers() {
        writeln!(out, "{}", Heading(layer))?;
    }
    if verbose {
        writeln!(out, "  merge driver: {}", resolved.settings().driver)?;
        for (name, answer) in resolved.answers() {
            writeln!(out, "  {name} = {answer:?}")?;
        }
    }
    let rows: Vec<(&Entry, Standing)> =
        survey.entries().iter().map(|e| (e, Standing::of(e))).collect();
    let listed = |standing: Standing| verbose || standing != Standing::InSync;
    let width = rows
        .iter()
        .filter(|(_, s)| listed(*s))
        .map(|(e, _)| e.path.as_str().len())
        .max()
        .unwrap_or(0);
    let layered = resolved.layers().len() > 1;
    for (heading, belongs) in SECTIONS {
        let members: Vec<_> = rows.iter().filter(|(_, s)| belongs(*s) && listed(*s)).collect();
        if members.is_empty() {
            continue;
        }
        writeln!(out, "\n{}", Emphasis(heading))?;
        for &&(entry, standing) in &members {
            let style = state_style(state(entry));
            let (state, path) = (state(entry), entry.path.as_str());
            let policy = entry.want.map_or("", |want| want.policy.as_str());
            let action = match standing {
                Standing::Pending(change) | Standing::Drifted(change) => {
                    format!("  {}", change.verb())
                },
                Standing::Conflict => format!("  → .devset/conflicts/{path}"),
                Standing::Local | Standing::InSync => String::new(),
            };
            let layer = resolved
                .provider(&entry.path)
                .filter(|_| layered)
                .map(|layer| format!("  ({layer})"));
            let layer = layer.unwrap_or_default();
            let row = format!(
                "    {style}{state:<9}{style:#}  {path:<width$}  {policy:<5}{action}{layer}"
            );
            writeln!(out, "{}", row.trim_end())?;
        }
    }
    if rows.iter().all(|(_, s)| *s == Standing::InSync) {
        let files = count(rows.len(), "file");
        let matched = if files == "1 file" {
            "1 file matches".to_owned()
        } else {
            format!("All {files} match")
        };
        writeln!(out, "\n{GOOD}{matched} the profile.{GOOD:#}")?;
    }
    drop(out);
    suggestions(shell, resolved.suggestions())
}

/// A state's colour.
fn state_style(state: &str) -> Style {
    let color = match state {
        "unchanged" => AnsiColor::Green,
        "edited" | "missing" | "conflict" => AnsiColor::Red,
        "cosmetic" => AnsiColor::BrightBlack,
        _ => AnsiColor::Yellow,
    };
    Style::new().fg_color(Some(color.into()))
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
        if let Some(rev) = layer.rev().and_then(|rev| rev.as_str().get(..7)) {
            write!(f, "  @{rev}")?;
        }
        match layer.applied() {
            Applied::Current => {},
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

/// A path in `status --json`.
#[derive(Serialize)]
struct FileJson<'a> {
    /// The path.
    path: &'a RelPath,
    /// The layer providing it.
    layer: Option<&'a str>,
    /// Its policy, while a layer provides it.
    policy: Option<Policy>,
    /// How it compares with its record.
    state: &'static str,
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
    driver: &'a str,
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
                    layer: resolved.provider(&entry.path),
                    policy: entry.want.map(|want| want.policy),
                    state: state(entry),
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
                .map(|s| SuggestionJson { layer: &s.layer, driver: &s.driver })
                .collect(),
            drifted: survey.drifted(),
        }
    }
}
