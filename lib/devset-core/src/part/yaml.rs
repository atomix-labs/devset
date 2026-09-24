//! YAML keys, experimental: read as values, written through `yaml-edit`, which keeps comments
//! and layout. Only the first document of a stream is managed.

use alloc::collections::{BTreeMap, BTreeSet};
use core::str::FromStr;

use serde_json::Value;
use yaml_edit::{Document, Mapping, ScalarValue, YamlFile, YamlValue};

use super::keys::{Key, Leaves, Written, lookup, walk};
use super::{Edit, Failure};

/// The first document's value, which must be a mapping; an empty stream is an empty one.
fn semantic(text: &str) -> Result<Value, Failure> {
    if text.trim().is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let documents: Vec<Value> =
        serde_saphyr::from_multiple(text).map_err(|e| (None, e.without_snippet().to_string()))?;
    match documents.into_iter().next() {
        None | Some(Value::Null) => Ok(Value::Object(serde_json::Map::new())),
        Some(value @ Value::Object(_)) => Ok(value),
        Some(_) => Err((
            None,
            "the document is not a mapping, so it has no keys".to_owned(),
        )),
    }
}

/// The leaves a partial document defines, in its order: mappings are containers, everything else is
/// a leaf.
pub(super) fn leaves(text: &str) -> Result<Written, Failure> {
    let mut leaves = Vec::new();
    walk(&semantic(text)?, &mut Vec::new(), &mut leaves);
    Ok(Written {
        leaves,
        tables: BTreeSet::new(),
    })
}

/// `text`'s values at `keys`; a key it lacks is left out.
pub(super) fn values(text: &str, keys: &BTreeSet<Key>) -> Result<Leaves, Failure> {
    let values = semantic(text)?;
    Ok(keys
        .iter()
        .filter_map(|key| Some((key.clone(), lookup(&values, key)?.clone())))
        .collect())
}

/// `text` with each edit made, in order.
pub(super) fn apply(text: &str, edits: &[Edit<'_>], _: &BTreeSet<Key>) -> Result<String, Failure> {
    let file = YamlFile::from_str(text).map_err(|e| (None, e.to_string()))?;
    let document = if let Some(document) = file.document() {
        document
    } else {
        file.push_document(Document::new_mapping());
        file.document()
            .ok_or_else(|| (None, "an empty stream takes no document".to_owned()))?
    };
    let root = document.as_mapping().ok_or_else(|| {
        (
            None,
            "the document is not a mapping, so it has no keys".to_owned(),
        )
    })?;
    for &(key, value) in edits {
        match value {
            Some(value) => set(&root, key, value)?,
            None => remove(&root, key),
        }
    }
    Ok(file.to_string())
}

/// Writes `value` at `key`, creating the mappings on the way.
fn set(mapping: &Mapping, key: &[String], value: &Value) -> Result<(), Failure> {
    let Some((leaf, parents)) = key.split_last() else {
        return Ok(());
    };
    let mut mapping = mapping.clone();
    for segment in parents {
        if mapping.get_mapping(segment.as_str()).is_none() {
            mapping.set(segment.as_str(), Mapping::new());
        }
        mapping = mapping.get_mapping(segment.as_str()).ok_or_else(|| {
            (
                None,
                format!("{segment} is not a mapping, so it takes no keys"),
            )
        })?;
    }
    mapping.set(leaf.as_str(), yaml(value));
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

/// `value` as a YAML value.
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
            object
                .iter()
                .map(|(k, v)| (k.clone(), yaml(v)))
                .collect::<BTreeMap<_, _>>(),
        ),
    }
}
