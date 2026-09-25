//! Apply versioned file bundles to a directory, and update them without losing local edits.
//!
//! Run from anywhere inside a target; `init` makes one. Status lines go to stdout, diagnostics
//! and progress to stderr. The exit code is 0 on success, 1 on a conflict or, under
//! `status --exit-code`, on drift, and 2 on an error.
//!
//! ```text
//! devset init --git https://github.com/acme/profiles --tag v1.4.0 --path rust
//! devset status --exit-code
//! devset update
//! devset update --continue
//! ```

extern crate alloc;

mod ask;
mod cli;
mod github;
mod help;
mod report;
mod shell;
mod words;

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use clap::{CommandFactory, Parser};
use clap_cargo::style::GOOD;
use devset_core::profile::{Manifest, VarName};
use devset_core::source::Source;
use devset_core::target::Config;
use devset_core::{
    Cache, Error, Mode, Refresh, RelPath, Rollback, Survey, Target, TargetError, commit, plan,
    resolve, survey,
};
use dialoguer::console;

use crate::cli::{Answers, Cli, Command, SchemaFile};
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
    // Built for the commands that resolve layers only: nothing else needs a home directory.
    let cache = || -> Result<Cache, Error> {
        Ok(Cache::user()?.prompting(shell.interactive()).on_fetch(shell.fetches()))
    };
    match command {
        Command::Init { source, answers, dry_run } => {
            let mut target = Target::open_or_new(&cwd)?;
            target.add_layer(Source::try_from(source.spec())?)?;
            apply(shell, &mut target, &cache()?, (Refresh::None, Mode::Apply), answers, dry_run)
        },
        Command::Remove { layer, dry_run } => {
            let mut target = Target::find(&cwd)?;
            let cache = cache()?;
            remove(shell, &mut target, &cache, &layer, dry_run)?;
            apply(
                shell,
                &mut target,
                &cache,
                (Refresh::None, Mode::Apply),
                Answers::default(),
                dry_run,
            )
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
        Command::Apply { force, answers, dry_run } => {
            let mode = if force { Mode::Force } else { Mode::Apply };
            let target = &mut Target::find(&cwd)?;
            apply(shell, target, &cache()?, (Refresh::None, mode), answers, dry_run)
        },
        Command::Update { abort: true, force, dry_run, .. } => abort(shell, &cwd, force, dry_run),
        Command::Update { layer, resume, answers, dry_run, .. } => {
            let how = match (resume, layer.as_deref()) {
                (true, _) => (Refresh::None, Mode::Continue),
                (false, Some(name)) => (Refresh::Layer(name), Mode::Apply),
                (false, None) => (Refresh::All, Mode::Apply),
            };
            apply(shell, &mut Target::find(&cwd)?, &cache()?, how, answers, dry_run)
        },
        Command::Schema { file } => schema(file),
        Command::Completions { shell } => completions(shell),
    }
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

/// Takes back an unfinished update, or with `dry_run` says what that would restore.
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

/// Prints the JSON schema of `file`.
fn schema(file: SchemaFile) -> Result<ExitCode, Error> {
    let schema = match file {
        SchemaFile::Profile => schemars::schema_for!(Manifest),
        SchemaFile::Config => schemars::schema_for!(Config),
    };
    let mut out = anstream::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &schema).map_err(io::Error::from)?;
    writeln!(out)?;
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
    shell: &Shell, target: &mut Target, cache: &Cache, name: &str, dry_run: bool,
) -> Result<(), Error> {
    let (verb, what) = if dry_run { ("Would", "remove ") } else { ("Removing", "") };
    // A layer never applied is not in the lock by name: read the layers to find it.
    let source = if let Some(source) = target.layer(name) {
        source.clone()
    } else {
        let resolved = resolve(target, cache, Refresh::None)?;
        let layers = resolved.layers();
        let named = layers.iter().find(|layer| layer.meta().name == name);
        named.map(|layer| layer.source().clone()).ok_or_else(|| TargetError::NoSuchLayer {
            name: name.to_owned(),
            layers: layers.iter().map(|layer| layer.meta().name.clone()).collect(),
        })?
    };
    if !target.config().layers.contains(&source) {
        let resolved = resolve(target, cache, Refresh::None)?;
        let by = resolved.layers().iter().find(|layer| layer.meta().name == name);
        let by = by.and_then(|layer| layer.required_by()).unwrap_or_default().to_owned();
        return Err(TargetError::Required { name: name.to_owned(), by }.into());
    }
    target.remove_layer(&source, name)?;
    shell.status(verb, GOOD, format_args!("{what}layer {name}  {source}"))?;
    // Overrides of paths only the removed layer provided are stale now: they go with it.
    loop {
        let Err(Error::Target(TargetError::StaleOverride { path, .. })) =
            resolve(target, cache, Refresh::None)
        else {
            return Ok(());
        };
        target.remove_override(&path)?;
        shell.status(verb, GOOD, format_args!("{what}override [files.\"{path}\"]"))?;
    }
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
            let full = lexical(&cwd.join(given));
            let inside = full.as_deref().and_then(|full| full.strip_prefix(target.root()).ok());
            let path = inside.and_then(|relative| RelPath::new(relative.as_str()).ok());
            path.filter(|path| managed.contains(path)).ok_or_else(|| {
                // Named from the target root, as `status` names managed files.
                let path = inside.map_or_else(|| given.to_string(), ToString::to_string);
                TargetError::NotManaged { path, managed: managed.clone() }.into()
            })
        })
        .collect()
}

/// Plans `target` and commits, unless `dry_run`; exit 1 when a file conflicts.
fn apply(
    shell: &Shell, target: &mut Target, cache: &Cache, (refresh, mode): (Refresh<'_>, Mode),
    answers: Answers, dry_run: bool,
) -> Result<ExitCode, Error> {
    let answered = target.answers().clone();
    let resolved = ask::resolved(shell, target, cache, refresh, answers.vars)?;
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

    use super::lexical;

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
}
