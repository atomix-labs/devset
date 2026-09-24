//! Blocks a profile owns in a line-based file: the lines between two comment markers.

use core::ops::Range;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::path::RelPath;

/// A line comment syntax, which a block's markers are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Comment {
    /// `#`: shells, TOML, YAML, Python, `.gitignore`.
    #[serde(rename = "#")]
    Hash,
    /// `//`: C, Rust, JavaScript, JSONC.
    #[serde(rename = "//")]
    Slashes,
    /// `--`: SQL, Lua, Haskell.
    #[serde(rename = "--")]
    Dashes,
    /// `;`: INI, Lisp, assembly.
    #[serde(rename = ";")]
    Semicolon,
    /// `%`: TeX, Erlang, MATLAB.
    #[serde(rename = "%")]
    Percent,
    /// `/* */`: CSS.
    #[serde(rename = "/*")]
    Star,
    /// `<!-- -->`: Markdown, HTML, XML.
    #[serde(rename = "<!--")]
    Angle,
}

impl Comment {
    /// What opens the comment, and what closes it on the same line.
    const fn delimiters(self) -> (&'static str, &'static str) {
        match self {
            Self::Hash => ("#", ""),
            Self::Slashes => ("//", ""),
            Self::Dashes => ("--", ""),
            Self::Semicolon => (";", ""),
            Self::Percent => ("%", ""),
            Self::Star => ("/*", " */"),
            Self::Angle => ("<!--", " -->"),
        }
    }
}

/// The comment syntax a file type writes markers in, when devset knows it.
pub(crate) fn comment(path: &RelPath) -> Option<Comment> {
    let by_name = match path.file_name() {
        ".gitignore" | ".gitattributes" | ".dockerignore" | ".editorconfig" | "CODEOWNERS"
        | "Makefile" | "justfile" | "Justfile" | "Dockerfile" | ".env" => Some(Comment::Hash),
        _ => None,
    };
    by_name.or_else(|| {
        Some(match path.extension()?.as_str() {
            "toml" | "yml" | "yaml" | "sh" | "bash" | "zsh" | "fish" | "py" | "rb" | "conf"
            | "cfg" | "ini" | "properties" | "tf" | "just" | "env" | "txt" | "gitignore" => {
                Comment::Hash
            }
            "jsonc" | "js" | "mjs" | "cjs" | "ts" | "jsx" | "tsx" | "rs" | "go" | "c" | "h"
            | "cc" | "cpp" | "hpp" | "java" | "kt" | "swift" | "scala" | "dart" | "zig"
            | "proto" => Comment::Slashes,
            "md" | "markdown" | "html" | "htm" | "xml" | "svg" | "vue" => Comment::Angle,
            "sql" | "lua" | "hs" => Comment::Dashes,
            "el" | "lisp" | "clj" | "asm" => Comment::Semicolon,
            "tex" | "erl" | "m" => Comment::Percent,
            "css" | "scss" | "less" => Comment::Star,
            _ => return None,
        })
    })
}

/// Where one block lies in a text.
struct Located {
    /// The block, its marker lines included.
    whole: Range<usize>,
    /// Its lines, marker lines excluded.
    lines: Range<usize>,
}

/// The lines that open and close one profile's block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Markers {
    /// The line that opens the block.
    open: String,
    /// The line that closes it.
    close: String,
}

impl Markers {
    /// The markers of `owner`'s block, in `comment` syntax.
    pub(crate) fn new(comment: Comment, owner: &str) -> Self {
        let (open, close) = comment.delimiters();
        Self {
            open: format!("{open} >>> devset: {owner} >>>{close}"),
            close: format!("{open} <<< devset: {owner} <<<{close}"),
        }
    }

    /// Where the block's lines lie in `text`, marker lines excluded; `None` when there is no
    /// block. `Err` holds the offending line's range and why it breaks the block.
    pub(crate) fn find(&self, text: &str) -> Result<Option<Range<usize>>, (Range<usize>, String)> {
        Ok(self.locate(text)?.map(|found| found.lines))
    }

    /// Where the block lies in `text`; `None` when there is no block.
    fn locate(&self, text: &str) -> Result<Option<Located>, (Range<usize>, String)> {
        let (mut start, mut end) = (None::<Range<usize>>, None::<(Range<usize>, usize)>);
        let mut at: usize = 0;
        for line in text.split_inclusive('\n') {
            let span = at..at.saturating_add(line.len());
            at = span.end;
            let trimmed = line.trim();
            if trimmed == self.open {
                if start.is_some() {
                    return Err((span, "the block opens twice".to_owned()));
                }
                start = Some(span);
            } else if trimmed == self.close {
                let Some(open) = &start else {
                    return Err((span, "the block closes before it opens".to_owned()));
                };
                if end.is_some() {
                    return Err((span, "the block closes twice".to_owned()));
                }
                end = Some((open.end..span.start, span.end));
            }
        }
        match (start, end) {
            (None, _) => Ok(None),
            (Some(open), None) => Err((open, "the block has no closing marker".to_owned())),
            (Some(open), Some((lines, close))) => Ok(Some(Located {
                whole: open.start..close,
                lines,
            })),
        }
    }

    /// `text` without the block, its markers included, and without the blank line
    /// [`splice`](Self::splice) set before it.
    pub(crate) fn remove(&self, text: &str) -> Result<String, (Range<usize>, String)> {
        let Some(Located { whole, .. }) = self.locate(text)? else {
            return Ok(text.to_owned());
        };
        let (before, after) = (
            text.get(..whole.start).unwrap_or_default(),
            text.get(whole.end..).unwrap_or_default(),
        );
        if after.trim().is_empty() {
            let kept = before.trim_end();
            let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
            return Ok(if kept.is_empty() {
                String::new()
            } else {
                format!("{kept}{newline}")
            });
        }
        let blank = |s: &str| s.ends_with("\n\n") || s.ends_with("\r\n\r\n");
        let after = if blank(before) {
            after
                .strip_prefix("\r\n")
                .or_else(|| after.strip_prefix('\n'))
                .unwrap_or(after)
        } else {
            after
        };
        Ok(format!("{before}{after}"))
    }

    /// `text` with the block holding `content`: in place, or appended after a blank line; in
    /// the file's line endings.
    pub(crate) fn splice(
        &self,
        text: &str,
        content: &str,
    ) -> Result<String, (Range<usize>, String)> {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut body = content.replace("\r\n", "\n");
        if newline == "\r\n" {
            body = body.replace('\n', newline);
        }
        if !body.is_empty() && !body.ends_with('\n') {
            body.push_str(newline);
        }
        if let Some(lines) = self.find(text)? {
            let (before, after) = (text.get(..lines.start), text.get(lines.end..));
            return Ok(format!(
                "{}{body}{}",
                before.unwrap_or_default(),
                after.unwrap_or_default()
            ));
        }
        let mut out = text.to_owned();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(newline);
        }
        if !out.trim().is_empty() {
            out.push_str(newline);
        }
        out.extend([
            self.open.as_str(),
            newline,
            &body,
            self.close.as_str(),
            newline,
        ]);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::{Comment, Markers, comment};
    use crate::path::RelPath;

    #[test]
    fn blocks_are_found_written_and_moved() {
        let markers = Markers::new(Comment::Hash, "rust");
        let first = markers
            .splice("mine\n", "/target\n")
            .expect("an empty file takes a block");
        assert_eq!(
            first, "mine\n\n# >>> devset: rust >>>\n/target\n# <<< devset: rust <<<\n",
            "appended after a blank line"
        );
        let moved = "# >>> devset: rust >>>\n/target\n# <<< devset: rust <<<\nmine\n";
        let updated = markers
            .splice(moved, "/target\n/out\n")
            .expect("a block in place");
        assert_eq!(
            updated, "# >>> devset: rust >>>\n/target\n/out\n# <<< devset: rust <<<\nmine\n",
            "a moved block is updated where it is"
        );
        let open_only = "# >>> devset: rust >>>\n/target\n";
        assert!(
            markers.find(open_only).is_err(),
            "a lone marker is an error, never a second block"
        );
        assert_eq!(markers.find("mine\n"), Ok(None), "no block, no error");
    }

    #[test]
    fn comment_syntax_follows_the_file_type() {
        let of = |path: &str| comment(&RelPath::new(path).expect("a valid path"));
        assert_eq!(of(".gitignore"), Some(Comment::Hash), "by name");
        assert_eq!(of("docs/README.md"), Some(Comment::Angle), "by extension");
        assert_eq!(
            of(".vscode/settings.jsonc"),
            Some(Comment::Slashes),
            "JSONC has comments"
        );
        assert_eq!(of("package.json"), None, "strict JSON has none");
        let html = Markers::new(Comment::Angle, "badges");
        let text = html
            .splice("", "[badge]\n")
            .expect("an empty file takes a block");
        assert!(
            text.starts_with("<!-- >>> devset: badges >>> -->\n"),
            "HTML comments close"
        );
    }
}
