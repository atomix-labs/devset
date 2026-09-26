//! TOML keys: read through `toml`, written through `toml_edit`, so comments and layout survive.

use alloc::collections::BTreeSet;

use serde_json::Value;
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, TableLike};

use super::keys::{Key, Written, display, lookup};
use super::{Edit, Failure};

/// The key `toml` gives a datetime when it becomes JSON.
const DATETIME: &str = "$__toml_private_datetime";

/// The document's values as JSON, datetimes kept in `toml`'s own form.
pub(super) fn semantic(text: &str) -> Result<Value, Failure> {
    let table: toml::Table =
        toml::from_str(text).map_err(|e| (e.span(), e.message().to_owned()))?;
    serde_json::to_value(table).map_err(|e| (None, e.to_string()))
}

/// The leaves a partial document defines, in its order: tables are containers; plain values,
/// arrays and inline tables are leaves, and so is an array of tables, noted as one.
pub(super) fn leaves(text: &str) -> Result<Written, Failure> {
    let doc: DocumentMut =
        text.parse().map_err(|e: toml_edit::TomlError| (e.span(), e.message().to_owned()))?;
    let values = semantic(text)?;
    let mut paths = Vec::new();
    let mut tables = BTreeSet::new();
    collect(doc.as_table(), &mut Vec::new(), &mut paths, &mut tables);
    let leaves = paths
        .into_iter()
        .filter_map(|key| Some((key.clone(), lookup(&values, &key)?.clone())))
        .collect();
    Ok(Written { leaves, tables, source: text.to_owned() })
}

/// Every leaf path under `table`, and those that are arrays of tables.
fn collect(
    table: &dyn TableLike, prefix: &mut Key, into: &mut Vec<Key>, tables: &mut BTreeSet<Key>,
) {
    for (key, item) in table.iter() {
        prefix.push(key.to_owned());
        match item {
            Item::Table(inner) => collect(inner, prefix, into, tables),
            // `a.b = 1` is a table written in dotted keys, not a value.
            Item::Value(toml_edit::Value::InlineTable(inner)) if inner.is_dotted() => {
                collect(inner, prefix, into, tables);
            },
            Item::ArrayOfTables(_) => {
                tables.insert(prefix.clone());
                into.push(prefix.clone());
            },
            Item::Value(_) => into.push(prefix.clone()),
            Item::None => {},
        }
        prefix.pop();
    }
}

/// `text` with each edit made, in order; a key the payload writes as an array of tables is written
/// as one, and a value the payload gives is written in the payload's layout, comments and all.
pub(super) fn apply(text: &str, edits: &[Edit<'_>], payload: &Written) -> Result<String, Failure> {
    let mut doc: DocumentMut =
        text.parse().map_err(|e: toml_edit::TomlError| (e.span(), e.message().to_owned()))?;
    // The payload as written, and its values; a payload that does not parse lends no layout.
    let source = payload.source.parse::<DocumentMut>().ok().zip(semantic(&payload.source).ok());
    for &(key, value) in edits {
        match value {
            Some(value) => {
                let styled = source
                    .as_ref()
                    .filter(|(_, values)| lookup(values, key) == Some(value))
                    .and_then(|(written, _)| layout(written, key));
                set(&mut doc, key, value, payload.tables.contains(key), styled)?;
            },
            None => remove(doc.as_table_mut(), key),
        }
    }
    Ok(doc.to_string())
}

/// How a payload writes a leaf.
#[derive(Clone, Copy)]
enum Styled<'a> {
    /// A plain value: the key, with the space around it, and the value.
    Value(&'a toml_edit::Key, &'a toml_edit::Value),
    /// An array of tables, each with its comments and the space around its keys.
    Tables(&'a ArrayOfTables),
}

/// How `doc` writes the leaf at `key`.
fn layout<'a>(doc: &'a DocumentMut, key: &[String]) -> Option<Styled<'a>> {
    let (leaf, parents) = key.split_last()?;
    let mut table: &dyn TableLike = doc.as_table();
    for segment in parents {
        table = table.get(segment)?.as_table_like()?;
    }
    match table.get_key_value(leaf)? {
        (written, Item::Value(value)) => Some(Styled::Value(written, value)),
        (_, Item::ArrayOfTables(tables)) => Some(Styled::Tables(tables)),
        (_, Item::None | Item::Table(_)) => None,
    }
}

/// Writes `value` at `key`, creating the tables on the way; a replaced value keeps its comments.
///
/// An array of objects keeps the form the file gives it, or else the payload's: an array of
/// tables, `[[key]]`, when `as_tables`, and inline otherwise. With `styled`, the payload's own
/// writing of the value, the value is written as the payload writes it, and a new key is spaced as
/// the payload spaces it; a key the file holds keeps the file's spacing. An array of tables is the
/// payload's whole, comments and all.
fn set(
    doc: &mut DocumentMut, key: &[String], value: &Value, as_tables: bool,
    styled: Option<Styled<'_>>,
) -> Result<(), Failure> {
    let Some((leaf, parents)) = key.split_last() else {
        return Ok(());
    };
    let mut table: &mut dyn TableLike = doc.as_table_mut();
    let mut standard = true;
    for (depth, segment) in parents.iter().enumerate() {
        let item = table.entry(segment).or_insert_with(|| {
            let mut created = Table::new();
            created.set_implicit(true);
            Item::Table(created)
        });
        standard &= item.is_table();
        table = item.as_table_like_mut().ok_or_else(|| {
            let parent = display(key.get(..=depth).unwrap_or(key));
            (None, format!("{parent} is not a table, so {} cannot be set", display(key)))
        })?;
    }
    let failed = |why: String| (None, format!("{}: {why}", display(key)));
    let form = match table.get(leaf) {
        Some(Item::ArrayOfTables(_)) => true,
        Some(Item::Value(_)) => false,
        Some(Item::None | Item::Table(_)) | None => as_tables,
    };
    let new = if standard && form && tables(value) {
        Item::ArrayOfTables(match styled {
            Some(Styled::Tables(written)) => unplaced(written.clone()),
            Some(Styled::Value(..)) | None => array_of_tables(value).map_err(failed)?,
        })
    } else if let Some(Styled::Value(_, written)) = styled {
        Item::Value(written.clone())
    } else {
        Item::Value(toml_value(value).map_err(failed)?)
    };
    // Replaced in place, so the key keeps the comments above it, and the value those after it.
    match (table.get_mut(leaf), new) {
        (Some(Item::Value(old)), Item::Value(mut new)) => {
            *new.decor_mut() = old.decor().clone();
            *old = new;
        },
        (Some(item), new) => *item = new,
        (None, new) => match styled {
            Some(Styled::Value(written, _)) => {
                table.entry_format(written).or_insert(new);
            },
            Some(Styled::Tables(_)) | None => {
                table.insert(leaf, new);
            },
        },
    }
    Ok(())
}

/// Whether `value` is an array of tables: a non-empty array of objects, no datetime among them.
fn tables(value: &Value) -> bool {
    matches!(value, Value::Array(items)
        if !items.is_empty() && items.iter().all(|item| item.is_object() && !datetime(item)))
}

/// Whether `value` is a datetime in `toml`'s JSON form.
fn datetime(value: &Value) -> bool {
    value.as_object().is_some_and(|object| object.len() == 1 && object.contains_key(DATETIME))
}

/// `tables` without their places in the payload, so each follows the table before it in the file.
fn unplaced(mut tables: ArrayOfTables) -> ArrayOfTables {
    tables.iter_mut().for_each(unplace);
    tables
}

/// `table` and the tables in it without their places in the document they came from.
fn unplace(table: &mut Table) {
    table.set_position(None);
    for (_, item) in table.iter_mut() {
        match item {
            Item::Table(inner) => unplace(inner),
            Item::ArrayOfTables(inner) => inner.iter_mut().for_each(unplace),
            Item::None | Item::Value(_) => {},
        }
    }
}

/// Each object of `value` as a table of an array of tables.
fn array_of_tables(value: &Value) -> Result<ArrayOfTables, String> {
    let Value::Array(items) = value else {
        return Ok(ArrayOfTables::new());
    };
    let tables: Result<Vec<Table>, String> =
        items.iter().filter_map(Value::as_object).map(table_of).collect();
    Ok(tables?.into_iter().collect())
}

/// `object` as a table: an object in it is a sub-table, an array of objects an array of tables.
fn table_of(object: &serde_json::Map<String, Value>) -> Result<Table, String> {
    let mut table = Table::new();
    for (key, value) in object {
        let item = match value {
            Value::Object(inner) if !datetime(value) => Item::Table(table_of(inner)?),
            Value::Array(_) if tables(value) => Item::ArrayOfTables(array_of_tables(value)?),
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::Object(_) => Item::Value(toml_value(value)?),
        };
        table.insert(key, item);
    }
    Ok(table)
}

/// Removes `key` if present, then any table it leaves empty.
fn remove(table: &mut dyn TableLike, key: &[String]) {
    let Some((first, rest)) = key.split_first() else {
        return;
    };
    if rest.is_empty() {
        table.remove(first);
        return;
    }
    let Some(inner) = table.get_mut(first).and_then(Item::as_table_like_mut) else {
        return;
    };
    remove(inner, rest);
    if inner.is_empty() {
        table.remove(first);
    }
}

/// `value` as a TOML value; `null` has no TOML form.
fn toml_value(value: &Value) -> Result<toml_edit::Value, String> {
    Ok(match value {
        Value::Null => return Err("null has no TOML form".to_owned()),
        Value::Bool(b) => (*b).into(),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => i.into(),
            (None, Some(f)) => f.into(),
            (None, None) => return Err(format!("{n} does not fit a TOML number")),
        },
        Value::String(s) => s.as_str().into(),
        Value::Array(items) => {
            let items: Result<Vec<_>, _> = items.iter().map(toml_value).collect();
            toml_edit::Value::Array(items?.into_iter().collect::<Array>())
        },
        Value::Object(object) => {
            if let (Some(Value::String(datetime)), 1) = (object.get(DATETIME), object.len()) {
                return datetime
                    .parse::<toml_edit::Datetime>()
                    .map(Into::into)
                    .map_err(|e| e.to_string());
            }
            let mut table = InlineTable::new();
            for (k, v) in object {
                table.insert(k, toml_value(v)?);
            }
            toml_edit::Value::InlineTable(table)
        },
    })
}
