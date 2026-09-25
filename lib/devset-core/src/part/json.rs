//! JSON and JSONC keys: read as values, written through `jsonc-parser`'s concrete syntax tree, so
//! comments, trailing commas and layout survive.

use jsonc_parser::cst::{CstInputValue, CstObject, CstRootNode};
use serde_json::Value;

use super::keys::Written;
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

/// `text` with each edit made, in order.
pub(super) fn apply(text: &str, edits: &[Edit<'_>], _: &Written) -> Result<String, Failure> {
    let root = CstRootNode::parse(text, &jsonc())
        .map_err(|e| (Some(e.range().start..e.range().end), e.kind().to_string()))?;
    let object = root.object_value_or_set();
    for &(key, value) in edits {
        match value {
            Some(value) => set(&object, key, value),
            None => remove(&object, key),
        }
    }
    Ok(root.to_string())
}

/// Writes `value` at `key`, creating the objects on the way.
fn set(object: &CstObject, key: &[String], value: &Value) {
    let Some((leaf, parents)) = key.split_last() else {
        return;
    };
    let parent =
        parents.iter().fold(object.clone(), |object, segment| object.object_value_or_set(segment));
    match parent.get(leaf) {
        Some(property) => property.set_value(input(value)),
        None => {
            parent.append(leaf, input(value));
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
