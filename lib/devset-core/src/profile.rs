//! What a profile declares, in its `profile.toml`.
//!
//! A profile is a directory: `profile.toml`, and `files/` mirroring the target. Every path
//! `[files]` lists must exist under `files/`; nothing else there is applied.
//!
//! ```text
//! rust/
//! ├── profile.toml
//! └── files/
//!     ├── rustfmt.toml
//!     └── .github/workflows/ci.yml
//! ```

use alloc::collections::BTreeMap;

use schemars::JsonSchema;
use semver::VersionReq;
use serde::{Deserialize, Serialize};

pub use crate::format::Format;
use crate::path::RelPath;
pub use crate::settings::{MergeSpec, OnConflict};
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
}

/// A profile's `[files."path"]` entry.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
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
