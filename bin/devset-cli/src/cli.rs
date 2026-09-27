//! The command line: devset's commands, their arguments, and how each is parsed.

use core::str::FromStr;

use camino::Utf8PathBuf;
use clap::{ArgGroup, Args, Parser, Subcommand};
use devset_core::name::{FeatureName, ProfileName, ScaffoldId, SourceName};
use devset_core::profile::VarName;
use devset_core::source::{GitRef, Oid, SourceSpec};
use devset_core::{NameError, SourceError};

/// Apply versioned file bundles to a directory, and update them without losing local edits.
#[derive(Debug, Parser)]
#[command(name = "devset", version, styles = clap_cargo::style::CLAP_STYLING, after_help = "\
Examples:
  devset add atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.6.2
  devset add atxp/mdbook --features katex
  devset status
  devset update --dry-run

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
    /// Make a directory a target, or a profile or a collection to author.
    #[command(after_help = "\
Examples:
  devset init                        this directory, a target to add layers to
  devset init hello                  a new directory, hello/, as a target
  devset init my-lint --profile      a profile: profile.toml, files/ and a README
  devset init acme --collection      a source of profiles: collection.toml and profiles/")]
    Init {
        /// The directory, created if missing; this one if not given.
        path: Option<Utf8PathBuf>,
        /// Make it a profile to author, not a target.
        #[arg(long, conflicts_with = "collection")]
        profile: bool,
        /// Make it a collection of profiles to publish, not a target.
        #[arg(long)]
        collection: bool,
        /// Show what would change; write nothing.
        #[arg(long, conflicts_with_all = ["profile", "collection"])]
        dry_run: bool,
    },
    /// Add a profile as a layer, or features to one, and apply it.
    ///
    /// Where there is no target yet, `add` starts one: at a git repository's top level, or in an
    /// empty directory; anywhere else, run `devset init` first.
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
    /// Apply the pinned profiles without destroying local edits.
    ///
    /// A run that conflicted, of any command, is finished with `--continue`, once the files in
    /// .devset/conflicts/ are resolved, or taken back with `--abort`.
    #[command(after_help = "\
Examples:
  devset apply --dry-run                 what would change
  devset apply --force                   also restore drifted owned files
  devset apply --rescaffold mdbook/book  write a scaffold's missing files again
  devset apply --continue                after resolving .devset/conflicts/
  devset apply --abort                   take back what the conflicted run wrote")]
    Apply {
        /// Also restore `owned` files that were edited, deleted or never recorded; with --abort,
        /// also discard changes made since the conflicted run.
        #[arg(long)]
        force: bool,
        /// Write the missing files of a scaffold again, `profile/group`; repeatable.
        #[arg(long, value_name = "PROFILE/GROUP")]
        rescaffold: Vec<ScaffoldId>,
        /// Install the conflicts resolved in .devset/conflicts/.
        #[arg(long = "continue", conflicts_with_all = ["abort", "force", "rescaffold", "vars"])]
        resume: bool,
        /// Take back the run that conflicted: every file it wrote, the lock and the state.
        #[arg(long, conflicts_with_all = ["rescaffold", "vars"])]
        abort: bool,
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Answers to the profile's variables.
        #[command(flatten)]
        answers: Answers,
    },
    /// Move sources to newer commits, or to another tag, merging local edits.
    ///
    /// A source pinned to a tag stays there: `update` names the releases newer than it, and
    /// `--tag`, `--branch` or `--rev` moves it.
    #[command(after_help = "\
Examples:
  devset update                    every source
  devset update atxp               one source, or the source of one layer
  devset update atxp --tag <tag>   move a source to another release
  devset update --dry-run          what would change")]
    Update {
        /// Only the source with this name, or the source of the layer with this name.
        name: Option<String>,
        /// Where to move it.
        #[command(flatten)]
        pin: Pin,
        /// Show what would change; write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Answers to the profile's variables.
        #[command(flatten)]
        answers: Answers,
    },
    /// Show where every managed file stands against the profile.
    #[command(after_help = "\
Examples:
  devset status               what needs doing
  devset status -v            and everything in sync
  devset status --exit-code   in CI: fail on drift")]
    Status {
        /// Exit 1 when `apply --force` would write a file, or a run that conflicted is unfinished.
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
    /// Show why a layer's features are on, or why a file is managed as it is.
    ///
    /// For a layer, its features and who turned each on, and those it leaves off; for a file,
    /// each layer that lists it, how it manages it, and its gates.
    #[command(after_help = "\
Examples:
  devset explain                  every layer, with its features and who turned each on
  devset explain mdbook           one layer, with the features it leaves off
  devset explain docs/book.toml   one file: each layer that lists it, and its gates")]
    Explain {
        /// A layer, by its profile's name, or a file, from the current directory: `./name` for a
        /// file a layer's name matches.
        name: Option<String>,
    },
    /// List the profiles a source holds, with their features.
    #[command(after_help = "\
Examples:
  devset list                                   every source the target names
  devset list atxp                              one of them
  devset list --git https://github.com/atomix-labs/atxp --tag v0.6.2")]
    List {
        /// A source the target names.
        #[arg(conflicts_with_all = ["git", "path"])]
        source: Option<SourceName>,
        /// Or any source.
        #[command(flatten)]
        location: Location,
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

/// Where `update` moves a git source: a tag, a branch or a commit.
#[derive(Debug, Args)]
#[command(next_help_heading = "Move the source")]
pub(crate) struct Pin {
    /// To this tag.
    #[arg(long, requires = "name", conflicts_with_all = ["branch", "rev"])]
    tag: Option<String>,
    /// To this branch.
    #[arg(long, requires = "name", conflicts_with = "rev")]
    branch: Option<String>,
    /// To this commit, by its full id.
    #[arg(long, requires = "name")]
    rev: Option<String>,
}

impl Pin {
    /// The ref these arguments name; `None` when they name none.
    ///
    /// # Errors
    /// [`SourceError`], `--rev` is not a full commit id.
    pub(crate) fn git_ref(self) -> Result<Option<GitRef>, SourceError> {
        Ok(match (self.tag, self.branch, self.rev) {
            (Some(tag), _, _) => Some(GitRef::Tag(tag)),
            (None, Some(branch), _) => Some(GitRef::Branch(branch)),
            (None, None, Some(rev)) => Some(GitRef::Rev(Oid::try_from(rev)?)),
            (None, None, None) => None,
        })
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

/// Parses `NAME=VALUE`.
fn var(arg: &str) -> Result<(VarName, String), String> {
    let (name, value) = arg.split_once('=').ok_or("expected NAME=VALUE")?;
    let name = VarName::try_from(name.to_owned()).map_err(|e| e.to_string())?;
    Ok((name, value.to_owned()))
}
