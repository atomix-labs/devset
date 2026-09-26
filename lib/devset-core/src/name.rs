//! The names profiles, sources, features and scaffolds go by, and `source/profile`.
//!
//! Every name is ASCII letters, digits, `-` and `_`, beginning with a letter or a digit, so it
//! reads the same in TOML keys, on the command line and in paths.

use alloc::borrow::Cow;
use core::fmt;
use core::str::FromStr;

use derive_more::{AsRef, Display, Into};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

use crate::errors::NameError;

/// Defines a name: a checked string, compared and sorted as written.
macro_rules! name {
    ($(#[$doc:meta])* $name:ident, $kind:literal) => {
        $(#[$doc])*
        #[derive(
            Clone,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            AsRef,
            Display,
            derive_more::Debug,
            Into,
            Serialize,
            Deserialize,
            JsonSchema,
        )]
        #[as_ref(str)]
        #[debug("{_0:?}")]
        #[into(String)]
        #[serde(try_from = "String", into = "String")]
        #[schemars(inline)]
        pub struct $name(Box<str>);

        impl $name {
            /// The name as written.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = NameError;

            fn try_from(name: String) -> Result<Self, NameError> {
                match check(&name, $kind) {
                    Ok(()) => Ok(Self(name.into_boxed_str())),
                    Err(rule) => Err(NameError { kind: $kind, name, rule }),
                }
            }
        }

        impl FromStr for $name {
            type Err = NameError;

            fn from_str(name: &str) -> Result<Self, NameError> {
                Self::try_from(name.to_owned())
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                *self.0 == *other
            }
        }
    };
}

name! {
    /// A profile's name, unique in its source and among a target's layers.
    ProfileName, "profile"
}

name! {
    /// The name a target gives a source in `[sources]`.
    SourceName, "source"
}

name! {
    /// A feature of a profile; never `default`, which names the list of features on by default.
    FeatureName, "feature"
}

name! {
    /// A group of starter files in a profile's `[scaffolds]`.
    ScaffoldName, "scaffold"
}

/// The first rule `name`, a name of `kind`, breaks.
fn check(name: &str, kind: &str) -> Result<(), &'static str> {
    let mut chars = name.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphanumeric()) {
        return Err("begin with a letter or a digit");
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
        return Err("use letters, digits, `-` and `_`");
    }
    if kind == "feature" && name == "default" {
        return Err("`default` names the list of features on by default");
    }
    Ok(())
}

/// A profile in a named source, `source/profile`: how a target names a layer.
#[derive(
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    derive_more::Debug,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[debug("{self}")]
pub struct ProfileRef {
    /// The source, as `[sources]` names it.
    pub source: SourceName,
    /// The profile, by its name in that source.
    pub profile: ProfileName,
}

impl fmt::Display for ProfileRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.source, self.profile)
    }
}

impl FromStr for ProfileRef {
    type Err = NameError;

    fn from_str(written: &str) -> Result<Self, NameError> {
        let unqualified = || NameError {
            kind: "layer",
            name: written.to_owned(),
            rule: "write `source/profile`",
        };
        let (source, profile) = written.split_once('/').ok_or_else(unqualified)?;
        Ok(Self { source: source.parse()?, profile: profile.parse()? })
    }
}

/// A scaffold of a profile, `profile/group`: how the command line and `state.toml` name it.
#[derive(
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    derive_more::Debug,
    SerializeDisplay,
    DeserializeFromStr,
)]
#[debug("{self}")]
pub struct ScaffoldId {
    /// The profile.
    pub profile: ProfileName,
    /// The group, in its `[scaffolds]`.
    pub group: ScaffoldName,
}

impl fmt::Display for ScaffoldId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.profile, self.group)
    }
}

impl FromStr for ScaffoldId {
    type Err = NameError;

    fn from_str(written: &str) -> Result<Self, NameError> {
        let unqualified = || NameError {
            kind: "scaffold",
            name: written.to_owned(),
            rule: "write `profile/group`",
        };
        let (profile, group) = written.split_once('/').ok_or_else(unqualified)?;
        Ok(Self { profile: profile.parse()?, group: group.parse()? })
    }
}

impl JsonSchema for ProfileRef {
    fn schema_name() -> Cow<'static, str> {
        "ProfileRef".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A profile in a named source: `source/profile`.",
            "type": "string",
            "pattern": "^[A-Za-z0-9][A-Za-z0-9_-]*/[A-Za-z0-9][A-Za-z0-9_-]*$",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{FeatureName, ProfileName, ProfileRef};

    #[test]
    fn names() {
        for ok in ["rust", "cargo-deny", "a_b", "x1", "1x"] {
            assert!(ProfileName::try_from(ok.to_owned()).is_ok(), "{ok} is a name");
        }
        for bad in ["", "-x", "_x", "a/b", "a b", "a.b", "é"] {
            assert!(ProfileName::try_from(bad.to_owned()).is_err(), "{bad:?} is not");
        }
        assert!(FeatureName::try_from("default".to_owned()).is_err(), "`default` names the list");
        assert!(ProfileName::try_from("default".to_owned()).is_ok(), "which a profile may be");
    }

    #[test]
    fn profile_refs() {
        let at: ProfileRef = "atxp/rust".parse().expect("a reference");
        assert_eq!((at.source.as_str(), at.profile.as_str()), ("atxp", "rust"), "its parts");
        assert_eq!(at.to_string(), "atxp/rust", "written as read");
        for bad in ["rust", "atxp/", "/rust", "a/b/c"] {
            assert!(bad.parse::<ProfileRef>().is_err(), "{bad} is not a reference");
        }
    }
}
