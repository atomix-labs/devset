//! Settings a profile defaults and a target overrides.

use derive_more::Display;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::errors::{ProfileError, Result};
use crate::merge::Driver;
use crate::name::ProfileName;
use crate::resolve::Layer;

/// A `[merge]` table: defaults in `profile.toml`, decisions in `.devset/config.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct MergeSpec {
    /// What an update does when a file conflicts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_conflict: Option<OnConflict>,
    /// A merge program, with `%O %A %B %P` substituted.
    ///
    /// From a profile it is only a suggestion: devset never runs a program a profile names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub driver: Option<Driver>,
}

/// What an update does when a file conflicts.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Display,
)]
#[serde(rename_all = "kebab-case")]
#[display(rename_all = "kebab-case")]
pub enum OnConflict {
    /// Write every other file and advance the lock; conflicts wait in `.devset/conflicts/`.
    #[default]
    ApplyOthers,
    /// Write nothing but the conflicts until every one is resolved.
    ApplyNone,
}

/// The settings in force: devset's defaults, then each layer's, then the target's.
#[derive(Clone, Debug, Default)]
pub struct Settings {
    /// What an update does when a file conflicts.
    pub on_conflict: OnConflict,
    /// How files are merged.
    pub driver: Driver,
}

/// A merge driver a layer suggests and the target has not configured.
#[derive(Clone, Debug)]
pub struct Suggestion {
    /// The layer's profile name.
    pub layer: ProfileName,
    /// The driver it names.
    pub driver: Driver,
}

impl Settings {
    /// The settings `target` decides, falling back to what `layers` agree on.
    pub(crate) fn compose(layers: &[Layer], target: &MergeSpec) -> Result<Self> {
        let on_conflict = match target.on_conflict {
            Some(value) => value,
            None => {
                agreed(layers, "merge.on-conflict", |merge| merge.on_conflict)?.unwrap_or_default()
            },
        };
        let driver = target.driver.clone().unwrap_or_default();
        Ok(Self { on_conflict, driver })
    }

    /// The drivers `layers` suggest, while `target` configures none.
    pub(crate) fn suggestions(layers: &[Layer], target: &MergeSpec) -> Vec<Suggestion> {
        if target.driver.is_some() {
            return Vec::new();
        }
        layers
            .iter()
            .filter_map(|layer| {
                let driver = layer.merge().driver.clone()?;
                Some(Suggestion { layer: layer.name().clone(), driver })
            })
            .collect()
    }
}

/// The value every layer that sets `key` agrees on.
fn agreed<T: Copy + PartialEq>(
    layers: &[Layer], key: &'static str, get: fn(&MergeSpec) -> Option<T>,
) -> Result<Option<T>> {
    let mut agreed: Option<(T, &Layer)> = None;
    for layer in layers {
        let Some(value) = get(layer.merge()) else {
            continue;
        };
        match agreed {
            Some((first, by)) if first != value => {
                let (first, second) = (by.name().clone(), layer.name().clone());
                return Err(ProfileError::Setting { key: key.to_owned(), first, second }.into());
            },
            Some(_) => {},
            None => agreed = Some((value, layer)),
        }
    }
    Ok(agreed.map(|(value, _)| value))
}
