//! Where a layer's profile comes from: a local directory, or a git repository at one commit.

use alloc::borrow::Cow;
use alloc::sync::Arc;
use core::fmt;
use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::Mutex;

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use etcetera::BaseStrategy;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use crate::errors::{Result, SourceError};
use crate::git;
use crate::path::RelPath;
use crate::profile::MANIFEST;
use crate::tree::Tree;

/// Where a layer's profile comes from; written as [`SourceSpec`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SourceSpec", into = "SourceSpec")]
pub enum Source {
    /// A directory, relative to the target root unless absolute.
    Dir(Utf8PathBuf),
    /// A git repository.
    Git {
        /// Anything `git fetch` accepts.
        url: String,
        /// Which commit.
        at: GitRef,
        /// The profile's directory in the repository; its root if `None`.
        path: Option<RelPath>,
    },
}

/// Which commit of a git source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitRef {
    /// The remote's default branch.
    Head,
    /// A branch.
    Branch(String),
    /// A tag.
    Tag(String),
    /// A commit.
    Rev(Oid),
}

/// A full commit id: 40 or 64 hex digits, lowercase.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Oid(Box<str>);

/// A source as written in `config.toml`: Cargo's git-dependency fields.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceSpec {
    /// Repository URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<String>,
    /// Tag to use; only with `git`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Branch to use; only with `git`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Full commit id to use; only with `git`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    /// The profile's directory: relative to the target, or within the repository with `git`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl TryFrom<SourceSpec> for Source {
    type Error = SourceError;

    fn try_from(spec: SourceSpec) -> Result<Self, SourceError> {
        let SourceSpec {
            git,
            tag,
            branch,
            rev,
            path,
        } = spec;
        let Some(url) = git else {
            if tag.is_some() || branch.is_some() || rev.is_some() {
                return Err(SourceError::RefWithoutGit);
            }
            let path = path.ok_or(SourceError::NoLocation)?;
            return dir(&path).map(Self::Dir).ok_or(SourceError::EmptyPath);
        };
        // A leading `-` would reach git as an option.
        if url.is_empty() || url.starts_with('-') {
            return Err(SourceError::Url { url });
        }
        let at = match (tag, branch, rev) {
            (None, None, None) => GitRef::Head,
            (Some(tag), None, None) => GitRef::Tag(tag),
            (None, Some(branch), None) => GitRef::Branch(branch),
            (None, None, Some(rev)) => GitRef::Rev(Oid::try_from(rev)?),
            _ => return Err(SourceError::TwoRefs),
        };
        let path = path
            .filter(|p| !p.is_empty())
            .map(RelPath::try_from)
            .transpose()?;
        Ok(Self::Git { url, at, path })
    }
}

/// `path` without `.` components, so one directory has one spelling; `None` when empty.
fn dir(path: &str) -> Option<Utf8PathBuf> {
    if path.is_empty() {
        return None;
    }
    let dir: Utf8PathBuf = Utf8Path::new(path)
        .components()
        .filter(|c| *c != Utf8Component::CurDir)
        .collect();
    Some(if dir.as_str().is_empty() {
        Utf8PathBuf::from(".")
    } else {
        dir
    })
}

impl From<Source> for SourceSpec {
    fn from(source: Source) -> Self {
        match source {
            Source::Dir(path) => Self {
                path: Some(path.into_string()),
                ..Self::default()
            },
            Source::Git { url, at, path } => {
                let mut spec = Self {
                    git: Some(url),
                    path: path.map(String::from),
                    ..Self::default()
                };
                match at {
                    GitRef::Head => {}
                    GitRef::Branch(branch) => spec.branch = Some(branch),
                    GitRef::Tag(tag) => spec.tag = Some(tag),
                    GitRef::Rev(rev) => spec.rev = Some(rev.into()),
                }
                spec
            }
        }
    }
}

impl JsonSchema for Source {
    fn schema_name() -> Cow<'static, str> {
        SourceSpec::schema_name()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        SourceSpec::json_schema(generator)
    }
}

impl Source {
    /// Whether `other` is this profile at another ref: one repository and path, two refs.
    pub(crate) fn twin(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Git { url, at, path },
                Self::Git {
                    url: other_url,
                    at: other_at,
                    path: other_path,
                },
            ) => url == other_url && path == other_path && at != other_at,
            (Self::Dir(_) | Self::Git { .. }, Self::Dir(_) | Self::Git { .. }) => false,
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dir(path) => f.write_str(path.as_str()),
            Self::Git { url, at, path } => {
                f.write_str(url)?;
                match at {
                    GitRef::Head => {}
                    GitRef::Branch(branch) => write!(f, " branch {branch}")?,
                    GitRef::Tag(tag) => write!(f, " tag {tag}")?,
                    GitRef::Rev(rev) => write!(f, " rev {rev}")?,
                }
                path.as_ref()
                    .map_or(Ok(()), |path| write!(f, " path {path}"))
            }
        }
    }
}

impl Oid {
    /// The id as hex.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Oid {
    type Error = SourceError;

    fn try_from(mut id: String) -> Result<Self, SourceError> {
        if !matches!(id.len(), 40 | 64) || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(SourceError::Rev { rev: id });
        }
        id.make_ascii_lowercase();
        Ok(Self(id.into_boxed_str()))
    }
}

impl From<Oid> for String {
    fn from(id: Oid) -> Self {
        id.0.into_string()
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Remote access for a git layer, reported through [`Cache::on_fetch`].
#[derive(Clone, Copy, Debug)]
pub enum Fetch<'a> {
    /// About to contact this URL.
    Start(&'a str),
    /// Finished with it.
    Done(&'a str),
}

/// What [`Cache::on_fetch`] installs.
type Hook = Arc<dyn Fn(Fetch<'_>) + Send + Sync>;

/// Fetched git profiles, shared by every target on the machine.
///
/// One process sees one commit per ref: each ref is asked of its remote once.
#[derive(Clone)]
pub struct Cache {
    /// Where it lives.
    dir: Utf8PathBuf,
    /// Told about remote access.
    hook: Option<Hook>,
    /// Refs resolved so far, by URL and ref name, so one process sees one commit per ref.
    refs: Arc<Mutex<HashMap<(String, String), Oid>>>,
    /// Whether git may ask for credentials on the terminal.
    prompts: bool,
}

impl Cache {
    /// The per-user cache: `$XDG_CACHE_HOME/devset`, or the platform's equivalent.
    ///
    /// # Errors
    /// [`Error::Io`](crate::Error::Io), there is no home directory, or its path is not UTF-8.
    pub fn user() -> Result<Self> {
        let base = etcetera::choose_base_strategy().map_err(io::Error::other)?;
        let dir = Utf8PathBuf::try_from(base.cache_dir()).map_err(io::Error::other)?;
        Ok(Self::at(dir.join("devset")))
    }

    /// A cache in `dir`.
    #[must_use]
    pub fn at(dir: Utf8PathBuf) -> Self {
        Self {
            dir,
            hook: None,
            refs: Arc::new(Mutex::new(HashMap::new())),
            prompts: false,
        }
    }

    /// This cache, calling `hook` around every remote access so a caller can show progress.
    #[must_use]
    pub fn on_fetch<F: Fn(Fetch<'_>) + Send + Sync + 'static>(mut self, hook: F) -> Self {
        self.hook = Some(Arc::new(hook));
        self
    }

    /// This cache, letting git ask for credentials on the terminal when `allowed`.
    ///
    /// Off unless asked for, so a run with no one to answer fails rather than waits.
    #[must_use]
    pub const fn prompting(mut self, allowed: bool) -> Self {
        self.prompts = allowed;
        self
    }

    /// Whether git may ask for credentials on the terminal.
    pub(crate) const fn prompts(&self) -> bool {
        self.prompts
    }

    /// Where the cache lives.
    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.dir
    }

    /// The commit `name` at `url` was resolved to earlier in this process.
    pub(crate) fn resolved(&self, url: &str, name: &str) -> Option<Oid> {
        let refs = self.refs.lock().ok()?;
        refs.get(&(url.to_owned(), name.to_owned())).cloned()
    }

    /// Remembers that `name` at `url` is `rev`.
    pub(crate) fn remember(&self, url: &str, name: &str, rev: &Oid) {
        if let Ok(mut refs) = self.refs.lock() {
            refs.insert((url.to_owned(), name.to_owned()), rev.clone());
        }
    }

    /// Reports `event` to the hook, if any.
    pub(crate) fn notify(&self, event: Fetch<'_>) {
        if let Some(hook) = &self.hook {
            hook(event);
        }
    }
}

impl fmt::Debug for Cache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cache")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

/// A source opened at one revision.
#[derive(Debug)]
pub(crate) enum Reader {
    /// A local directory.
    Dir(Utf8PathBuf),
    /// A commit in the cache.
    Git(git::Commit),
}

impl Source {
    /// Opens the source: a git source at `pin` if given, else at its ref, fetching as needed.
    pub(crate) fn open(&self, root: &Utf8Path, pin: Option<&Oid>, cache: &Cache) -> Result<Reader> {
        match self {
            Self::Dir(dir) => Ok(Reader::Dir(root.join(dir))),
            Self::Git { url, at, path } => {
                git::Commit::open(&locate(url, root), url, at, path.as_ref(), pin, cache)
                    .map(Reader::Git)
            }
        }
    }
}

/// `url` as `git` sees it from any directory: a local path is made absolute against `root`.
///
/// Git's rule: `host:path` is scp-like only when the colon precedes any slash.
fn locate<'a>(url: &'a str, root: &Utf8Path) -> Cow<'a, str> {
    let remote = url.contains("://")
        || url
            .find(':')
            .is_some_and(|colon| url.find('/').is_none_or(|slash| colon < slash));
    if remote {
        Cow::Borrowed(url)
    } else {
        Cow::Owned(root.join(url).into_string())
    }
}

impl Reader {
    /// The commit, for a versioned source.
    pub(crate) const fn rev(&self) -> Option<&Oid> {
        match self {
            Self::Dir(_) => None,
            Self::Git(commit) => Some(commit.rev()),
        }
    }

    /// Every directory the source holds that is a profile, as its `path` would name it.
    pub(crate) fn profiles(&self, configured: &Source) -> Vec<String> {
        match (self, configured) {
            (Self::Git(commit), _) => commit.profiles(),
            (Self::Dir(root), Source::Dir(written)) => local_profiles(root, written),
            (Self::Dir(_), Source::Git { .. }) => Vec::new(),
        }
    }

    /// The files at `paths` under `dir` in the profile, keyed by `paths`; absent ones left out.
    pub(crate) fn read(&self, dir: &str, paths: &[RelPath]) -> Result<Tree> {
        match self {
            Self::Dir(root) => read_dir(&root.join(dir), paths),
            Self::Git(commit) => commit.read(dir, paths),
        }
    }
}

/// The profiles around `root`, as `path` would name them.
///
/// The two directories above it, for a path pointing inside a profile, and two levels below.
fn local_profiles(root: &Utf8Path, written: &Utf8Path) -> Vec<String> {
    let mut found: Vec<String> = root
        .ancestors()
        .zip(written.ancestors())
        .skip(1)
        .take(2)
        .filter(|(dir, name)| !name.as_str().is_empty() && dir.join(MANIFEST).is_file())
        .map(|(_, name)| name.to_string())
        .collect();
    let mut level = vec![(root.to_path_buf(), written.to_path_buf())];
    for _ in 0..2 {
        let mut next = Vec::new();
        for (dir, name) in level {
            let Ok(entries) = fs_err::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(child) = Utf8PathBuf::try_from(entry.path()) else {
                    continue;
                };
                if !child.is_dir() || child.file_name().is_some_and(|n| n.starts_with('.')) {
                    continue;
                }
                let child_name = name.join(child.file_name().unwrap_or_default());
                if child.join(MANIFEST).is_file() {
                    found.push(child_name.to_string());
                }
                next.push((child, child_name));
            }
        }
        level = next;
    }
    found.sort();
    found
}

/// [`Reader::read`] for a local directory.
fn read_dir(dir: &Utf8Path, paths: &[RelPath]) -> Result<Tree> {
    let mut tree = Tree::default();
    for path in paths {
        let file = path.under(dir);
        match fs_err::symlink_metadata(&file) {
            Ok(meta) if meta.is_file() => {
                tree.insert(path.clone(), |buf| {
                    fs_err::File::open(&file)?.read_to_end(buf)?;
                    Ok(())
                })?;
            }
            Ok(_) => {
                return Err(SourceError::NotAFile {
                    file: file.into_string(),
                }
                .into());
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::{GitRef, Source, SourceSpec};

    fn parse(toml: &str) -> Result<Source, String> {
        toml::from_str::<SourceSpec>(toml)
            .map_err(|e| e.to_string())
            .and_then(|spec| Source::try_from(spec).map_err(|e| e.to_string()))
    }

    #[test]
    fn valid() {
        assert_eq!(
            parse(r#"path = "../p""#),
            Ok(Source::Dir("../p".into())),
            "local directory"
        );
        for (written, dir) in [("./../p/", "../p"), ("./p", "p"), (".", "."), ("./", ".")] {
            let toml = format!("path = \"{written}\"");
            assert_eq!(
                parse(&toml),
                Ok(Source::Dir(dir.into())),
                "{written:?} has one spelling"
            );
        }
        let Ok(Source::Git { url, at, path }) = parse(
            r#"git = "https://h/r"
tag = "v1"
path = "rust""#,
        ) else {
            panic!("git source should parse");
        };
        assert_eq!(
            (url.as_str(), at, path.map(String::from)),
            ("https://h/r", GitRef::Tag("v1".into()), Some("rust".into())),
            "fields"
        );
        let rev = "A".repeat(40);
        let Ok(Source::Git {
            at: GitRef::Rev(oid),
            ..
        }) = parse(&format!("git = \"u\"\nrev = \"{rev}\""))
        else {
            panic!("rev should parse");
        };
        assert_eq!(oid.as_str(), "a".repeat(40), "commit ids are lowercased");
    }

    #[test]
    fn invalid() {
        for toml in [
            "",
            r#"path = """#,
            r#"tag = "v1""#,
            r#"path = "p"
branch = "b""#,
            r#"git = "u"
tag = "v1"
branch = "b""#,
            r#"git = "u"
rev = "abc""#,
            r#"git = "--upload-pack=x""#,
            r#"git = "u"
path = "../x""#,
            r#"git = "u"
bogus = 1"#,
        ] {
            assert!(parse(toml).is_err(), "should be rejected: {toml:?}");
        }
    }

    #[test]
    fn local_git_paths_are_anchored() {
        let root = camino::Utf8Path::new("/work/repo");
        for (url, want) in [
            ("../p.git", "/work/repo/../p.git"),
            ("/abs/p.git", "/abs/p.git"),
            ("https://h/p", "https://h/p"),
            ("file:///p", "file:///p"),
            ("git@h:org/p", "git@h:org/p"),
            ("./a:b", "/work/repo/./a:b"),
        ] {
            assert_eq!(super::locate(url, root), want, "{url}");
        }
    }

    #[test]
    fn round_trips() {
        let toml = "git = \"https://h/r\"\nbranch = \"main\"\npath = \"a/b\"\n";
        let source = parse(toml).unwrap();
        assert_eq!(
            toml::to_string(&source).unwrap(),
            toml,
            "serialized as written"
        );
    }
}
