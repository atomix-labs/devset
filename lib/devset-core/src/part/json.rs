//! JSON and JSONC keys: read as values, written through `jsonc-parser`'s concrete syntax tree, so
//! comments, trailing commas and layout survive, and a value the payload gives is written as the
//! payload writes it.

use jsonc_parser::cst::{CstInputValue, CstObject, CstObjectProp, CstRootNode};
use serde_json::Value;

use super::keys::{Written, lookup};
use super::{Edit, Failure};
use crate::format::jsonc;

/// The document's value, which must be an object; an empty document is an empty one.
pub(super) fn semantic(text: &str) -> Result<Value, Failure> {
    let value: Option<Value> = jsonc_parser::parse_to_serde_value(text, &jsonc())
        .map_err(|e| (Some(e.range().start..e.range().end), e.kind().to_string()))?;
    match value {
        None => Ok(Value::Object(serde_json::Map::new())),
        Some(value @ Value::Object(_)) => Ok(value),
        Some(_) => Err((None, "the document is not an object, so it has no keys".to_owned())),
    }
}

/// `text` with each edit made, in order. A value the payload gives is written as the payload
/// writes it, and so is a key the file lacks, whole, where the edits leave it as the payload has
/// it: an object the payload writes on one line stays on one.
pub(super) fn apply(text: &str, edits: &[Edit<'_>], payload: &Written) -> Result<String, Failure> {
    // The payload as written, and its values; a payload that does not parse lends no layout.
    let parsed = CstRootNode::parse(&payload.source, &jsonc()).ok();
    let Some((written, values)) = parsed.zip(semantic(&payload.source).ok()) else {
        return edit(text, edits, None);
    };
    // The file's values once every edit is made, which say where the payload's writing holds.
    let after = semantic(&edit(text, edits, None)?)?;
    edit(text, edits, Some(&Layout { written, values, after, source: &payload.source }))
}

/// How the payload writes its values, and where that writing holds.
struct Layout<'a> {
    /// The payload as written.
    written: CstRootNode,
    /// The payload's values.
    values: Value,
    /// The file's values once every edit is made.
    after: Value,
    /// The payload's text.
    source: &'a str,
}

impl Layout<'_> {
    /// The part of `key` to write as the payload writes it, and that writing: the shortest part the
    /// file lacks, whole, where the edits leave it as the payload has it; else `key` itself, where
    /// the payload gives its value.
    fn writing<'k>(
        &self, object: &CstObject, key: &'k [String],
    ) -> Option<(&'k [String], Writing)> {
        let lacks = (1..=key.len())
            .filter_map(|len| key.get(..len))
            .find(|part| property(object, part).is_none());
        lacks.into_iter().chain([key]).find_map(|part| {
            let value = lookup(&self.values, part)?;
            if lookup(&self.after, part) != Some(value) {
                return None;
            }
            let written = property(&self.written.object_value()?, part)?;
            Some((part, Writing::of(&written)?))
        })
    }
}

/// A value as a document writes it.
struct Writing {
    /// Its text.
    text: String,
    /// The indent of the line its key is on, where the key opens the line.
    indent: Option<String>,
}

impl Writing {
    /// How `property` writes its value.
    fn of(property: &CstObjectProp) -> Option<Self> {
        Some(Self { text: property.value()?.to_string(), indent: property.indent_text() })
    }
}

/// The values to write as the payload writes them. The syntax tree takes no text, so each is
/// written first as a marker, a string neither document holds, then swapped for its writing.
struct Marks {
    /// What every marker starts with.
    prefix: String,
    /// Each marker's writing, by its number.
    writings: Vec<Writing>,
}

impl Marks {
    /// Markers that neither `text` nor `source` holds.
    fn new(text: &str, source: &str) -> Self {
        let mut prefix = String::from("devset-written-");
        while text.contains(&prefix) || source.contains(&prefix) {
            prefix.push('-');
        }
        Self { prefix, writings: Vec::new() }
    }

    /// A marker that stands for `writing`.
    fn mark(&mut self, writing: Writing) -> CstInputValue {
        let marker = format!("{}{}", self.prefix, self.writings.len());
        self.writings.push(writing);
        CstInputValue::String(marker)
    }

    /// `out` with each marker swapped for its writing, whose lines after the first move from the
    /// payload's indent to the file's.
    fn fill(self, mut out: String) -> String {
        for (number, writing) in self.writings.into_iter().enumerate() {
            let marker = format!("\"{}{number}\"", self.prefix);
            let Some(at) = out.find(&marker) else {
                continue;
            };
            let line = out.get(..at).and_then(|before| before.rsplit('\n').next()).unwrap_or("");
            let indent: String = line.chars().take_while(|c| matches!(c, ' ' | '\t')).collect();
            let text = match writing.indent {
                Some(from) => writing.text.replace(&format!("\n{from}"), &format!("\n{indent}")),
                None => writing.text,
            };
            out = out.replacen(&marker, &text, 1);
        }
        out
    }
}

/// `text` with each edit made, in order: as `layout` writes each, where given.
fn edit(text: &str, edits: &[Edit<'_>], layout: Option<&Layout<'_>>) -> Result<String, Failure> {
    let root = CstRootNode::parse(text, &jsonc())
        .map_err(|e| (Some(e.range().start..e.range().end), e.kind().to_string()))?;
    let object = root.object_value_or_set();
    let mut marks = Marks::new(text, layout.map_or("", |layout| layout.source));
    let mut whole: Vec<&[String]> = Vec::new();
    for &(key, value) in edits {
        // A key under one written whole was written with it.
        if whole.iter().any(|part| key.starts_with(part)) {
            continue;
        }
        let Some(value) = value else {
            remove(&object, key);
            continue;
        };
        match layout.and_then(|layout| layout.writing(&object, key)) {
            Some((part, writing)) => {
                set(&object, part, marks.mark(writing));
                whole.push(part);
            },
            None => set(&object, key, input(value)),
        }
    }
    Ok(marks.fill(root.to_string()))
}

/// The property at `key` under `object`, if every step on the way is an object.
fn property(object: &CstObject, key: &[String]) -> Option<CstObjectProp> {
    let (leaf, parents) = key.split_last()?;
    let parent =
        parents.iter().try_fold(object.clone(), |object, segment| object.object_value(segment))?;
    parent.get(leaf)
}

/// Writes `value` at `key`, creating the objects on the way.
fn set(object: &CstObject, key: &[String], value: CstInputValue) {
    let Some((leaf, parents)) = key.split_last() else {
        return;
    };
    let parent =
        parents.iter().fold(object.clone(), |object, segment| object.object_value_or_set(segment));
    match parent.get(leaf) {
        Some(property) => property.set_value(value),
        None => {
            parent.append(leaf, value);
        },
    }
}

/// Removes `key` if present, then any object it leaves empty.
fn remove(object: &CstObject, key: &[String]) {
    let Some((first, rest)) = key.split_first() else {
        return;
    };
    let Some(property) = object.get(first) else {
        return;
    };
    if rest.is_empty() {
        property.remove();
        return;
    }
    let Some(inner) = property.object_value() else {
        return;
    };
    remove(&inner, rest);
    if inner.properties().is_empty() {
        property.remove();
    }
}

/// `value` as the syntax tree's input.
fn input(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => CstInputValue::Number(n.to_string()),
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(items) => CstInputValue::Array(items.iter().map(input).collect()),
        Value::Object(object) => {
            CstInputValue::Object(object.iter().map(|(k, v)| (k.clone(), input(v))).collect())
        },
    }
}
