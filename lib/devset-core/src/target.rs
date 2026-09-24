//! The directory devset manages, and what its `.devset/` records.
//!
//! ```text
//! .devset/
//! ├── config.toml     the layers, and the target's word on their settings (committed)
//! ├── lock.toml       each layer's exact revision and digest (committed)
//! ├── answers.toml    answers to the layers' variables (committed)
//! ├── state.toml      what devset last wrote, by file and part (committed)
//! ├── base/           those bytes, by digest, as merge bases (committed)
//! └── conflicts/      an unfinished update: merges awaiting resolution (ignored)
//!     └── .devset/    what it withheld, and what it would take to undo it
//! ```

use alloc::collections::{BTreeMap, BTreeSet};
use core::str;
use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use schemars::JsonSchema;
use serde::de::{self, DeserializeOwned};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use toml_edit::{ArrayOfTables, DocumentMut, Item, TomlError, Value, ser};

use crate::digest::{Digest, Fingerprint};
use crate::errors::{ParseError, Result, TargetError};
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
    /// Digest of `state.toml` as read, to catch a concurrent writer.
    state_digest: Option<Digest>,
}

/// `.devset/config.toml`: the layers, and the target's word on every setting they carry.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Profiles to apply, in order.
    #[serde(default)]
    pub layers: Vec<Source>,
    /// Merge settings; these override every layer's.
    #[serde(default)]
    pub merge: MergeSpec,
    /// Overrides of the layers' `[files]` entries.
    #[serde(default)]
    pub files: BTreeMap<RelPath, Override>,
}

/// A `[files."path"]` entry in `config.toml`, overriding the providing layer's.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Override {
    /// The layer that provides the file, when several do.
    #[serde(default)]
    pub from: Option<String>,
    /// How devset manages the file.
    #[serde(default)]
    pub policy: Option<Policy>,
    /// How a merged result is checked.
    #[serde(default)]
    pub validate: Option<Format>,
}

impl Target {
    /// The target containing `dir`: the nearest ancestor holding `.devset/config.toml`.
    ///
    /// # Errors
    /// - [`TargetError::NotFound`], no ancestor holds one.
    /// - [`Error::Parse`](crate::Error::Parse), one of its files does not parse.
    pub fn find(dir: &Utf8Path) -> Result<Self> {
        let root = enclosing(dir).ok_or_else(|| TargetError::NotFound {
            dir: dir.to_owned(),
        })?;
        Self::load(root)
    }

    /// The target rooted at `root`, or a new, empty one there. Never one inside another target.
    ///
    /// # Errors
    /// - [`TargetError::Nested`], `root` lies inside another target.
    /// - [`Error::Parse`](crate::Error::Parse), one of its files does not parse.
    pub fn open_or_new(root: &Utf8Path) -> Result<Self> {
        match enclosing(root) {
            Some(found) if found == root => Self::load(root),
            Some(outer) => Err(TargetError::Nested {
                dir: root.to_owned(),
                root: outer.to_owned(),
            }
            .into()),
            None => Ok(Self {
                root: root.to_owned(),
                config: Config::default(),
                config_text: String::new(),
                lock: Lock::default(),
                pending: None,
                state: State::default(),
                answers: BTreeMap::new(),
                given: BTreeSet::new(),
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

    /// Appends `source` as the last layer; written by the next [`commit`](crate::commit()).
    ///
    /// # Errors
    /// - [`TargetError::DuplicateLayer`], the layer is already present.
    /// - [`Error::Parse`](crate::Error::Parse), `layers` in `config.toml` is not a list of tables.
    /// - [`Error::Io`](crate::Error::Io), the layer cannot be written as TOML.
    pub fn add_layer(&mut self, source: Source) -> Result<()> {
        if self.config.layers.contains(&source) {
            return Err(TargetError::DuplicateLayer {
                layer: source.to_string(),
            }
            .into());
        }
        let table = ser::to_document(&SourceSpec::from(source))
            .map_err(io::Error::other)?
            .into_table();
        self.edit(|doc| {
            match doc
                .entry("layers")
                .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()))
            {
                Item::ArrayOfTables(layers) => layers.push(table),
                Item::Value(Value::Array(layers)) => layers.push(table.into_inline_table()),
                Item::None | Item::Value(_) | Item::Table(_) => return false,
            }
            true
        })
    }

    /// The source of the layer whose profile has `name`, as the lock last recorded it.
    #[must_use]
    pub fn layer(&self, name: &str) -> Option<&Source> {
        let lock = self.pending.as_ref().unwrap_or(&self.lock);
        let locked = lock
            .layers
            .iter()
            .find(|locked| locked.name.as_deref() == Some(name))?;
        self.config
            .layers
            .iter()
            .find(|source| **source == locked.source)
    }

    /// Removes the layer at `source`; written by the next [`commit`](crate::commit()).
    ///
    /// Every `[files]` override that picks it, `from = "<name>"`, goes with it.
    ///
    /// # Errors
    /// [`Error::Parse`](crate::Error::Parse), `config.toml` changed shape so that the layer
    /// cannot be found in it.
    pub fn remove_layer(&mut self, source: &Source, name: &str) -> Result<()> {
        let Some(index) = self.config.layers.iter().position(|layer| layer == source) else {
            return Ok(());
        };
        self.edit(|doc| {
            match doc.get_mut("layers") {
                Some(Item::ArrayOfTables(layers)) if index < layers.len() => {
                    layers.remove(index);
                }
                Some(Item::Value(Value::Array(layers))) if index < layers.len() => {
                    layers.remove(index);
                }
                Some(_) | None => return false,
            }
            if let Some(files) = doc.get_mut("files").and_then(Item::as_table_like_mut) {
                let picks = |item: &Item| item.get("from").and_then(Item::as_str) == Some(name);
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

    /// Removes the `[files."path"]` override; written by the next [`commit`](crate::commit()).
    ///
    /// # Errors
    /// [`Error::Parse`](crate::Error::Parse), as [`add_layer`](Self::add_layer).
    pub fn remove_override(&mut self, path: &RelPath) -> Result<()> {
        self.edit(|doc| {
            if let Some(files) = doc.get_mut("files").and_then(Item::as_table_like_mut) {
                files.remove(path.as_str());
            }
            true
        })
    }

    /// Applies `edit` to `config.toml`, keeping its formatting and comments, and reads it again.
    ///
    /// `edit` returns `false` when the file is not the shape it expects.
    fn edit(&mut self, edit: impl FnOnce(&mut DocumentMut) -> bool) -> Result<()> {
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
        if !edit(&mut doc) {
            let message = "`layers` must be a list of tables".to_owned();
            return Err(failed(&self.config_text, message).into());
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

    /// The lock entry of the layer at `source`.
    ///
    /// Found by source, not position, so removing or reordering layers leaves the others pinned.
    /// The pending lock stands in while an update is withheld.
    pub(crate) fn locked(&self, source: &Source) -> Option<&Locked> {
        let lock = self.pending.as_ref().unwrap_or(&self.lock);
        lock.layers.iter().find(|locked| locked.source == *source)
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
        let config = from_toml(&config_raw, &label(CONFIG))?;
        let config_text = String::from_utf8(config_raw).map_err(io::Error::other)?;
        let state_raw = read_optional(&dir.join(STATE))?;
        let state_digest = state_raw.as_deref().map(Digest::of);
        let state = state_raw
            .map(|raw| from_toml(&raw, &label(STATE)))
            .transpose()?;
        Ok(Self {
            root: root.to_owned(),
            config,
            config_text,
            lock: read_toml(&dir, LOCK)?.unwrap_or_default(),
            pending: read_toml(&dir, &format!("{CONFLICTS}/{DIR}/{PENDING}"))?,
            state: state.unwrap_or_default(),
            state_digest,
            answers: read_toml(&dir, ANSWERS)?.unwrap_or_default(),
            given: BTreeSet::new(),
        })
    }
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
    version: V1,
    /// One per layer, in `config.toml` order.
    #[serde(default)]
    layers: Vec<Locked>,
}

/// What one layer resolved to.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Locked {
    /// The profile's name, so a layer can be named without reading its source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Commit, for a versioned source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<Oid>,
    /// Digest of manifest and payload.
    pub digest: Digest,
    /// The layer as configured when locked.
    pub source: Source,
}

impl Lock {
    /// A lock of `layers`, in order.
    pub(crate) const fn new(layers: Vec<Locked>) -> Self {
        Self {
            version: V1,
            layers,
        }
    }
}

/// `state.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    /// Format version.
    version: V1,
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
    /// A state recording `records`.
    pub(crate) fn new(records: BTreeMap<Slot, Record>) -> Self {
        let mut state = Self::default();
        for (
            Slot { path, part },
            Record {
                scope,
                fingerprint,
                policy,
            },
        ) in records
        {
            let Fingerprint { exact, canonical } = fingerprint;
            match part {
                None => {
                    state.files.insert(
                        path,
                        FileRecord {
                            exact,
                            canonical,
                            policy,
                        },
                    );
                }
                Some(owner) => {
                    let record = PartRecord {
                        scope,
                        exact,
                        canonical,
                        policy,
                    };
                    state.parts.entry(path).or_default().insert(owner, record);
                }
            }
        }
        state
    }

    /// Every record, whole files first.
    fn records(&self) -> impl Iterator<Item = (Slot, Record)> {
        let files = self.files.iter().map(|(path, record)| {
            let FileRecord {
                exact,
                canonical,
                policy,
            } = *record;
            let fingerprint = Fingerprint { exact, canonical };
            (
                Slot::whole(path.clone()),
                Record {
                    scope: Scope::File,
                    fingerprint,
                    policy,
                },
            )
        });
        let parts = self.parts.iter().flat_map(|(path, parts)| {
            parts.iter().map(|(owner, record)| {
                let slot = Slot {
                    path: path.clone(),
                    part: Some(owner.clone()),
                };
                let PartRecord {
                    scope,
                    exact,
                    canonical,
                    policy,
                } = *record;
                (
                    slot,
                    Record {
                        scope,
                        fingerprint: Fingerprint { exact, canonical },
                        policy,
                    },
                )
            })
        });
        files.chain(parts)
    }

    /// The bases it references.
    pub(crate) fn bases(&self) -> impl Iterator<Item = Digest> {
        self.records().map(|(_, record)| record.fingerprint.exact)
    }
}

/// The only on-disk format version, `1`.
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
            other => Err(de::Error::custom(format_args!(
                "unsupported format version {other}"
            ))),
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
    read_optional(&dir.join(name))?
        .map(|raw| from_toml(&raw, &label(name)))
        .transpose()
}

/// The contents of `path`, or `None` if it does not exist.
pub(crate) fn read_optional(path: &Utf8Path) -> Result<Option<Vec<u8>>> {
    match fs_err::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
