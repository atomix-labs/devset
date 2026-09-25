//! The command line: devset's commands, their arguments, and how each is parsed.

use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand, ValueEnum};
use devset_core::profile::VarName;
use devset_core::source::SourceSpec;

/// Apply versioned file bundles to a directory, and update them without losing local edits.
#[derive(Debug, Parser)]
#[command(version, styles = clap_cargo::style::CLAP_STYLING, after_help = "\
Examples:
  devset init --git https://github.com/acme/profiles --tag v1.4.0 --path rust
  devset status
  devset update")]
pub(crate) struct Cli {
    /// Print only results and errors.
    #[arg(long, short, global = true, help_heading = "Global Options")]
    pub(crate) quiet: bool,
    /// Never prompt; fail with the flags to pass instead.
    #[arg(long, global = true, help_heading = "Global Options")]
    pub(crate) no_input: bool,
    /// Never colour output.
    #[arg(long, global = true, help_heading = "Global Options")]
    pub(crate) no_color: bool,
    /// What to do.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// A devset command.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
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
pub(crate) struct SourceArgs {
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
    pub(crate) fn spec(self) -> SourceSpec {
        let Self { git, tag, branch, rev, path } = self;
        SourceSpec { git, tag, branch, rev, path }
    }
}

/// Variable answers given on the command line.
#[derive(Debug, Default, Args)]
#[command(next_help_heading = "Variables")]
pub(crate) struct Answers {
    /// Answer a profile variable; repeatable.
    #[arg(long = "var", value_name = "NAME=VALUE", value_parser = var)]
    pub(crate) vars: Vec<(VarName, String)>,
}

/// A devset file with a schema.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum SchemaFile {
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
