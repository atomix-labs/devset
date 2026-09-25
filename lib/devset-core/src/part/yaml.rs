//! YAML keys, experimental: read as values, written through `yaml-edit`, which keeps comments
//! and layout. Only the first document of a stream is managed.

use alloc::collections::BTreeMap;
use core::str::FromStr;

use serde_json::Value;
use yaml_edit::{Document, Mapping, ScalarValue, YamlFile, YamlNode, YamlValue};

use super::keys::{Written, lookup};
use super::{Edit, Failure};

/// The first document's value, which must be a mapping; an empty stream is an empty one.
pub(super) fn semantic(text: &str) -> Result<Value, Failure> {
    if text.trim().is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let documents: Vec<Value> =
        serde_saphyr::from_multiple(text).map_err(|e| (None, e.without_snippet().to_string()))?;
    match documents.into_iter().next() {
        None | Some(Value::Null) => Ok(Value::Object(serde_json::Map::new())),
        Some(value @ Value::Object(_)) => Ok(value),
        Some(_) => Err((None, "the document is not a mapping, so it has no keys".to_owned())),
    }
}

/// `text` with each edit made, in order; a value the payload gives is written as the payload writes
/// it.
pub(super) fn apply(text: &str, edits: &[Edit<'_>], payload: &Written) -> Result<String, Failure> {
    let file = YamlFile::from_str(text).map_err(|e| (None, e.to_string()))?;
    let document = if let Some(document) = file.document() {
        document
    } else {
        file.push_document(Document::new_mapping());
        file.document().ok_or_else(|| (None, "an empty stream takes no document".to_owned()))?
    };
    let root = document
        .as_mapping()
        .ok_or_else(|| (None, "the document is not a mapping, so it has no keys".to_owned()))?;
    // The payload as written, and its values; a payload that does not parse lends no layout.
    let source = YamlFile::from_str(&payload.source).ok().zip(semantic(&payload.source).ok());
    for &(key, value) in edits {
        match value {
            Some(value) => {
                let written = source
                    .as_ref()
                    .filter(|(_, values)| lookup(values, key) == Some(value))
                    .and_then(|(written, _)| node(written, key));
                set(&root, key, value, written)?;
            },
            None => remove(&root, key),
        }
    }
    Ok(file.to_string())
}

/// The node `file`'s first document writes at `key`.
fn node(file: &YamlFile, key: &[String]) -> Option<YamlNode> {
    let (leaf, parents) = key.split_last()?;
    let mut mapping = file.document()?.as_mapping()?;
    for segment in parents {
        mapping = mapping.get_mapping(segment.as_str())?;
    }
    mapping.get(leaf.as_str())
}

/// Writes `value` at `key`, creating the mappings on the way: as `written`, the payload's own
/// writing of it, if given.
fn set(
    mapping: &Mapping, key: &[String], value: &Value, written: Option<YamlNode>,
) -> Result<(), Failure> {
    let Some((leaf, parents)) = key.split_last() else {
        return Ok(());
    };
    let mut mapping = mapping.clone();
    for segment in parents {
        if mapping.get_mapping(segment.as_str()).is_none() {
            mapping.set(segment.as_str(), Mapping::new());
        }
        mapping = mapping
            .get_mapping(segment.as_str())
            .ok_or_else(|| (None, format!("{segment} is not a mapping, so it takes no keys")))?;
    }
    match written {
        Some(written) => mapping.set(leaf.as_str(), written),
        None => mapping.set(leaf.as_str(), yaml(value)),
    }
    Ok(())
}

/// Removes `key` if present, then any mapping it leaves empty.
fn remove(mapping: &Mapping, key: &[String]) {
    let Some((first, rest)) = key.split_first() else {
        return;
    };
    if rest.is_empty() {
        mapping.remove(first.as_str());
        return;
    }
    let Some(inner) = mapping.get_mapping(first.as_str()) else {
        return;
    };
    remove(&inner, rest);
    if inner.is_empty() {
        mapping.remove(first.as_str());
    }
}

/// `value` as a YAML value, for a value the payload does not give as it is, such as a merge's.
///
/// A mapping's keys come out sorted: `yaml-edit` indents only the values it builds itself, and it
/// builds mappings from sorted maps.
fn yaml(value: &Value) -> YamlValue {
    match value {
        Value::Null => YamlValue::scalar(ScalarValue::null()),
        Value::Bool(b) => YamlValue::scalar(*b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => YamlValue::scalar(i),
            (None, Some(f)) => YamlValue::scalar(f),
            (None, None) => YamlValue::scalar(n.to_string()),
        },
        Value::String(s) => YamlValue::scalar(s.as_str()),
        Value::Array(items) => YamlValue::Sequence(items.iter().map(yaml).collect()),
        Value::Object(object) => YamlValue::Mapping(
            object.iter().map(|(k, v)| (k.clone(), yaml(v))).collect::<BTreeMap<_, _>>(),
        ),
    }
}
