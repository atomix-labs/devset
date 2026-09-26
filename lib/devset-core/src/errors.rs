//! Why devset did not go through: a path, a source, a profile, a target, a variable, or a merge.

use core::fmt::Display;
use core::ops::Range;
use core::result;
use std::io;

use camino::Utf8PathBuf;
use thiserror::Error;

use crate::name::{FeatureName, ProfileName, ProfileRef, SourceName};
use crate::part::Scope;
use crate::path::RelPath;
use crate::source::Oid;
use crate::vars::{Question, VarName};

/// A [`Result`](result::Result) that fails with an [`Error`](enum@Error) unless told otherwise.
pub type Result<T, E = Error> = result::Result<T, E>;

/// Why a devset operation did not go through.
///
/// Each domain has its own error, so a caller matches as narrowly as it needs to:
/// `Error::Target(TargetError::Busy)` is one lock, [`Error::Target`] every target failure.
#[derive(Debug, Error)]
#[non_exhaustive]
#[expect(
    clippy::error_impl_error,
    reason = "the crate's one umbrella error, named as `io::Error` and `serde_json::Error` are"
)]
pub enum Error {
    /// Filesystem or process I/O failed; the message names the path.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// A file devset reads does not parse.
    #[error(transparent)]
    Parse(Box<ParseError>),
    /// A string is not a [`RelPath`].
    #[error(transparent)]
    Path(#[from] PathError),
    /// A string is not a name.
    #[error(transparent)]
    Name(#[from] NameError),
    /// A source is misconfigured, or git failed on it.
    #[error(transparent)]
    Source(#[from] SourceError),
    /// A profile, or the layers together, do not resolve.
    #[error(transparent)]
    Profile(#[from] ProfileError),
    /// The target cannot be found, read or written.
    #[error(transparent)]
    Target(#[from] TargetError),
    /// A variable is unanswered, unknown, or breaks a template.
    #[error(transparent)]
    Var(#[from] VarError),
    /// A merge cannot run, or its resolution cannot be installed.
    #[error(transparent)]
    Merge(#[from] MergeError),
}

impl From<ParseError> for Error {
    fn from(error: ParseError) -> Self {
        Self::Parse(Box::new(error))
    }
}

/// A file that does not parse, with the text to point into.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{file}: {message}")]
pub struct ParseError {
    /// The file, as users know it.
    pub file: String,
    /// Its text; empty when it is not UTF-8.
    pub text: String,
    /// The offending bytes of `text`, when the parser knows them.
    pub span: Option<Range<usize>>,
    /// What is wrong.
    pub message: String,
}

/// Why a string is not a [`RelPath`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("invalid path {path:?}: {rule}")]
pub struct PathError {
    /// The rejected input.
    pub(crate) path: String,
    /// The rule it breaks.
    pub(crate) rule: &'static str,
}

/// Why a string is not the name of a profile, a source, a feature or a scaffold.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("invalid {kind} name {name:?}: {rule}")]
pub struct NameError {
    /// What it would name: `profile`, `source`, `feature`, `scaffold`, or `layer`.
    pub(crate) kind: &'static str,
    /// The rejected input.
    pub(crate) name: String,
    /// The rule it breaks.
    pub(crate) rule: &'static str,
}

/// Why a source cannot be read.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceError {
    /// Neither `git` nor `path` is set.
    #[error("invalid source: needs `git` or `path`")]
    NoLocation,
    /// A local `path` is empty.
    #[error("invalid source: `path` is empty")]
    EmptyPath,
    /// `tag`, `branch` or `rev` is set without `git`.
    #[error("invalid source: `tag`, `branch` and `rev` need `git`")]
    RefWithoutGit,
    /// More than one of `tag`, `branch` and `rev` is set.
    #[error("invalid source: at most one of `tag`, `branch` and `rev`")]
    TwoRefs,
    /// `git` is empty, or would reach git as an option.
    #[error("invalid source: `git` must be a repository URL, not {url:?}")]
    Url {
        /// As configured.
        url: String,
    },
    /// `rev` is not a full commit id.
    #[error("invalid source: `rev` must be a full commit id, not {rev:?}")]
    Rev {
        /// As configured.
        rev: String,
    },
    /// `path` within a repository is not a [`RelPath`].
    #[error(transparent)]
    Path(#[from] PathError),
    /// A file the profile lists is a symlink, a directory or a submodule.
    #[error("{file} is not a regular file")]
    NotAFile {
        /// The file, as a path or `<commit>:<path>`.
        file: String,
    },
    /// `git` is not on `PATH`.
    #[error("git sources need `git`, which is not on PATH")]
    NoGit,
    /// The remote has no such branch or tag, or no default branch.
    #[error("{url} has no {reference}")]
    NoRef {
        /// The remote, as configured.
        url: String,
        /// What was asked for: `branch <name>`, `tag <name>` or `a default branch`.
        reference: String,
    },
    /// The remote has no commit with the pinned id.
    #[error("{url} has no commit {rev}")]
    NoCommit {
        /// The remote, as configured.
        url: String,
        /// The commit asked for.
        rev: Oid,
    },
    /// The remote needs credentials that git could not get.
    #[error("{url} needs credentials, and git could not get them")]
    Credentials {
        /// The remote, as configured.
        url: String,
    },
    /// A `git` command failed.
    #[error("git {command} failed: {}", stderr.lines().next().unwrap_or("with no message"))]
    Git {
        /// The subcommand, and the remote as configured when it reached one.
        command: String,
        /// What git reported, one message per line.
        stderr: String,
    },
}

/// Why a profile, or the layers together, do not resolve.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfileError {
    /// A source holds no profile at all.
    #[error("{location} holds no profile: no profile.toml in it")]
    NoProfiles {
        /// The source, as configured.
        location: String,
    },
    /// A source holds no profile of the name asked for.
    #[error("{location} has no profile named {name}")]
    NotInSource {
        /// The source, as users know it.
        location: String,
        /// The name asked for.
        name: ProfileName,
        /// Every profile it holds.
        profiles: Vec<ProfileName>,
    },
    /// Two profiles in one source have one name, which must tell them apart.
    #[error("two profiles in {location} are named {name}: {first} and {second}")]
    SameNameInSource {
        /// The source, as users know it.
        location: String,
        /// The name.
        name: ProfileName,
        /// The first one's directory in the source.
        first: String,
        /// The second one's.
        second: String,
    },
    /// A manifest says something of itself that does not hold.
    #[error("{file}: {message}")]
    Manifest {
        /// The manifest, as users know it.
        file: String,
        /// What is wrong, and where in it.
        message: String,
    },
    /// The profile's `devset` requirement excludes this build.
    #[error("profile {profile} requires devset {requires}; this is {version}", version = crate::VERSION)]
    Incompatible {
        /// The profile's name.
        profile: ProfileName,
        /// Its requirement.
        requires: String,
    },
    /// `[files]` lists a path, or a starter, that `files/` does not hold.
    #[error("profile {profile} lists {path}, but files/{path} does not exist")]
    MissingPayload {
        /// The profile's name.
        profile: ProfileName,
        /// The listed path, as written.
        path: RelPath,
    },
    /// A profile is asked for a feature it does not declare.
    #[error("profile {profile} has no feature {feature}")]
    UnknownFeature {
        /// The profile.
        profile: ProfileName,
        /// The feature asked for.
        feature: FeatureName,
        /// Who asked: the target, or a requiring profile.
        by: String,
        /// Every feature it declares.
        known: Vec<FeatureName>,
    },
    /// Two of a profile's paths name one file once their variables are answered.
    #[error("profile {profile} lists {path} twice: as {first} and as {second}")]
    SamePath {
        /// The profile.
        profile: ProfileName,
        /// The path both render to.
        path: RelPath,
        /// The first, as written.
        first: RelPath,
        /// The second, as written.
        second: RelPath,
    },
    /// A path, or a glob, in `when.exists` or a scaffold's `unless` cannot be read.
    #[error("profile {profile}: {pattern:?} is not a path or a glob in the target: {reason}")]
    Pattern {
        /// The profile.
        profile: ProfileName,
        /// The pattern, variables answered.
        pattern: String,
        /// Why not.
        reason: String,
    },
    /// Several layers provide one path, and the target has not chosen.
    #[error("{path} is provided by {}", list(layers))]
    Collision {
        /// The path.
        path: RelPath,
        /// Every layer providing it.
        layers: Vec<ProfileName>,
    },
    /// Several layers would start one file the target lacks, and the target has not chosen.
    #[error("{path} is started by {}", list(layers))]
    Starters {
        /// The path.
        path: RelPath,
        /// Every layer that would start it.
        layers: Vec<ProfileName>,
    },
    /// An override's `from` names a layer that does not list the path.
    #[error("{from} does not provide {path}")]
    NotProvided {
        /// The path.
        path: RelPath,
        /// The layer named.
        from: ProfileName,
        /// The layers that do provide it.
        layers: Vec<ProfileName>,
    },
    /// Two paths name one file on a case-insensitive filesystem.
    #[error("{first} and {second} are the same file on case-insensitive filesystems")]
    FoldCollision {
        /// The path met first.
        first: RelPath,
        /// The path that folds onto it.
        second: RelPath,
    },
    /// Requirements lead back to a profile that requires them.
    #[error("requirements form a cycle: {}", list_chain(chain))]
    Cycle {
        /// Each profile on the way round, the repeated one last.
        chain: Vec<ProfileName>,
    },
    /// One profile is required at two refs, which would provide one set of files twice.
    #[error("{first} and {second} are one source at two refs")]
    Diverged {
        /// The source met first.
        first: String,
        /// The source met second.
        second: String,
    },
    /// Keys are owned in a file devset does not read as TOML, JSON or YAML.
    #[error("{path} is not read as TOML, JSON or YAML, so a profile cannot own its keys")]
    NoKeys {
        /// The file.
        path: RelPath,
    },
    /// A block is owned in a file whose comment syntax devset does not know.
    #[error("devset knows no comment syntax for {path}, which a block's markers need")]
    NoComment {
        /// The file.
        path: RelPath,
    },
    /// Two layers own overlapping keys of one file.
    #[error("{first} and {second} both own {key} in {path}")]
    Overlap {
        /// The file.
        path: RelPath,
        /// The key, as people write it.
        key: String,
        /// The layer met first.
        first: ProfileName,
        /// The layer that overlaps it.
        second: ProfileName,
    },
    /// Layers own one file in different scopes: one whole and another in part, or one by keys
    /// and another by a block. A whole `once` file may start one that others own parts of.
    #[error("{path} is owned in different scopes: {}", scopes(layers))]
    Scopes {
        /// The file.
        path: RelPath,
        /// Each layer, with its scope.
        layers: Vec<(ProfileName, Scope)>,
    },
    /// Two active profiles have one name, which must tell layers apart.
    #[error("two layers are named {name}: {first} and {second}")]
    SameName {
        /// The name.
        name: ProfileName,
        /// The first one's source.
        first: String,
        /// The second one's.
        second: String,
    },
    /// Two layers set one setting differently, and the target does not decide.
    #[error("{first} and {second} set {key} differently")]
    Setting {
        /// The setting, as `table.key`.
        key: String,
        /// The layer that set it first.
        first: ProfileName,
        /// The layer that disagrees.
        second: ProfileName,
    },
}

/// Why a target cannot be found, read or written.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TargetError {
    /// No directory from here up holds `.devset/config.toml`.
    #[error("no devset target in {dir} or any parent")]
    NotFound {
        /// Where the search began.
        dir: Utf8PathBuf,
    },
    /// A new target would lie inside another.
    #[error("{dir} is inside the devset target at {root}")]
    Nested {
        /// The directory asked for.
        dir: Utf8PathBuf,
        /// The enclosing target's root.
        root: Utf8PathBuf,
    },
    /// `config.toml` is devset 0.1's: its layers name locations, not profiles.
    #[error(
        ".devset/config.toml is written for devset 0.1: its layers name locations, not profiles"
    )]
    OldConfig,
    /// A layer, or a command, names a source `[sources]` does not.
    #[error("no source is named {name}{}", naming(layers))]
    NoSuchSource {
        /// The name asked for.
        name: SourceName,
        /// Every source's name.
        sources: Vec<SourceName>,
        /// The layers `config.toml` names it for.
        layers: Vec<ProfileRef>,
    },
    /// `[sources]` already names another source so.
    #[error("the source {name} is already {location}")]
    SourceExists {
        /// The name.
        name: SourceName,
        /// The source it names, as configured.
        location: String,
    },
    /// A profile is named without its source, and the target names several, or none.
    #[error("which source is {profile} in? name it as `source/{profile}`")]
    WhichSource {
        /// The profile.
        profile: ProfileName,
        /// Every source the target names.
        sources: Vec<SourceName>,
    },
    /// A source holds several profiles, and none was named.
    #[error("{location} holds {} profiles; name the one to add", profiles.len())]
    Ambiguous {
        /// The source, as configured.
        location: String,
        /// Every profile it holds.
        profiles: Vec<ProfileName>,
    },
    /// `config.toml` already lists the layer.
    #[error("the layer {layer} is already applied")]
    DuplicateLayer {
        /// The layer's profile.
        layer: ProfileName,
    },
    /// `config.toml` lists one layer twice, which a layer's name must tell apart.
    #[error("`config.toml` lists the layer {layer} twice")]
    LayerTwice {
        /// The layer's profile.
        layer: ProfileName,
    },
    /// No layer, and no source, has the name asked for.
    #[error("no layer is named {name}")]
    NoSuchLayer {
        /// The name asked for.
        name: String,
        /// Every layer's name, and every source's.
        names: Vec<String>,
    },
    /// A scaffold named on the command line is no active profile's.
    #[error("no scaffold is named {name}")]
    NoSuchScaffold {
        /// The name asked for, `profile/group`.
        name: String,
        /// Every active profile's scaffolds.
        scaffolds: Vec<String>,
    },
    /// A path named on the command line is not a file devset manages.
    #[error("{path} is not a file devset manages")]
    NotManaged {
        /// The path, as given.
        path: String,
        /// Every managed path.
        managed: Vec<RelPath>,
    },
    /// A layer named where only a configured one will do is one another profile requires.
    #[error("{name} is required by {by}, not configured by this target")]
    Required {
        /// The layer asked for.
        name: ProfileName,
        /// The profile that requires it.
        by: ProfileName,
    },
    /// `config.toml` overrides a path no layer provides.
    #[error(".devset/config.toml overrides {path}, which no layer provides")]
    StaleOverride {
        /// The overridden path.
        path: RelPath,
        /// Every path the layers provide.
        provided: Vec<RelPath>,
    },
    /// A managed path on disk is a symlink, a directory or a special file.
    #[error("{path} is {kind}, not a regular file")]
    NotAFile {
        /// The managed path.
        path: RelPath,
        /// What is there instead: `a symlink`, `a directory` or `a special file`.
        kind: &'static str,
    },
    /// Another devset holds the target's lock.
    #[error("another devset is running in this target")]
    Busy,
    /// `state.toml` changed while this devset ran.
    #[error(".devset/state.toml changed while devset ran")]
    Concurrent,
}

/// Why the layers' variables cannot be answered or rendered.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VarError {
    /// A name is not an identifier.
    #[error(
        "invalid variable name {name:?}: use letters, digits and `_`, not starting with a digit"
    )]
    Name {
        /// The rejected input.
        name: String,
    },
    /// A variable is named `devset`, which templates see the target's profiles under.
    #[error("the variable name `devset` is reserved: templates see the target's profiles under it")]
    Reserved,
    /// The target answers a variable no layer declares.
    #[error("no profile declares a variable named {name}")]
    Unknown {
        /// The answered name.
        name: VarName,
        /// Every declared name.
        declared: Vec<VarName>,
    },
    /// The target has not answered these variables.
    #[error("{} unanswered: {}", plural(questions.len(), "variable"), list(questions.iter().map(|q| &q.name)))]
    Unanswered {
        /// One per variable.
        questions: Vec<Question>,
    },
    /// A template uses a variable no profile declares.
    #[error("template {path} uses `{name}`, which no profile declares")]
    Undeclared {
        /// The template.
        path: RelPath,
        /// The variable.
        name: String,
        /// Every declared name.
        declared: Vec<VarName>,
    },
    /// A template cannot be read as text.
    #[error("template {path} cannot be rendered: {reason}")]
    Template {
        /// The template.
        path: RelPath,
        /// Why not.
        reason: String,
    },
}

/// Why a merge cannot run, or its resolution cannot be installed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MergeError {
    /// A `driver` line does not parse.
    #[error("merge driver `{line}`: {reason}")]
    Driver {
        /// As configured.
        line: String,
        /// The rule it breaks.
        reason: &'static str,
    },
    /// The driver's program is not installed, or not on `PATH`.
    #[error("merge driver `{program}` is not installed")]
    NoDriver {
        /// The program.
        program: String,
    },
    /// A driver program did not run to completion.
    #[error("merge driver `{program}`: {reason}")]
    Run {
        /// The program.
        program: String,
        /// Why it stopped.
        reason: String,
    },
    /// Conflicts wait in `.devset/conflicts/`.
    #[error("{} unresolved: {}", plural(paths.len(), "conflict"), list(paths))]
    Unresolved {
        /// Each conflicted path.
        paths: Vec<RelPath>,
    },
    /// Resolutions were to be installed, and no conflict waits.
    #[error("there are no conflicts to continue from")]
    NothingToContinue,
    /// An update was to be taken back, and none is unfinished.
    #[error("there is no unfinished update to abort")]
    NothingToAbort,
    /// Taking an update back would discard changes made since it.
    #[error("{} changed since the update: {}", plural(paths.len(), "file"), list(paths))]
    ChangedSince {
        /// Each changed file, relative to the target root.
        paths: Vec<String>,
    },
    /// A copy an update saved, to take it back, is missing or damaged.
    #[error("the saved copy of {path} in .devset/conflicts/.devset/ is missing or damaged")]
    CorruptUndo {
        /// The file it would restore, relative to the target root.
        path: String,
    },
    /// A resolution still has conflict markers.
    #[error(".devset/conflicts/{path} still has conflict markers")]
    Unmerged {
        /// The conflicted path.
        path: RelPath,
    },
    /// A resolution does not pass its file's check.
    #[error(".devset/conflicts/{path}: {reason}")]
    Invalid {
        /// The conflicted path.
        path: RelPath,
        /// Why the check failed.
        reason: String,
    },
}

/// `items`, comma-separated, with `and` before the last.
pub(crate) fn list<T: Display>(items: impl IntoIterator<Item = T>) -> String {
    let items: Vec<String> = items.into_iter().map(|item| item.to_string()).collect();
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => items.concat(),
    }
}

/// Each layer and its scope: "`file` by base and `keys` by lints".
fn scopes(layers: &[(ProfileName, Scope)]) -> String {
    list(layers.iter().map(|(layer, scope)| format!("`{scope}` by {layer}")))
}

/// `: the layers a/b and a/c name it`, when `layers` is not empty.
fn naming(layers: &[ProfileRef]) -> String {
    match layers {
        [] => String::new(),
        [one] => format!(": the layer {one} names it"),
        _ => format!(": the layers {} name it", list(layers)),
    }
}

/// Each profile of a cycle, arrowed: `a → b → a`.
fn list_chain(chain: &[ProfileName]) -> String {
    chain.iter().map(ProfileName::as_str).collect::<Vec<_>>().join(" → ")
}

/// `n` and `noun`, pluralised.
fn plural(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}
