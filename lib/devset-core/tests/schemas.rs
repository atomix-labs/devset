//! The JSON schemas the manual publishes, in `docs/src/schema/`, are the ones devset's types
//! define. `SNAPSHOTS=overwrite` writes them again; `just fix` then lays them out.
#![cfg(test)]

use std::path::PathBuf;
use std::{env, fs};

use devset_core::collection::CollectionFile;
use devset_core::profile::Manifest;
use devset_core::target::Config;
use schemars::schema_for;
use serde_json::Value;

#[test]
fn the_published_schemas_are_current() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/src/schema");
    let overwrite = env::var("SNAPSHOTS").is_ok_and(|how| how == "overwrite");
    let mut stale = Vec::new();
    for (name, schema) in [
        ("profile", schema_for!(Manifest)),
        ("config", schema_for!(Config)),
        ("collection", schema_for!(CollectionFile)),
    ] {
        let path = dir.join(format!("{name}.json"));
        let want = serde_json::to_value(&schema).unwrap();
        let have: Option<Value> =
            fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str(&text).ok());
        if have.as_ref() == Some(&want) {
            continue;
        }
        if overwrite {
            fs::write(&path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
        } else {
            stale.push(format!("docs/src/schema/{name}.json"));
        }
    }
    assert!(
        stale.is_empty(),
        "stale: {stale:?}; run `SNAPSHOTS=overwrite cargo test -p devset-core --test schemas`, \
         then `just fix`"
    );
}
