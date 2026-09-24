//! What a profile declares, in its `profile.toml`.
//!
//! A profile is a directory: `profile.toml`, and `files/` mirroring the target. Every path
//! `[files]` lists must exist under `files/`; nothing else there is applied. An entry owns its
//! whole file unless its [`Scope`] says less, when its payload is the part: a partial document,
//! or a block's lines.
//!
//! ```text
//! rust/
//! ├── profile.toml
//! └── files/
//!     ├── rustfmt.toml
//!     └── .github/workflows/ci.yml
//! ```

use alloc::collections::BTreeMap;
use core::fmt;

use schemars::JsonSchema;
use semver::VersionReq;
use serde::de::value::MapAccessDeserializer;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

pub use crate::format::Format;
pub use crate::part::{Comment, Scope};
use crate::path::RelPath;
pub use crate::settings::{MergeSpec, OnConflict};
use crate::source::{Source, SourceSpec};
pub use crate::vars::{VarName, VarSpec};

/// A profile's manifest, at its root.
pub(crate) const MANIFEST: &str = "profile.toml";

/// The directory holding a profile's payload, mirroring the target.
pub(crate) const PAYLOAD: &str = "files";

/// `profile.toml`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Identity and requirements.
    pub profile: Meta,
    /// Merge defaults.
    #[serde(default)]
    pub merge: MergeSpec,
    /// Variables templates use; the target answers them.
    #[serde(default)]
    pub vars: BTreeMap<VarName, VarSpec>,
    /// Managed files, by path in the target and under `files/`.
    #[serde(default)]
    pub files: BTreeMap<RelPath, FileSpec>,
}

/// The `[profile]` table.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Meta {
    /// Names the profile in output, in collisions, and to `update`.
    pub name: String,
    /// For humans; a git layer is pinned by commit.
    #[serde(default)]
    pub version: Option<String>,
    /// One line for humans.
    #[serde(default)]
    pub description: Option<String>,
    /// The devset versions the profile works with.
    #[serde(default)]
    #[schemars(with = "Option<String>")]
    pub devset: Option<VersionReq>,
    /// Profiles this one builds on, each a layer of its own before it.
    #[serde(default)]
    pub requires: Vec<Requirement>,
}

/// A profile another profile builds on.
///
/// A string is a sibling: a path relative to the requiring profile, inside its source and at its
/// revision. A table is a git source, written as a `[[layers]]` entry is.
#[derive(Clone, Debug, PartialEq, Eq, JsonSchema)]
#[schemars(untagged)]
pub enum Requirement {
    /// A profile beside this one, by relative path.
    Sibling(String),
    /// A profile in a git repository, at its own ref.
    Git(#[schemars(with = "SourceSpec")] Source),
}

impl<'de> Deserialize<'de> for Requirement {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// Reads a string as a sibling and a table as a git source.
        struct Visit;

        impl<'de> Visitor<'de> for Visit {
            type Value = Requirement;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a relative path, or a table naming a git source")
            }

            fn visit_str<E: de::Error>(self, path: &str) -> Result<Requirement, E> {
                Ok(Requirement::Sibling(path.to_owned()))
            }

            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Requirement, M::Error> {
                let spec = SourceSpec::deserialize(MapAccessDeserializer::new(map))?;
                if spec.git.is_none() {
                    return Err(de::Error::custom(
                        "a requirement table names a git source; a sibling is a relative path",
                    ));
                }
                Source::try_from(spec)
                    .map(Requirement::Git)
                    .map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_any(Visit)
    }
}

/// A profile's `[files."path"]` entry.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileSpec {
    /// How devset manages the file; `owned` unless set.
    #[serde(default)]
    pub policy: Option<Policy>,
    /// How a merged result is checked; inferred from the extension unless set.
    #[serde(default)]
    pub validate: Option<Format>,
    /// Whether the file is a template, rendered with the target's answers.
    #[serde(default)]
    pub template: bool,
    /// How much of the file devset owns: all of it, the keys the payload defines, or a block.
    #[serde(default)]
    pub scope: Scope,
    /// The comment syntax of a block's markers, where devset does not know the file type's.
    #[serde(default)]
    pub comment: Option<Comment>,
    /// Whether devset writes the file executable, mode 755, as a script it ships must be.
    #[serde(default)]
    pub executable: bool,
}

/// How devset manages a file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Policy {
    /// The profile is authoritative; local edits are drift.
    #[default]
    Owned,
    /// Local edits are kept, and merged with the profile's.
    Merge,
    /// Written once if absent; the target's from then on.
    Once,
}

impl Policy {
    /// Whether it is [`Policy::Owned`], the default.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's `skip_serializing_if` passes a reference"
    )]
    pub(crate) const fn is_owned(&self) -> bool {
        matches!(self, Self::Owned)
    }

    /// The name `profile.toml` uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::Merge => "merge",
            Self::Once => "once",
        }
    }
}
