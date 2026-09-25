//! Resolving a target's layers into one [`Resolved`] profile: fetched, composed, answered.

use alloc::collections::{BTreeMap, BTreeSet};
use core::slice;

use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use semver::Version;
use serde::Serialize;

use crate::digest::Digest;
use crate::errors::{ProfileError, Result, TargetError};
use crate::format::Format;
pub use crate::merge::Driver;
use crate::part::{Key, Scope, Shape, Slot, Written, display, overlap};
use crate::path::RelPath;
use crate::profile::{FileSpec, MANIFEST, Manifest, MergeSpec, Meta, PAYLOAD, Policy, Requirement};
pub use crate::settings::{Settings, Suggestion};
use crate::source::{Cache, Oid, Source};
use crate::survey::Entry;
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
    /// The profile that requires this one, for a layer not configured by the target.
    required_by: Option<String>,
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

    /// The profile that requires this layer; `None` for a layer the target configures.
    #[must_use]
    pub fn required_by(&self) -> Option<&str> {
        self.required_by.as_deref()
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

    /// Where `file` in its profile is, as users know it.
    pub(crate) fn label(&self, file: &str) -> String {
        label(&self.source, file)
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
        let Some(raw) = head.get(&manifest_path) else {
            let profiles = reader.profiles(source);
            return Err(ProfileError::NotAProfile { location: source.to_string(), profiles }.into());
        };
        let manifest: Manifest = from_toml(raw, &label(source, MANIFEST))?;
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
            required_by: None,
        })
    }
}

/// What a layer provides at one slot, overrides applied.
#[derive(Clone, Debug)]
pub(crate) struct Provided {
    /// Index into the layers.
    pub layer: usize,
    /// How it is managed.
    pub policy: Policy,
    /// How a merge of its file is checked.
    pub validate: Format,
    /// Whether the file is written executable.
    pub executable: bool,
    /// Its part of the file; `None` when it provides the whole file.
    pub portion: Option<Portion>,
}

/// One layer's part of a file.
#[derive(Clone, Debug)]
pub(crate) struct Portion {
    /// How much of the file it is.
    pub scope: Scope,
    /// How it is found, read and written.
    pub shape: Shape,
    /// What the profile puts there, templates rendered, as [`Shape::read`] reads it.
    pub content: Vec<u8>,
    /// For keys, the payload's leaves as it writes them, which new keys follow.
    pub payload: Written,
    /// The payload file itself, templates rendered.
    pub text: Vec<u8>,
}

impl Portion {
    /// `file` with the part holding `content` over `keys`, as [`Shape::splice`] writes it.
    ///
    /// A file with nothing in it yet takes the profile's keys as the payload writes them, its
    /// comments and layout included.
    pub(crate) fn splice(
        &self, file: &[u8], content: &[u8], keys: &BTreeSet<Key>, label: &str,
    ) -> Result<Vec<u8>> {
        let keyed = matches!(self.shape, Shape::Keys(_));
        if keyed && content == self.content && file.trim_ascii().is_empty() {
            return Ok(self.text.clone());
        }
        self.shape.splice(file, content, keys, &self.payload, label)
    }
}

/// Where `source`'s manifest is, as a path users recognise.
fn label(source: &Source, file: &str) -> String {
    match source {
        Source::Dir(dir) => dir.join(file).into_string(),
        Source::Git { url, path, .. } => path
            .as_ref()
            .map_or_else(|| format!("{url}/{file}"), |path| format!("{url}/{path}/{file}")),
    }
}

/// A target's layers, composed: each file has one provider, or one per part, and settings are
/// decided.
#[derive(Debug)]
pub struct Resolved {
    /// In `config.toml` order.
    layers: Vec<Layer>,
    /// Each slot's provider.
    providers: BTreeMap<Slot, Provided>,
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

    /// The name of the layer that provides `entry`.
    #[must_use]
    pub fn provider(&self, entry: &Entry) -> Option<&str> {
        Some(&self.layers.get(entry.want?.layer)?.meta.name)
    }

    /// Every slot, its provider, and what it provides: a whole file's bytes, or a part's content.
    pub(crate) fn provided(&self) -> impl Iterator<Item = (&Slot, &Provided, &[u8])> {
        self.providers
            .iter()
            .filter_map(|(slot, provided)| Some((slot, provided, self.bytes(slot, provided)?)))
    }

    /// What the profile provides for `entry`: a whole file's bytes, or a part's content.
    pub(crate) fn payload(&self, entry: &Entry) -> Result<&[u8]> {
        let slot = entry.slot();
        let bytes = self.providers.get(&slot).and_then(|provided| self.bytes(&slot, provided));
        bytes.ok_or_else(|| {
            let profile = self.provider(entry).unwrap_or_default().to_owned();
            ProfileError::MissingPayload { profile, path: entry.path.clone() }.into()
        })
    }

    /// What `provided` gives `slot`.
    fn bytes<'a>(&'a self, slot: &Slot, provided: &'a Provided) -> Option<&'a [u8]> {
        match &provided.portion {
            Some(portion) => Some(&portion.content),
            None => self.layers.get(provided.layer)?.payload.get(&slot.path),
        }
    }

    /// `entry`'s part as the profile provides it; `None` for a whole file.
    pub(crate) fn portion(&self, entry: &Entry) -> Option<&Portion> {
        self.providers.get(&entry.slot())?.portion.as_ref()
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
    let mut providers = providers(&layers, &config.files)?;
    let answers = vars::answers(&layers, target)?;
    let layers = vars::render(layers, &answers)?;
    portions(&layers, &mut providers)?;
    let settings = Settings::compose(&layers, &config.merge)?;
    let suggestions = Settings::suggestions(&layers, &config.merge);
    Ok(Resolved { layers, providers, settings, suggestions, answers })
}

/// How deep requirements may nest; a longer chain is a mistake, not a design.
const DEPTH: usize = 16;

/// Every layer of `target`, each after the profiles it requires, at its locked commit unless
/// `refresh` moves it.
fn load(target: &Target, cache: &Cache, refresh: Refresh<'_>) -> Result<Vec<Layer>> {
    let mut expansion = Expansion { target, cache, layers: Vec::new(), chain: Vec::new() };
    let mut refreshed = false;
    for source in &target.config().layers {
        let moves = matches!(refresh, Refresh::All);
        let mut layer = expansion.load(source, None, moves)?;
        let named = matches!(refresh, Refresh::Layer(name) if layer.meta.name == name);
        if named {
            refreshed = true;
            if layer.rev.is_some() {
                layer = expansion.load(source, None, true)?;
            }
        }
        expansion.expand(layer, moves || named, 0)?;
    }
    let layers = expansion.layers;
    for (index, layer) in layers.iter().enumerate() {
        let twin = layers
            .iter()
            .skip(index.saturating_add(1))
            .find(|other| other.meta.name == layer.meta.name);
        if let Some(twin) = twin {
            let (first, second) = (layer.source.to_string(), twin.source.to_string());
            return Err(
                ProfileError::SameName { name: layer.meta.name.clone(), first, second }.into()
            );
        }
    }
    if let Refresh::Layer(name) = refresh
        && !refreshed
    {
        if let Some(by) = layers.iter().find(|l| l.meta.name == name).and_then(Layer::required_by) {
            return Err(TargetError::Required { name: name.into(), by: by.to_owned() }.into());
        }
        let names = layers
            .iter()
            .filter(|layer| layer.required_by.is_none())
            .map(|layer| layer.meta.name.clone())
            .collect();
        return Err(TargetError::NoSuchLayer { name: name.into(), layers: names }.into());
    }
    Ok(layers)
}

/// Layers being expanded: each configured layer, after what it requires.
struct Expansion<'a> {
    /// Whose layers.
    target: &'a Target,
    /// Where git sources are fetched.
    cache: &'a Cache,
    /// Expanded so far, in order.
    layers: Vec<Layer>,
    /// The sources being expanded, outermost first; one met again is a cycle.
    chain: Vec<Source>,
}

impl Expansion<'_> {
    /// The layer at `source`: at `pin` if given, else at its locked commit, or at what its ref
    /// names now when it `moves`.
    fn load(&self, source: &Source, pin: Option<&Oid>, moves: bool) -> Result<Layer> {
        let locked = self.target.locked(source);
        let pin = pin.or_else(|| locked.and_then(|locked| locked.rev.as_ref()).filter(|_| !moves));
        let mut layer = Layer::load(source, self.target.root(), pin, self.cache)?;
        layer.locked = locked.map(|locked| locked.digest);
        Ok(layer)
    }

    /// Adds `layer`, after everything it requires; what it requires by git `moves` with it.
    fn expand(&mut self, layer: Layer, moves: bool, depth: usize) -> Result<()> {
        if self.layers.iter().any(|expanded| expanded.source == layer.source) {
            return Ok(());
        }
        if let Some(twin) = self.layers.iter().find(|expanded| expanded.source.twin(&layer.source))
        {
            let (first, second) = (twin.source.to_string(), layer.source.to_string());
            return Err(ProfileError::Diverged { first, second }.into());
        }
        self.chain.push(layer.source.clone());
        if depth >= DEPTH {
            return Err(ProfileError::TooDeep { chain: names(&self.chain) }.into());
        }
        for requirement in &layer.meta.requires {
            let (source, pin) = match requirement {
                Requirement::Sibling(relative) => sibling(&layer, relative)?,
                Requirement::Git(source) => (source.clone(), None),
            };
            if self.chain.contains(&source) {
                let mut chain = names(&self.chain);
                chain.push(source.to_string());
                return Err(ProfileError::Cycle { chain }.into());
            }
            if self.layers.iter().any(|expanded| expanded.source == source) {
                continue;
            }
            let mut required = self.load(&source, pin.as_ref(), moves)?;
            required.required_by = Some(layer.meta.name.clone());
            self.expand(required, moves, depth.saturating_add(1))?;
        }
        self.chain.pop();
        self.layers.push(layer);
        Ok(())
    }
}

/// Each of `sources`, as users know it.
fn names(sources: &[Source]) -> Vec<String> {
    sources.iter().map(ToString::to_string).collect()
}

/// The source of the profile at `relative` beside `requirer`, and the commit it is pinned to: the
/// requirer's own, for a git source.
fn sibling(requirer: &Layer, relative: &str) -> Result<(Source, Option<Oid>)> {
    let escapes =
        || ProfileError::Escapes { profile: requirer.meta.name.clone(), path: relative.to_owned() };
    match &requirer.source {
        Source::Dir(dir) => {
            Ok((Source::Dir(lexical(&dir.join(relative)).ok_or_else(escapes)?), None))
        },
        Source::Git { url, at, path } => {
            let base = Utf8Path::new(path.as_ref().map_or("", RelPath::as_str));
            let joined = lexical(&base.join(relative)).ok_or_else(escapes)?;
            if joined.starts_with("..") {
                return Err(escapes().into());
            }
            let path = match joined.as_str() {
                "" | "." => None,
                inside => Some(RelPath::new(inside)?),
            };
            let source = Source::Git { url: url.clone(), at: at.clone(), path };
            Ok((source, requirer.rev.clone()))
        },
    }
}

/// `path` with `.` dropped and each `..` taking back the directory before it, by name alone;
/// `None` for an absolute path that climbs above its root.
fn lexical(path: &Utf8Path) -> Option<Utf8PathBuf> {
    let mut out: Vec<Utf8Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Utf8Component::CurDir => {},
            Utf8Component::ParentDir => match out.last() {
                Some(Utf8Component::Normal(_)) => {
                    out.pop();
                },
                Some(Utf8Component::RootDir | Utf8Component::Prefix(_)) => return None,
                Some(Utf8Component::ParentDir | Utf8Component::CurDir) | None => {
                    out.push(component);
                },
            },
            Utf8Component::Prefix(_) | Utf8Component::RootDir | Utf8Component::Normal(_) => {
                out.push(component);
            },
        }
    }
    Some(out.into_iter().collect())
}

/// The one layer that provides each file, or each layer's part of it, with `overrides` applied.
///
/// A target's `from` narrows a file to one layer, whatever the scopes; otherwise every layer's
/// part is taken, so long as all own it in one scope, and one layer at most owns it whole.
fn providers(
    layers: &[Layer], overrides: &BTreeMap<RelPath, Override>,
) -> Result<BTreeMap<Slot, Provided>> {
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
    for (path, mut candidates) in owners {
        let over = overrides.get(path);
        let spec = |index: usize| layers.get(index).and_then(|layer| layer.files.get(path));
        let scope = |index: usize| spec(index).map_or(Scope::File, |spec| spec.scope);
        if let Some(from) = over.and_then(|over| over.from.as_deref()) {
            let picked =
                candidates.iter().copied().find(|&i| name(i) == from).ok_or_else(|| {
                    let layers = candidates.iter().map(|&i| name(i)).collect();
                    ProfileError::NotProvided { path: path.clone(), from: from.into(), layers }
                })?;
            candidates = vec![picked];
        }
        share(path, &candidates, name, scope)?;
        for index in candidates {
            let spec = spec(index).cloned().unwrap_or_default();
            let owner = name(index);
            let validate = over.and_then(|o| o.validate).or(spec.validate);
            let format = validate.unwrap_or_else(|| Format::of(path));
            let shape = Shape::of(spec.scope, path, format, spec.comment, &owner)?;
            let portion = shape.map(|shape| Portion {
                scope: spec.scope,
                shape,
                content: Vec::new(),
                payload: Written::default(),
                text: Vec::new(),
            });
            let slot = Slot { path: path.clone(), part: portion.as_ref().map(|_| owner) };
            let provided = Provided {
                layer: index,
                policy: over.and_then(|o| o.policy).or(spec.policy).unwrap_or_default(),
                validate: format,
                executable: spec.executable,
                portion,
            };
            providers.insert(slot, provided);
        }
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

/// Checks that the layers in `candidates` may share `path`: one of them provides it, or all own
/// parts of it in one scope.
///
/// # Errors
/// - [`ProfileError::Collision`], several own it whole.
/// - [`ProfileError::Scopes`], they own it in different scopes.
fn share(
    path: &RelPath, candidates: &[usize], name: impl Fn(usize) -> String,
    scope: impl Fn(usize) -> Scope,
) -> Result<()> {
    let scopes: Vec<Scope> = candidates.iter().map(|&i| scope(i)).collect();
    match scopes.as_slice() {
        [_] => Ok(()),
        [first, rest @ ..] if rest.iter().all(|s| s == first) && *first != Scope::File => Ok(()),
        _ if scopes.iter().all(|&s| s == Scope::File) => {
            let layers = candidates.iter().map(|&i| name(i)).collect();
            Err(ProfileError::Collision { path: path.clone(), layers }.into())
        },
        _ => {
            let layers = candidates.iter().map(|&i| (name(i), scope(i))).collect();
            Err(ProfileError::Scopes { path: path.clone(), layers }.into())
        },
    }
}

/// Reads each part's content out of its layer's rendered payload, and checks that no two layers
/// own overlapping keys of one file.
fn portions(layers: &[Layer], providers: &mut BTreeMap<Slot, Provided>) -> Result<()> {
    let mut owned: Vec<(&RelPath, usize, Key)> = Vec::new();
    for (slot, provided) in providers.iter_mut() {
        let Some(portion) = &mut provided.portion else {
            continue;
        };
        let Some(layer) = layers.get(provided.layer) else {
            continue;
        };
        let bytes = layer.payload.get(&slot.path).ok_or_else(|| ProfileError::MissingPayload {
            profile: layer.meta.name.clone(),
            path: slot.path.clone(),
        })?;
        let label = layer.label(&format!("{PAYLOAD}/{}", slot.path));
        (portion.content, portion.payload) = portion.shape.read(bytes, &label)?;
        portion.text = bytes.to_vec();
        let keys = portion.shape.keys(&portion.content);
        owned.extend(keys.into_iter().map(|key| (&slot.path, provided.layer, key)));
    }
    owned.sort_by_key(|&(path, layer, _)| (path, layer));
    let name = |index: usize| layers.get(index).map_or_else(String::new, |l| l.meta.name.clone());
    for (at, (path, layer, key)) in owned.iter().enumerate() {
        let earlier = owned.get(..at).unwrap_or_default().iter().rev();
        let clash = earlier
            .take_while(|(other, ..)| other == path)
            .find(|(_, other, theirs)| other != layer && overlap(key, theirs));
        if let Some((_, other, theirs)) = clash {
            let key = display(if key.len() <= theirs.len() { key } else { theirs });
            let (first, second) = (name(*other), name(*layer));
            return Err(ProfileError::Overlap { path: (*path).clone(), key, first, second }.into());
        }
    }
    Ok(())
}
