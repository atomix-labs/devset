//! Why devset did not go through: a path, a source, a profile, a target, a variable, or a merge.

use core::fmt::Display;
use core::ops::Range;
use core::result;
use std::io;

use camino::Utf8PathBuf;
use thiserror::Error;

use crate::path::RelPath;
use crate::vars::{Question, VarName};

/// A [`Result`](result::Result) that fails with an [`Error`](enum@Error) unless told otherwise.
pub type Result<T, E = Error> = result::Result<T, E>;

/// Why a devset operation did not go through.
///
/// Each domain has its own error, so a caller matches as narrowly as it needs to:
/// `Error::Target(TargetError::Busy)` is one lock, [`Error::Target`] every target failure.
#[derive(Debug, Error)]
#[non_exhaustive]
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

/// Why a source cannot be read.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceError {
    /// Neither `git` nor `path` is set.
    #[error("invalid source: needs `git` or `path`")]
    NoLocation,
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
    /// A `git` command failed.
    #[error("git {command} failed: {stderr}")]
    Git {
        /// The subcommand and its arguments.
        command: String,
        /// What git reported.
        stderr: String,
    },
}

/// Why a profile, or the layers together, do not resolve.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfileError {
    /// The source has no `profile.toml`.
    #[error("{location} is not a profile: it has no profile.toml")]
    NotAProfile {
        /// The source, as configured.
        location: String,
    },
    /// The profile's `devset` requirement excludes this build.
    #[error("profile {profile} requires devset {requires}; this is {version}", version = crate::VERSION)]
    Incompatible {
        /// The profile's name.
        profile: String,
        /// Its requirement.
        requires: String,
    },
    /// `[files]` lists a path that `files/` does not hold.
    #[error("profile {profile} lists {path}, but files/{path} does not exist")]
    MissingPayload {
        /// The profile's name.
        profile: String,
        /// The listed path.
        path: RelPath,
    },
    /// Several layers provide one path, and the target has not chosen.
    #[error("{path} is provided by {}", list(layers))]
    Collision {
        /// The path.
        path: RelPath,
        /// Every layer providing it.
        layers: Vec<String>,
    },
    /// An override's `from` names a layer that does not provide the path.
    #[error("{from} does not provide {path}")]
    NotProvided {
        /// The path.
        path: RelPath,
        /// The layer named.
        from: String,
        /// The layers that do provide it.
        layers: Vec<String>,
    },
    /// Two paths name one file on a case-insensitive filesystem.
    #[error("{first} and {second} are the same file on case-insensitive filesystems")]
    FoldCollision {
        /// The path met first.
        first: RelPath,
        /// The path that folds onto it.
        second: RelPath,
    },
    /// Two layers set one setting differently, and the target does not decide.
    #[error("{first} and {second} set {key} differently")]
    Setting {
        /// The setting, as `table.key`.
        key: String,
        /// The layer that set it first.
        first: String,
        /// The layer that disagrees.
        second: String,
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
    /// `config.toml` already lists the layer.
    #[error("the layer {layer} is already applied")]
    DuplicateLayer {
        /// The layer's source.
        layer: String,
    },
    /// No layer's profile has the name asked for.
    #[error("no layer is named {name}")]
    NoSuchLayer {
        /// The name asked for.
        name: String,
        /// Every layer's name.
        layers: Vec<String>,
    },
    /// `config.toml` overrides a path no layer provides.
    #[error(".devset/config.toml overrides {path}, which no layer provides")]
    StaleOverride {
        /// The overridden path.
        path: RelPath,
        /// Every path the layers provide.
        provided: Vec<RelPath>,
    },
    /// A managed path on disk is a symlink or a directory.
    #[error("{path} is not a regular file")]
    NotAFile {
        /// The path on disk.
        path: Utf8PathBuf,
    },
    /// A recorded base is missing, or does not match its digest.
    #[error("the recorded base of {path} in .devset/base/ is missing or damaged")]
    CorruptBase {
        /// The managed path.
        path: RelPath,
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
    /// A template does not render.
    #[error("template {path}: {reason}")]
    Template {
        /// The template.
        path: RelPath,
        /// What the renderer reported.
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
fn list<T: Display>(items: impl IntoIterator<Item = T>) -> String {
    let items: Vec<String> = items.into_iter().map(|item| item.to_string()).collect();
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => items.concat(),
    }
}

/// `n` and `noun`, pluralised.
fn plural(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}
