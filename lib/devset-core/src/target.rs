//! The directory devset manages, and what its `.devset/` records.
//!
//! ```text
//! .devset/
//! ├── config.toml     the sources, the layers, and the target's word on settings (committed)
//! ├── lock.toml       each source's revision, each layer's digest and features (committed)
//! ├── answers.toml    answers to the layers' variables (committed)
//! ├── state.toml      what devset last wrote, by file and part, and each scaffold (committed)
//! ├── base/           those bytes, by digest, as merge bases (committed)
//! └── conflicts/      an unfinished update: merges awaiting resolution (ignored)
//!     └── .devset/    what it withheld, and what it would take to undo it
//! ```

use alloc::collections::{BTreeMap, BTreeSet};
use core::str;
use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use derive_more::Display;
use schemars::JsonSchema;
use serde::de::{self, DeserializeOwned};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, TableLike, TomlError, Value, ser};

use crate::digest::{Digest, Fingerprint};
use crate::errors::{ParseError, Result, TargetError};
use crate::graph::Enabler;
use crate::name::{FeatureName, ProfileName, ProfileRef, ScaffoldId, SourceName};
use crate::part::{Scope, Slot};
use crate::path::RelPath;
use crate::profile::{Format, MergeSpec, Policy};
use crate::source::{Oid, Source, SourceSpec};
use crate::vars::VarName;

/// devset's directory in a target.
pub(crate) const DIR: &str = ".devset";

/// Intent: the layers, in order.
pub(crate) const CONFIG: &str = "config.toml";

/// What each layer resolved to.
pub(crate) const LOCK: &str = "lock.toml";

/// What devset last recorded for each managed path.
pub(crate) const STATE: &str = "state.toml";

/// Conflicted merge results awaiting resolution, by path.
pub(crate) const CONFLICTS: &str = "conflicts";

/// In an unfinished update's directory, the lock that `on-conflict = "apply-none"` withheld.
pub(crate) const PENDING: &str = "pending.toml";

/// In an unfinished update's directory, what the update changed.
pub(crate) const UNDO: &str = "undo.toml";

/// In an unfinished update's directory, the bytes it replaced, by digest.
pub(crate) const UNDO_BLOBS: &str = "undo";

/// Recorded bases, named by digest.
pub(crate) const BASE: &str = "base";

/// The target's answers to the layers' variables.
pub(crate) const ANSWERS: &str = "answers.toml";

/// A directory devset manages.
#[derive(Debug)]
pub struct Target {
    /// The directory holding `.devset/`.
    root: Utf8PathBuf,
    /// `config.toml`, parsed.
    config: Config,
    /// `config.toml` as written: layers are appended to it, so the user's formatting survives.
    config_text: String,
    /// `lock.toml`.
    lock: Lock,
    /// `pending.toml`, while conflicts wait in `.devset/conflicts/`.
    pending: Option<Lock>,
    /// `state.toml`.
    state: State,
    /// `answers.toml`, with answers given since.
    answers: BTreeMap<VarName, String>,
    /// Variables answered since loading, which must be declared.
    given: BTreeSet<VarName>,
    /// Scaffolds to write again, whatever was decided.
    rescaffold: BTreeSet<ScaffoldId>,
    /// Digest of `state.toml` as read, to catch a concurrent writer.
    state_digest: Option<Digest>,
}

/// What a new target's `config.toml` says before it has a source or a layer.
const SKELETON: &str = "\
# devset applies the profiles below to this directory, and keeps them up to date.
# The manual: https://atomix-labs.github.io/devset/
#
# Where profiles come from, each named once: a git repository at a ref, or a directory.
#
#   [sources]
#   atxp = { git = \"https://github.com/atomix-labs/atxp\", tag = \"v0.4.0\" }
#
# The profiles to apply, in order, by source and name, with the features to turn on.
#
#   [[layers]]
#   profile  = \"atxp/rust\"
#   features = [\"docs\"]
";

/// `.devset/config.toml`: the sources, the layers, and the target's word on every setting they
/// carry.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Where profiles come from, each named once.
    #[serde(default)]
    pub sources: BTreeMap<SourceName, Source>,
    /// Profiles to apply, in order.
    #[serde(default)]
    pub layers: Vec<LayerSpec>,
    /// Merge settings; these override every layer's.
    #[serde(default)]
    pub merge: MergeSpec,
    /// Overrides of the layers' `[files]` entries.
    #[serde(default)]
    pub files: BTreeMap<RelPath, Override>,
}

/// A `[[layers]]` entry: a profile, and the features the target turns on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct LayerSpec {
    /// The profile, `source/name`.
    pub profile: ProfileRef,
    /// Its features to turn on, beside its default ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<FeatureName>,
    /// Whether its default features are on; unless a profile that requires it turns them on.
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub default_features: bool,
}

impl LayerSpec {
    /// A layer of `profile`, its default features on and no other.
    #[must_use]
    pub const fn new(profile: ProfileRef) -> Self {
        Self { profile, features: Vec::new(), default_features: true }
    }
}

/// `true`, serde's default for a flag that is on unless set.
const fn yes() -> bool {
    true
}

/// Whether `flag` is on, its default.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's `skip_serializing_if` passes a reference"
)]
const fn is_yes(flag: &bool) -> bool {
    *flag
}

/// A `[files."path"]` entry in `config.toml`, overriding the providing layer's.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Override {
    /// The layer that provides the file, when several do.
    #[serde(default)]
    pub from: Option<ProfileName>,
    /// How devset manages the file.
    #[serde(default)]
    pub policy: Option<Policy>,
    /// How a merged result is checked.
    #[serde(default)]
    pub validate: Option<Format>,
}

/// What was decided of a scaffold, and where it wrote its files: `state.toml`'s record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scaffold {
    /// Whether the group was written, or found the target's own.
    pub decision: Scaffolded,
    /// Each file it wrote, by its path as the profile writes it, at its path in the target: a
    /// changed answer moves no file a scaffold wrote.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<RelPath, RelPath>,
}

/// What was decided of a scaffold, once and for good.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Display)]
#[serde(rename_all = "lowercase")]
#[display(rename_all = "lowercase")]
pub enum Scaffolded {
    /// Its sentinels were absent, so its files were written.
    Written,
    /// A sentinel was there: the target has its own, and the group was left out.
    Found,
}

impl Target {
    /// The target containing `dir`: the nearest ancestor holding `.devset/config.toml`.
    ///
    /// # Errors
    /// - [`TargetError::NotFound`], no ancestor holds one.
    /// - [`TargetError::OldConfig`], its `config.toml` is devset 0.1's.
    /// - [`Error::Parse`](crate::Error::Parse), one of its files does not parse.
    pub fn find(dir: &Utf8Path) -> Result<Self> {
        let root = enclosing(dir).ok_or_else(|| TargetError::NotFound { dir: dir.to_owned() })?;
        Self::load(root)
    }

    /// The target rooted at `root`, or a new one there, its `config.toml` a commented skeleton.
    /// Never one inside another target.
    ///
    /// # Errors
    /// - [`TargetError::Nested`], `root` lies inside another target.
    /// - [`TargetError::OldConfig`], its `config.toml` is devset 0.1's.
    /// - [`Error::Parse`](crate::Error::Parse), one of its files does not parse.
    pub fn open_or_new(root: &Utf8Path) -> Result<Self> {
        match enclosing(root) {
            Some(found) if found == root => Self::load(root),
            Some(outer) => {
                Err(TargetError::Nested { dir: root.to_owned(), root: outer.to_owned() }.into())
            },
            None => Ok(Self {
                root: root.to_owned(),
                config: Config::default(),
                config_text: SKELETON.to_owned(),
                lock: Lock::default(),
                pending: None,
                state: State::default(),
                answers: BTreeMap::new(),
                given: BTreeSet::new(),
                rescaffold: BTreeSet::new(),
                state_digest: None,
            }),
        }
    }

    /// The directory holding `.devset/`.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// `config.toml`.
    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// Whether `config.toml` exists yet: a new target's is written by its first
    /// [`commit`](crate::commit()).
    #[must_use]
    pub fn exists(&self) -> bool {
        self.dir().join(CONFIG).is_file()
    }

    /// Names `source` `name` in `[sources]`; written by the next [`commit`](crate::commit()).
    ///
    /// Nothing changes when it already does.
    ///
    /// # Errors
    /// - [`TargetError::SourceExists`], `name` already names another source.
    /// - [`Error::Parse`](crate::Error::Parse), `sources` in `config.toml` is not a table.
    pub fn add_source(&mut self, name: SourceName, source: Source) -> Result<()> {
        match self.config.sources.get(&name) {
            Some(named) if *named == source => return Ok(()),
            Some(named) => {
                return Err(TargetError::SourceExists { name, location: named.to_string() }.into());
            },
            None => {},
        }
        let spec = ser::to_document(&SourceSpec::from(source)).map_err(io::Error::other)?;
        let inline = spec.into_table().into_inline_table();
        self.edit("`sources` must be a table", |doc| {
            let sources = doc.entry("sources").or_insert_with(|| Item::Table(Table::new()));
            let Some(sources) = sources.as_table_like_mut() else { return false };
            sources.insert(name.as_str(), Item::Value(Value::InlineTable(inline)));
            true
        })
    }

    /// Appends `layer` as the last layer; written by the next [`commit`](crate::commit()).
    ///
    /// # Errors
    /// - [`TargetError::DuplicateLayer`], a layer already applies the profile.
    /// - [`TargetError::NoSuchSource`], `[sources]` does not name its source.
    /// - [`Error::Parse`](crate::Error::Parse), `layers` in `config.toml` is not a list of tables.
    pub fn add_layer(&mut self, layer: LayerSpec) -> Result<()> {
        if self.layer(&layer.profile.profile).is_some() {
            return Err(TargetError::DuplicateLayer { layer: layer.profile.profile }.into());
        }
        if !self.config.sources.contains_key(&layer.profile.source) {
            let sources = self.config.sources.keys().cloned().collect();
            let (name, layers) = (layer.profile.source.clone(), vec![layer.profile]);
            return Err(TargetError::NoSuchSource { name, sources, layers }.into());
        }
        let table = ser::to_document(&layer).map_err(io::Error::other)?.into_table();
        self.edit("`layers` must be a list of tables", |doc| {
            match doc.entry("layers").or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new())) {
                Item::ArrayOfTables(layers) => layers.push(table),
                Item::Value(Value::Array(layers)) => layers.push(table.into_inline_table()),
                Item::None | Item::Value(_) | Item::Table(_) => return false,
            }
            true
        })
    }

    /// The layer that applies the profile named `name`.
    #[must_use]
    pub fn layer(&self, name: &ProfileName) -> Option<&LayerSpec> {
        self.config.layers.iter().find(|layer| layer.profile.profile == *name)
    }

    /// Removes the layer that applies the profile named `name`; written by the next
    /// [`commit`](crate::commit()). Its source stays, for another layer to use.
    ///
    /// Every `[files]` override that picks it, `from = "<name>"`, goes with it.
    ///
    /// # Errors
    /// [`Error::Parse`](crate::Error::Parse), `config.toml` changed shape so that the layer
    /// cannot be found in it.
    pub fn remove_layer(&mut self, name: &ProfileName) -> Result<()> {
        let Some(index) = self.config.layers.iter().position(|l| l.profile.profile == *name) else {
            return Ok(());
        };
        self.edit("`layers` must be a list of tables", |doc| {
            match doc.get_mut("layers") {
                Some(Item::ArrayOfTables(layers)) if index < layers.len() => {
                    layers.remove(index);
                },
                Some(Item::Value(Value::Array(layers))) if index < layers.len() => {
                    layers.remove(index);
                },
                Some(_) | None => return false,
            }
            if let Some(files) = doc.get_mut("files").and_then(Item::as_table_like_mut) {
                let picks =
                    |item: &Item| item.get("from").and_then(Item::as_str) == Some(name.as_str());
                let picked: Vec<String> = files
                    .iter()
                    .filter(|(_, item)| picks(item))
                    .map(|(key, _)| key.to_owned())
                    .collect();
                for key in picked {
                    files.remove(&key);
                }
            }
            true
        })
    }

    /// Turns on `features` of the layer that applies the profile named `name`, beside those it
    /// lists, and its default features on or off where `defaults` says; written by the next
    /// [`commit`](crate::commit()). Returns what changed: the features it did not list, and the
    /// defaults, where they change.
    ///
    /// # Errors
    /// - [`TargetError::NoSuchLayer`], no layer applies the profile.
    /// - [`Error::Parse`](crate::Error::Parse), as [`remove_layer`](Self::remove_layer).
    pub fn turn_on(
        &mut self, name: &ProfileName, features: &[FeatureName], defaults: Option<bool>,
    ) -> Result<(Vec<FeatureName>, Option<bool>)> {
        let (index, layer) = self.find_layer(name)?;
        let mut listed = layer.features.clone();
        let added: Vec<FeatureName> =
            features.iter().filter(|feature| !listed.contains(feature)).cloned().collect();
        listed.extend(added.iter().cloned());
        let defaults = defaults.filter(|&on| on != layer.default_features);
        if !added.is_empty() || defaults.is_some() {
            self.edit_layer(index, |table| {
                set_features(table, &listed);
                match defaults {
                    Some(true) => {
                        table.remove("default-features");
                    },
                    Some(false) => {
                        table.insert("default-features", toml_edit::value(false));
                    },
                    None => {},
                }
            })?;
        }
        Ok((added, defaults))
    }

    /// Turns off `features` of the layer that applies the profile named `name`: takes them out of
    /// those it lists; written by the next [`commit`](crate::commit()). The layer stays.
    ///
    /// # Errors
    /// - [`TargetError::NoSuchLayer`], no layer applies the profile.
    /// - [`TargetError::NotListed`], the layer does not list one of `features`.
    /// - [`Error::Parse`](crate::Error::Parse), as [`remove_layer`](Self::remove_layer).
    pub fn turn_off(&mut self, name: &ProfileName, features: &[FeatureName]) -> Result<()> {
        let (index, layer) = self.find_layer(name)?;
        if let Some(feature) = features.iter().find(|feature| !layer.features.contains(feature)) {
            let (feature, listed) = (feature.clone(), layer.features.clone());
            return Err(TargetError::NotListed { layer: name.clone(), feature, listed }.into());
        }
        let listed: Vec<FeatureName> =
            layer.features.into_iter().filter(|feature| !features.contains(feature)).collect();
        self.edit_layer(index, |table| set_features(table, &listed))
    }

    /// The position of the layer that applies the profile named `name`, and the layer.
    fn find_layer(&self, name: &ProfileName) -> Result<(usize, LayerSpec)> {
        let layers = &self.config.layers;
        let found = layers.iter().enumerate().find(|(_, layer)| layer.profile.profile == *name);
        found.map(|(index, layer)| (index, layer.clone())).ok_or_else(|| {
            let names = layers.iter().map(|layer| layer.profile.profile.to_string()).collect();
            TargetError::NoSuchLayer { name: name.to_string(), names }.into()
        })
    }

    /// Applies `edit` to the `index`th layer's table in `config.toml`.
    fn edit_layer(&mut self, index: usize, edit: impl FnOnce(&mut dyn TableLike)) -> Result<()> {
        self.edit("`layers` must be a list of tables", |doc| {
            let table: &mut dyn TableLike = match doc.get_mut("layers") {
                Some(Item::ArrayOfTables(layers)) => match layers.get_mut(index) {
                    Some(layer) => layer,
                    None => return false,
                },
                Some(Item::Value(Value::Array(layers))) => {
                    match layers.get_mut(index).and_then(Value::as_inline_table_mut) {
                        Some(layer) => layer,
                        None => return false,
                    }
                },
                Some(_) | None => return false,
            };
            edit(table);
            true
        })
    }

    /// Removes the `[files."path"]` override; written by the next [`commit`](crate::commit()).
    ///
    /// # Errors
    /// [`Error::Parse`](crate::Error::Parse), as [`add_layer`](Self::add_layer).
    pub fn remove_override(&mut self, path: &RelPath) -> Result<()> {
        self.edit("`files` must be a table", |doc| {
            if let Some(files) = doc.get_mut("files").and_then(Item::as_table_like_mut) {
                files.remove(path.as_str());
            }
            true
        })
    }

    /// Applies `edit` to `config.toml`, keeping its formatting and comments, and reads it again.
    ///
    /// `edit` returns `false` when the file is not the shape it expects, which `shape` says. A
    /// skeleton's comments stay at the top, above the first table.
    fn edit(&mut self, shape: &str, edit: impl FnOnce(&mut DocumentMut) -> bool) -> Result<()> {
        let file = label(CONFIG);
        let failed = |text: &str, message: String| ParseError {
            file: file.clone(),
            text: text.to_owned(),
            span: None,
            message,
        };
        let mut doc: DocumentMut = self
            .config_text
            .parse()
            .map_err(|e: TomlError| failed(&self.config_text, e.to_string()))?;
        let lead = doc.is_empty().then(|| doc.trailing().as_str().map(str::to_owned)).flatten();
        if !edit(&mut doc) {
            return Err(failed(&self.config_text, shape.to_owned()).into());
        }
        if let Some(lead) = lead.filter(|lead| !lead.trim().is_empty()) {
            doc.set_trailing("");
            if let Some((_, first)) = doc.iter_mut().next() {
                let prefix = format!("{}\n\n", lead.trim_end());
                match first {
                    Item::Table(table) => table.decor_mut().set_prefix(prefix),
                    Item::ArrayOfTables(tables) => {
                        if let Some(table) = tables.get_mut(0) {
                            table.decor_mut().set_prefix(prefix);
                        }
                    },
                    Item::Value(_) | Item::None => {},
                }
            }
        }
        let text = doc.to_string();
        self.config = from_toml(text.as_bytes(), &file)?;
        self.config_text = text;
        Ok(())
    }

    /// Answers `name`; written by the next [`commit`](crate::commit()).
    pub fn answer(&mut self, name: VarName, value: String) {
        self.given.insert(name.clone());
        self.answers.insert(name, value);
    }

    /// Every answer: recorded, or given since.
    #[must_use]
    pub const fn answers(&self) -> &BTreeMap<VarName, String> {
        &self.answers
    }

    /// Writes the absent files of `scaffold` again at the next [`survey`](crate::survey()),
    /// whatever was decided of it.
    pub fn rescaffold(&mut self, scaffold: ScaffoldId) {
        self.rescaffold.insert(scaffold);
    }

    /// The scaffolds to write again.
    pub(crate) const fn rescaffolds(&self) -> &BTreeSet<ScaffoldId> {
        &self.rescaffold
    }

    /// What was decided of each scaffold, as `state.toml` recorded it.
    pub(crate) const fn scaffolds(&self) -> &BTreeMap<ScaffoldId, Scaffold> {
        &self.state.scaffolds
    }

    /// Variables answered since loading.
    pub(crate) fn given(&self) -> impl Iterator<Item = &VarName> {
        self.given.iter()
    }

    /// `.devset/`.
    pub(crate) fn dir(&self) -> Utf8PathBuf {
        self.root.join(DIR)
    }

    /// `config.toml` as it should be written.
    pub(crate) fn config_text(&self) -> &str {
        &self.config_text
    }

    /// The lock in force: the pending one while an update is withheld.
    fn lock(&self) -> &Lock {
        self.pending.as_ref().unwrap_or(&self.lock)
    }

    /// The commit the lock pins `source` to.
    ///
    /// Found by source, not position, so removing or reordering layers leaves the others pinned.
    #[must_use]
    pub fn pinned(&self, source: &Source) -> Option<&Oid> {
        let locked = self.lock().profiles.iter();
        locked.filter(|locked| locked.source == *source).find_map(|locked| locked.rev.as_ref())
    }

    /// The digest the lock recorded for the profile `name` from `source`.
    pub(crate) fn locked(&self, source: &Source, name: &ProfileName) -> Option<Digest> {
        let lock = self.lock();
        let locked = lock.profiles.iter().find(|l| l.source == *source && l.name == *name);
        locked.map(|locked| locked.digest)
    }

    /// Where the conflicted merge of `path` waits.
    pub(crate) fn sidecar(&self, path: &RelPath) -> Utf8PathBuf {
        path.under(&self.dir().join(CONFLICTS))
    }

    /// An unfinished update's own files, beside its sidecars.
    ///
    /// Named `.devset`, which no managed path may contain, so it never meets a sidecar; and inside
    /// `conflicts/`, so deleting that directory discards the whole update.
    pub(crate) fn unfinished(&self) -> Utf8PathBuf {
        self.dir().join(CONFLICTS).join(DIR)
    }

    /// The recorded base with fingerprint `record`, or `None` when it is missing or damaged.
    pub(crate) fn base(&self, record: &Fingerprint) -> Result<Option<Vec<u8>>> {
        let blob = read_optional(&self.dir().join(BASE).join(record.exact.to_string()))?;
        Ok(blob.filter(|bytes| Digest::of(bytes) == record.exact))
    }

    /// Every recorded file and part.
    pub(crate) fn records(&self) -> impl Iterator<Item = (Slot, Record)> {
        self.state.records()
    }

    /// Digest of `state.toml` as read.
    pub(crate) const fn state_digest(&self) -> Option<Digest> {
        self.state_digest
    }

    /// Reads the target at `root`.
    fn load(root: &Utf8Path) -> Result<Self> {
        let dir = root.join(DIR);
        let config_raw = fs_err::read(dir.join(CONFIG))?;
        let config = read_config(&config_raw)?;
        let config_text = String::from_utf8(config_raw).map_err(io::Error::other)?;
        let state_raw = read_optional(&dir.join(STATE))?;
        let state_digest = state_raw.as_deref().map(Digest::of);
        let state = state_raw.map(|raw| from_toml(&raw, &label(STATE))).transpose()?;
        let lock = |name: &str| -> Result<Option<Lock>> {
            let raw = read_optional(&dir.join(name))?;
            raw.map(|raw| Lock::read(&raw, &label(name))).transpose()
        };
        Ok(Self {
            root: root.to_owned(),
            config,
            config_text,
            lock: lock(LOCK)?.unwrap_or_default(),
            pending: lock(&format!("{CONFLICTS}/{DIR}/{PENDING}"))?,
            state: state.unwrap_or_default(),
            state_digest,
            answers: read_toml(&dir, ANSWERS)?.unwrap_or_default(),
            given: BTreeSet::new(),
            rescaffold: BTreeSet::new(),
        })
    }
}

/// Writes `features` as the layer's `features`, in the file's form where it has one; none takes
/// the key out.
fn set_features(table: &mut dyn TableLike, features: &[FeatureName]) {
    if features.is_empty() {
        table.remove("features");
        return;
    }
    let mut array: Array = features.iter().map(FeatureName::as_str).collect();
    array.fmt();
    match table.get_mut("features").and_then(Item::as_value_mut) {
        Some(value) => {
            let decor = value.decor().clone();
            *value = Value::Array(array);
            *value.decor_mut() = decor;
        },
        None => {
            table.insert("features", toml_edit::value(array));
        },
    }
}

/// `config.toml`, refused when it is devset 0.1's, its layers named locations, or when it lists
/// one layer twice.
fn read_config(raw: &[u8]) -> Result<Config> {
    let file = label(CONFIG);
    let table: toml::Table = from_toml(raw, &file)?;
    let located = |layer: &toml::Value| {
        layer
            .as_table()
            .is_some_and(|layer| layer.contains_key("git") || layer.contains_key("path"))
    };
    let layers = table.get("layers").and_then(toml::Value::as_array);
    if layers.is_some_and(|layers| layers.iter().any(located)) {
        return Err(TargetError::OldConfig.into());
    }
    let config: Config = from_toml(raw, &file)?;
    let mut named = BTreeSet::new();
    if let Some(layer) = config.layers.iter().find(|layer| !named.insert(&layer.profile.profile)) {
        return Err(TargetError::LayerTwice { layer: layer.profile.profile.clone() }.into());
    }
    Ok(config)
}

/// `name` in `.devset/`, as users know it.
pub(crate) fn label(name: &str) -> String {
    format!("{DIR}/{name}")
}

/// The nearest ancestor of `dir`, itself included, holding `.devset/config.toml`.
fn enclosing(dir: &Utf8Path) -> Option<&Utf8Path> {
    dir.ancestors().find(|d| d.join(DIR).join(CONFIG).is_file())
}

/// `lock.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Lock {
    /// Format version.
    version: V2,
    /// One per active profile, in the order they apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    profiles: Vec<Locked>,
}

/// What one active profile resolved to.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Locked {
    /// The profile's name.
    pub name: ProfileName,
    /// Commit of its source, for a versioned one; every profile of a source has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<Oid>,
    /// Digest of its manifest, rendered payload and features.
    pub digest: Digest,
    /// Its source, as configured when locked.
    pub source: Source,
    /// Every feature that was on, with who turned it on.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub features: BTreeMap<FeatureName, Vec<String>>,
}

impl Locked {
    /// Who turned each of `features` on, as the lock writes it.
    pub(crate) fn features(
        features: &BTreeMap<FeatureName, BTreeSet<Enabler>>,
    ) -> BTreeMap<FeatureName, Vec<String>> {
        features
            .iter()
            .map(|(feature, by)| (feature.clone(), by.iter().map(ToString::to_string).collect()))
            .collect()
    }
}

impl Lock {
    /// A lock of `profiles`, in order.
    pub(crate) const fn new(profiles: Vec<Locked>) -> Self {
        Self { version: V2, profiles }
    }

    /// The lock in `raw`, read from `file`; devset 0.1's is empty, so every source resolves anew.
    fn read(raw: &[u8], file: &str) -> Result<Self> {
        let table: toml::Table = from_toml(raw, file)?;
        if table.get("version").and_then(toml::Value::as_integer) == Some(1) {
            return Ok(Self::default());
        }
        from_toml(raw, file)
    }
}

/// `state.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    /// Format version; devset 0.1's reads as this one.
    version: V2,
    /// What was decided of each scaffold.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    scaffolds: BTreeMap<ScaffoldId, Scaffold>,
    /// Each managed file's recorded base.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    files: BTreeMap<RelPath, FileRecord>,
    /// Each part's recorded base, by file, then by the profile that owns it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    parts: BTreeMap<RelPath, BTreeMap<String, PartRecord>>,
}

/// What devset recorded for one file or part.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Record {
    /// How much of the file it is.
    pub scope: Scope,
    /// Its content, as recorded.
    pub fingerprint: Fingerprint,
    /// How it was managed when recorded.
    pub policy: Policy,
}

/// What devset recorded for one whole file: the digests of its content, and its policy.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileRecord {
    /// As [`Fingerprint::exact`].
    exact: Digest,
    /// As [`Fingerprint::canonical`].
    canonical: Digest,
    /// How it was managed; `owned`, the default, is not written.
    #[serde(default, skip_serializing_if = "Policy::is_owned")]
    policy: Policy,
}

/// What devset recorded for one part: its scope, the digests of its content, and its policy.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartRecord {
    /// How much of the file the part is.
    scope: Scope,
    /// As [`Fingerprint::exact`].
    exact: Digest,
    /// As [`Fingerprint::canonical`].
    canonical: Digest,
    /// How it was managed; `owned`, the default, is not written.
    #[serde(default, skip_serializing_if = "Policy::is_owned")]
    policy: Policy,
}

impl State {
    /// A state recording `records`, and what was decided of `scaffolds`.
    pub(crate) fn new(
        records: BTreeMap<Slot, Record>, scaffolds: BTreeMap<ScaffoldId, Scaffold>,
    ) -> Self {
        let mut state = Self { scaffolds, ..Self::default() };
        for (Slot { path, part }, Record { scope, fingerprint, policy }) in records {
            let Fingerprint { exact, canonical } = fingerprint;
            match part {
                None => {
                    state.files.insert(path, FileRecord { exact, canonical, policy });
                },
                Some(owner) => {
                    let record = PartRecord { scope, exact, canonical, policy };
                    state.parts.entry(path).or_default().insert(owner, record);
                },
            }
        }
        state
    }

    /// Every record, whole files first.
    fn records(&self) -> impl Iterator<Item = (Slot, Record)> {
        let files = self.files.iter().map(|(path, record)| {
            let FileRecord { exact, canonical, policy } = *record;
            let fingerprint = Fingerprint { exact, canonical };
            (Slot::whole(path.clone()), Record { scope: Scope::File, fingerprint, policy })
        });
        let parts = self.parts.iter().flat_map(|(path, parts)| {
            parts.iter().map(|(owner, record)| {
                let slot = Slot { path: path.clone(), part: Some(owner.clone()) };
                let PartRecord { scope, exact, canonical, policy } = *record;
                (slot, Record { scope, fingerprint: Fingerprint { exact, canonical }, policy })
            })
        });
        files.chain(parts)
    }

    /// The bases it references.
    pub(crate) fn bases(&self) -> impl Iterator<Item = Digest> {
        self.records().map(|(_, record)| record.fingerprint.exact)
    }
}

/// Format version `1`: `undo.toml`'s.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct V1;

impl Serialize for V1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(1)
    }
}

impl<'de> Deserialize<'de> for V1 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match u8::deserialize(deserializer)? {
            1 => Ok(Self),
            other => Err(de::Error::custom(format_args!("unsupported format version {other}"))),
        }
    }
}

/// Format version `2`: `lock.toml`'s and `state.toml`'s; version `1` reads as it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct V2;

impl Serialize for V2 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(2)
    }
}

impl<'de> Deserialize<'de> for V2 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match u8::deserialize(deserializer)? {
            1 | 2 => Ok(Self),
            other => Err(de::Error::custom(format_args!("unsupported format version {other}"))),
        }
    }
}

/// Parses TOML `bytes` read from `file`.
pub(crate) fn from_toml<T: DeserializeOwned>(bytes: &[u8], file: &str) -> Result<T> {
    let failed = |text: &str, span, message| ParseError {
        file: file.into(),
        text: text.into(),
        span,
        message,
    };
    let text = str::from_utf8(bytes).map_err(|e| failed("", None, e.to_string()))?;
    Ok(toml::from_str(text).map_err(|e| failed(text, e.span(), e.message().to_owned()))?)
}

/// `name` in the `.devset/` at `dir`, parsed; `None` if it does not exist.
pub(crate) fn read_toml<T: DeserializeOwned>(dir: &Utf8Path, name: &str) -> Result<Option<T>> {
    read_optional(&dir.join(name))?.map(|raw| from_toml(&raw, &label(name))).transpose()
}

/// The contents of `path`, or `None` if it does not exist.
pub(crate) fn read_optional(path: &Utf8Path) -> Result<Option<Vec<u8>>> {
    match fs_err::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{LayerSpec, Lock, State, Target};
    use crate::errors::{Error, TargetError};
    use crate::source::Source;

    #[test]
    fn a_new_target_keeps_its_skeleton_above_what_is_added() {
        let dir = camino_tempfile::tempdir().expect("a directory");
        let mut target = Target::open_or_new(dir.path()).expect("a new target");
        target
            .add_source("house".parse().expect("a name"), Source::Dir("../p".into()))
            .expect("added");
        target.add_layer(LayerSpec::new("house/base".parse().expect("a layer"))).expect("added");
        let text = target.config_text();
        assert!(text.starts_with("# devset applies"), "the skeleton leads: {text}");
        assert!(
            text.contains("\n\n[sources]\nhouse = { path = \"../p\" }\n"),
            "a source, inline: {text}"
        );
        assert!(text.ends_with("[[layers]]\nprofile = \"house/base\"\n"), "then the layer: {text}");
        let same = target.add_source("house".parse().expect("a name"), Source::Dir("../p".into()));
        assert!(same.is_ok(), "naming a source again as it is changes nothing");
        let other = target.add_source("house".parse().expect("a name"), Source::Dir("../q".into()));
        assert!(matches!(other, Err(Error::Target(TargetError::SourceExists { .. }))), "{other:?}");
        let twice = target.add_layer(LayerSpec::new("house/base".parse().expect("a layer")));
        assert!(
            matches!(twice, Err(Error::Target(TargetError::DuplicateLayer { .. }))),
            "{twice:?}"
        );
        let unknown = target.add_layer(LayerSpec::new("nope/other".parse().expect("a layer")));
        assert!(
            matches!(unknown, Err(Error::Target(TargetError::NoSuchSource { .. }))),
            "{unknown:?}"
        );
    }

    #[test]
    fn devset_0_1_records() {
        let dir = camino_tempfile::tempdir().expect("a directory");
        let devset = dir.path().join(".devset");
        fs_err::create_dir_all(&devset).expect("a directory");
        fs_err::write(devset.join("config.toml"), "[[layers]]\npath = \"../p\"\n")
            .expect("written");
        let old = Target::find(dir.path()).expect_err("an old config");
        assert!(matches!(old, Error::Target(TargetError::OldConfig)), "refused: {old}");
        let lock =
            Lock::read(b"version = 1\n[[layers]]\ndigest = \"x\"\n", "lock.toml").expect("read");
        assert!(lock.profiles.is_empty(), "an old lock pins nothing");
        let state: State = toml::from_str(
            "version = 1\n[files.\"a\"]\nexact = \"0000000000000000000000000000000000000000000000000000000000000000\"\ncanonical = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
        )
        .expect("an old state reads");
        assert_eq!(state.records().count(), 1, "with its records");
        let written = toml::to_string(&state).expect("written");
        assert!(written.starts_with("version = 2\n"), "as version 2: {written}");
    }
}
