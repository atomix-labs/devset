//! Git sources, read through the user's own `git` into a bare repository per URL.

use alloc::collections::BTreeMap;
use core::str;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

use camino::{Utf8Path, Utf8PathBuf};

use crate::digest::Digest;
use crate::errors::{Error, Result, SourceError};
use crate::path::RelPath;
use crate::profile::MANIFEST;
use crate::source::{Cache, Fetch, GitRef, Oid};
use crate::tree::Tree;

/// What git says when it needed credentials and could not get them.
const NO_CREDENTIALS: [&str; 5] = [
    "terminal prompts disabled",
    "could not read Username",
    "Authentication failed",
    "Permission denied (publickey",
    "Host key verification failed",
];

/// What a server says when it has no commit with the id asked for.
const NO_COMMIT: [&str; 3] = ["not our ref", "no such remote ref", "unadvertised object"];

/// A remote as git reaches it, and as users know it.
#[derive(Clone, Copy, Debug)]
struct Remote<'a> {
    /// What git is given: a local path made absolute.
    url: &'a str,
    /// As configured, for messages.
    shown: &'a str,
    /// Whether git may ask for credentials on the terminal.
    prompts: bool,
}

/// A commit in a cached bare repository, with every file in it.
#[derive(Debug)]
pub(crate) struct Commit {
    /// The bare repository.
    repo: Utf8PathBuf,
    /// The commit.
    rev: Oid,
    /// The profile's directory in the repository; its root if `None`.
    dir: Option<RelPath>,
    /// Every file in the commit, by path from the repository root.
    files: BTreeMap<String, Blob>,
}

/// A file in a commit.
#[derive(Debug)]
struct Blob {
    /// Object id.
    id: String,
    /// Whether it is a regular file rather than a symlink or submodule.
    regular: bool,
}

impl Commit {
    /// The commit of `url` at `pin`, or at `at` resolved now, fetched into `cache` if absent.
    ///
    /// `shown` is the URL as configured, for progress and messages.
    pub(crate) fn open(
        url: &str,
        shown: &str,
        at: &GitRef,
        dir: Option<&RelPath>,
        pin: Option<&Oid>,
        cache: &Cache,
    ) -> Result<Self> {
        let remote = Remote {
            url,
            shown,
            prompts: cache.prompts(),
        };
        let repo = cache
            .path()
            .join("git")
            .join(Digest::of(url.as_bytes()).to_string());
        if !repo.join("HEAD").is_file() {
            fs_err::create_dir_all(&repo)?;
            git(Some(&repo), &["init", "--bare", "-q"], None)?;
        }
        let mut contacted = false;
        let mut contact = || {
            if !contacted {
                cache.notify(Fetch::Start(shown));
                contacted = true;
            }
        };
        let rev = match (pin, at) {
            (Some(rev), _) | (None, GitRef::Rev(rev)) => rev.clone(),
            (None, _) => {
                if let Some(rev) = cache.resolved(url, &wanted(at)) {
                    rev
                } else {
                    contact();
                    let rev = resolve(remote, at)?;
                    cache.remember(url, &wanted(at), &rev);
                    rev
                }
            }
        };
        if !has(&repo, &rev)? {
            contact();
            fetch(&repo, remote, at, &rev)?;
        }
        if contacted {
            cache.notify(Fetch::Done(shown));
        }
        let listing = git(Some(&repo), &["ls-tree", "-r", "-z", rev.as_str()], None)?;
        Ok(Self {
            repo,
            rev,
            dir: dir.cloned(),
            files: parse_listing(&listing),
        })
    }

    /// The commit.
    pub(crate) const fn rev(&self) -> &Oid {
        &self.rev
    }

    /// Every directory in the commit that holds a profile, as `path` would name it.
    pub(crate) fn profiles(&self) -> Vec<String> {
        let manifest = format!("/{MANIFEST}");
        self.files
            .keys()
            .filter_map(|path| path.strip_suffix(&manifest))
            .map(str::to_owned)
            .collect()
    }

    /// [`Reader::read`](crate::source::Reader::read) for this commit.
    pub(crate) fn read(&self, dir: &str, paths: &[RelPath]) -> Result<Tree> {
        let prefix: String = [self.dir.as_ref().map(RelPath::as_str), Some(dir)]
            .into_iter()
            .flatten()
            .filter(|part| !part.is_empty())
            .flat_map(|part| [part, "/"])
            .collect();
        let mut wanted = Vec::with_capacity(paths.len());
        for path in paths {
            let key = format!("{prefix}{path}");
            match self.files.get(&key) {
                None => {}
                Some(blob) if blob.regular => wanted.push((path, blob.id.as_str())),
                Some(_) => {
                    let short = self.rev.as_str().get(..7).unwrap_or(self.rev.as_str());
                    return Err(SourceError::NotAFile {
                        file: format!("{key} @{short}"),
                    }
                    .into());
                }
            }
        }
        let mut tree = Tree::default();
        if wanted.is_empty() {
            return Ok(tree);
        }
        // Lockstep, one object per request: `--batch` flushes each reply, so this cannot
        // deadlock on full pipes however many files there are.
        let mut child = command(Some(&self.repo), &["cat-file", "--batch"], false)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(spawn_error)?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("git: no stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("git: no stdout"))?;
        let mut stdout = BufReader::new(stdout);
        let mut header = String::new();
        for (path, id) in wanted {
            writeln!(stdin, "{id}")?;
            header.clear();
            stdout.read_line(&mut header)?;
            let size = blob_size(&header)
                .ok_or_else(|| io::Error::other(format!("git cat-file: {}", header.trim_end())))?;
            tree.insert(path.clone(), |buf| {
                let read = stdout.by_ref().take(size).read_to_end(buf)?;
                if u64::try_from(read).ok() == Some(size) {
                    Ok(())
                } else {
                    Err(io::ErrorKind::UnexpectedEof.into())
                }
            })?;
            stdout.read_exact(&mut [0])?;
        }
        drop(stdin);
        let status = child.wait()?;
        if !status.success() {
            let stderr = status.to_string();
            return Err(SourceError::Git {
                command: "cat-file".to_owned(),
                stderr,
            }
            .into());
        }
        Ok(tree)
    }
}

/// The size in a `cat-file --batch` header for a blob: `<id> blob <size>`.
fn blob_size(header: &str) -> Option<u64> {
    let mut fields = header.trim_end().split(' ');
    let (_, kind, size) = (fields.next()?, fields.next()?, fields.next()?);
    (kind == "blob").then(|| size.parse().ok()).flatten()
}

/// `ls-tree -r -z` output, by path.
fn parse_listing(raw: &[u8]) -> BTreeMap<String, Blob> {
    raw.split(|&b| b == 0)
        .filter_map(|entry| {
            let (meta, path) = str::from_utf8(entry).ok()?.split_once('\t')?;
            let mut meta = meta.split(' ');
            let (mode, kind, id) = (meta.next()?, meta.next()?, meta.next()?);
            let regular = kind == "blob" && matches!(mode, "100644" | "100755");
            Some((
                path.to_owned(),
                Blob {
                    id: id.to_owned(),
                    regular,
                },
            ))
        })
        .collect()
}

/// What to ask the remote for: a full ref name, or the commit id itself.
fn wanted(at: &GitRef) -> String {
    match at {
        GitRef::Head => "HEAD".to_owned(),
        GitRef::Branch(branch) => format!("refs/heads/{branch}"),
        GitRef::Tag(tag) => format!("refs/tags/{tag}"),
        GitRef::Rev(rev) => rev.to_string(),
    }
}

/// The commit `at` names on `remote` now.
fn resolve(remote: Remote<'_>, at: &GitRef) -> Result<Oid> {
    let reference = match at {
        GitRef::Rev(rev) => return Ok(rev.clone()),
        GitRef::Head => "a default branch".to_owned(),
        GitRef::Branch(branch) => format!("branch {branch}"),
        GitRef::Tag(tag) => format!("tag {tag}"),
    };
    let name = wanted(at);
    let peeled = format!("{name}^{{}}");
    let listing = git(
        None,
        &["ls-remote", "--", remote.url, &name, &peeled],
        Some(remote),
    )?;
    let listing = String::from_utf8_lossy(&listing);
    let mut found = None;
    for (id, listed) in listing.lines().filter_map(|line| line.split_once('\t')) {
        if listed == peeled {
            found = Some(id);
            break;
        }
        if listed == name {
            found = Some(id);
        }
    }
    let id = found.ok_or_else(|| SourceError::NoRef {
        url: remote.shown.to_owned(),
        reference,
    })?;
    Oid::try_from(id.to_owned()).map_err(Error::from)
}

/// Whether `repo` has commit `rev`.
fn has(repo: &Utf8Path, rev: &Oid) -> Result<bool> {
    let object = format!("{rev}^{{commit}}");
    let status = command(Some(repo), &["cat-file", "-e", &object], false)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(spawn_error)?;
    Ok(status.success())
}

/// Fetches `rev` into `repo`: by id, or through `at`'s ref from a server that refuses ids.
///
/// Kept under `refs/devset/`, so git's garbage collection leaves it for offline runs.
fn fetch(repo: &Utf8Path, remote: Remote<'_>, at: &GitRef, rev: &Oid) -> Result<()> {
    let args = [
        "fetch",
        "-q",
        "--depth",
        "1",
        "--no-tags",
        "--",
        remote.url,
        rev.as_str(),
    ];
    if let Err(refused) = git(Some(repo), &args, Some(remote)) {
        if let (GitRef::Rev(_), Error::Source(SourceError::Git { stderr, .. })) = (at, &refused)
            && NO_COMMIT.iter().any(|said| stderr.contains(said))
        {
            return Err(SourceError::NoCommit {
                url: remote.shown.to_owned(),
                rev: rev.clone(),
            }
            .into());
        }
        if matches!(at, GitRef::Rev(_)) {
            return Err(refused);
        }
        git(
            Some(repo),
            &["fetch", "-q", "--no-tags", "--", remote.url, &wanted(at)],
            Some(remote),
        )?;
        if !has(repo, rev)? {
            return Err(refused);
        }
    }
    let keep = format!("refs/devset/{rev}");
    git(Some(repo), &["update-ref", &keep, rev.as_str()], None)?;
    Ok(())
}

/// A `git` invocation, in `repo` if given; unless `prompts`, git may not ask for credentials.
fn command(repo: Option<&Utf8Path>, args: &[&str], prompts: bool) -> Command {
    let mut command = Command::new("git");
    if let Some(repo) = repo {
        command.arg("-C").arg(repo);
    }
    if !prompts {
        command
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never");
    }
    command.args(args);
    command
}

/// Runs `git`, returning its output; a failure reaching `remote` says which remote.
fn git(repo: Option<&Utf8Path>, args: &[&str], remote: Option<Remote<'_>>) -> Result<Vec<u8>> {
    let prompts = remote.is_some_and(|remote| remote.prompts);
    let output = command(repo, args, prompts)
        .stdin(Stdio::null())
        .output()
        .map_err(spawn_error)?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr: Vec<&str> = stderr
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches("fatal: ")
                .trim_start_matches("error: ")
        })
        .filter(|line| !line.is_empty())
        .collect();
    let subcommand = args.first().copied().unwrap_or_default();
    let Some(remote) = remote else {
        return Err(SourceError::Git {
            command: subcommand.to_owned(),
            stderr: stderr.join("\n"),
        }
        .into());
    };
    if stderr
        .iter()
        .any(|line| NO_CREDENTIALS.iter().any(|said| line.contains(said)))
    {
        return Err(SourceError::Credentials {
            url: remote.shown.to_owned(),
        }
        .into());
    }
    let command = format!("{subcommand} {}", remote.shown);
    let stderr = stderr.join("\n").replace(remote.url, remote.shown);
    Err(SourceError::Git { command, stderr }.into())
}

/// A failure to start `git`.
fn spawn_error(error: io::Error) -> Error {
    if error.kind() == io::ErrorKind::NotFound {
        SourceError::NoGit.into()
    } else {
        error.into()
    }
}
