//! The directory devset manages, and what its `.devset/` records.
//!
//! ```text
//! .devset/
//! ├── config.toml     the layers, and the target's word on their settings (committed)
//! ├── lock.toml       each layer's exact revision and digest (committed)
//! ├── answers.toml    answers to the layers' variables (committed)
//! ├── state.toml      what devset last wrote, by path (committed)
//! ├── base/           those bytes, by digest, as merge bases (committed)
//! └── conflicts/      merges awaiting resolution (ignored)
//! ```

use alloc::collections::{BTreeMap, BTreeSet};
use core::str;
use std::io;

use camino::{Utf8Path, Utf8PathBuf};
use schemars::JsonSchema;
use serde::de::{self, DeserializeOwned};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::digest::{Digest, Fingerprint};
use crate::errors::{ParseError, Result, TargetError};
use crate::path::RelPath;
use crate::profile::{Format, MergeSpec, Policy};
use crate::source::{Oid, Source};
use crate::vars::VarName;

/// devset's directory in a target.
pub(crate) const DIR: &str = ".devset";

/// Intent: the layers, in order.
pub(crate) const CONFIG: &str = "config.toml";

/// What each layer resolved to.
pub(crate) const LOCK: &str = "lock.toml";

/// What devset last recorded for each managed path.
pub(crate) const STATE: &str = "state.toml";

/// The lock an update withheld by `on-conflict = "apply-none"` will write.
pub(crate) const PENDING: &str = "pending.toml";

/// Conflicted merge results awaiting resolution, by path.
pub(crate) const CONFLICTS: &str = "conflicts";

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

/// One `[[layers]]` table, serialized for appending to `config.toml`.
#[derive(Serialize)]
struct Appended<'a> {
    /// The layer.
    layers: [&'a Source; 1],
}

impl Target {
    /// The target containing `dir`: the nearest ancestor holding `.devset/config.toml`.
    ///
    /// # Errors
    /// - [`TargetError::NotFound`], no ancestor holds one.
    /// - [`Error::Parse`](crate::Error::Parse), one of its files does not parse.
    pub fn find(dir: &Utf8Path) -> Result<Self> {
        let root = enclosing(dir).ok_or_else(|| TargetError::NotFound { dir: dir.to_owned() })?;
        Self::load(root)
    }

    /// The target rooted at `root`, or a new, empty one there. Never one inside another target.
    ///
    /// # Errors
    /// - [`TargetError::Nested`], `root` lies inside another target.
    /// - [`Error::Parse`](crate::Error::Parse), one of its files does not parse.
    pub fn at(root: &Utf8Path) -> Result<Self> {
        match enclosing(root) {
            Some(found) if found == root => Self::load(root),
            Some(outer) => {
                Err(TargetError::Nested { dir: root.to_owned(), root: outer.to_owned() }.into())
            },
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
    /// - [`Error::Parse`](crate::Error::Parse), `config.toml` lists its layers inline, so a
    ///   `[[layers]]` table cannot follow them.
    pub fn add_layer(&mut self, source: Source) -> Result<()> {
        if self.config.layers.contains(&source) {
            return Err(TargetError::DuplicateLayer { layer: source.to_string() }.into());
        }
        let block = toml::to_string(&Appended { layers: [&source] }).map_err(io::Error::other)?;
        let mut text = self.config_text.clone();
        if !text.is_empty() {
            text.push_str(if text.ends_with('\n') { "\n" } else { "\n\n" });
        }
        text.push_str(&block);
        let config: Config = from_toml(text.as_bytes(), CONFIG)?;
        self.config.layers.push(source);
        if config.layers != self.config.layers {
            let message = "layers must be `[[layers]]` tables".to_owned();
            return Err(ParseError { file: CONFIG.into(), text, span: None, message }.into());
        }
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

    /// Layer `index`'s lock entry, if it still describes `source`.
    ///
    /// The pending lock stands in while an update is withheld.
    pub(crate) fn locked(&self, index: usize, source: &Source) -> Option<&Locked> {
        let lock = self.pending.as_ref().unwrap_or(&self.lock);
        lock.layers.get(index).filter(|locked| locked.source == *source)
    }

    /// Where the conflicted merge of `path` waits.
    pub(crate) fn sidecar(&self, path: &RelPath) -> Utf8PathBuf {
        path.under(&self.dir().join(CONFLICTS))
    }

    /// The recorded base with fingerprint `record`, checked against it.
    pub(crate) fn base(&self, path: &RelPath, record: &Fingerprint) -> Result<Vec<u8>> {
        let blob = read_optional(&self.dir().join(BASE).join(record.exact.to_string()))?;
        blob.filter(|bytes| Digest::of(bytes) == record.exact)
            .ok_or_else(|| TargetError::CorruptBase { path: path.clone() }.into())
    }

    /// Every recorded path.
    pub(crate) fn records(&self) -> impl Iterator<Item = (&RelPath, &Fingerprint)> {
        self.state.files.iter()
    }

    /// Digest of `state.toml` as read.
    pub(crate) const fn state_digest(&self) -> Option<Digest> {
        self.state_digest
    }

    /// Reads the target at `root`.
    fn load(root: &Utf8Path) -> Result<Self> {
        let dir = root.join(DIR);
        let config_raw = fs_err::read(dir.join(CONFIG))?;
        let config = from_toml(&config_raw, CONFIG)?;
        let config_text = String::from_utf8(config_raw).map_err(io::Error::other)?;
        let lock = read_optional(&dir.join(LOCK))?
            .map_or_else(|| Ok(Lock::default()), |raw| from_toml(&raw, LOCK))?;
        // Deleting `.devset/conflicts/` abandons a withheld update.
        let pending =
            if dir.join(CONFLICTS).is_dir() { read_optional(&dir.join(PENDING))? } else { None };
        let pending = pending.map(|raw| from_toml(&raw, PENDING)).transpose()?;
        let state_raw = read_optional(&dir.join(STATE))?;
        let state_digest = state_raw.as_deref().map(Digest::of);
        let state = state_raw.map_or_else(|| Ok(State::default()), |raw| from_toml(&raw, STATE))?;
        let answers =
            read_optional(&dir.join(ANSWERS))?.map(|raw| from_toml(&raw, ANSWERS)).transpose()?;
        Ok(Self {
            root: root.to_owned(),
            config,
            config_text,
            lock,
            pending,
            state,
            state_digest,
            answers: answers.unwrap_or_default(),
            given: BTreeSet::new(),
        })
    }
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
        Self { version: V1, layers }
    }
}

/// `state.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    /// Format version.
    version: V1,
    /// Each managed path's recorded base.
    #[serde(default)]
    files: BTreeMap<RelPath, Fingerprint>,
}

impl State {
    /// A state recording `files`.
    pub(crate) const fn new(files: BTreeMap<RelPath, Fingerprint>) -> Self {
        Self { version: V1, files }
    }
}

/// The only on-disk format version, `1`.
#[derive(Clone, Copy, Debug, Default)]
struct V1;

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

/// The contents of `path`, or `None` if it does not exist.
pub(crate) fn read_optional(path: &Utf8Path) -> Result<Option<Vec<u8>>> {
    match fs_err::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
