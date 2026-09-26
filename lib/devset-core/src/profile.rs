//! What a profile declares, in its `profile.toml`.
//!
//! A profile is a directory in a source: `profile.toml`, and `files/` mirroring the target. Every
//! path `[files]` lists must exist under `files/`; nothing else there is applied. An entry owns its
//! whole file unless its [`Scope`] says less, when its payload is the part: a partial document, or
//! a block's lines. A path may use variables, `crates/{{ name }}/Cargo.toml`, and its payload is
//! found under `files/` as written.
//!
//! ```text
//! mdbook/
//! ├── profile.toml
//! └── files/
//!     ├── {{ book_dir }}/book.toml
//!     └── {{ book_dir }}/theme/katex.css
//! ```
//!
//! A profile builds on others by name, `[requires]`, and offers features, `[features]`, which add
//! files, parts, requirements and features of what it requires; an entry applies only when its
//! [`When`] holds. A scaffold, `[scaffolds]`, is a group of starter files written once, when none
//! of its sentinels is there.

use alloc::collections::{BTreeMap, BTreeSet};
use core::fmt;
use core::str::FromStr;

use derive_more::Display;
use schemars::JsonSchema;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use serde_with::DeserializeFromStr;

use crate::errors::{NameError, ProfileError, Result};
pub use crate::format::Format;
pub use crate::name::{FeatureName, ProfileName, ScaffoldName};
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
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Identity.
    pub profile: Meta,
    /// Profiles this one builds on, by name: each a layer before it.
    #[serde(default)]
    #[schemars(with = "BTreeMap<ProfileName, RequirementSpec>")]
    pub requires: BTreeMap<ProfileName, Requirement>,
    /// What a target or a requirer may turn on.
    #[serde(default)]
    pub features: Features,
    /// Variables templates and paths use; the target answers them.
    #[serde(default)]
    pub vars: BTreeMap<VarName, VarSpec>,
    /// Merge defaults.
    #[serde(default)]
    pub merge: MergeSpec,
    /// Groups of starter files, each written once, when none of its sentinels is there.
    #[serde(default)]
    pub scaffolds: BTreeMap<ScaffoldName, ScaffoldSpec>,
    /// Managed files, by path in the target, as found under `files/`; a path may use variables.
    #[serde(default)]
    pub files: BTreeMap<RelPath, FileSpec>,
}

/// The `[profile]` table.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Meta {
    /// Names the profile: in its source, among the layers, in output, and in `from`.
    pub name: ProfileName,
    /// For humans; a git source is pinned by commit.
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

/// A profile another builds on, keyed by its name in `[requires]`.
///
/// In the requirer's own source unless it names another by `git`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RequirementSpec")]
pub struct Requirement {
    /// Its features to turn on.
    pub features: Vec<FeatureName>,
    /// Whether its default features are on; unless a requirer or the target turns them on.
    pub default_features: bool,
    /// Whether it applies only when a feature activates it, with `dep:` or `name/feature`.
    pub optional: bool,
    /// Another source it comes from; `None` for the requirer's own.
    pub source: Option<Source>,
}

/// A requirement as written.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RequirementSpec {
    /// Its features to turn on.
    #[serde(default)]
    features: Vec<FeatureName>,
    /// Whether its default features are on; `true` unless set.
    #[serde(default = "yes")]
    default_features: bool,
    /// Whether it applies only when a feature activates it.
    #[serde(default)]
    optional: bool,
    /// The repository of another source.
    #[serde(default)]
    git: Option<String>,
    /// Its tag; only with `git`.
    #[serde(default)]
    tag: Option<String>,
    /// Its branch; only with `git`.
    #[serde(default)]
    branch: Option<String>,
    /// Its commit; only with `git`.
    #[serde(default)]
    rev: Option<String>,
    /// The directory in the repository its profiles are in; only with `git`.
    #[serde(default)]
    path: Option<String>,
}

/// `true`, serde's default for a flag that is on unless set.
const fn yes() -> bool {
    true
}

impl TryFrom<RequirementSpec> for Requirement {
    type Error = String;

    fn try_from(spec: RequirementSpec) -> Result<Self, String> {
        let RequirementSpec { features, default_features, optional, git, tag, branch, rev, path } =
            spec;
        let located = tag.is_some() || branch.is_some() || rev.is_some() || path.is_some();
        let source = match git {
            Some(git) => {
                let spec = SourceSpec { git: Some(git), tag, branch, rev, path };
                Some(Source::try_from(spec).map_err(|e| e.to_string())?)
            },
            None if located => {
                return Err("`tag`, `branch`, `rev` and `path` name another source, with `git`; \
                            a profile in this one is required by name alone"
                    .to_owned());
            },
            None => None,
        };
        Ok(Self { features, default_features, optional, source })
    }
}

/// The `[features]` table: `default`, and every feature with what it turns on.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(try_from = "BTreeMap<String, Vec<String>>")]
#[schemars(with = "BTreeMap<String, Vec<String>>")]
pub struct Features {
    /// The features on unless a requirer or the target sets `default-features = false`.
    pub default: Vec<FeatureName>,
    /// Every feature, and what turning it on turns on.
    pub declared: BTreeMap<FeatureName, Vec<FeatureRef>>,
}

impl TryFrom<BTreeMap<String, Vec<String>>> for Features {
    type Error = NameError;

    fn try_from(table: BTreeMap<String, Vec<String>>) -> Result<Self, NameError> {
        let mut features = Self::default();
        for (name, list) in table {
            if name == "default" {
                features.default =
                    list.into_iter().map(FeatureName::try_from).collect::<Result<_, _>>()?;
            } else {
                let refs = list.iter().map(|each| each.parse()).collect::<Result<_, _>>()?;
                features.declared.insert(FeatureName::try_from(name)?, refs);
            }
        }
        Ok(features)
    }
}

/// What a feature turns on.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, DeserializeFromStr)]
pub enum FeatureRef {
    /// Another feature of the profile: `katex`.
    Feature(FeatureName),
    /// An optional requirement, activated: `dep:lychee`.
    Dep(ProfileName),
    /// A feature of a requirement: `github-ci/pages`, which activates it too; or, when `weak`,
    /// `mdbook?/agents`, only if it is active by other means.
    Of {
        /// The requirement.
        profile: ProfileName,
        /// Its feature.
        feature: FeatureName,
        /// Whether it leaves the requirement inactive.
        weak: bool,
    },
}

impl FromStr for FeatureRef {
    type Err = NameError;

    fn from_str(written: &str) -> Result<Self, NameError> {
        if let Some(profile) = written.strip_prefix("dep:") {
            return Ok(Self::Dep(profile.parse()?));
        }
        let Some((profile, feature)) = written.split_once('/') else {
            return Ok(Self::Feature(written.parse()?));
        };
        let (profile, weak) =
            profile.strip_suffix('?').map_or((profile, false), |profile| (profile, true));
        Ok(Self::Of { profile: profile.parse()?, feature: feature.parse()?, weak })
    }
}

impl fmt::Display for FeatureRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Feature(feature) => write!(f, "{feature}"),
            Self::Dep(profile) => write!(f, "dep:{profile}"),
            Self::Of { profile, feature, weak } => {
                write!(f, "{profile}{}/{feature}", if *weak { "?" } else { "" })
            },
        }
    }
}

/// A `[scaffolds.name]` entry: a group of starter files, written together once.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScaffoldSpec {
    /// Paths or globs of which any, there before the run, means the target has its own: the group
    /// is not written. A string or a list; none writes the group when the profile is first
    /// applied.
    #[serde(default)]
    #[schemars(with = "Patterns")]
    pub unless: Patterns,
}

/// Paths or globs in the target: a string, or a list of them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(from = "OneOrMany")]
pub struct Patterns(pub Vec<String>);

/// One pattern, or several.
#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
enum OneOrMany {
    /// One.
    One(String),
    /// Several.
    Many(Vec<String>),
}

impl From<OneOrMany> for Patterns {
    fn from(written: OneOrMany) -> Self {
        match written {
            OneOrMany::One(one) => Self(vec![one]),
            OneOrMany::Many(many) => Self(many),
        }
    }
}

/// A profile's `[files."path"]` entry.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileSpec {
    /// How devset manages the file; `owned` unless set, and `once` in a scaffold.
    #[serde(default)]
    pub policy: Option<Policy>,
    /// How a merged result is checked; inferred from the extension unless set.
    #[serde(default)]
    pub validate: Option<Format>,
    /// Whether the file is a template, rendered with the answers and the `devset` object.
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
    /// The scaffold group the file belongs to, written with it.
    #[serde(default)]
    pub scaffold: Option<ScaffoldName>,
    /// For a part, a file under `files/` the target's file starts from when it is absent.
    #[serde(default)]
    pub starter: Option<RelPath>,
    /// When the entry applies; always, unless set.
    #[serde(default)]
    pub when: When,
}

/// When an entry applies: every condition holds.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct When {
    /// Features of this profile that are on.
    #[serde(default)]
    pub features: Vec<FeatureName>,
    /// Profiles active in the target.
    #[serde(default)]
    pub profiles: Vec<ProfileName>,
    /// Variables of this profile, each answered with one of its values.
    #[serde(default)]
    pub vars: BTreeMap<VarName, Vec<String>>,
    /// Paths or globs that will exist when the run finishes, whole files or parts; may use
    /// variables.
    #[serde(default)]
    pub exists: Vec<String>,
}

/// How devset manages a file.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Display,
)]
#[serde(rename_all = "lowercase")]
#[display(rename_all = "lowercase")]
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
}

impl FileSpec {
    /// Its policy as written, `once` in a scaffold, or `owned`.
    #[must_use]
    pub fn policy(&self) -> Policy {
        let default = if self.scaffold.is_some() { Policy::Once } else { Policy::Owned };
        self.policy.unwrap_or(default)
    }
}

impl Manifest {
    /// Checks what the manifest says of itself: every feature, requirement, scaffold and variable
    /// it names is one it declares, and every entry is one devset can apply.
    ///
    /// # Errors
    /// [`ProfileError::Manifest`], naming what is wrong in `file`.
    pub(crate) fn check(&self, file: &str) -> Result<()> {
        let invalid = |message: String| ProfileError::Manifest { file: file.to_owned(), message };
        let features = &self.features;
        for feature in &features.default {
            if !features.declared.contains_key(feature) {
                return Err(invalid(format!(
                    "`default` lists `{feature}`, which [features] lacks"
                ))
                .into());
            }
        }
        for (feature, refs) in &features.declared {
            for each in refs {
                self.check_ref(each).map_err(|why| {
                    invalid(format!("feature `{feature}` turns on `{each}`, but {why}"))
                })?;
            }
        }
        for (path, spec) in &self.files {
            self.check_entry(spec).map_err(|why| invalid(format!("[files.\"{path}\"]: {why}")))?;
        }
        Ok(())
    }

    /// Why `each`, in a feature's list, names nothing this manifest declares.
    fn check_ref(&self, each: &FeatureRef) -> Result<(), String> {
        match each {
            FeatureRef::Feature(feature) if !self.features.declared.contains_key(feature) => {
                Err(format!("[features] lacks `{feature}`"))
            },
            FeatureRef::Dep(profile) => match self.requires.get(profile) {
                None => Err(format!("[requires] lacks `{profile}`")),
                Some(requirement) if !requirement.optional => {
                    Err(format!("`{profile}` is not an optional requirement"))
                },
                Some(_) => Ok(()),
            },
            FeatureRef::Of { profile, .. } if !self.requires.contains_key(profile) => {
                Err(format!("[requires] lacks `{profile}`"))
            },
            FeatureRef::Feature(_) | FeatureRef::Of { .. } => Ok(()),
        }
    }

    /// Why `spec` cannot be applied as written.
    fn check_entry(&self, spec: &FileSpec) -> Result<(), String> {
        let when = &spec.when;
        if let Some(feature) =
            when.features.iter().find(|f| !self.features.declared.contains_key(*f))
        {
            return Err(format!("`when` names feature `{feature}`, which [features] lacks"));
        }
        if let Some(name) = when.vars.keys().find(|name| !self.vars.contains_key(*name)) {
            return Err(format!("`when` names variable `{name}`, which [vars] lacks"));
        }
        if let Some(group) = &spec.scaffold {
            if !self.scaffolds.contains_key(group) {
                return Err(format!("scaffold `{group}` is not in [scaffolds]"));
            }
            if spec.scope != Scope::File || spec.policy.is_some_and(|p| p != Policy::Once) {
                return Err("a scaffold's file is a whole file, `once`".to_owned());
            }
        }
        if spec.starter.is_some() && spec.scope == Scope::File {
            return Err("`starter` is for a part: a whole file is its own".to_owned());
        }
        Ok(())
    }

    /// Every payload file the manifest names, as written: each entry's, and each starter.
    pub(crate) fn payload_paths(&self) -> BTreeSet<RelPath> {
        let starters = self.files.values().filter_map(|spec| spec.starter.clone());
        self.files.keys().cloned().chain(starters).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{FeatureRef, Manifest};

    /// The manifest `toml`, checked: its error as text if either step fails.
    fn read(toml: &str) -> Result<Manifest, String> {
        let manifest: Manifest = toml::from_str(toml).map_err(|e| e.message().to_owned())?;
        manifest.check("profile.toml").map_err(|e| e.to_string())?;
        Ok(manifest)
    }

    const MDBOOK: &str = r#"
[profile]
name = "mdbook"

[requires]
just = {}
lychee = { optional = true }
github-ci = { optional = true, features = ["pages"], default-features = false }
extern = { git = "https://example.com/p", tag = "v1" }

[features]
default = ["katex", "links"]
katex = []
api = ["katex"]
pages = ["github-ci/pages"]
links = ["dep:lychee"]
agents = ["github-ci?/agents"]

[vars.book_dir]
default = "docs"

[scaffolds.book]
unless = "{{ book_dir }}/book.toml"

[files."{{ book_dir }}/SUMMARY.md"]
scaffold = "book"

[files."{{ book_dir }}/book.toml"]
scope = "keys"
starter = "book.starter.toml"
template = true

[files."{{ book_dir }}/theme/api-link.js"]
when = { features = ["api"], vars = { book_dir = ["docs"] }, exists = ["**/*.rs"] }
"#;

    #[test]
    fn a_full_manifest_reads() {
        let manifest = read(MDBOOK).expect("valid");
        let features = &manifest.features;
        assert_eq!(features.default.len(), 2, "the default list");
        assert_eq!(features.declared.len(), 5, "every feature but `default`");
        let github = manifest.requires.iter().find(|(name, _)| name.as_str() == "github-ci");
        let github = github.expect("required").1;
        assert!(github.optional && !github.default_features, "its flags");
        assert!(
            manifest.requires.iter().any(|(_, r)| r.source.is_some()),
            "one names another source"
        );
        assert_eq!(manifest.payload_paths().len(), 4, "every entry's file, and the starter");
    }

    #[test]
    fn feature_refs() {
        for (written, want) in [
            ("katex", "Feature"),
            ("dep:lychee", "Dep"),
            ("github-ci/pages", "Of"),
            ("mdbook?/agents", "Weak"),
        ] {
            let parsed: FeatureRef = written.parse().expect("a reference");
            let kind = match &parsed {
                FeatureRef::Feature(_) => "Feature",
                FeatureRef::Dep(_) => "Dep",
                FeatureRef::Of { weak: false, .. } => "Of",
                FeatureRef::Of { weak: true, .. } => "Weak",
            };
            assert_eq!(kind, want, "{written}");
            assert_eq!(parsed.to_string(), written, "written as read");
        }
        for bad in ["dep:", "a/b/c", "?x", "a?/", "x?"] {
            assert!(bad.parse::<FeatureRef>().is_err(), "{bad:?} is not a reference");
        }
    }

    #[test]
    fn mistakes_are_named() {
        let base = "[profile]\nname = \"p\"\n";
        for (tail, says) in [
            ("[features]\ndefault = [\"x\"]\n", "`default` lists `x`"),
            ("[features]\na = [\"b\"]\n", "[features] lacks `b`"),
            ("[features]\na = [\"dep:q\"]\n", "[requires] lacks `q`"),
            ("[requires]\nq = {}\n[features]\na = [\"dep:q\"]\n", "not an optional requirement"),
            ("[features]\na = [\"q/f\"]\n", "[requires] lacks `q`"),
            ("[files.\"a\"]\nwhen = { features = [\"x\"] }\n", "feature `x`"),
            ("[files.\"a\"]\nwhen = { vars = { v = [\"1\"] } }\n", "variable `v`"),
            ("[files.\"a\"]\nscaffold = \"s\"\n", "scaffold `s` is not"),
            ("[scaffolds.s]\n[files.\"a\"]\nscaffold = \"s\"\npolicy = \"owned\"\n", "`once`"),
            ("[scaffolds.s]\n[files.\"a\"]\nscaffold = \"s\"\nscope = \"keys\"\n", "`once`"),
            ("[files.\"a\"]\nstarter = \"s\"\n", "`starter` is for a part"),
            ("[vars.devset]\n", "reserved"),
            ("[requires]\nq = { tag = \"v1\" }\n", "with `git`"),
            ("[requires]\nq = { path = \"x\" }\n", "with `git`"),
        ] {
            let error = read(&format!("{base}{tail}")).expect_err(tail);
            assert!(error.contains(says), "{tail}: {error}");
        }
    }
}
