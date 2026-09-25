//! Three-way merges, built in or through a configured [`Driver`].

use core::str::FromStr;
use std::io;
use std::process::{Command, Stdio};

use camino::Utf8Path;
use derive_more::Display;
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::digest::is_binary;
use crate::errors::{MergeError, Result};
use crate::path::RelPath;

/// Lines that open, separate or close a conflict hunk.
///
/// `=======` is left out: it is also a Markdown heading underline.
const MARKERS: [&[u8]; 3] = [b"<<<<<<<", b"|||||||", b">>>>>>>"];

/// How three-way merges are performed; written as its `driver` line, checked as it is read, so a
/// mistake points into the file that holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Display, SerializeDisplay, DeserializeFromStr)]
pub enum Driver {
    /// A line merge, `diffy`'s.
    #[default]
    #[display("builtin")]
    Builtin,
    /// A program, on git's merge-driver convention.
    ///
    /// `%O` is the ancestor, `%A` the local version and where the result is left, `%B` the
    /// incoming version, `%P` the path; `%%` is `%`. Exit zero means merged cleanly.
    #[display("{line}")]
    Command {
        /// As configured.
        line: String,
        /// `line` split into arguments, placeholders intact.
        args: Vec<String>,
    },
}

/// A merge's result.
#[derive(Debug)]
pub(crate) struct Merged {
    /// The merged bytes, with conflict markers if not clean.
    pub bytes: Vec<u8>,
    /// Why the merge did not succeed; `None` when clean.
    pub conflict: Option<String>,
}

/// The driver a `driver` line names, split as a POSIX shell would; no shell ever runs it.
///
/// Fails with [`MergeError::Driver`] when quotes are unbalanced, a placeholder is unknown, or
/// `%A` is missing.
impl FromStr for Driver {
    type Err = MergeError;

    fn from_str(line: &str) -> Result<Self, MergeError> {
        let invalid = |reason| MergeError::Driver { line: line.to_owned(), reason };
        let args = shlex::split(line).ok_or_else(|| invalid("unbalanced quotes"))?;
        if args.is_empty() {
            return Err(invalid("empty"));
        }
        for arg in &args {
            let mut chars = arg.chars();
            while let Some(c) = chars.next() {
                if c == '%' && !matches!(chars.next(), Some('O' | 'A' | 'B' | 'P' | '%')) {
                    return Err(invalid("placeholders are %O, %A, %B, %P and %%"));
                }
            }
        }
        if !args.iter().any(|arg| arg.contains("%A")) {
            return Err(invalid("must name %A, where the merged result is left"));
        }
        Ok(Self::Command { line: line.to_owned(), args })
    }
}

impl Driver {
    /// Merges `ours` and `theirs` from `base` for `path`, running a command in `root`.
    pub(crate) fn merge(
        &self, root: &Utf8Path, path: &RelPath, base: &[u8], ours: &[u8], theirs: &[u8],
    ) -> Result<Merged> {
        let Self::Command { line, args } = self else {
            if [base, ours, theirs].into_iter().any(is_binary) {
                let conflict = Some("binary: the sidecar holds the profile's version".to_owned());
                return Ok(Merged { bytes: theirs.to_vec(), conflict });
            }
            return Ok(match diffy::merge_bytes(base, ours, theirs) {
                Ok(bytes) => Merged { bytes, conflict: None },
                Err(bytes) => Merged { bytes, conflict: Some("conflicting changes".to_owned()) },
            });
        };
        // Each version under its real file name, so drivers can detect the language.
        let scratch = camino_tempfile::tempdir()?;
        let [o, a, b] =
            ["O", "A", "B"].map(|side| scratch.path().join(side).join(path.file_name()));
        for (file, bytes) in [(&o, base), (&a, ours), (&b, theirs)] {
            if let Some(dir) = file.parent() {
                fs_err::create_dir_all(dir)?;
            }
            fs_err::write(file, bytes)?;
        }
        let expand =
            |arg: &String| expand(arg, [o.as_str(), a.as_str(), b.as_str(), path.as_str()]);
        let argv: Vec<String> = args.iter().map(expand).collect();
        let (program, rest) = argv
            .split_first()
            .ok_or_else(|| MergeError::Driver { line: line.clone(), reason: "empty" })?;
        let stopped = |reason| MergeError::Run { program: program.clone(), reason };
        let output = Command::new(program)
            .args(rest)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .output()
            .map_err(|e| {
                if e.kind() == io::ErrorKind::NotFound {
                    MergeError::NoDriver { program: program.clone() }
                } else {
                    stopped(e.to_string())
                }
            })?;
        let bytes = fs_err::read(&a)?;
        match output.status.code() {
            Some(0) => Ok(Merged { bytes, conflict: None }),
            Some(code) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let why = stderr.trim();
                let conflict = if why.is_empty() {
                    format!("`{program}` exited {code}")
                } else {
                    why.to_owned()
                };
                Ok(Merged { bytes, conflict: Some(conflict) })
            },
            None => Err(stopped("killed by a signal".to_owned()).into()),
        }
    }
}

/// `arg` with `%O %A %B %P` replaced by `values`, in that order, and `%%` by `%`.
fn expand(arg: &str, [o, a, b, p]: [&str; 4]) -> String {
    let mut out = String::with_capacity(arg.len());
    let mut chars = arg.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('O') => out.push_str(o),
            Some('A') => out.push_str(a),
            Some('B') => out.push_str(b),
            Some('P') => out.push_str(p),
            Some(other) => out.push(other),
            None => {},
        }
    }
    out
}

/// Whether `bytes` still hold a conflict hunk.
pub(crate) fn has_markers(bytes: &[u8]) -> bool {
    bytes.split(|&b| b == b'\n').any(|line| MARKERS.iter().any(|marker| line.starts_with(marker)))
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use super::{Driver, expand, has_markers};
    use crate::path::RelPath;

    #[test]
    fn parses_driver_lines() {
        let Ok(Driver::Command { args, .. }) = "mergiraf merge --git %O %A %B -p '%P'".parse()
        else {
            panic!("valid driver");
        };
        assert_eq!(
            args,
            ["mergiraf", "merge", "--git", "%O", "%A", "%B", "-p", "%P"],
            "split without a shell"
        );
        for bad in ["", "tool 'open", "tool %O %B", "tool %A %Q"] {
            assert!(bad.parse::<Driver>().is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn expands_placeholders() {
        assert_eq!(expand("--base=%O", ["o", "a", "b", "p"]), "--base=o", "inside an argument");
        assert_eq!(expand("100%%", ["o", "a", "b", "p"]), "100%", "escaped percent");
    }

    #[test]
    fn builtin_merges_and_conflicts() {
        let merge = |base: &str, ours: &str, theirs: &str| {
            let path = RelPath::new("f.txt").unwrap();
            Driver::Builtin
                .merge(
                    Utf8Path::new("."),
                    &path,
                    base.as_bytes(),
                    ours.as_bytes(),
                    theirs.as_bytes(),
                )
                .unwrap()
        };
        let clean = merge("a\nb\nc\n", "A\nb\nc\n", "a\nb\nC\n");
        assert_eq!(
            (clean.bytes.as_slice(), clean.conflict),
            (&b"A\nb\nC\n"[..], None),
            "disjoint edits merge"
        );
        let conflict = merge("a\n", "x\n", "y\n");
        assert!(
            conflict.conflict.is_some() && has_markers(&conflict.bytes),
            "overlapping edits conflict"
        );
    }

    #[test]
    fn commands_follow_git_convention() {
        let path = RelPath::new("f.txt").unwrap();
        let run = |line: &str| {
            line.parse::<Driver>()
                .unwrap()
                .merge(Utf8Path::new("."), &path, b"o\n", b"a\n", b"b\n")
                .unwrap()
        };
        let theirs = run("cp %B %A");
        assert_eq!(
            (theirs.bytes.as_slice(), theirs.conflict),
            (&b"b\n"[..], None),
            "exit 0: result read from %A"
        );
        assert!(run("false %A").conflict.is_some(), "non-zero exit is a conflict");
    }

    #[test]
    fn detects_markers() {
        assert!(has_markers(b"a\n<<<<<<< ours\nb\n"), "opening marker");
        assert!(!has_markers(b"Title\n=======\n"), "a Markdown underline is not a marker");
    }
}
