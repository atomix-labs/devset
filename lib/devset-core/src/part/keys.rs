//! Keys a profile owns in a structured file: its leaves, their canonical content, and the merge.

use alloc::collections::{BTreeMap, BTreeSet};
use core::str;

use serde_json::Value;

/// A leaf's path from the document root, one segment per key.
pub(crate) type Key = Vec<String>;

/// Leaves and their values, compared after parsing so that layout never matters.
pub(crate) type Leaves = BTreeMap<Key, Value>;

/// A partial document as its payload writes it: its leaves in its order, each object's keys in
/// theirs, which leaves it writes as arrays of tables, and its text.
#[derive(Clone, Debug, Default)]
pub(crate) struct Written {
    /// Each leaf and its value, in the document's order.
    pub leaves: Vec<(Key, Value)>,
    /// The leaves written as arrays of tables, `[[key]]`: TOML's alone.
    pub tables: BTreeSet<Key>,
    /// The document as written, whose layout a value it adds to a file keeps, in TOML and YAML.
    pub source: String,
}

/// `leaves` as content: one line per leaf, sorted, each its key and its value as canonical JSON.
pub(crate) fn encode(leaves: &Leaves) -> Vec<u8> {
    let line = |(key, value): (&Key, &Value)| {
        format!("{}\t{}\n", serde_json::to_string(key).unwrap_or_default(), canonical(value))
    };
    leaves.iter().map(line).collect::<String>().into_bytes()
}

/// The leaves `content` encodes; `None` when it is not what [`encode`] writes.
pub(crate) fn decode(content: &[u8]) -> Option<Leaves> {
    str::from_utf8(content)
        .ok()?
        .lines()
        .map(|line| {
            let (key, value) = line.split_once('\t')?;
            Some((serde_json::from_str(key).ok()?, serde_json::from_str(value).ok()?))
        })
        .collect()
}

/// `value` as JSON with every object's keys in order, so equal values encode equally.
fn canonical(value: &Value) -> String {
    let mut sorted = value.clone();
    sorted.sort_all_objects();
    sorted.to_string()
}

/// The leaves of `value`, in order: objects are containers, everything else is a leaf.
pub(crate) fn walk(value: &Value, prefix: &mut Key, into: &mut Vec<(Key, Value)>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                prefix.push(key.clone());
                walk(value, prefix, into);
                prefix.pop();
            }
        },
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            into.push((prefix.clone(), value.clone()));
        },
    }
}

/// The value at `key` in `value`, if every step on the way is an object.
pub(crate) fn lookup<'a>(value: &'a Value, key: &[String]) -> Option<&'a Value> {
    key.iter().try_fold(value, |node, segment| node.as_object()?.get(segment))
}

/// `key` as people write it: dotted, with a segment that needs it quoted.
pub(crate) fn display(key: &[String]) -> String {
    let plain = |s: &str| {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    let segments: Vec<String> = key
        .iter()
        .map(|s| if plain(s) { s.clone() } else { Value::String(s.clone()).to_string() })
        .collect();
    segments.join(".")
}

/// Whether two keys overlap: one equals the other or lies under it.
pub(crate) fn overlap(a: &[String], b: &[String]) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// The three-way merge of `ours` and `theirs` from `base`, leaf by leaf over `keys`.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Merge {
    /// The merged leaves; a conflicted leaf keeps ours.
    pub leaves: Leaves,
    /// Leaves both sides changed differently.
    pub conflicts: BTreeSet<Key>,
}

/// Merges leaf by leaf: a leaf only one side changed takes that side; both, differently, conflicts.
pub(crate) fn merge(base: &Leaves, ours: &Leaves, theirs: &Leaves, keys: &BTreeSet<Key>) -> Merge {
    let mut merge = Merge::default();
    for key in keys {
        let (b, o, t) = (base.get(key), ours.get(key), theirs.get(key));
        let taken = if o == t || t == b {
            o
        } else if o == b {
            t
        } else {
            merge.conflicts.insert(key.clone());
            o
        };
        if let Some(value) = taken {
            merge.leaves.insert(key.clone(), value.clone());
        }
    }
    merge
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeSet;

    use serde_json::json;

    use super::{Leaves, decode, display, encode, merge, overlap};

    fn leaves(entries: &[(&[&str], serde_json::Value)]) -> Leaves {
        entries
            .iter()
            .map(|(key, value)| (key.iter().map(|s| (*s).to_owned()).collect(), value.clone()))
            .collect()
    }

    #[test]
    fn content_round_trips_whatever_the_key_order() {
        let a = leaves(&[(&["a", "b"], json!({ "y": 1, "x": [1, 2] })), (&["c"], json!("d"))]);
        let b = leaves(&[(&["c"], json!("d")), (&["a", "b"], json!({ "x": [1, 2], "y": 1 }))]);
        assert_eq!(encode(&a), encode(&b), "equal leaves encode equally");
        assert_eq!(decode(&encode(&a)), Some(a), "and decode to themselves");
    }

    #[test]
    fn leaves_merge_one_by_one() {
        let base = leaves(&[
            (&["a"], json!(1)),
            (&["b"], json!(1)),
            (&["c"], json!(1)),
            (&["gone"], json!(1)),
        ]);
        let ours = leaves(&[
            (&["a"], json!(2)),
            (&["b"], json!(1)),
            (&["c"], json!(2)),
            (&["gone"], json!(1)),
        ]);
        let theirs = leaves(&[
            (&["a"], json!(1)),
            (&["b"], json!(3)),
            (&["c"], json!(3)),
            (&["new"], json!(1)),
        ]);
        let keys: BTreeSet<_> = base.keys().chain(theirs.keys()).cloned().collect();
        let merged = merge(&base, &ours, &theirs, &keys);
        let want = leaves(&[
            (&["a"], json!(2)),
            (&["b"], json!(3)),
            (&["c"], json!(2)),
            (&["new"], json!(1)),
        ]);
        assert_eq!(merged.leaves, want, "ours, theirs, a conflict kept as ours, a drop and an add");
        assert_eq!(merged.conflicts.len(), 1, "only the leaf both changed conflicts");
    }

    #[test]
    fn keys_display_and_overlap() {
        let key = |parts: &[&str]| parts.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(display(&key(&["workspace", "lints"])), "workspace.lints", "plain");
        assert_eq!(
            display(&key(&["[rust]", "editor.tabSize"])),
            r#""[rust]"."editor.tabSize""#,
            "quoted"
        );
        assert!(overlap(&key(&["a", "b"]), &key(&["a"])), "a key under another overlaps it");
        assert!(!overlap(&key(&["a", "b"]), &key(&["a", "c"])), "siblings do not");
    }
}
