//! A source's profiles, found by name wherever they are in it, and what `collection.toml` says of
//! the source.
//!
//! Every `profile.toml` in a source is a profile, named by its `[profile] name`, unless it lies in
//! another profile's `files/`, where it is that profile's payload. Names are unique in a source, so
//! moving a profile within one breaks no target.

use alloc::collections::BTreeMap;
use core::slice;

use camino::Utf8Path;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::errors::{ProfileError, Result};
use crate::name::{FeatureName, ProfileName, SourceName};
use crate::path::RelPath;
use crate::profile::{MANIFEST, Manifest, PAYLOAD};
use crate::source::{Cache, Oid, Reader, Source};
use crate::target::from_toml;

/// The file at a source's root that describes it.
pub(crate) const COLLECTION: &str = "collection.toml";

/// `collection.toml`: what a source says of itself.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectionFile {
    /// The `[collection]` table.
    pub collection: CollectionMeta,
}

/// The `[collection]` table.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectionMeta {
    /// The name a target gives the source unless it chooses another.
    pub name: SourceName,
    /// One line for humans.
    #[serde(default)]
    pub description: Option<String>,
}

/// Just enough of a manifest to name its profile.
#[derive(Deserialize)]
struct Head {
    /// The `[profile]` table.
    profile: HeadMeta,
}

/// Just enough of `[profile]`.
#[derive(Deserialize)]
struct HeadMeta {
    /// The profile's name.
    name: ProfileName,
}

/// A source's profiles by name, each with its directory in the source.
#[derive(Debug)]
pub(crate) struct Index {
    /// What `collection.toml` says, if the source has one.
    meta: Option<CollectionMeta>,
    /// Each profile's directory, relative to the source's root; `""` for the root.
    profiles: BTreeMap<ProfileName, String>,
}

impl Index {
    /// The profiles `reader` holds; `source` names it in messages.
    ///
    /// # Errors
    /// - [`ProfileError::SameNameInSource`], two profiles have one name.
    /// - [`Error::Parse`](crate::Error::Parse), a manifest, or `collection.toml`, does not parse.
    pub(crate) fn read(reader: &Reader, source: &Source) -> Result<Self> {
        let dirs = profiles(reader.manifests());
        let paths = dirs.iter().map(|dir| manifest(dir)).collect::<Result<Vec<_>>>()?;
        let found = reader.read("", &paths)?;
        let mut profiles = BTreeMap::new();
        for (dir, path) in dirs.into_iter().zip(&paths) {
            let Some(raw) = found.get(path) else { continue };
            let Head { profile } = from_toml(raw, &label(source, path.as_str()))?;
            if let Some(first) = profiles.insert(profile.name.clone(), dir.clone()) {
                let (source, name, second) = (source.to_string(), profile.name, dir);
                return Err(ProfileError::SameNameInSource {
                    location: source,
                    name,
                    first,
                    second,
                }
                .into());
            }
        }
        let collection = RelPath::new(COLLECTION)?;
        let meta = reader.read("", slice::from_ref(&collection))?;
        let meta = meta
            .get(&collection)
            .map(|raw| from_toml::<CollectionFile>(raw, &label(source, COLLECTION)))
            .transpose()?
            .map(|file| file.collection);
        Ok(Self { meta, profiles })
    }

    /// Every profile's name, in order.
    pub(crate) fn names(&self) -> impl Iterator<Item = &ProfileName> {
        self.profiles.keys()
    }

    /// The directory of the profile named `name`.
    ///
    /// # Errors
    /// [`ProfileError::NotInSource`], the source has none, with every name it does have.
    pub(crate) fn find(&self, name: &ProfileName, source: &Source) -> Result<&str> {
        self.profiles.get(name).map(String::as_str).ok_or_else(|| {
            let profiles = self.profiles.keys().cloned().collect();
            ProfileError::NotInSource { location: source.to_string(), name: name.clone(), profiles }
                .into()
        })
    }
}

/// Of `dirs`, every one holding a `profile.toml`, those that are profiles: not in another's
/// `files/`.
fn profiles(mut dirs: Vec<String>) -> Vec<String> {
    dirs.sort();
    let mut kept: Vec<String> = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let payload = kept.iter().any(|profile| {
            let files = if profile.is_empty() {
                format!("{PAYLOAD}/")
            } else {
                format!("{profile}/{PAYLOAD}/")
            };
            dir.starts_with(&files)
        });
        if !payload {
            kept.push(dir);
        }
    }
    kept
}

/// The manifest of the profile in `dir`.
fn manifest(dir: &str) -> Result<RelPath> {
    Ok(if dir.is_empty() {
        RelPath::new(MANIFEST)?
    } else {
        RelPath::new(&format!("{dir}/{MANIFEST}"))?
    })
}

/// Where `file`, relative to `source`'s root, is, as users recognise it.
pub(crate) fn label(source: &Source, file: &str) -> String {
    match source {
        Source::Dir(dir) => dir.join(file).into_string(),
        Source::Git { url, path, .. } => path
            .as_ref()
            .map_or_else(|| format!("{url}/{file}"), |path| format!("{url}/{path}/{file}")),
    }
}

/// A source's profiles, as `devset list` shows them.
#[derive(Debug)]
pub struct Listing {
    /// What `collection.toml` says, if the source has one.
    pub meta: Option<CollectionMeta>,
    /// Every profile, by name.
    pub profiles: Vec<Summary>,
}

/// One profile in a [`Listing`].
#[derive(Debug)]
pub struct Summary {
    /// Its manifest.
    pub manifest: Manifest,
    /// Its directory in the source.
    pub dir: String,
}

impl Summary {
    /// Its features, each marked whether it is on by default.
    pub fn features(&self) -> impl Iterator<Item = (&FeatureName, bool)> {
        let features = &self.manifest.features;
        features.declared.keys().map(|name| (name, features.default.contains(name)))
    }
}

/// The profiles `source` holds, read with `root` as the target's root: at `pin` if given, else at
/// what its ref names now.
///
/// # Errors
/// - [`Error::Source`](crate::Error::Source), the source cannot be read.
/// - [`Error::Parse`](crate::Error::Parse) or [`ProfileError`], a manifest is not valid.
pub fn list(source: &Source, root: &Utf8Path, pin: Option<&Oid>, cache: &Cache) -> Result<Listing> {
    let reader = source.open(root, pin, cache)?;
    let Index { meta, profiles } = Index::read(&reader, source)?;
    let paths = profiles.values().map(|dir| manifest(dir)).collect::<Result<Vec<_>>>()?;
    let found = reader.read("", &paths)?;
    let mut listed = Vec::with_capacity(paths.len());
    for (dir, path) in profiles.into_values().zip(&paths) {
        let Some(raw) = found.get(path) else { continue };
        let file = label(source, path.as_str());
        let manifest: Manifest = from_toml(raw, &file)?;
        manifest.check(&file)?;
        listed.push(Summary { manifest, dir });
    }
    Ok(Listing { meta, profiles: listed })
}

#[cfg(test)]
mod tests {
    use super::profiles;

    #[test]
    fn a_manifest_in_a_payload_is_not_a_profile() {
        let found = profiles(
            ["b", "a", "a/files/x", "a/extra", "files/y", "c/files", ""]
                .map(str::to_owned)
                .to_vec(),
        );
        assert_eq!(
            found,
            ["", "a", "a/extra", "b", "c/files"],
            "files/ under a profile is its payload"
        );
    }
}
