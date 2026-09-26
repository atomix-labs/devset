//! The command line: devset's commands, their arguments, and how each is parsed.

use core::str::FromStr;

use camino::Utf8PathBuf;
use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use devset_core::NameError;
use devset_core::name::{FeatureName, ProfileName, ScaffoldId, SourceName};
use devset_core::profile::VarName;
use devset_core::source::SourceSpec;

/// Apply versioned file bundles to a directory, and update them without losing local edits.
#[derive(Debug, Parser)]
#[command(name = "devset", version, styles = clap_cargo::style::CLAP_STYLING, after_help = "\
Examples:
  devset new hello atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.4.0
  devset add atxp/mdbook --features katex
  devset status
  devset update

Manual: https://atomix-labs.github.io/devset/")]
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
    /// Start a target in a new directory, or a profile or a collection to author.
    #[command(after_help = "\
Examples:
  devset new hello                    a target: hello/.devset/config.toml, to add layers to
  devset new hello atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.4.0
  devset new --profile my-lint        a profile: profile.toml, files/ and a README
  devset new --collection acme        a source of profiles: collection.toml and profiles/")]
    New {
        /// The directory to create it in.
        dir: Utf8PathBuf,
        /// Create a profile to author, not a target.
        #[arg(long, conflicts_with_all = ["collection", "layer", "git", "path"])]
        profile: bool,
        /// Create a collection of profiles to publish, not a target.
        #[arg(long, conflicts_with_all = ["layer", "git", "path"])]
        collection: bool,
        /// The first layer, if any.
        #[command(flatten)]
        add: AddArgs,
        /// Answers to its variables.
        #[command(flatten)]
        answers: Answers,
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Start a target in the current directory, with a first layer if given.
    #[command(after_help = "\
Examples:
  devset init
  devset init atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.4.0 --features docs
  devset init --path ../profiles/base")]
    Init {
        /// The first layer, if any.
        #[command(flatten)]
        add: AddArgs,
        /// Answers to its variables.
        #[command(flatten)]
        answers: Answers,
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Add a profile as a layer, or features to a layer, and apply it.
    #[command(
        group(ArgGroup::new("what").required(true).multiple(true).args(["layer", "git", "path"])),
        after_help = "\
Examples:
  devset add atxp/mdbook                      from a source the target names
  devset add atxp/mdbook --features katex     with features beside the defaults
  devset add atxp/mdbook --features mermaid   a feature, to a layer already applied
  devset add house/deploy --git git@github.com:acme/profiles --branch main
  devset add --path ../profiles/base          a source holding one profile"
    )]
    Add {
        /// The layer.
        #[command(flatten)]
        add: AddArgs,
        /// Turn a layer's default features back on.
        #[arg(long, help_heading = "Features", conflicts_with = "no_default_features")]
        default_features: bool,
        /// Answers to its variables.
        #[command(flatten)]
        answers: Answers,
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove a layer, or features from one: unchanged files go, edited ones stay.
    #[command(after_help = "\
Examples:
  devset remove mdbook                      by its profile's name
  devset remove mdbook --features mermaid   a feature, keeping the layer
  devset remove mdbook --dry-run            what would change")]
    Remove {
        /// The layer, by its profile's name.
        layer: ProfileName,
        /// Only these features, which the layer lists; the layer stays. Comma-separated or
        /// repeated.
        #[arg(long, short = 'F', value_delimiter = ',', help_heading = "Features")]
        features: Vec<FeatureName>,
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
  devset apply --dry-run                what would change
  devset apply --force                  also restore drifted owned files
  devset apply --rescaffold mdbook/book write a scaffold's missing files again")]
    Apply {
        /// Also restore `owned` files that were edited, deleted or never recorded.
        #[arg(long)]
        force: bool,
        /// Write the missing files of a scaffold again, `profile/group`; repeatable.
        #[arg(long, value_name = "PROFILE/GROUP")]
        rescaffold: Vec<ScaffoldId>,
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Answers to the profile's variables.
        #[command(flatten)]
        answers: Answers,
    },
    /// Move sources to what their refs name now, merging local edits.
    #[command(after_help = "\
Examples:
  devset update              every source
  devset update atxp         one source, or the source of one layer
  devset update --continue   after resolving .devset/conflicts/
  devset update --abort      take back an update that conflicted")]
    Update {
        /// Only the source with this name, or the source of the layer with this name.
        name: Option<String>,
        /// Install the conflicts resolved in .devset/conflicts/.
        #[arg(long = "continue", conflicts_with_all = ["name", "abort"])]
        resume: bool,
        /// Take back the unfinished update: every file it wrote, the lock and the state.
        #[arg(long, conflicts_with_all = ["name", "vars"])]
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
    /// Show each layer's features: which are on, and who turned them on.
    #[command(after_help = "\
Examples:
  devset features          every layer
  devset features mdbook   one, with the features it leaves off")]
    Features {
        /// Only the layer whose profile has this name.
        layer: Option<ProfileName>,
    },
    /// Show why a file is managed as it is: each layer that lists it, and its gates.
    #[command(after_help = "\
Examples:
  devset explain docs/book.toml")]
    Explain {
        /// The file, from the current directory.
        path: Utf8PathBuf,
    },
    /// List the profiles a source holds, with their features.
    #[command(after_help = "\
Examples:
  devset list                                   every source the target names
  devset list atxp                              one of them
  devset list --git https://github.com/atomix-labs/atxp --tag v0.4.0")]
    List {
        /// A source the target names.
        #[arg(conflicts_with_all = ["git", "path"])]
        source: Option<SourceName>,
        /// Or any source.
        #[command(flatten)]
        location: Location,
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

/// A layer to add: the profile, where it is, and its features.
#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    /// The profile, `source/profile`; with --git or --path, `profile` alone names it in the
    /// source they name.
    pub(crate) layer: Option<LayerArg>,
    /// Where the source is, when the target does not name it yet.
    #[command(flatten)]
    pub(crate) location: Location,
    /// Features to turn on, beside the default ones; comma-separated or repeated.
    #[arg(long, short = 'F', value_delimiter = ',', help_heading = "Features")]
    pub(crate) features: Vec<FeatureName>,
    /// Leave the profile's default features off.
    #[arg(long, help_heading = "Features")]
    pub(crate) no_default_features: bool,
}

/// A layer as the command line names it: `source/profile`, or `profile` in the source the flags
/// name.
#[derive(Clone, Debug)]
pub(crate) struct LayerArg {
    /// The source, as `[sources]` names it.
    pub(crate) source: Option<SourceName>,
    /// The profile.
    pub(crate) profile: ProfileName,
}

impl FromStr for LayerArg {
    type Err = NameError;

    fn from_str(written: &str) -> Result<Self, NameError> {
        Ok(match written.split_once('/') {
            Some((source, profile)) => {
                Self { source: Some(source.parse()?), profile: profile.parse()? }
            },
            None => Self { source: None, profile: written.parse()? },
        })
    }
}

/// Where a source is: the fields of an entry in `[sources]`.
#[derive(Debug, Args)]
#[command(next_help_heading = "Source")]
pub(crate) struct Location {
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
    /// A local directory; with --git, the directory in the repository its profiles are in.
    #[arg(long)]
    path: Option<String>,
}

impl Location {
    /// The source these arguments name, as `[sources]` would; `None` when they name none.
    pub(crate) fn spec(self) -> Option<SourceSpec> {
        let Self { git, tag, branch, rev, path } = self;
        (git.is_some() || path.is_some()).then_some(SourceSpec { git, tag, branch, rev, path })
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
    /// A source's `collection.toml`.
    Collection,
}

/// Parses `NAME=VALUE`.
fn var(arg: &str) -> Result<(VarName, String), String> {
    let (name, value) = arg.split_once('=').ok_or("expected NAME=VALUE")?;
    let name = VarName::try_from(name.to_owned()).map_err(|e| e.to_string())?;
    Ok((name, value.to_owned()))
}
