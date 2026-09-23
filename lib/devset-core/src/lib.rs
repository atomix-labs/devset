//! Versioned file bundles, applied to a directory and updated without losing local edits.
//!
//! A *profile* is a directory of files, on disk or in git; a *target* is a directory that
//! applies profiles as *layers*. devset records what it wrote in the target's `.devset/`, so a
//! later update tells a local edit from a change the profile made: the edit is kept, and merged
//! with the change where the file's [`Policy`](profile::Policy) says so.
//!
//! # Applying a profile
//! Every operation is a prefix of one chain, and only [`commit`](commit()) writes:
//!
//! ```
//! use std::fs;
//!
//! use camino::Utf8Path;
//! use devset_core::source::Source;
//! use devset_core::{Cache, Mode, Refresh, Target, commit, plan, resolve, survey};
//!
//! let dir = tempfile::tempdir()?;
//! let root = Utf8Path::from_path(dir.path()).expect("a UTF-8 temporary directory");
//! # fs::create_dir_all(root.join("base/files"))?;
//! # fs::write(root.join("base/profile.toml"), "[profile]\nname = \"base\"\n\n[files.\".editorconfig\"]\n")?;
//! # fs::write(root.join("base/files/.editorconfig"), "root = true\n")?;
//! # fs::create_dir(root.join("repo"))?;
//! let mut target = Target::at(&root.join("repo"))?;
//! target.add_layer(Source::Dir("../base".into()))?;
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
//! [`Refresh::All`] moves every layer to what its ref names now; the rest of the chain is the
//! same. A `merge` file then merges its local edits with the profile's new version. A conflict
//! waits in `.devset/conflicts/` until a chain under [`Mode::Continue`] installs the fix.
//!
//! # Types
//! - [`Target`]: the directory devset manages, and what its `.devset/` records.
//! - [`Resolved`]: the target's layers composed, one provider per path.
//! - [`Survey`]: every managed path as the profile wants it, as recorded, and as on disk.
//! - [`Plan`]: what committing does to each path.
//! - [`Error`]: why any of it did not go through, one type per domain.
//!
//! The files people write have modules of their own: [`profile`] for `profile.toml`,
//! [`target`] for `.devset/config.toml`, and [`source`] for where a layer comes from.

extern crate alloc;

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
mod git;
mod merge;
mod path;
mod settings;
mod tree;
mod vars;

pub use crate::commit::commit;
pub use crate::errors::{
    Error, MergeError, ParseError, PathError, ProfileError, Result, SourceError, TargetError,
    VarError,
};
pub use crate::path::RelPath;
#[doc(inline)]
pub use crate::plan::{Mode, Plan, plan};
#[doc(inline)]
pub use crate::resolve::{Refresh, Resolved, resolve};
#[doc(inline)]
pub use crate::source::Cache;
#[doc(inline)]
pub use crate::survey::{Survey, survey};
#[doc(inline)]
pub use crate::target::Target;

/// This build's version, which a profile's `devset` requirement is checked against.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
