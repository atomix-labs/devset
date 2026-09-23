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
mod github;
mod help;
mod report;
mod shell;

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand, ValueEnum};
use devset_core::profile::{Manifest, VarName};
use devset_core::source::{Source, SourceSpec};
use devset_core::target::Config;
use devset_core::{Cache, Error, Mode, Refresh, Target, commit, plan, resolve, survey};
use dialoguer::console;

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
    /// Show how every managed file compares with the profile.
    #[command(after_help = "\
Examples:
  devset status               what needs doing
  devset status -v            and everything in sync
  devset status --exit-code   in CI: fail on drift")]
    Status {
        /// Exit 1 when `apply --force` would write a file.
        #[arg(long)]
        exit_code: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
        /// Also list files that match, and the settings in force.
        #[arg(long, short)]
        verbose: bool,
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
  devset update --continue   after resolving .devset/conflicts/")]
    Update {
        /// Only the layer whose profile has this name.
        layer: Option<String>,
        /// Install the conflicts resolved in .devset/conflicts/.
        #[arg(long = "continue", conflicts_with = "layer")]
        resume: bool,
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

/// Variable answers given on the command line.
#[derive(Debug, Args)]
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
    match run(&shell, cli.command) {
        Ok(code) => code,
        // A reader that stops early, as `devset status | head` does, is not an error.
        Err(Error::Io(e)) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(error) => match shell.error(&error) {
            Ok(()) | Err(_) => ExitCode::from(2),
        },
    }
}

/// Runs `command`, returning the exit code for a clean finish.
fn run(shell: &Shell, command: Command) -> Result<ExitCode, Error> {
    let cwd = Utf8PathBuf::try_from(env::current_dir()?).map_err(io::Error::other)?;
    let cache = Cache::user()?.on_fetch(shell.fetches());
    match command {
        Command::Init { source, answers, dry_run } => {
            let SourceArgs { git, tag, branch, rev, path } = source;
            let mut target = Target::at(&cwd)?;
            let source = Source::try_from(SourceSpec { git, tag, branch, rev, path })?;
            target.add_layer(source)?;
            apply(shell, &mut target, &cache, (Refresh::None, Mode::Apply), answers, dry_run)
        },
        Command::Status { exit_code, json, verbose } => {
            let target = Target::find(&cwd)?;
            let survey = survey(resolve(&target, &cache, Refresh::None)?, &target)?;
            report::status(shell, &survey, json, verbose)?;
            github::status(&survey, &target)?;
            Ok(if exit_code && survey.drifted() { ExitCode::FAILURE } else { ExitCode::SUCCESS })
        },
        Command::Apply { force, answers, dry_run } => {
            let mode = if force { Mode::Force } else { Mode::Apply };
            apply(shell, &mut Target::find(&cwd)?, &cache, (Refresh::None, mode), answers, dry_run)
        },
        Command::Update { layer, resume, answers, dry_run } => {
            let how = match (resume, layer.as_deref()) {
                (true, _) => (Refresh::None, Mode::Continue),
                (false, Some(name)) => (Refresh::Layer(name), Mode::Apply),
                (false, None) => (Refresh::All, Mode::Apply),
            };
            apply(shell, &mut Target::find(&cwd)?, &cache, how, answers, dry_run)
        },
        Command::Schema { file } => {
            let schema = match file {
                SchemaFile::Profile => schemars::schema_for!(Manifest),
                SchemaFile::Config => schemars::schema_for!(Config),
            };
            let mut out = anstream::stdout().lock();
            serde_json::to_writer_pretty(&mut out, &schema).map_err(io::Error::from)?;
            writeln!(out)?;
            Ok(ExitCode::SUCCESS)
        },
    }
}

/// Plans `target` and commits, unless `dry_run`; exit 1 when a file conflicts.
fn apply(
    shell: &Shell, target: &mut Target, cache: &Cache, (refresh, mode): (Refresh<'_>, Mode),
    answers: Answers, dry_run: bool,
) -> Result<ExitCode, Error> {
    let resolved = ask::resolved(shell, target, cache, refresh, answers.vars)?;
    let plan = plan(survey(resolved, target)?, mode, target)?;
    let (held, suggestions) = (plan.held(), plan.resolved().suggestions().to_vec());
    let steps = if dry_run { plan.steps().to_vec() } else { commit(plan, target)? };
    let conflicts = report::applied(shell, &steps, dry_run, held)?;
    github::conflicts(&steps, target)?;
    report::suggestions(shell, &suggestions)?;
    Ok(if conflicts { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}
