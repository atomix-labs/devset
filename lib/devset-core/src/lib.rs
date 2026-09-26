//! Versioned file bundles, applied to a directory and updated without losing local edits.
//!
//! A *profile* is a directory of files in a *source*, a git repository or a local directory; a
//! *target* is a directory that names its sources and applies profiles from them as *layers*.
//! Profiles compose as crates do: a profile requires others by name, and offers features that add
//! files, parts, requirements and features of what it requires; an entry applies only when its
//! gates hold. devset records what it wrote in the target's `.devset/`, so a later update tells a
//! local edit from a change the profile made: the edit is kept, and merged with the change where
//! the file's [`Policy`](profile::Policy) says so. A profile owns a whole file, or a part of one
//! its [`Scope`](profile::Scope) names: the keys its payload defines, or a marked block.
//!
//! # Applying a profile
//!
//! Every operation is a prefix of one chain, and only [`commit`](commit()) writes:
//!
//! ```
//! use std::fs;
//!
//! use devset_core::source::Source;
//! use devset_core::target::LayerSpec;
//! use devset_core::{Cache, Mode, Refresh, Target, commit, plan, resolve, survey};
//!
//! let dir = camino_tempfile::tempdir()?;
//! let root = dir.path();
//! # fs::create_dir_all(root.join("house/base/files"))?;
//! # fs::write(root.join("house/base/profile.toml"), "[profile]\nname = \"base\"\n\n[files.\".editorconfig\"]\n")?;
//! # fs::write(root.join("house/base/files/.editorconfig"), "root = true\n")?;
//! # fs::create_dir(root.join("repo"))?;
//! let mut target = Target::open_or_new(&root.join("repo"))?;
//! target.add_source("house".parse()?, Source::Dir("../house".into()))?;
//! target.add_layer(LayerSpec::new("house/base".parse()?))?;
//!
//! let resolved = resolve(&target, &Cache::at(root.join("cache")), Refresh::None)?; // read, compose
//! let survey = survey(resolved, &target)?; // compare with the disk
//! let plan = plan(survey, Mode::Apply, &target)?; // decide, merge
//! commit(plan, &target)?; // write, state last
//!
//! let applied = fs::read_to_string(root.join("repo/.editorconfig"))?;
//! assert_eq!(applied, "root = true\n", "the profile's file, in the target");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Updating
//!
//! [`Refresh::All`] moves every source to what its ref names now; the rest of the chain is the
//! same. A `merge` file then merges its local edits with the profile's new version. A conflict
//! leaves the update unfinished, waiting in `.devset/conflicts/`: a chain under
//! [`Mode::Continue`] installs the fix, or a [`Rollback`] takes the whole update back.
//!
//! # Types
//!
//! - [`Target`]: the directory devset manages, and what its `.devset/` records.
//! - [`Resolved`]: the target's layers composed, one provider per path.
//! - [`Survey`]: every managed path as the profile wants it, as recorded, and as on disk.
//! - [`Plan`]: what committing does to each path.
//! - [`Rollback`]: an unfinished update, and what taking it back restores.
//! - [`Error`]: why any of it did not go through, one type per domain.
//!
//! The files people write have modules of their own: [`profile`] for `profile.toml`,
//! [`collection`] for a source's profiles, [`target`] for `.devset/config.toml`, and [`source`]
//! for where profiles come from; [`name`] holds the names they use.

extern crate alloc;

pub mod collection;
pub mod name;
pub mod plan;
pub mod profile;
pub mod resolve;
pub mod source;
pub mod survey;
pub mod target;

mod commit;
mod digest;
mod errors;
mod format;
mod gate;
mod git;
mod graph;
mod merge;
mod part;
mod path;
mod rollback;
mod settings;
mod tree;
mod vars;

pub use crate::commit::commit;
pub use crate::errors::{
    Error, MergeError, NameError, ParseError, PathError, ProfileError, Result, SourceError,
    TargetError, VarError,
};
pub use crate::path::RelPath;
#[doc(inline)]
pub use crate::plan::{Mode, Plan, plan};
#[doc(inline)]
pub use crate::resolve::{Refresh, Resolved, resolve};
pub use crate::rollback::{Revert, Rollback};
#[doc(inline)]
pub use crate::source::Cache;
#[doc(inline)]
pub use crate::survey::{Survey, survey};
#[doc(inline)]
pub use crate::target::Target;

/// This build's version, which a profile's `devset` requirement is checked against.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
