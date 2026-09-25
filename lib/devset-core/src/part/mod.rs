//! Parts of a file: the keys, or the marked block, one profile owns in a file the target otherwise
//! owns.
//!
//! A part is read out of its file as *content*, bytes the rest of devset treats as a file's: for
//! keys, one line per owned leaf with its value as canonical JSON, so layout is never drift; for a
//! block, its lines as written.

mod block;
mod json;
mod keys;
mod toml;
mod yaml;

use alloc::collections::{BTreeMap, BTreeSet};
use core::ops::Range;
use core::str;

use derive_more::Display;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use self::block::Comment;
use self::block::Markers;
pub(crate) use self::keys::{Key, Leaves, Written, decode, display, encode, merge, overlap};
use self::keys::{lookup, walk};
use crate::digest::BOM;
use crate::errors::{ParseError, ProfileError, Result};
use crate::format::Format;
use crate::path::RelPath;

/// Where a managed thing lives: a whole file, or one profile's part of it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Slot {
    /// The file.
    pub path: RelPath,
    /// The profile whose part it is; `None` for the whole file.
    pub part: Option<String>,
}

impl Slot {
    /// The whole of `path`.
    pub(crate) const fn whole(path: RelPath) -> Self {
        Self { path, part: None }
    }
}

/// How much of a file a profile owns.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    Display,
)]
#[serde(rename_all = "lowercase")]
#[display(rename_all = "lowercase")]
pub enum Scope {
    /// The whole file.
    #[default]
    File,
    /// Every leaf the payload, a partial document, defines; the file's other keys are the target's.
    Keys,
    /// One comment-marked block, named after the profile; the file's other lines are the target's.
    Block,
}

/// A structured format whose keys a profile can own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Syntax {
    /// TOML, through `toml_edit`.
    Toml,
    /// JSON and JSONC, through `jsonc-parser`.
    Json,
    /// YAML, through `yaml-edit`; experimental.
    Yaml,
}

/// Why a document could not be read or written, and where.
type Failure = (Option<Range<usize>>, String);

/// A document's reading or writing: done, or the failure.
type Parsed<T> = Result<T, Failure>;

/// One key to set to a value, or to remove.
type Edit<'a> = (&'a Key, Option<&'a Value>);

impl Syntax {
    /// The syntax of `format`, if it is a structured one.
    const fn of(format: Format) -> Option<Self> {
        match format {
            Format::Toml => Some(Self::Toml),
            Format::Json => Some(Self::Json),
            Format::Yaml => Some(Self::Yaml),
            Format::None => None,
        }
    }

    /// The document's values, which must be a table; an empty document is an empty one.
    fn semantic(self, text: &str) -> Parsed<Value> {
        match self {
            Self::Toml => toml::semantic(text),
            Self::Json => json::semantic(text),
            Self::Yaml => yaml::semantic(text),
        }
    }

    /// The leaves a partial document defines, in its order.
    fn leaves(self, text: &str) -> Parsed<Written> {
        match self {
            // TOML's syntax says more than its values: which arrays are arrays of tables.
            Self::Toml => toml::leaves(text),
            Self::Json | Self::Yaml => {
                let mut leaves = Vec::new();
                walk(&self.semantic(text)?, &mut Vec::new(), &mut leaves);
                Ok(Written { leaves, source: text.to_owned(), ..Written::default() })
            },
        }
    }

    /// A document's values at `keys`; a key it lacks is left out.
    fn values(self, text: &str, keys: &BTreeSet<Key>) -> Parsed<Leaves> {
        let values = self.semantic(text)?;
        Ok(keys
            .iter()
            .filter_map(|key| Some((key.clone(), lookup(&values, key)?.clone())))
            .collect())
    }

    /// A document with each edit made, in order, as `payload` writes each: TOML and YAML keep the
    /// layout of a value the payload gives, and TOML its arrays of tables.
    fn apply(self, text: &str, edits: &[Edit<'_>], payload: &Written) -> Parsed<String> {
        match self {
            Self::Toml => toml::apply(text, edits, payload),
            Self::Json => json::apply(text, edits, payload),
            Self::Yaml => yaml::apply(text, edits, payload),
        }
    }
}

/// How one profile's part of a file is found, read and written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    /// The leaves of a partial document.
    Keys(Syntax),
    /// The lines between two markers.
    Block(Markers),
}

impl Shape {
    /// The shape of `owner`'s part of `path` under `scope`, read as `format`; `None` for the
    /// whole file.
    ///
    /// # Errors
    /// - [`ProfileError::NoKeys`], `format` is not TOML, JSON or YAML.
    /// - [`ProfileError::NoComment`], devset knows no comment syntax for `path` and none is given.
    pub(crate) fn of(
        scope: Scope, path: &RelPath, format: Format, comment: Option<Comment>, owner: &str,
    ) -> Result<Option<Self>> {
        match scope {
            Scope::File => Ok(None),
            Scope::Keys => Syntax::of(format)
                .map(|syntax| Some(Self::Keys(syntax)))
                .ok_or_else(|| ProfileError::NoKeys { path: path.clone() }.into()),
            Scope::Block => {
                let comment = comment
                    .or_else(|| block::comment(path))
                    .ok_or_else(|| ProfileError::NoComment { path: path.clone() })?;
                Ok(Some(Self::Block(Markers::new(comment, owner))))
            },
        }
    }

    /// What `payload` gives the part: its content, a partial document's leaves or a block's
    /// lines; and for keys, the leaves as the payload writes them.
    pub(crate) fn read(&self, payload: &[u8], label: &str) -> Result<(Vec<u8>, Written)> {
        match self {
            Self::Keys(syntax) => {
                let (text, _) = text(payload, label)?;
                let written = syntax.leaves(text).map_err(|f| failed(label, text, f))?;
                let leaves: Leaves = written.leaves.iter().cloned().collect();
                Ok((encode(&leaves), written))
            },
            Self::Block(_) => Ok((payload.to_vec(), Written::default())),
        }
    }

    /// The keys `content` owns; none for a block.
    pub(crate) fn keys(&self, content: &[u8]) -> BTreeSet<Key> {
        match self {
            Self::Keys(_) => decode(content).map_or_default(|leaves| leaves.into_keys().collect()),
            Self::Block(_) => BTreeSet::new(),
        }
    }

    /// What `file` holds of the part over `keys`, as content; `None` when none of it is there.
    pub(crate) fn view(
        &self, file: &[u8], keys: &BTreeSet<Key>, label: &str,
    ) -> Result<Option<Vec<u8>>> {
        let (text, _) = text(file, label)?;
        match self {
            Self::Keys(syntax) => {
                let found = syntax.values(text, keys).map_err(|f| failed(label, text, f))?;
                // A part of no keys is all there whenever its file is.
                Ok((!found.is_empty() || keys.is_empty()).then(|| encode(&found)))
            },
            Self::Block(markers) => {
                let lines = markers
                    .find(text)
                    .map_err(|(span, why)| failed(label, text, (Some(span), why)))?;
                Ok(lines.and_then(|lines| text.get(lines)).map(|lines| lines.as_bytes().to_vec()))
            },
        }
    }

    /// `file` with the part holding `content` over `keys`; a key `content` lacks is removed.
    ///
    /// Only what changes is rewritten, so the rest keeps its layout; a new key goes where
    /// `payload`, the profile's leaves as written, puts it, and a value the profile wrote is
    /// written as it wrote it.
    pub(crate) fn splice(
        &self, file: &[u8], content: &[u8], keys: &BTreeSet<Key>, payload: &Written, label: &str,
    ) -> Result<Vec<u8>> {
        let (text, bom) = text(file, label)?;
        let spliced = match self {
            Self::Keys(syntax) => {
                let target = decode(content).unwrap_or_default();
                let current = syntax.values(text, keys).map_err(|f| failed(label, text, f))?;
                let written: BTreeMap<&Key, &Value> =
                    payload.leaves.iter().map(|(k, v)| (k, v)).collect();
                let order = payload
                    .leaves
                    .iter()
                    .map(|(key, _)| key)
                    .filter(|key| keys.contains(*key))
                    .chain(keys.iter().filter(|key| !written.contains_key(key)));
                let edits: Vec<Edit<'_>> = order
                    .filter(|key| current.get(*key) != target.get(*key))
                    .map(|key| {
                        let value = target.get(key);
                        let styled = written.get(key).copied().filter(|w| Some(*w) == value);
                        (key, styled.or(value))
                    })
                    .collect();
                if edits.is_empty() {
                    text.to_owned()
                } else {
                    syntax.apply(text, &edits, payload).map_err(|f| failed(label, text, f))?
                }
            },
            Self::Block(markers) => {
                let (content, _) = self::text(content, label)?;
                markers
                    .splice(text, content)
                    .map_err(|(span, why)| failed(label, text, (Some(span), why)))?
            },
        };
        let mut out = if bom { BOM.to_vec() } else { Vec::new() };
        out.extend_from_slice(spliced.as_bytes());
        Ok(out)
    }

    /// `file` without the part: its keys over `keys`, or its block, markers included.
    pub(crate) fn remove(&self, file: &[u8], keys: &BTreeSet<Key>, label: &str) -> Result<Vec<u8>> {
        match self {
            Self::Keys(_) => {
                self.splice(file, &encode(&Leaves::new()), keys, &Written::default(), label)
            },
            Self::Block(markers) => {
                let (text, bom) = text(file, label)?;
                let removed = markers
                    .remove(text)
                    .map_err(|(span, why)| failed(label, text, (Some(span), why)))?;
                let mut out = if bom && !removed.is_empty() { BOM.to_vec() } else { Vec::new() };
                out.extend_from_slice(removed.as_bytes());
                Ok(out)
            },
        }
    }
}

/// `bytes` as text, a leading BOM set aside, and whether there was one.
fn text<'a>(bytes: &'a [u8], label: &str) -> Result<(&'a str, bool)> {
    let bom = bytes.starts_with(BOM);
    let bytes = bytes.strip_prefix(BOM).unwrap_or(bytes);
    let text = str::from_utf8(bytes).map_err(|e| ParseError {
        file: label.to_owned(),
        text: String::new(),
        span: None,
        message: e.to_string(),
    })?;
    Ok((text, bom))
}

/// `failure` in `text` of `label`, as a parse error that points at it.
fn failed(label: &str, text: &str, (span, message): Failure) -> crate::Error {
    ParseError { file: label.to_owned(), text: text.to_owned(), span, message }.into()
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeSet;

    use super::{Scope, Shape, Syntax};
    use crate::format::Format;
    use crate::path::RelPath;

    /// The keys shape of `path`.
    fn keys(path: &str) -> Shape {
        let path = RelPath::new(path).expect("a valid path");
        let format = Format::of(&path);
        Shape::of(Scope::Keys, &path, format, None, "p")
            .expect("a structured file")
            .expect("a part")
    }

    /// Splices `payload`'s keys into `file` as `shape` does, returning the file.
    fn splice(shape: &Shape, file: &str, payload: &str) -> String {
        let (content, written) = shape.read(payload.as_bytes(), "payload").expect("a payload");
        let owned: BTreeSet<_> = shape.keys(&content);
        let out =
            shape.splice(file.as_bytes(), &content, &owned, &written, "file").expect("splices");
        String::from_utf8(out).expect("UTF-8 out")
    }

    #[test]
    fn toml_keys_join_the_target_tables() {
        let shape = keys("Cargo.toml");
        let file = "[workspace]\nmembers = [\"lib/*\"]\n\n[workspace.lints.clippy]\n# why\nunwrap_used = \"warn\" # ours\nlocal = \"allow\"\n";
        let payload = "[workspace.lints.clippy]\nunwrap_used = \"deny\"\npedantic = { level = \"deny\", priority = -1 }\n";
        let out = splice(&shape, file, payload);
        assert!(out.contains("members = [\"lib/*\"]"), "the target's own keys stay: {out}");
        assert!(out.contains("local = \"allow\""), "and its keys beside the shared ones");
        assert!(
            out.contains("unwrap_used = \"deny\" # ours"),
            "a replaced value keeps its comment"
        );
        assert!(out.contains("# why"), "and the comment above it: {out}");
        assert!(
            out.contains("pedantic = { level = \"deny\", priority = -1 }"),
            "a new leaf is added"
        );
    }

    #[test]
    fn toml_new_keys_keep_the_payload_layout() {
        let shape = keys("deny.toml");
        let payload = "[bans]\nmultiple-versions = \"deny\"\nwildcards         = \"deny\"\ndeny = [\n    { name = \"a\" },\n    { name = \"b\" },\n]\n";
        let out = splice(
            &shape,
            "[bans]\nwildcards = \"allow\"\n\n[graph]\nall-features = true\n",
            payload,
        );
        assert!(
            out.contains("multiple-versions = \"deny\"\n"),
            "a new key is aligned as the payload aligns it: {out}"
        );
        assert!(
            out.contains("wildcards = \"deny\"\n"),
            "a key the file holds keeps the file's layout: {out}"
        );
        assert!(
            out.contains("deny = [\n    { name = \"a\" },\n    { name = \"b\" },\n]"),
            "and a new value is laid out as the payload lays it out: {out}"
        );
        let out = splice(&shape, "[graph]\nall-features = true\n", payload);
        assert!(
            out.contains("[bans]\nmultiple-versions = \"deny\"\nwildcards         = \"deny\"\n"),
            "keys of a new table are aligned as the payload aligns them: {out}"
        );
    }

    #[test]
    fn toml_tables_are_created_and_pruned() {
        let shape = keys("Cargo.toml");
        let out = splice(
            &shape,
            "[package]\nname = \"x\"\n",
            "[workspace.lints.rust]\nunsafe_code = \"deny\"\n",
        );
        assert!(out.contains("[workspace.lints.rust]\nunsafe_code = \"deny\""), "created: {out}");
        let (content, written) = shape
            .read(
                b"[workspace.lints.rust]
unsafe_code = \"deny\"
",
                "p",
            )
            .expect("valid");
        let owned = shape.keys(&content);
        let emptied = shape.splice(out.as_bytes(), b"", &owned, &written, "file").expect("splices");
        let emptied = String::from_utf8(emptied).expect("UTF-8");
        assert_eq!(emptied.trim(), "[package]\nname = \"x\"", "a table the part leaves empty goes");
    }

    #[test]
    fn toml_arrays_of_objects_are_arrays_of_tables() {
        let shape = keys(".config/mise/mise.lock");
        let file = "[[tools.dprint]]\nversion = \"0.57.4\"\n\n[tools.dprint.\"platforms.linux-x64\"]\nchecksum = \"sha256:a\"\n";
        let payload = "[[tools.taplo]]\nversion = \"0.10.0\"\nbackend = \"aqua:tamasfe/taplo\"\n\n[tools.taplo.\"platforms.linux-x64\"]\nurl = \"https://example.com/taplo.gz\"\n";
        let out = splice(&shape, file, payload);
        assert!(out.starts_with(file), "the entry already there is untouched: {out}");
        assert!(out.contains("[[tools.taplo]]\nversion = \"0.10.0\""), "an array of tables: {out}");
        assert!(
            out.contains("[tools.taplo.\"platforms.linux-x64\"]\nurl = "),
            "with its sub-tables as tables: {out}"
        );
    }

    #[test]
    fn toml_arrays_keep_the_form_the_payload_writes() {
        let shape = keys("deny.toml");
        let out =
            splice(&shape, "[bans]\nwildcards = \"deny\"\n", "[bans]\ndeny = [{ name = \"a\" }]\n");
        assert!(
            out.contains("deny = [{ name = \"a\" }]"),
            "written inline, as the payload is: {out}"
        );
    }

    #[test]
    fn json_keys_keep_comments_and_commas() {
        let shape = keys(".vscode/settings.json");
        assert_eq!(shape, Shape::Keys(Syntax::Json), "JSON by extension");
        let file = "{\n  // ours\n  \"rust-analyzer.cargo.target\": \"aarch64\",\n  \"editor.formatOnSave\": false,\n}\n";
        let out = splice(
            &shape,
            file,
            "{ \"editor.formatOnSave\": true, \"[toml]\": { \"editor.tabSize\": 2 } }",
        );
        assert!(out.contains("// ours"), "comments survive: {out}");
        assert!(
            out.contains("\"rust-analyzer.cargo.target\": \"aarch64\""),
            "the target's keys stay"
        );
        assert!(out.contains("\"editor.formatOnSave\": true"), "an owned key is set");
        assert!(out.contains("\"editor.tabSize\": 2"), "a nested key is added");
    }

    #[test]
    fn yaml_keys_keep_comments() {
        let shape = keys(".github/dependabot.yml");
        let file =
            "# ours\nversion: 2\nupdates:\n  - package-ecosystem: cargo\n    directory: \"/\"\n";
        let out =
            splice(&shape, file, "version: 2\nregistries:\n  crates:\n    type: cargo-registry\n");
        assert!(out.contains("# ours"), "comments survive: {out}");
        assert!(out.contains("package-ecosystem: cargo"), "the target's keys stay");
        assert!(out.contains("type: cargo-registry"), "a nested key is added");
    }

    #[test]
    fn yaml_new_keys_keep_the_payload_layout() {
        let shape = keys(".yamllint.yaml");
        let payload = "extends: default\nignore-from-file: [.gitignore]\nrules:\n  truthy:\n    allowed-values: [\"true\", \"false\"]\n";
        let out = splice(&shape, "extends: default\n", payload);
        assert!(
            out.contains("ignore-from-file: [.gitignore]\n"),
            "a new sequence is written as the payload writes it: {out}"
        );
        assert!(
            out.contains("rules:\n  truthy:\n    allowed-values: [\"true\", \"false\"]\n"),
            "and a new value under mappings made for it, quotes and all: {out}"
        );
        let shape = keys(".github/dependabot.yml");
        let updates = "updates:\n  - package-ecosystem: cargo\n    directory: \"/\"\n";
        let out = splice(&shape, "version: 2\n", updates);
        assert!(out.ends_with(updates), "a sequence of mappings keeps its keys' order: {out}");
    }

    #[test]
    fn views_compare_values_not_layout() {
        let shape = keys("a.toml");
        let owned = shape.keys(&shape.read(b"a = 1\nb = \"x\"\n", "p").expect("valid").0);
        let one = shape.view(b"a = 1\nb = \"x\"\n", &owned, "f").expect("reads");
        let two = shape.view(b"b    =   'x'   # hi\na=1\nother = 2\n", &owned, "f").expect("reads");
        assert_eq!(one, two, "reformatting and other keys are not part of the view");
        assert_eq!(shape.view(b"other = 1\n", &owned, "f").expect("reads"), None, "absent part");
    }
}
