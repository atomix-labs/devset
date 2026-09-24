//! Post-merge validation of structured files.

use core::ops::Range;
use core::str;
use std::collections::HashSet;

use jsonc_parser::ast::Value;
use jsonc_parser::{CollectOptions, ParseOptions, parse_to_ast};
use schemars::JsonSchema;
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};

use crate::digest::BOM;
use crate::path::RelPath;

/// How a file is checked after a merge: it must parse, with no duplicate keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// TOML.
    Toml,
    /// JSON with comments and trailing commas (JSONC), a superset of JSON.
    Json,
    /// YAML.
    Yaml,
    /// Not checked.
    None,
}

impl Format {
    /// The format `path`'s extension implies.
    #[must_use]
    pub fn of(path: &RelPath) -> Self {
        let name = path.file_name();
        // Lockfiles that are TOML whatever their extension says.
        if matches!(name, "Cargo.lock" | "mise.lock") || name.ends_with(".mise.lock") {
            return Self::Toml;
        }
        match path.extension().as_deref() {
            Some("toml") => Self::Toml,
            Some("json" | "jsonc") => Self::Json,
            Some("yaml" | "yml") => Self::Yaml,
            _ => Self::None,
        }
    }

    /// Checks that `bytes` parse, with no duplicate key at any depth.
    ///
    /// A UTF-8 BOM is allowed, and YAML may hold several documents.
    ///
    /// # Errors
    /// Why they do not: the parser's message, or the first duplicate key, located by line.
    pub fn check(self, bytes: &[u8]) -> Result<(), String> {
        let bytes = bytes.strip_prefix(BOM).unwrap_or(bytes);
        let text = || str::from_utf8(bytes).map_err(|e| e.to_string());
        match self {
            Self::None => Ok(()),
            Self::Toml => {
                let text = text()?;
                toml::from_str::<IgnoredAny>(text).map(drop).map_err(|e| {
                    let at = |span: Range<usize>| {
                        format!("{} at line {}", e.message(), line_at(text, span.start))
                    };
                    e.span().map_or_else(|| e.message().to_owned(), at)
                })
            }
            Self::Yaml => serde_saphyr::from_multiple::<IgnoredAny>(text()?)
                .map(drop)
                .map_err(|e| yaml(&e)),
            Self::Json => {
                let text = text()?;
                let parsed = parse_to_ast(text, &CollectOptions::default(), &jsonc())
                    .map_err(|e| e.to_string())?;
                parsed
                    .value
                    .as_ref()
                    .map_or(Ok(()), |value| unique_keys(value, text))
            }
        }
    }
}

/// JSON as devset reads it: comments and trailing commas allowed, as in JSONC, and nothing looser.
pub(crate) const fn jsonc() -> ParseOptions {
    ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
    }
}

/// A YAML error in the words the other formats use.
fn yaml(error: &serde_saphyr::Error) -> String {
    let error = error.without_snippet();
    if let serde_saphyr::Error::DuplicateMappingKey { key, location } = error {
        let key = key
            .as_deref()
            .map_or_else(String::new, |key| format!(" `{key}`"));
        return format!("duplicate key{key} at line {}", location.line());
    }
    let message = error.to_string();
    message
        .strip_prefix("error: ")
        .unwrap_or(&message)
        .to_owned()
}

/// The 1-based line of byte `offset` in `text`.
fn line_at(text: &str, offset: usize) -> usize {
    text.get(..offset)
        .unwrap_or(text)
        .matches('\n')
        .count()
        .saturating_add(1)
}

/// The first duplicate key in `value`, parsed from `text`, at any depth.
fn unique_keys(value: &Value<'_>, text: &str) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            let mut seen = HashSet::with_capacity(object.properties.len());
            for property in &object.properties {
                let key = property.name.as_str();
                if !seen.insert(key) {
                    let line = line_at(text, property.range.start);
                    return Err(format!("duplicate key `{key}` at line {line}"));
                }
                unique_keys(&property.value, text)?;
            }
            Ok(())
        }
        Value::Array(array) => array
            .elements
            .iter()
            .try_for_each(|element| unique_keys(element, text)),
        Value::StringLit(_)
        | Value::NumberLit(_)
        | Value::BooleanLit(_)
        | Value::NullKeyword(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::Format;
    use crate::path::RelPath;

    #[test]
    fn inferred_from_extension() {
        for (path, want) in [
            ("deny.toml", Format::Toml),
            (".vscode/settings.JSON", Format::Json),
            ("ci.yml", Format::Yaml),
            ("justfile", Format::None),
        ] {
            assert_eq!(Format::of(&RelPath::new(path).unwrap()), want, "{path}");
        }
    }

    #[test]
    fn real_files_are_valid() {
        for (format, bytes) in [
            (Format::Yaml, &b"a: 1\n---\nb: 2\n"[..]),
            (Format::Yaml, b"---\na: 1\n...\n"),
            (Format::Toml, b"\xEF\xBB\xBFa = 1\n"),
            (Format::Json, b"\xEF\xBB\xBF{\"a\": 1}"),
            (Format::Yaml, b"\xEF\xBB\xBFa: 1\n"),
            (Format::Toml, b""),
            (Format::Yaml, b""),
            (Format::Json, b""),
        ] {
            assert_eq!(
                format.check(bytes),
                Ok(()),
                "{format:?} {:?}",
                String::from_utf8_lossy(bytes)
            );
        }
        assert!(
            Format::Yaml.check(b"a: 1\n---\na: 1\na: 2\n").is_err(),
            "each document is checked"
        );
    }

    #[test]
    fn duplicate_keys_are_invalid() {
        for (format, valid, duplicated) in [
            (Format::Toml, "a = 1\n[t]\nb = 2\n", "a = 1\nb = 2\na = 3\n"),
            (
                Format::Json,
                "{\n  // why\n  \"a\": 1,\n  \"o\": {\"b\": [1, {\"c\": 2}]},\n}",
                r#"{"o": {"b": 1, "b": 2}}"#,
            ),
            (Format::Json, "[1, 2]", r#"[{"a": 1, "a": 2}]"#),
            (
                Format::Yaml,
                "a: 1\nt:\n  b: 2\n",
                "name: app\ntimeout: 30\nport: 80\ntimeout: 60\n",
            ),
        ] {
            assert_eq!(
                format.check(valid.as_bytes()),
                Ok(()),
                "{format:?} valid: {valid:?}"
            );
            assert!(
                format.check(duplicated.as_bytes()).is_err(),
                "{format:?} duplicate: {duplicated:?}"
            );
        }
        assert!(
            Format::Json.check(b"{'single': 1}").is_err(),
            "only comments and trailing commas are loosened"
        );
        assert_eq!(
            Format::None.check(b"\0 anything"),
            Ok(()),
            "none checks nothing"
        );
        let located = [
            (
                Format::Json,
                &b"{\"a\": 1,\n \"a\": 2}"[..],
                "duplicate key `a` at line 2",
            ),
            (Format::Toml, b"a = 1\n\na = 2\n", "duplicate key at line 3"),
            (Format::Yaml, b"a: 1\na: 2\n", "duplicate key `a` at line 2"),
        ];
        for (format, bytes, want) in located {
            assert_eq!(
                format.check(bytes),
                Err(want.into()),
                "{format:?} names the line"
            );
        }
    }
}
