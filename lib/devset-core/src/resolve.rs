//! Resolving a target's layers into one [`Resolved`] profile: read, composed, answered, rendered.
//!
//! In order, each step reading only what the steps before it decided:
//!
//! 1. **Graph.** The configured layers and everything they require, by name, their features
//!    unified, in the order they apply.
//! 2. **Variables.** Every variable a layer declares, answered.
//! 3. **Paths.** Each entry's path, and the paths and globs its gates name, rendered.
//! 4. **Gates.** Entries whose `features`, `profiles` or `vars` do not hold are off; `exists` and
//!    scaffolds wait for [`survey`](crate::survey()), which sees the disk.
//! 5. **Providers.** One layer per file, or one per part, the target's overrides applied.
//! 6. **Content.** Templates rendered with the answers and the graph; parts read out of it.

use alloc::collections::{BTreeMap, BTreeSet};
use core::{mem, slice};

use semver::Version;
use serde::Serialize;

use crate::collection::{Index, label};
use crate::digest::Digest;
use crate::errors::{ProfileError, Result, TargetError};
use crate::format::Format;
use crate::gate::{self, Pattern, Reason};
use crate::graph::{Configured, Graph, Load, Node};
pub use crate::graph::{Enabler, Warning};
pub use crate::merge::Driver;
use crate::name::{FeatureName, ProfileName, ScaffoldName, SourceName};
use crate::part::{Key, Scope, Shape, Slot, Written, display, overlap};
use crate::path::RelPath;
use crate::profile::{
    FileSpec, MANIFEST, Manifest, MergeSpec, Meta, PAYLOAD, Policy, ScaffoldSpec,
};
pub use crate::settings::{Settings, Suggestion};
use crate::source::{Cache, Oid, Reader, Source};
use crate::survey::Entry;
use crate::target::{Override, Target, from_toml};
use crate::tree::Tree;
pub use crate::vars::Question;
use crate::vars::{self, Context, VarName, VarSpec};

/// Which sources [`resolve`] may move off their locked commit.
#[derive(Clone, Copy, Debug)]
pub enum Refresh<'a> {
    /// None: every source at its locked commit.
    None,
    /// Every source, to what its ref names now.
    All,
    /// The source with this name, or the one the layer with this name comes from.
    Only(&'a str),
}

/// Whether a layer's content is what the lock recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Applied {
    /// It is.
    Current,
    /// Its source moved, its local directory changed, or an answer or the graph did, since it
    /// was applied.
    Changed,
    /// The layer has never been applied.
    Never,
}

/// An active profile, read from its source at one revision, and rendered for the target.
#[derive(Debug)]
pub struct Layer {
    /// Where it came from.
    source: Source,
    /// The name the target gives its source; `None` for a source a profile requires by `git`.
    source_name: Option<SourceName>,
    /// Its directory in the source.
    dir: String,
    /// Its `[profile]` table.
    meta: Meta,
    /// Its `[merge]` table.
    merge: MergeSpec,
    /// Its variables.
    vars: BTreeMap<VarName, VarSpec>,
    /// Every feature it declares.
    declared: Vec<FeatureName>,
    /// Every feature that is on, with who turned it on.
    features: BTreeMap<FeatureName, BTreeSet<Enabler>>,
    /// The profiles that require it.
    required_by: Vec<ProfileName>,
    /// Whether the target configures it as a layer.
    configured: bool,
    /// The commit, for a versioned source.
    rev: Option<Oid>,
    /// Digest of its manifest as read.
    manifest: Digest,
    /// Digest of manifest, rendered payload and features together.
    digest: Digest,
    /// The digest the lock recorded for it.
    locked: Option<Digest>,
    /// Its `[files]` and `[scaffolds]`, as written, until their paths are rendered.
    specs: Specs,
    /// Its `[scaffolds]`: each group's sentinels, rendered.
    scaffolds: BTreeMap<ScaffoldName, Vec<Pattern>>,
    /// Its files, by rendered path.
    files: BTreeMap<RelPath, LayerFile>,
    /// Its payload as read, by path as written, until rendered.
    written: Tree,
    /// Each file's bytes, by rendered path, templates rendered; of the files that apply.
    payload: Tree,
    /// Each part's starter, by the rendered path of its file, templates rendered.
    starters: Tree,
}

/// A profile's `[files]` and `[scaffolds]`, as written.
pub(crate) type Specs = (BTreeMap<RelPath, FileSpec>, BTreeMap<ScaffoldName, ScaffoldSpec>);

/// One of a layer's files: its entry, where its payload is, and whether its gates let it apply.
#[derive(Clone, Debug)]
pub struct LayerFile {
    /// Its path as the profile wrote it: under `files/`, and before variables are answered.
    pub written: RelPath,
    /// Its entry.
    pub spec: FileSpec,
    /// The paths and globs of its `when.exists`, rendered.
    pub(crate) exists: Vec<Pattern>,
    /// Why it does not apply, when its `features`, `profiles` or `vars` say so.
    pub gated: Option<Reason>,
}

impl LayerFile {
    /// The paths and globs of its `when.exists`, rendered.
    pub fn exists(&self) -> impl Iterator<Item = &str> {
        self.exists.iter().map(Pattern::as_str)
    }
}

impl Layer {
    /// Where it came from.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// The name the target gives its source; `None` for a source a profile requires by `git`.
    #[must_use]
    pub const fn source_name(&self) -> Option<&SourceName> {
        self.source_name.as_ref()
    }

    /// Its profile's name, which names the layer.
    #[must_use]
    pub const fn name(&self) -> &ProfileName {
        &self.meta.name
    }

    /// `source/profile`, as the target names it; the profile's name for a source a profile
    /// requires by `git`.
    #[must_use]
    pub fn qualified(&self) -> String {
        let name = &self.meta.name;
        self.source_name
            .as_ref()
            .map_or_else(|| name.to_string(), |source| format!("{source}/{name}"))
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

    /// Digest of manifest, rendered payload and features together.
    #[must_use]
    pub const fn digest(&self) -> Digest {
        self.digest
    }

    /// Every feature it declares, in order.
    #[must_use]
    pub fn declared(&self) -> &[FeatureName] {
        &self.declared
    }

    /// Every feature that is on, with who turned it on.
    #[must_use]
    pub const fn features(&self) -> &BTreeMap<FeatureName, BTreeSet<Enabler>> {
        &self.features
    }

    /// The profiles that require it.
    #[must_use]
    pub fn required_by(&self) -> &[ProfileName] {
        &self.required_by
    }

    /// Whether the target configures it as a layer, rather than a profile requiring it.
    #[must_use]
    pub const fn configured(&self) -> bool {
        self.configured
    }

    /// Its files, by rendered path: those that apply, and those whose gates say they do not.
    #[must_use]
    pub const fn files(&self) -> &BTreeMap<RelPath, LayerFile> {
        &self.files
    }

    /// Its scaffold groups, each with its sentinels.
    pub fn scaffolds(&self) -> impl Iterator<Item = (&ScaffoldName, impl Iterator<Item = &str>)> {
        self.scaffolds.iter().map(|(name, unless)| (name, unless.iter().map(Pattern::as_str)))
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

    /// Its scaffold groups' sentinels, rendered.
    pub(crate) const fn sentinels(&self) -> &BTreeMap<ScaffoldName, Vec<Pattern>> {
        &self.scaffolds
    }

    /// Where `file`, relative to its profile, is, as users know it.
    pub(crate) fn label(&self, file: &str) -> String {
        if self.dir.is_empty() {
            label(&self.source, file)
        } else {
            label(&self.source, &format!("{}/{file}", self.dir))
        }
    }

    /// Its `[files]` and `[scaffolds]` as written, taken for their paths to be rendered.
    pub(crate) fn take_specs(&mut self) -> Specs {
        mem::take(&mut self.specs)
    }

    /// Sets its files, and its scaffolds' sentinels, rendered.
    pub(crate) fn set_files(
        &mut self, files: BTreeMap<RelPath, LayerFile>,
        scaffolds: BTreeMap<ScaffoldName, Vec<Pattern>>,
    ) {
        self.files = files;
        self.scaffolds = scaffolds;
    }

    /// Its files, to gate.
    pub(crate) const fn files_mut(&mut self) -> &mut BTreeMap<RelPath, LayerFile> {
        &mut self.files
    }

    /// Its payload as read, by path as written.
    pub(crate) const fn written(&self) -> &Tree {
        &self.written
    }

    /// Sets its rendered payload and starters, and digests them.
    pub(crate) fn set_payload(&mut self, payload: Tree, starters: Tree) {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.manifest.as_bytes());
        hasher.update(payload.digest().as_bytes());
        hasher.update(starters.digest().as_bytes());
        for feature in self.features.keys() {
            hasher.update(feature.as_str().as_bytes());
            hasher.update(&[0]);
        }
        self.digest = Digest::new(hasher.finalize());
        self.payload = payload;
        self.starters = starters;
        self.written = Tree::default();
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
    /// Whether its starter, if it has one, is the file's.
    pub starts: bool,
    /// The file the target's starts from when it is absent, templates rendered.
    pub starter: Option<Vec<u8>>,
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

/// A target's layers, composed: each file has one provider, or one per part, and settings are
/// decided.
#[derive(Debug)]
pub struct Resolved {
    /// In the order they apply: each requirement before its requirers.
    layers: Vec<Layer>,
    /// Each slot's provider.
    providers: BTreeMap<Slot, Provided>,
    /// The settings in force.
    settings: Settings,
    /// Drivers the layers suggest.
    suggestions: Vec<Suggestion>,
    /// The answer to every declared variable.
    answers: BTreeMap<VarName, String>,
    /// What the graph settled that the target may not expect.
    warnings: Vec<Warning>,
}

impl Resolved {
    /// The layers, each requirement before its requirers.
    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// The layer whose profile is `name`.
    #[must_use]
    pub fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.meta.name == *name)
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

    /// What the graph settled that the target may not expect.
    #[must_use]
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// The name of the layer that provides `entry`.
    #[must_use]
    pub fn provider(&self, entry: &Entry) -> Option<&ProfileName> {
        Some(&self.layers.get(entry.want?.layer)?.meta.name)
    }

    /// The layer file behind `entry`, while a layer provides it.
    #[must_use]
    pub fn file(&self, entry: &Entry) -> Option<&LayerFile> {
        self.layers.get(entry.want?.layer)?.files.get(&entry.path)
    }

    /// Every slot, its provider, and what it provides: a whole file's bytes, or a part's content.
    pub(crate) fn provided(&self) -> impl Iterator<Item = (&Slot, &Provided, &[u8])> {
        self.providers
            .iter()
            .filter_map(|(slot, provided)| Some((slot, provided, self.bytes(slot, provided)?)))
    }

    /// What the profile provides for `entry`: a whole file's bytes, or a part's content.
    ///
    /// # Errors
    /// [`TargetError::NotManaged`], no layer provides it: only an entry with a
    /// [`want`](Entry::want) has a payload.
    pub(crate) fn payload(&self, entry: &Entry) -> Result<&[u8]> {
        let slot = entry.slot();
        let bytes = self.providers.get(&slot).and_then(|provided| self.bytes(&slot, provided));
        bytes.ok_or_else(|| {
            TargetError::NotManaged { path: entry.path.to_string(), managed: Vec::new() }.into()
        })
    }

    /// What `provided` gives `slot`.
    fn bytes<'a>(&'a self, slot: &Slot, provided: &'a Provided) -> Option<&'a [u8]> {
        match &provided.portion {
            Some(portion) => Some(&portion.content),
            None => self.layers.get(provided.layer)?.payload.get(&slot.path),
        }
    }

    /// `entry`'s part as its profile provides it, or provided before its gates turned it off;
    /// `None` for a whole file.
    pub(crate) fn portion(&self, entry: &Entry) -> Option<&Portion> {
        self.providers.get(&entry.slot())?.portion.as_ref()
    }
}

/// Resolves `target`'s layers into one profile, the target's overrides and settings on top.
///
/// Each source is read at its locked commit unless `refresh` moves it; then the graph is built,
/// the variables answered, the paths rendered and gated, the layers composed, and their templates
/// rendered.
///
/// # Errors
/// - [`Error::Source`](crate::Error::Source), a source cannot be read.
/// - [`Error::Profile`](crate::Error::Profile), a profile is not valid or not found, a feature is
///   unknown, or the layers collide on a path or a setting the target does not decide.
/// - [`Error::Target`](crate::Error::Target), a layer names no source, or an override or `refresh`
///   names nothing.
/// - [`VarError::Unanswered`](crate::VarError::Unanswered), a variable needs an answer; answer it
///   with [`Target::answer`] and resolve again.
pub fn resolve(target: &Target, cache: &Cache, refresh: Refresh<'_>) -> Result<Resolved> {
    let moves = Moves::of(target, cache, refresh)?;
    let (mut layers, warnings) = layers(target, cache, &moves)?;
    let answers = vars::answers(&layers, target)?;
    let context = Context::of(target, &layers);
    vars::render_paths(&mut layers, &answers, &context)?;
    gate::statics(&mut layers, &answers);
    let config = target.config();
    let mut providers = providers(&layers, &config.files)?;
    vars::render(&mut layers, &answers, &context)?;
    portions(&layers, &mut providers)?;
    let settings = Settings::compose(&layers, &config.merge)?;
    let suggestions = Settings::suggestions(&layers, &config.merge);
    Ok(Resolved { layers, providers, settings, suggestions, answers, warnings })
}

/// The sources a resolution moves to what their refs name now.
enum Moves {
    /// None.
    None,
    /// Every one.
    All,
    /// These.
    Only(Vec<Source>),
}

impl Moves {
    /// What `refresh` moves in `target`: a source it names, or the source of a layer it names.
    fn of(target: &Target, cache: &Cache, refresh: Refresh<'_>) -> Result<Self> {
        let name = match refresh {
            Refresh::None => return Ok(Self::None),
            Refresh::All => return Ok(Self::All),
            Refresh::Only(name) => name,
        };
        let config = target.config();
        if let Some(source) = config.sources.iter().find(|(named, _)| named.as_str() == name) {
            return Ok(Self::Only(vec![source.1.clone()]));
        }
        let configured = config.layers.iter().find(|layer| layer.profile.profile == *name);
        if let Some(source) = configured.and_then(|layer| config.sources.get(&layer.profile.source))
        {
            return Ok(Self::Only(vec![source.clone()]));
        }
        let (layers, _) = layers(target, cache, &Self::None)?;
        if let Some(layer) = layers.iter().find(|layer| layer.meta.name == *name) {
            return Ok(Self::Only(vec![layer.source.clone()]));
        }
        let layers = layers.iter().map(|layer| layer.meta.name.to_string());
        let names = layers.chain(config.sources.keys().map(ToString::to_string)).collect();
        Err(TargetError::NoSuchLayer { name: name.to_owned(), names }.into())
    }

    /// Whether `source` moves.
    fn moves(&self, source: &Source) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Only(sources) => sources.contains(source),
        }
    }
}

/// Every active profile of `target`, read and in order, and what the graph warns of.
fn layers(target: &Target, cache: &Cache, moves: &Moves) -> Result<(Vec<Layer>, Vec<Warning>)> {
    let config = target.config();
    let configured = config
        .layers
        .iter()
        .map(|layer| {
            let source = config.sources.get(&layer.profile.source).ok_or_else(|| {
                let sources = config.sources.keys().cloned().collect();
                let name = layer.profile.source.clone();
                let naming = config.layers.iter().filter(|l| l.profile.source == name);
                let layers = naming.map(|l| l.profile.clone()).collect();
                TargetError::NoSuchSource { name, sources, layers }
            })?;
            Ok(Configured {
                source,
                name: &layer.profile.profile,
                features: &layer.features,
                defaults: layer.default_features,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut loader = Loader { target, cache, moves, opened: Vec::new() };
    let Graph { nodes, warnings } = Graph::build(&configured, &mut loader)?;
    let layers = nodes.into_iter().map(|node| loader.layer(node)).collect::<Result<_>>()?;
    Ok((layers, warnings))
}

/// Reads profiles for the graph: each source opened once, at one revision.
struct Loader<'a> {
    /// Whose layers.
    target: &'a Target,
    /// Where git sources are fetched.
    cache: &'a Cache,
    /// Which sources move off their locked commit.
    moves: &'a Moves,
    /// Every source opened so far.
    opened: Vec<Opened>,
}

/// A source, opened at its revision, and its profiles by name.
struct Opened {
    /// The source.
    source: Source,
    /// Its files.
    reader: Reader,
    /// Its profiles.
    index: Index,
}

/// A profile the graph read: where it is, and what it declares.
pub(crate) struct Loaded {
    /// Which opened source it is in.
    opened: usize,
    /// Its directory in the source.
    dir: String,
    /// Its manifest, parsed and checked.
    manifest: Manifest,
    /// Digest of its manifest as read.
    digest: Digest,
}

impl Load for Loader<'_> {
    type Profile = Loaded;

    fn load(&mut self, source: &Source, name: &ProfileName) -> Result<Loaded> {
        let opened = self.open(source)?;
        let Some(Opened { reader, index, .. }) = self.opened.get(opened) else {
            return Err(ProfileError::NoProfiles { location: source.to_string() }.into());
        };
        let dir = index.find(name, source)?.to_owned();
        let path = if dir.is_empty() {
            RelPath::new(MANIFEST)?
        } else {
            RelPath::new(&format!("{dir}/{MANIFEST}"))?
        };
        let read = reader.read("", slice::from_ref(&path))?;
        let raw = read
            .get(&path)
            .ok_or_else(|| ProfileError::NoProfiles { location: source.to_string() })?;
        let file = label(source, path.as_str());
        let manifest: Manifest = from_toml(raw, &file)?;
        if let Some(required) = &manifest.profile.devset
            && !Version::parse(crate::VERSION).is_ok_and(|version| required.matches(&version))
        {
            let (profile, requires) = (manifest.profile.name, required.to_string());
            return Err(ProfileError::Incompatible { profile, requires }.into());
        }
        manifest.check(&file)?;
        Ok(Loaded { opened, dir, manifest, digest: Digest::of(raw) })
    }

    fn manifest(profile: &Loaded) -> &Manifest {
        &profile.manifest
    }
}

impl Loader<'_> {
    /// The position of `source` among the opened: opened now, at its locked commit unless it
    /// moves, if it was not already.
    ///
    /// # Errors
    /// [`ProfileError::Diverged`], another ref of it is open: one source at two refs.
    fn open(&mut self, source: &Source) -> Result<usize> {
        if let Some(at) = self.opened.iter().position(|opened| opened.source == *source) {
            return Ok(at);
        }
        if let Some(twin) = self.opened.iter().find(|opened| opened.source.twin(source)) {
            let (first, second) = (twin.source.to_string(), source.to_string());
            return Err(ProfileError::Diverged { first, second }.into());
        }
        let pin = if self.moves.moves(source) { None } else { self.target.pinned(source) };
        let reader = source.open(self.target.root(), pin, self.cache)?;
        let index = Index::read(&reader, source)?;
        if index.names().next().is_none() {
            return Err(ProfileError::NoProfiles { location: source.to_string() }.into());
        }
        self.opened.push(Opened { source: source.clone(), reader, index });
        Ok(self.opened.len().saturating_sub(1))
    }

    /// The layer of `node`: its payload read, as written.
    fn layer(&self, node: Node<Loaded>) -> Result<Layer> {
        let Node { profile, name, source, features, required_by, configured, .. } = node;
        let Loaded { opened, dir, manifest, digest } = profile;
        let Some(Opened { reader, .. }) = self.opened.get(opened) else {
            return Err(ProfileError::NoProfiles { location: source.to_string() }.into());
        };
        let paths: Vec<RelPath> = manifest.payload_paths().into_iter().collect();
        let payload_dir =
            if dir.is_empty() { PAYLOAD.to_owned() } else { format!("{dir}/{PAYLOAD}") };
        let written = reader.read(&payload_dir, &paths)?;
        if let Some(missing) = paths.into_iter().find(|path| written.get(path).is_none()) {
            return Err(ProfileError::MissingPayload { profile: name, path: missing }.into());
        }
        let config = self.target.config();
        let source_name = config
            .sources
            .iter()
            .find(|(_, named)| **named == source)
            .map(|(name, _)| name.clone());
        let locked = self.target.locked(&source, &name);
        let Manifest { profile: meta, merge, vars, features: declared, files, scaffolds, .. } =
            manifest;
        Ok(Layer {
            rev: reader.rev().cloned(),
            source,
            source_name,
            dir,
            meta,
            merge,
            vars,
            declared: declared.declared.into_keys().collect(),
            features,
            required_by: required_by.into_iter().collect(),
            configured,
            manifest: digest,
            digest,
            locked,
            specs: (files, scaffolds),
            scaffolds: BTreeMap::new(),
            files: BTreeMap::new(),
            written,
            payload: Tree::default(),
            starters: Tree::default(),
        })
    }
}

/// The one layer that provides each file, or each layer's part of it, with `overrides` applied.
///
/// Every layer's part is taken, so long as all own it in one scope, beside at most one starter: a
/// whole `once` file, or a part's own. A target's `from` settles which layer provides the file:
/// the starter, beside every layer's part, or the one layer that has it, whatever the scopes. A
/// `from` naming a layer whose gates leave the file out stands aside.
fn providers(
    layers: &[Layer], overrides: &BTreeMap<RelPath, Override>,
) -> Result<BTreeMap<Slot, Provided>> {
    let mut owners: BTreeMap<&RelPath, Vec<usize>> = BTreeMap::new();
    for (index, layer) in layers.iter().enumerate() {
        for (path, file) in &layer.files {
            if file.gated.is_none() {
                owners.entry(path).or_default().push(index);
            }
        }
    }
    let listed = |path: &RelPath| layers.iter().any(|layer| layer.files.contains_key(path));
    if let Some(path) = overrides.keys().find(|path| !listed(path)) {
        let mut provided: Vec<RelPath> =
            layers.iter().flat_map(|layer| layer.files.keys().cloned()).collect();
        provided.sort();
        provided.dedup();
        return Err(TargetError::StaleOverride { path: path.clone(), provided }.into());
    }
    let mut providers = BTreeMap::new();
    let mut folds = BTreeMap::<String, &RelPath>::new();
    for (path, candidates) in owners {
        providers.extend(provide(layers, path, candidates, overrides.get(path))?);
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

/// The providers of `path`, which the layers at `candidates` list and `over` overrides.
fn provide(
    layers: &[Layer], path: &RelPath, mut candidates: Vec<usize>, over: Option<&Override>,
) -> Result<Vec<(Slot, Provided)>> {
    let name = |index: usize| layers.get(index).map(|layer| layer.meta.name.clone());
    let spec = |index: usize| {
        layers.get(index).and_then(|layer| layer.files.get(path)).map(|file| &file.spec)
    };
    let policy = |index: usize| {
        over.and_then(|o| o.policy)
            .unwrap_or_else(|| spec(index).map_or_else(Policy::default, FileSpec::policy))
    };
    let starts = |index: usize| {
        spec(index).is_some_and(|spec| match spec.scope {
            Scope::File => policy(index) == Policy::Once,
            Scope::Keys | Scope::Block => spec.starter.is_some(),
        })
    };
    let mut starter = None;
    if let Some(from) = over.and_then(|over| over.from.as_ref()) {
        let listing: Vec<usize> = (0..layers.len())
            .filter(|&i| layers.get(i).is_some_and(|l| l.files.contains_key(path)))
            .collect();
        if !listing.iter().any(|&i| name(i).as_ref() == Some(from)) {
            let layers = listing.iter().filter_map(|&i| name(i)).collect();
            return Err(ProfileError::NotProvided {
                path: path.clone(),
                from: from.clone(),
                layers,
            }
            .into());
        }
        // A `from` whose layer's gates leave the file out stands aside.
        if let Some(picked) = candidates.iter().copied().find(|&i| name(i).as_ref() == Some(from)) {
            if starts(picked) {
                let part = |i: usize| spec(i).is_some_and(|spec| spec.scope != Scope::File);
                candidates.retain(|&i| i == picked || part(i));
                starter = Some(picked);
            } else {
                candidates = vec![picked];
            }
        }
    }
    let starts = |index: usize| starter.map_or_else(|| starts(index), |picked| picked == index);
    let claims: Vec<Claim> = candidates
        .iter()
        .filter_map(|&i| {
            Some(Claim {
                layer: name(i)?,
                scope: spec(i)?.scope,
                policy: policy(i),
                starts: starts(i),
            })
        })
        .collect();
    share(path, &claims)?;
    let mut provided = Vec::with_capacity(candidates.len());
    for index in candidates {
        let (Some(spec), Some(owner)) = (spec(index), name(index)) else { continue };
        let validate = over.and_then(|o| o.validate).or(spec.validate);
        let format = validate.unwrap_or_else(|| Format::of(path));
        let shape = Shape::of(spec.scope, path, format, spec.comment, owner.as_str())?;
        let portion = shape.map(|shape| Portion {
            scope: spec.scope,
            shape,
            content: Vec::new(),
            payload: Written::default(),
            text: Vec::new(),
            starts: starts(index),
            starter: None,
        });
        let slot = Slot { path: path.clone(), part: portion.as_ref().map(|_| owner.to_string()) };
        let policy = policy(index);
        provided.push((
            slot,
            Provided {
                layer: index,
                policy,
                validate: format,
                executable: spec.executable,
                portion,
            },
        ));
    }
    Ok(provided)
}

/// How one layer owns a file, for [`share`].
struct Claim {
    /// The layer.
    layer: ProfileName,
    /// How much of the file it owns.
    scope: Scope,
    /// How it manages it.
    policy: Policy,
    /// Whether it starts the file when the target has none.
    starts: bool,
}

/// Checks that the layers `claims` names may share `path`: one of them provides it, or all own
/// parts of it in one scope, beside at most one starter.
///
/// # Errors
/// - [`ProfileError::Collision`], several own it whole.
/// - [`ProfileError::Starters`], several would start it.
/// - [`ProfileError::Scopes`], they own it in different scopes, or one owns it whole and is not
///   `once`.
fn share(path: &RelPath, claims: &[Claim]) -> Result<()> {
    if claims.len() <= 1 {
        return Ok(());
    }
    let named = |keep: &dyn Fn(&Claim) -> bool| -> Vec<ProfileName> {
        claims.iter().filter(|claim| keep(claim)).map(|claim| claim.layer.clone()).collect()
    };
    let wholes = named(&|claim| claim.scope == Scope::File);
    if wholes.len() > 1 {
        return Err(ProfileError::Collision { path: path.clone(), layers: wholes }.into());
    }
    let mut parts =
        claims.iter().filter(|claim| claim.scope != Scope::File).map(|claim| claim.scope);
    let one_scope = parts.next().is_none_or(|first| parts.all(|scope| scope == first));
    let owned =
        claims.iter().any(|claim| claim.scope == Scope::File && claim.policy != Policy::Once);
    if !one_scope || owned {
        let layers = claims.iter().map(|claim| (claim.layer.clone(), claim.scope)).collect();
        return Err(ProfileError::Scopes { path: path.clone(), layers }.into());
    }
    let starters = named(&|claim| claim.starts);
    if starters.len() > 1 {
        return Err(ProfileError::Starters { path: path.clone(), layers: starters }.into());
    }
    Ok(())
}

/// Reads each part's content, and its starter, out of its layer's rendered payload, and checks
/// that no two layers own overlapping keys of one file.
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
        let written =
            layer.files.get(&slot.path).map_or(slot.path.as_str(), |file| file.written.as_str());
        let label = layer.label(&format!("{PAYLOAD}/{written}"));
        (portion.content, portion.payload) = portion.shape.read(bytes, &label)?;
        portion.text = bytes.to_vec();
        portion.starter =
            layer.starters.get(&slot.path).filter(|_| portion.starts).map(<[u8]>::to_vec);
        let keys = portion.shape.keys(&portion.content);
        owned.extend(keys.into_iter().map(|key| (&slot.path, provided.layer, key)));
    }
    owned.sort_by_key(|&(path, layer, _)| (path, layer));
    let name = |index: usize| layers.get(index).map(|l| l.meta.name.clone());
    for (at, (path, layer, key)) in owned.iter().enumerate() {
        let earlier = owned.get(..at).unwrap_or_default().iter().rev();
        let clash = earlier
            .take_while(|(other, ..)| other == path)
            .find(|(_, other, theirs)| other != layer && overlap(key, theirs));
        if let Some((_, other, theirs)) = clash
            && let (Some(first), Some(second)) = (name(*other), name(*layer))
        {
            let key = display(if key.len() <= theirs.len() { key } else { theirs });
            return Err(ProfileError::Overlap { path: (*path).clone(), key, first, second }.into());
        }
    }
    Ok(())
}
