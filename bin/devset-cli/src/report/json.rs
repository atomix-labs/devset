//! `status --json`: the survey as scripts and CI read it.

use alloc::collections::BTreeMap;

use devset_core::profile::{OnConflict, Policy, VarName};
use devset_core::resolve::Applied;
use devset_core::source::{Oid, Source};
use devset_core::survey::{Digest, Entry, Scope};
use devset_core::{RelPath, Survey};
use serde::Serialize;

use super::{Change, State};

/// `status --json`.
#[derive(Serialize)]
pub(super) struct StatusJson<'a> {
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
    pub(super) fn of(survey: &'a Survey) -> Self {
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
                .map(|s| SuggestionJson { layer: &s.layer, driver: s.driver.to_string() })
                .collect(),
            drifted: survey.drifted(),
            unfinished: survey.unfinished(),
        }
    }
}
