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

#![feature(non_exhaustive_omitted_patterns_lint, normalize_lexically, strict_provenance_lints)]

extern crate alloc;

mod ask;
mod github;
mod help;
mod report;
mod shell;
mod words;

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use camino::{Utf8Path, Utf8PathBuf};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_cargo::style::GOOD;
use devset_core::profile::{Manifest, VarName};
use devset_core::source::{Source, SourceSpec};
use devset_core::target::Config;
use devset_core::{
    Cache, Error, Mode, Refresh, RelPath, Rollback, Survey, Target, TargetError, commit, plan,
    resolve, survey,
};
use dialoguer::console;

use crate::report::Wrote;
use crate::shell::Shell;

/// Apply versioned file bundles to a directory, and update them without losing local edits.
#[derive(Debug, Parser)]
#[command(version, styles = clap_cargo::style::CLAP_STYLING, after_help = "\
Examples:
  devset init --git https://github.com/acme/profiles --tag v1.4.0 --path rust
  devset status
  devset update")]
struct Cli {
    /// Print only results and errors.
    #[arg(long, short, global = true, help_heading = "Global Options")]
    quiet: bool,
    /// Never prompt; fail with the flags to pass instead.
    #[arg(long, global = true, help_heading = "Global Options")]
    no_input: bool,
    /// Never colour output.
    #[arg(long, global = true, help_heading = "Global Options")]
    no_color: bool,
    /// What to do.
    #[command(subcommand)]
    command: Command,
}

/// A devset command.
#[derive(Debug, Subcommand)]
enum Command {
    /// Add a profile as a layer, and apply it.
    #[command(after_help = "\
Examples:
  devset init --path ../profiles/base
  devset init --git https://github.com/acme/profiles --tag v1.4.0 --path rust
  devset init --git git@github.com:acme/profiles --branch main --var author=Ada")]
    Init {
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
        /// The layer to add.
        #[command(flatten)]
        source: SourceArgs,
        /// Answers to the profile's variables.
        #[command(flatten)]
        answers: Answers,
    },
    /// Remove a layer; its files stay, no longer tracked.
    #[command(after_help = "\
Examples:
  devset remove base             by its profile name
  devset remove base --dry-run   what would change")]
    Remove {
        /// The layer, by its profile's name.
        layer: String,
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Show where every managed file stands against the profile.
    #[command(after_help = "\
Examples:
  devset status               what needs doing
  devset status -v            and everything in sync
  devset status --exit-code   in CI: fail on drift")]
    Status {
        /// Exit 1 when `apply --force` would write a file, or an update is unfinished.
        #[arg(long)]
        exit_code: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Also list files that match, and the settings in force.
        #[arg(long, short)]
        verbose: bool,
    },
    /// Show, line by line, how files differ from the profile.
    #[command(after_help = "\
Examples:
  devset diff             every file that differs
  devset diff deny.toml   one file")]
    Diff {
        /// Only these files.
        paths: Vec<Utf8PathBuf>,
    },
    /// Apply the pinned profile without destroying local edits.
    #[command(after_help = "\
Examples:
  devset apply --dry-run   what would change
  devset apply --force     also restore drifted owned files")]
    Apply {
        /// Also restore `owned` files that were edited, deleted or never recorded.
        #[arg(long)]
        force: bool,
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Answers to the profile's variables.
        #[command(flatten)]
        answers: Answers,
    },
    /// Move layers to what their refs name now, merging local edits.
    #[command(after_help = "\
Examples:
  devset update              every layer
  devset update rust         one layer, by its profile name
  devset update --continue   after resolving .devset/conflicts/
  devset update --abort      take back an update that conflicted")]
    Update {
        /// Only the layer whose profile has this name.
        layer: Option<String>,
        /// Install the conflicts resolved in .devset/conflicts/.
        #[arg(long = "continue", conflicts_with_all = ["layer", "abort"])]
        resume: bool,
        /// Take back the unfinished update: every file it wrote, the lock and the state.
        #[arg(long, conflicts_with_all = ["layer", "vars"])]
        abort: bool,
        /// With --abort, also discard changes made since the update.
        #[arg(long, requires = "abort")]
        force: bool,
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Answers to the profile's variables.
        #[command(flatten)]
        answers: Answers,
    },
    /// Print the JSON Schema of a devset file, for editor completion.
    Schema {
        /// Which file.
        file: SchemaFile,
    },
    /// Print a shell completion script.
    #[command(after_help = "\
Examples:
  devset completions bash > ~/.local/share/bash-completion/completions/devset
  devset completions zsh > ~/.zfunc/_devset
  devset completions fish > ~/.config/fish/completions/devset.fish")]
    Completions {
        /// The shell.
        shell: clap_complete::Shell,
    },
}

/// A layer's source: the fields of `[[layers]]` in `.devset/config.toml`.
#[derive(Debug, Args)]
#[command(next_help_heading = "Source")]
struct SourceArgs {
    /// Git repository URL.
    #[arg(long)]
    git: Option<String>,
    /// Tag to use.
    #[arg(long, requires = "git", conflicts_with_all = ["branch", "rev"])]
    tag: Option<String>,
    /// Branch to use.
    #[arg(long, requires = "git", conflicts_with = "rev")]
    branch: Option<String>,
    /// Full commit id to use.
    #[arg(long, requires = "git")]
    rev: Option<String>,
    /// Profile directory: local, or within the repository with --git.
    #[arg(long, required_unless_present = "git")]
    path: Option<String>,
}

impl SourceArgs {
    /// The source these arguments name, as `[[layers]]` would.
    fn spec(self) -> SourceSpec {
        let Self { git, tag, branch, rev, path } = self;
        SourceSpec { git, tag, branch, rev, path }
    }
}

/// Variable answers given on the command line.
#[derive(Debug, Default, Args)]
#[command(next_help_heading = "Variables")]
struct Answers {
    /// Answer a profile variable; repeatable.
    #[arg(long = "var", value_name = "NAME=VALUE", value_parser = var)]
    vars: Vec<(VarName, String)>,
}

/// A devset file with a schema.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum SchemaFile {
    /// A profile's `profile.toml`.
    Profile,
    /// A target's `.devset/config.toml`.
    Config,
}

/// Parses `NAME=VALUE`.
fn var(arg: &str) -> Result<(VarName, String), String> {
    let (name, value) = arg.split_once('=').ok_or("expected NAME=VALUE")?;
    let name = VarName::try_from(name.to_owned()).map_err(|e| e.to_string())?;
    Ok((name, value.to_owned()))
}

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
            // `..` resolved by name alone, as a shell resolves `cd ../x`.
            let full = cwd.join(given).as_std_path().normalize_lexically().ok();
            let full = full.and_then(|full| Utf8PathBuf::from_path_buf(full).ok());
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
