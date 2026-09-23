//! Resolving a target's layers into one [`Resolved`] profile: fetched, composed, answered.

use alloc::collections::BTreeMap;
use core::slice;

use camino::Utf8Path;
use semver::Version;
use serde::Serialize;

use crate::digest::Digest;
use crate::errors::{ProfileError, Result, TargetError};
use crate::format::Format;
pub use crate::merge::Driver;
use crate::path::RelPath;
use crate::profile::{FileSpec, MANIFEST, Manifest, MergeSpec, Meta, PAYLOAD, Policy};
pub use crate::settings::{Settings, Suggestion};
use crate::source::{Cache, Oid, Source};
use crate::survey::Want;
use crate::target::{Override, Target, from_toml};
use crate::tree::Tree;
pub use crate::vars::Question;
use crate::vars::{self, VarName, VarSpec};

/// Which layers [`resolve`] may move off their locked commit.
#[derive(Clone, Copy, Debug)]
pub enum Refresh<'a> {
    /// None: every layer at its locked commit.
    None,
    /// Every layer, to what its ref names now.
    All,
    /// The layer whose profile has this name.
    Layer(&'a str),
}

/// Whether a layer's content is what the lock recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Applied {
    /// It is.
    Current,
    /// The layer moved, or its local directory changed, since it was applied.
    Changed,
    /// The layer has never been applied.
    Never,
}

/// A profile resolved from its source at one revision.
#[derive(Debug)]
pub struct Layer {
    /// Where it came from.
    source: Source,
    /// Its `[profile]` table.
    meta: Meta,
    /// Its `[merge]` table.
    merge: MergeSpec,
    /// Its variables.
    vars: BTreeMap<VarName, VarSpec>,
    /// The commit, for a versioned source.
    rev: Option<Oid>,
    /// Digest of manifest and payload together.
    digest: Digest,
    /// The digest the lock recorded for it.
    locked: Option<Digest>,
    /// Its files.
    files: BTreeMap<RelPath, FileSpec>,
    /// Their bytes.
    payload: Tree,
}

impl Layer {
    /// Where it came from.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Its `[profile]` table.
    #[must_use]
    pub const fn meta(&self) -> &Meta {
        &self.meta
    }

    /// Its `[merge]` table.
    #[must_use]
    pub const fn merge(&self) -> &MergeSpec {
        &self.merge
    }

    /// The commit, for a versioned source.
    #[must_use]
    pub const fn rev(&self) -> Option<&Oid> {
        self.rev.as_ref()
    }

    /// Digest of manifest and payload together.
    #[must_use]
    pub const fn digest(&self) -> Digest {
        self.digest
    }

    /// Whether this content is what the lock recorded.
    #[must_use]
    pub fn applied(&self) -> Applied {
        match self.locked {
            None => Applied::Never,
            Some(locked) if locked == self.digest => Applied::Current,
            Some(_) => Applied::Changed,
        }
    }

    /// Its variables.
    pub(crate) fn vars(&self) -> impl Iterator<Item = (&VarName, &VarSpec)> {
        self.vars.iter()
    }

    /// Its files' bytes.
    pub(crate) const fn payload(&self) -> &Tree {
        &self.payload
    }

    /// Replaces its files' bytes, templates rendered.
    pub(crate) fn set_payload(&mut self, payload: Tree) {
        self.payload = payload;
    }

    /// Whether any of its files is a template.
    pub(crate) fn has_templates(&self) -> bool {
        self.files.values().any(|spec| spec.template)
    }

    /// Whether `path` is a template.
    pub(crate) fn is_template(&self, path: &RelPath) -> bool {
        self.files.get(path).is_some_and(|spec| spec.template)
    }

    /// Reads and checks the profile at `source`, at `pin` if given.
    fn load(source: &Source, root: &Utf8Path, pin: Option<&Oid>, cache: &Cache) -> Result<Self> {
        let reader = source.open(root, pin, cache)?;
        let manifest_path = RelPath::new(MANIFEST)?;
        let head = reader.read("", slice::from_ref(&manifest_path))?;
        let raw = head
            .get(&manifest_path)
            .ok_or_else(|| ProfileError::NotAProfile { location: source.to_string() })?;
        let manifest: Manifest = from_toml(raw, &manifest_label(source))?;
        let meta = manifest.profile;
        if let Some(required) = &meta.devset
            && !Version::parse(crate::VERSION).is_ok_and(|version| required.matches(&version))
        {
            let requires = required.to_string();
            return Err(ProfileError::Incompatible { profile: meta.name, requires }.into());
        }
        let paths: Vec<RelPath> = manifest.files.keys().cloned().collect();
        let payload = reader.read(PAYLOAD, &paths)?;
        if let Some(missing) = paths.into_iter().find(|path| payload.get(path).is_none()) {
            return Err(ProfileError::MissingPayload { profile: meta.name, path: missing }.into());
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(Digest::of(raw).as_bytes());
        hasher.update(payload.digest().as_bytes());
        Ok(Self {
            source: source.clone(),
            meta,
            merge: manifest.merge,
            vars: manifest.vars,
            rev: reader.rev().cloned(),
            digest: Digest::new(hasher.finalize()),
            locked: None,
            files: manifest.files,
            payload,
        })
    }
}

/// What a layer provides at one path, overrides applied.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Provided {
    /// Index into the layers.
    pub layer: usize,
    /// How it is managed.
    pub policy: Policy,
    /// How a merge of it is checked.
    pub validate: Format,
}

/// Where `source`'s manifest is, as a path users recognise.
fn manifest_label(source: &Source) -> String {
    match source {
        Source::Dir(dir) => dir.join(MANIFEST).into_string(),
        Source::Git { url, path, .. } => path
            .as_ref()
            .map_or_else(|| format!("{url}/{MANIFEST}"), |path| format!("{url}/{path}/{MANIFEST}")),
    }
}

/// A target's layers, composed: each path has one provider, and settings are decided.
#[derive(Debug)]
pub struct Resolved {
    /// In `config.toml` order.
    layers: Vec<Layer>,
    /// Each path's provider.
    providers: BTreeMap<RelPath, Provided>,
    /// The settings in force.
    settings: Settings,
    /// Drivers the layers suggest.
    suggestions: Vec<Suggestion>,
    /// The answer to every declared variable.
    answers: BTreeMap<VarName, String>,
}

impl Resolved {
    /// The layers, in `config.toml` order.
    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// The settings in force.
    #[must_use]
    pub const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Merge drivers the layers suggest that the target has not configured.
    #[must_use]
    pub fn suggestions(&self) -> &[Suggestion] {
        &self.suggestions
    }

    /// The answer to every declared variable.
    #[must_use]
    pub const fn answers(&self) -> &BTreeMap<VarName, String> {
        &self.answers
    }

    /// The name of the layer that provides `path`.
    #[must_use]
    pub fn provider(&self, path: &RelPath) -> Option<&str> {
        let provided = self.providers.get(path)?;
        Some(&self.layers.get(provided.layer)?.meta.name)
    }

    /// Every provided path, with its bytes.
    pub(crate) fn provided(&self) -> impl Iterator<Item = (&RelPath, Provided, &[u8])> {
        self.providers.iter().filter_map(|(path, &provided)| {
            Some((path, provided, self.bytes(provided.layer, path)?))
        })
    }

    /// The bytes `want` names at `path`.
    pub(crate) fn payload(&self, want: &Want, path: &RelPath) -> Result<&[u8]> {
        self.bytes(want.layer, path).ok_or_else(|| {
            let profile = self
                .layers
                .get(want.layer)
                .map_or_else(String::new, |layer| layer.meta.name.clone());
            ProfileError::MissingPayload { profile, path: path.clone() }.into()
        })
    }

    /// The bytes layer `index` provides at `path`.
    pub(crate) fn bytes(&self, index: usize, path: &RelPath) -> Option<&[u8]> {
        self.layers.get(index)?.payload.get(path)
    }
}

/// Resolves `target`'s layers into one profile, the target's overrides and settings on top.
///
/// Each layer is read at its locked commit unless `refresh` moves it; then the layers are
/// composed, their variables answered, and their templates rendered.
///
/// # Errors
/// - [`Error::Source`](crate::Error::Source), a source cannot be read.
/// - [`Error::Profile`](crate::Error::Profile), a source is not a valid profile, or the layers
///   collide on a path or a setting the target does not decide.
/// - [`Error::Target`](crate::Error::Target), an override or `refresh` names nothing.
/// - [`VarError::Unanswered`](crate::VarError::Unanswered), a variable needs an answer; answer it
///   with [`Target::answer`] and resolve again.
pub fn resolve(target: &Target, cache: &Cache, refresh: Refresh<'_>) -> Result<Resolved> {
    let config = target.config();
    let layers = load(target, cache, refresh)?;
    let providers = providers(&layers, &config.files)?;
    let answers = vars::answers(&layers, target)?;
    let layers = vars::render(layers, &answers)?;
    let settings = Settings::compose(&layers, &config.merge)?;
    let suggestions = Settings::suggestions(&layers, &config.merge);
    Ok(Resolved { layers, providers, settings, suggestions, answers })
}

/// Every layer of `target`, at its locked commit unless `refresh` moves it.
fn load(target: &Target, cache: &Cache, refresh: Refresh<'_>) -> Result<Vec<Layer>> {
    let mut layers: Vec<Layer> = Vec::new();
    let mut refreshed = false;
    for (index, source) in target.config().layers.iter().enumerate() {
        let locked = target.locked(index, source);
        let pin = locked.and_then(|locked| locked.rev.as_ref());
        let moves_all = matches!(refresh, Refresh::All);
        let mut layer = Layer::load(source, target.root(), pin.filter(|_| !moves_all), cache)?;
        if let Refresh::Layer(name) = refresh
            && layer.meta.name == name
        {
            refreshed = true;
            if pin.is_some() {
                layer = Layer::load(source, target.root(), None, cache)?;
            }
        }
        layer.locked = locked.map(|locked| locked.digest);
        layers.push(layer);
    }
    if let Refresh::Layer(name) = refresh
        && !refreshed
    {
        let names = layers.iter().map(|layer| layer.meta.name.clone()).collect();
        return Err(TargetError::NoSuchLayer { name: name.into(), layers: names }.into());
    }
    Ok(layers)
}

/// The one layer that provides each path, with `overrides` applied.
fn providers(
    layers: &[Layer], overrides: &BTreeMap<RelPath, Override>,
) -> Result<BTreeMap<RelPath, Provided>> {
    let mut owners: BTreeMap<&RelPath, Vec<usize>> = BTreeMap::new();
    for (index, layer) in layers.iter().enumerate() {
        for path in layer.files.keys() {
            owners.entry(path).or_default().push(index);
        }
    }
    if let Some(path) = overrides.keys().find(|path| !owners.contains_key(path)) {
        let provided = owners.keys().map(|&path| path.clone()).collect();
        return Err(TargetError::StaleOverride { path: path.clone(), provided }.into());
    }
    let name =
        |index: usize| layers.get(index).map_or_else(String::new, |layer| layer.meta.name.clone());
    let mut providers = BTreeMap::new();
    let mut folds = BTreeMap::<String, &RelPath>::new();
    for (path, candidates) in owners {
        let over = overrides.get(path);
        let from = over.and_then(|over| over.from.as_deref());
        let index = match (from, candidates.as_slice()) {
            (Some(from), _) => {
                candidates.iter().copied().find(|&i| name(i) == from).ok_or_else(|| {
                    let layers = candidates.iter().map(|&i| name(i)).collect();
                    ProfileError::NotProvided { path: path.clone(), from: from.into(), layers }
                })?
            },
            (None, &[only]) => only,
            (None, _) => {
                let layers = candidates.iter().map(|&i| name(i)).collect();
                return Err(ProfileError::Collision { path: path.clone(), layers }.into());
            },
        };
        let spec =
            layers.get(index).and_then(|layer| layer.files.get(path)).copied().unwrap_or_default();
        let provided = Provided {
            layer: index,
            policy: over.and_then(|o| o.policy).or(spec.policy).unwrap_or_default(),
            validate: over
                .and_then(|o| o.validate)
                .or(spec.validate)
                .unwrap_or_else(|| Format::of(path)),
        };
        providers.insert(path.clone(), provided);
        if let Some(other) = folds.insert(path.fold(), path)
            && other != path
        {
            return Err(
                ProfileError::FoldCollision { first: other.clone(), second: path.clone() }.into()
            );
        }
    }
    Ok(providers)
}
