//! Apply versioned file bundles to a directory, and update them without losing local edits.
//!
//! Run from anywhere inside a target; `init` makes one, and `add` starts one where there is none.
//! What a command answers goes to stdout; its log, diagnostics and progress to stderr. The exit
//! code is 0 on success, 1 on a conflict or, under `status --exit-code`, on drift, and 2 on an
//! error.
//!
//! ```text
//! devset add atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.8.0
//! devset add atxp/mdbook --features katex
//! devset status --exit-code
//! devset update
//! ```

extern crate alloc;

mod ask;
mod cli;
mod github;
mod help;
mod report;
mod shell;
mod skeleton;
mod words;

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use clap::{CommandFactory, Parser};
use clap_cargo::style::GOOD;
use devset_core::collection::{self, Listing};
use devset_core::name::{FeatureName, ProfileName, ProfileRef, SourceName};
use devset_core::profile::VarName;
use devset_core::source::{GitRef, Source, SourceSpec};
use devset_core::target::LayerSpec;
use devset_core::{
    Cache, Error, Mode, ProfileError, Refresh, RelPath, Rollback, Survey, Target, TargetError,
    commit, plan, resolve, survey,
};
use dialoguer::console;

use crate::cli::{AddArgs, Answers, Cli, Command, LayerArg, Location};
use crate::report::Wrote;
use crate::shell::Shell;

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.no_color {
        anstream::ColorChoice::Never.write_global();
        console::set_colors_enabled(false);
        console::set_colors_enabled_stderr(false);
    }
    let shell = Shell::new(cli.quiet, cli.no_input);
    let error = match run(&shell, cli.command) {
        Ok(code) => return code,
        Err(error) => error,
    };
    // A reader that stops early, as `devset status | head` does, is not an error.
    if let Error::Io(e) = &error
        && e.kind() == io::ErrorKind::BrokenPipe
    {
        return ExitCode::SUCCESS;
    }
    match shell.error(&error) {
        Ok(()) | Err(_) => ExitCode::from(2),
    }
}

/// Runs `command`, returning the exit code for a clean finish.
fn run(shell: &Shell, command: Command) -> Result<ExitCode, Error> {
    let cwd = Utf8PathBuf::try_from(env::current_dir()?).map_err(io::Error::other)?;
    // Built for the commands that read sources only: nothing else needs a home directory.
    let cache = || -> Result<Cache, Error> {
        Ok(Cache::user()?.prompting(shell.interactive()).on_fetch(shell.fetches()))
    };
    match command {
        Command::Init { path, profile, collection, dry_run } => {
            let dir = path.unwrap_or_else(|| Utf8PathBuf::from("."));
            match (profile, collection) {
                (true, _) => author(shell, (&cwd, &dir), skeleton::profile),
                (_, true) => author(shell, (&cwd, &dir), skeleton::collection),
                _ => init(shell, (&cwd, &dir), &cache()?, dry_run),
            }
        },
        Command::Add { add, default_features, answers, dry_run } => {
            let mut target = found_or_started(shell, &cwd, dry_run)?;
            let cache = cache()?;
            let defaults = match (add.no_default_features, default_features) {
                (true, _) => Some(false),
                (false, true) => Some(true),
                (false, false) => None,
            };
            if let Some(layer) = layer(&mut target, &cache, add, &cwd)? {
                let applied = target.layer(&layer.profile.profile).map(|l| l.profile.clone());
                if applied.as_ref() == Some(&layer.profile) {
                    turn_on(shell, &mut target, &layer, defaults, dry_run)?;
                } else {
                    target.add_layer(layer)?;
                }
            }
            apply(shell, &mut target, &cache, (Refresh::None, Mode::Apply), answers, dry_run)
        },
        Command::Remove { layer, features, dry_run } => {
            let mut target = Target::find(&cwd)?;
            let cache = cache()?;
            if features.is_empty() {
                remove(shell, &mut target, &cache, &layer, dry_run)?;
            } else {
                turn_off(shell, &mut target, &cache, (&layer, &features), dry_run)?;
            }
            let how = (Refresh::None, Mode::Apply);
            apply(shell, &mut target, &cache, how, Answers::default(), dry_run)
        },
        Command::Diff { paths } => {
            let target = Target::find(&cwd)?;
            let survey = survey(resolve(&target, &cache()?, Refresh::None)?, &target)?;
            let paths = managed(&survey, &target, &cwd, &paths)?;
            report::diff(&survey, &target, &paths)?;
            Ok(ExitCode::SUCCESS)
        },
        Command::Status { exit_code, json, verbose } => {
            status(shell, &cwd, &cache()?, exit_code, json, verbose)
        },
        Command::Apply { abort: true, force, dry_run, .. } => abort(shell, &cwd, force, dry_run),
        Command::Apply { resume: true, answers, dry_run, .. } => {
            let how = (Refresh::None, Mode::Continue);
            apply(shell, &mut Target::find(&cwd)?, &cache()?, how, answers, dry_run)
        },
        Command::Apply { force, rescaffold, answers, dry_run, .. } => {
            let mode = if force { Mode::Force } else { Mode::Apply };
            let target = &mut Target::find(&cwd)?;
            for scaffold in rescaffold {
                target.rescaffold(scaffold);
            }
            apply(shell, target, &cache()?, (Refresh::None, mode), answers, dry_run)
        },
        Command::Update { name, pin, answers, dry_run } => {
            let mut target = Target::find(&cwd)?;
            if let (Some(given), Some(at)) = (name.as_deref(), pin.git_ref()?) {
                let source = source_named(&target, given)?;
                target.repin(&source, at)?;
            }
            let (cache, only) = (cache()?, name.as_deref());
            let how = (only.map_or(Refresh::All, Refresh::Only), Mode::Apply);
            let code = apply(shell, &mut target, &cache, how, answers, dry_run)?;
            newer(shell, &target, &cache, only)?;
            Ok(code)
        },
        Command::Explain { name } => {
            let target = Target::find(&cwd)?;
            let survey = survey(resolve(&target, &cache()?, Refresh::None)?, &target)?;
            match name.map(|given| explained(&survey, &target, &cwd, &given)).transpose()? {
                None => report::features(survey.resolved(), None)?,
                Some(Explained::Layer { name, file }) => {
                    report::features(survey.resolved(), Some(&name))?;
                    if file {
                        let text = format!("{name} is also a file devset manages");
                        shell
                            .note(&text, Some(&format!("`devset explain ./{name}` explains it")))?;
                    }
                },
                Some(Explained::File(path)) => report::explain(&survey, &path)?,
            }
            Ok(ExitCode::SUCCESS)
        },
        Command::List { source, location } => list(&cwd, &cache()?, source.as_ref(), location),
        Command::Completions { shell } => completions(shell),
    }
}

/// Creates, in `dir` relative to `cwd`, what `write` writes: a profile or a collection to author.
fn author(
    shell: &Shell, (cwd, dir): (&Utf8Path, &Utf8Path),
    write: fn(&Utf8Path, &Utf8Path) -> Result<skeleton::Authored, Error>,
) -> Result<ExitCode, Error> {
    let full = lexical(&cwd.join(dir)).unwrap_or_else(|| cwd.join(dir));
    let authored = write(&full, dir)?;
    shell.status("Created", GOOD, &authored.what)?;
    shell.help(&authored.next)?;
    Ok(ExitCode::SUCCESS)
}

/// Makes `dir`, relative to `cwd`, a target, its `config.toml` a commented skeleton; notes one
/// that is a target already.
fn init(
    shell: &Shell, (cwd, dir): (&Utf8Path, &Utf8Path), cache: &Cache, dry_run: bool,
) -> Result<ExitCode, Error> {
    let root = lexical(&cwd.join(dir)).unwrap_or_else(|| cwd.join(dir));
    let target = Target::open_or_new(&root)?;
    let config = shown(dir, ".devset/config.toml");
    if target.exists() {
        shell.note(&format!("{config} is there already: this is a target"), Some(ADD_A_LAYER))?;
        return Ok(ExitCode::SUCCESS);
    }
    if !dry_run {
        let survey = survey(resolve(&target, cache, Refresh::None)?, &target)?;
        commit(plan(survey, Mode::Apply, &target)?, &target)?;
    }
    let (verb, what) = if dry_run { ("Would", "create ") } else { ("Created", "") };
    shell.status(verb, GOOD, format_args!("{what}{config}"))?;
    shell.help(ADD_A_LAYER)?;
    Ok(ExitCode::SUCCESS)
}

/// The next step in a target that applies nothing yet.
const ADD_A_LAYER: &str =
    "add a layer: `devset add <source>/<profile> --git <url>`, or `--path <dir>`";

/// `rel` in `dir`, as a message names it: `rel` alone when `dir` is this directory.
fn shown(dir: &Utf8Path, rel: &str) -> Utf8PathBuf {
    if dir == Utf8Path::new(".") { Utf8PathBuf::from(rel) } else { dir.join(rel) }
}

/// The target containing `cwd`; or, where there is none and `cwd` may start one, a new target
/// there, which the first commit writes.
fn found_or_started(shell: &Shell, cwd: &Utf8Path, dry_run: bool) -> Result<Target, Error> {
    let found = Target::find(cwd);
    if let Err(Error::Target(TargetError::NotFound { .. })) = &found
        && startable(cwd)
    {
        let (verb, what) = if dry_run { ("Would", "start ") } else { ("Starting", "") };
        shell.status(verb, GOOD, format_args!("{what}a target: .devset/config.toml"))?;
        return Target::open_or_new(cwd);
    }
    found
}

/// Whether `dir` may start a target unasked: a git repository's top level, or an empty directory.
fn startable(dir: &Utf8Path) -> bool {
    dir.join(".git").exists()
        || fs_err::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none())
}

/// The layer `add` names, its source named in `target` when `add` locates a new one, written
/// from `cwd`; `None` when `add` names nothing.
///
/// A profile named alone is in the source `add` locates, or in the target's only source. A
/// source `add` locates is named as the target names it already, as `add` names it, as its
/// `collection.toml` does, or after its directory; its profile, unless named, is its only one.
fn layer(
    target: &mut Target, cache: &Cache, add: AddArgs, cwd: &Utf8Path,
) -> Result<Option<LayerSpec>, Error> {
    let AddArgs { layer, location, features, no_default_features } = add;
    let spec = location.spec();
    let named = |layer: &Option<LayerArg>| layer.as_ref().and_then(|layer| layer.source.clone());
    let (source, profile) = match (layer, spec) {
        (None, None) => return Ok(None),
        (Some(LayerArg { source: Some(source), profile }), None) => (source, profile),
        (Some(LayerArg { source: None, profile }), None) => {
            let sources: Vec<SourceName> = target.config().sources.keys().cloned().collect();
            match sources.as_slice() {
                [only] => (only.clone(), profile),
                _ => return Err(TargetError::WhichSource { profile, sources }.into()),
            }
        },
        (layer, Some(spec)) => {
            let derived = derive(&spec);
            let source = Source::try_from(spec)?.rebased(cwd, target.root());
            let Listing { meta, profiles } = collection::list(&source, target.root(), None, cache)?;
            let known =
                target.config().sources.iter().find(|(_, s)| **s == source).map(|(n, _)| n.clone());
            let name = match named(&layer).or(known).or_else(|| meta.map(|meta| meta.name)) {
                Some(name) => name,
                None => derived?,
            };
            let profile = match (layer.map(|layer| layer.profile), profiles.as_slice()) {
                (Some(profile), _) => profile,
                (None, [only]) => only.manifest.profile.name.clone(),
                (None, []) => {
                    return Err(ProfileError::NoProfiles { location: source.to_string() }.into());
                },
                (None, many) => {
                    let profiles = many.iter().map(|p| p.manifest.profile.name.clone()).collect();
                    let location = source.to_string();
                    return Err(TargetError::Ambiguous { location, profiles }.into());
                },
            };
            target.add_source(name.clone(), source)?;
            (name, profile)
        },
    };
    let profile = ProfileRef { source, profile };
    Ok(Some(LayerSpec { profile, features, default_features: !no_default_features }))
}

/// The name a source takes after where it is: its directory's, or its repository's.
fn derive(spec: &SourceSpec) -> Result<SourceName, Error> {
    let located = spec.path.as_deref().or(spec.git.as_deref()).unwrap_or_default();
    let last = located.trim_end_matches('/').rsplit(['/', ':']).next().unwrap_or_default();
    Ok(last.trim_end_matches(".git").parse()?)
}

/// Turns on the features `layer` names of the layer the target applies already, and its default
/// features where `defaults` says, reporting each change.
fn turn_on(
    shell: &Shell, target: &mut Target, layer: &LayerSpec, defaults: Option<bool>, dry_run: bool,
) -> Result<(), Error> {
    let (name, profile) = (&layer.profile.profile, &layer.profile);
    if layer.features.is_empty() && defaults.is_none() {
        return Err(TargetError::DuplicateLayer { layer: name.clone() }.into());
    }
    let (added, defaults) = target.turn_on(name, &layer.features, defaults)?;
    let (verb, what) = if dry_run { ("Would", "add ") } else { ("Adding", "") };
    for feature in &added {
        shell.status(verb, GOOD, format_args!("{what}feature {feature} to layer {profile}"))?;
    }
    if let Some(on) = defaults {
        let how = if on { "on" } else { "off" };
        let (verb, what) = if dry_run { ("Would", "turn ") } else { ("Turning", "") };
        shell.status(verb, GOOD, format_args!("{what}{how} the default features of {profile}"))?;
    }
    if added.is_empty() && defaults.is_none() {
        shell.note(&format!("the layer {profile} already turns these on"), None)?;
    }
    Ok(())
}

/// Turns off `features` of the layer applying the profile `name`, which stays, reporting each; and
/// notes a feature that stays on, since something else turns it on.
fn turn_off(
    shell: &Shell, target: &mut Target, cache: &Cache,
    (name, features): (&ProfileName, &[FeatureName]), dry_run: bool,
) -> Result<(), Error> {
    let profile = match target.layer(name) {
        Some(layer) => layer.profile.clone(),
        None => return Err(missing(target, cache, name)),
    };
    target.turn_off(name, features)?;
    let (verb, what) = if dry_run { ("Would", "remove ") } else { ("Removing", "") };
    for feature in features {
        shell.status(verb, GOOD, format_args!("{what}feature {feature} from layer {profile}"))?;
    }
    // Any error is the apply's that follows to report.
    let on = resolve(target, cache, Refresh::None)
        .ok()
        .and_then(|resolved| resolved.layer(name.as_str()).map(|layer| layer.features().clone()));
    for feature in features {
        if let Some(by) = on.as_ref().and_then(|on| on.get(feature)) {
            let by: Vec<String> = by.iter().map(ToString::to_string).collect();
            shell.note(&format!("{feature} stays on: {} turns it on", by.join(", ")), None)?;
        }
    }
    Ok(())
}

/// Why the target has no layer named `name`: a profile requires it, or no layer is named so.
fn missing(target: &Target, cache: &Cache, name: &ProfileName) -> Error {
    let required = resolve(target, cache, Refresh::None).ok().and_then(|resolved| {
        resolved.layer(name.as_str()).and_then(|layer| layer.required_by().first().cloned())
    });
    if let Some(by) = required {
        return TargetError::Required { name: name.clone(), by }.into();
    }
    let names = target.config().layers.iter().map(|layer| layer.profile.profile.to_string());
    TargetError::NoSuchLayer { name: name.to_string(), names: names.collect() }.into()
}

/// The source `given` names: a source by its name, or the source of the layer so named.
fn source_named(target: &Target, given: &str) -> Result<SourceName, Error> {
    let config = target.config();
    if let Some(name) = config.sources.keys().find(|name| name.as_str() == given) {
        return Ok(name.clone());
    }
    if let Some(layer) = config.layers.iter().find(|layer| layer.profile.profile.as_str() == given)
    {
        return Ok(layer.profile.source.clone());
    }
    let sources = config.sources.keys().map(ToString::to_string);
    let names = sources.chain(config.layers.iter().map(|layer| layer.profile.profile.to_string()));
    Err(TargetError::NoSuchLayer { name: given.to_owned(), names: names.collect() }.into())
}

/// Notes the newer releases of each source `only` names, every one when `None`, that is pinned to
/// a tag; a remote whose tags cannot be listed is warned of, and the update stands.
fn newer(shell: &Shell, target: &Target, cache: &Cache, only: Option<&str>) -> Result<(), Error> {
    let config = target.config();
    let named = |name: &SourceName| {
        only.is_none_or(|only| {
            name.as_str() == only
                || config.layers.iter().any(|layer| {
                    layer.profile.profile.as_str() == only && layer.profile.source == *name
                })
        })
    };
    for (name, source) in config.sources.iter().filter(|(name, _)| named(name)) {
        let Source::Git { at: GitRef::Tag(tag), .. } = source else { continue };
        match source.newer_tags(target.root(), cache) {
            Ok(tags) => {
                if let Some(last) = tags.last() {
                    let text = format!("{name} is pinned to {tag}; newer: {}", tags.join(", "));
                    let help = format!("take the newest: `devset update {name} --tag {last}`");
                    shell.note(&text, Some(&help))?;
                }
            },
            Err(error) => {
                let text = format!("the releases of {name} could not be listed: {error}");
                shell.warn(&text, None)?;
            },
        }
    }
    Ok(())
}

/// Reports where the target stands; with `exit_code`, fails when `apply --force` would write or
/// an update is unfinished.
fn status(
    shell: &Shell, cwd: &Utf8Path, cache: &Cache, exit_code: bool, json: bool, verbose: bool,
) -> Result<ExitCode, Error> {
    let target = Target::find(cwd)?;
    let survey = survey(resolve(&target, cache, Refresh::None)?, &target)?;
    report::status(shell, &survey, json, verbose)?;
    github::status(&survey, &target)?;
    let failed = exit_code && (survey.drifted() || survey.unfinished());
    Ok(if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

/// Takes back the run that conflicted, or with `dry_run` says what that would restore.
fn abort(shell: &Shell, cwd: &Utf8Path, force: bool, dry_run: bool) -> Result<ExitCode, Error> {
    let target = Target::find(cwd)?;
    let rollback = Rollback::of(&target)?;
    if dry_run {
        report::rollback(shell, &rollback)?;
    } else {
        let reverts = rollback.apply(&target, force)?;
        report::rolled_back(shell, &reverts)?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Lists the profiles of the source `location` names, of the one the target names `source`, or
/// of every source the target names, each at its locked commit.
fn list(
    cwd: &Utf8Path, cache: &Cache, source: Option<&SourceName>, location: Location,
) -> Result<ExitCode, Error> {
    if let Some(spec) = location.spec() {
        let source = Source::try_from(spec)?;
        let listing = collection::list(&source, cwd, None, cache)?;
        report::list(None, &source, &listing)?;
        return Ok(ExitCode::SUCCESS);
    }
    let target = Target::find(cwd)?;
    let sources = &target.config().sources;
    if let Some(name) = source.filter(|name| !sources.contains_key(*name)) {
        let sources = sources.keys().cloned().collect();
        let layers = Vec::new();
        return Err(TargetError::NoSuchSource { name: name.clone(), sources, layers }.into());
    }
    let picked = sources.iter().filter(|(name, _)| source.is_none_or(|s| s == *name));
    for (name, each) in picked {
        let listing = collection::list(each, target.root(), target.pinned(each), cache)?;
        report::list(Some(name), each, &listing)?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Prints the completion script for `shell`.
fn completions(shell: clap_complete::Shell) -> Result<ExitCode, Error> {
    // Generated whole, then written: `clap_complete` panics on a reader that stops early.
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut Cli::command(), "devset", &mut script);
    io::stdout().lock().write_all(&script)?;
    Ok(ExitCode::SUCCESS)
}

/// Removes the layer named `name`, and the overrides only it gave meaning to, from `target`.
///
/// Written by the commit that follows, unless `dry_run`.
fn remove(
    shell: &Shell, target: &mut Target, cache: &Cache, name: &ProfileName, dry_run: bool,
) -> Result<(), Error> {
    let (verb, what) = if dry_run { ("Would", "remove ") } else { ("Removing", "") };
    let Some(layer) = target.layer(name) else {
        return Err(missing(target, cache, name));
    };
    let profile = layer.profile.clone();
    target.remove_layer(name)?;
    shell.status(verb, GOOD, format_args!("{what}layer {profile}"))?;
    // Overrides of paths only the removed layer provided are stale now: they go with it.
    loop {
        let resolved = resolve(target, cache, Refresh::None);
        if let Err(Error::Target(TargetError::StaleOverride { path, .. })) = &resolved {
            target.remove_override(path)?;
            shell.status(verb, GOOD, format_args!("{what}override [files.\"{path}\"]"))?;
            continue;
        }
        // Any other error is the apply's that follows to report.
        let by = resolved.ok().and_then(|resolved| {
            resolved.layer(name.as_str()).and_then(|l| l.required_by().first().cloned())
        });
        if let Some(by) = by {
            shell.note(&format!("{name} stays active: {by} requires it"), None)?;
        }
        return Ok(());
    }
}

/// `given`, relative to `cwd`, as a path in `target`, when it is inside it.
fn inside(target: &Target, cwd: &Utf8Path, given: &Utf8Path) -> Option<RelPath> {
    let full = lexical(&cwd.join(given))?;
    let relative = full.strip_prefix(target.root()).ok()?;
    RelPath::new(relative.as_str()).ok()
}

/// `paths`, given relative to `cwd`, as the managed paths of `target` they name.
fn managed(
    survey: &Survey, target: &Target, cwd: &Utf8Path, paths: &[Utf8PathBuf],
) -> Result<Vec<RelPath>, Error> {
    let mut managed: Vec<RelPath> =
        survey.entries().iter().map(|entry| entry.path.clone()).collect();
    managed.dedup();
    paths
        .iter()
        .map(|given| {
            let path = inside(target, cwd, given);
            path.filter(|path| managed.contains(path)).ok_or_else(|| {
                // Named from the target root, as `status` names managed files.
                let path = relative(target, cwd, given);
                TargetError::NotManaged { path, managed: managed.clone() }.into()
            })
        })
        .collect()
}

/// What `explain` was given: a layer, or a file.
enum Explained {
    /// A layer, by its profile's name.
    Layer {
        /// Its name.
        name: ProfileName,
        /// Whether a managed file has the name too, which `./name` explains.
        file: bool,
    },
    /// A path some layer lists.
    File(RelPath),
}

/// Every path some layer of `survey` lists, applied or not.
fn listed_paths(survey: &Survey) -> Vec<RelPath> {
    let layers = survey.resolved().layers();
    let mut listed: Vec<RelPath> =
        layers.iter().flat_map(|layer| layer.files().keys().cloned()).collect();
    listed.extend(survey.entries().iter().map(|entry| entry.path.clone()));
    listed.sort();
    listed.dedup();
    listed
}

/// `given` as a layer of `survey`, or as a path, from `cwd`, that some layer lists: a layer's name
/// when a layer has it, and a file's otherwise, as `./name` always is.
fn explained(
    survey: &Survey, target: &Target, cwd: &Utf8Path, given: &str,
) -> Result<Explained, Error> {
    let listed = listed_paths(survey);
    let layer = given
        .parse::<ProfileName>()
        .ok()
        .filter(|name| survey.resolved().layer(name.as_str()).is_some());
    let file = inside(target, cwd, Utf8Path::new(given)).filter(|path| listed.contains(path));
    match (layer, file) {
        (Some(name), file) => Ok(Explained::Layer { name, file: file.is_some() }),
        (None, Some(path)) => Ok(Explained::File(path)),
        (None, None) => {
            let layers = survey.resolved().layers().iter().map(|layer| layer.name().to_string());
            let names = layers.chain(listed.iter().map(ToString::to_string)).collect();
            let name = relative(target, cwd, Utf8Path::new(given));
            Err(TargetError::Unexplained { name, names }.into())
        },
    }
}

/// `given`, relative to `cwd`, named from `target`'s root when it is inside it.
fn relative(target: &Target, cwd: &Utf8Path, given: &Utf8Path) -> String {
    let full = lexical(&cwd.join(given));
    let inside = full.as_deref().and_then(|full| full.strip_prefix(target.root()).ok());
    inside.map_or_else(|| given.to_string(), ToString::to_string)
}

/// Plans `target` and commits, unless `dry_run`; exit 1 when a file conflicts.
fn apply(
    shell: &Shell, target: &mut Target, cache: &Cache, (refresh, mode): (Refresh<'_>, Mode),
    answers: Answers, dry_run: bool,
) -> Result<ExitCode, Error> {
    let answered = target.answers().clone();
    let resolved = ask::resolved(shell, target, cache, refresh, answers.vars)?;
    report::warnings(shell, resolved.warnings())?;
    let dropped: Vec<VarName> =
        answered.into_keys().filter(|name| !resolved.answers().contains_key(name)).collect();
    let plan = plan(survey(resolved, target)?, mode, target)?;
    let (wrote, suggestions) =
        (Wrote::of(dry_run, plan.held()), plan.resolved().suggestions().to_vec());
    let steps = if dry_run { plan.steps().to_vec() } else { commit(plan, target)? };
    let conflicts = report::applied(shell, &steps, wrote)?;
    github::conflicts(&steps, target, wrote)?;
    report::suggestions(shell, &suggestions)?;
    report::dropped(shell, &dropped, dry_run)?;
    Ok(if conflicts { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

/// `path` with `.` and `..` resolved by name alone, as a shell resolves `cd ../x`; `None` where a
/// `..` climbs above the root.
fn lexical(path: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut resolved = Utf8PathBuf::new();
    for component in path.components() {
        match component {
            Utf8Component::CurDir => {},
            Utf8Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            },
            Utf8Component::Prefix(_) | Utf8Component::RootDir | Utf8Component::Normal(_) => {
                resolved.push(component);
            },
        }
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use devset_core::source::SourceSpec;

    use super::{derive, lexical};

    #[test]
    fn lexical_resolves_dots_by_name() {
        let resolved = lexical(Utf8Path::new("/repo/./sub/../deny.toml"));
        assert_eq!(
            resolved.as_deref(),
            Some(Utf8Path::new("/repo/deny.toml")),
            "`.` and `..` resolved"
        );
        assert_eq!(lexical(Utf8Path::new("/..")), None, "and none above the root");
    }

    #[test]
    fn sources_are_named_after_where_they_are() {
        let git = |url: &str, path: Option<&str>| SourceSpec {
            git: Some(url.to_owned()),
            path: path.map(str::to_owned),
            ..SourceSpec::default()
        };
        for (spec, name) in [
            (git("https://github.com/atomix-labs/atxp", None), "atxp"),
            (git("git@github.com:acme/profiles.git", None), "profiles"),
            (git("../profiles.git/", Some("rust")), "rust"),
            (SourceSpec { path: Some("../base".to_owned()), ..SourceSpec::default() }, "base"),
        ] {
            assert_eq!(derive(&spec).expect("a name").as_str(), name, "{spec:?}");
        }
    }
}
