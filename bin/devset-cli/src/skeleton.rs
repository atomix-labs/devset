//! What `new --profile` and `new --collection` write: a profile, or a source of them, to author,
//! its manifest a commented tour of what it can say.

use std::io::Write as _;

use camino::Utf8Path;
use devset_core::Error;
use devset_core::name::{ProfileName, SourceName};

/// Where each template below takes its name.
const NAME: &str = "NAME";

/// Where a profile's manifest takes the devset versions it works with.
const DEVSET: &str = "DEVSET";

/// A profile's manifest, `NAME` its name.
const PROFILE: &str = r#"[profile]
name        = "NAME"
description = "One line: what this profile sets up"
devset      = ">=DEVSET"

# Profiles this one builds on, by name in its source; each applies before it.
# [requires]
# just = {}
# lychee = { optional = true }                # applies only when a feature activates it

# What a target, or a profile requiring this one, may turn on. `default` is on unless a
# layer sets `default-features = false`. A feature turns on other features, `dep:` an
# optional requirement, and `name/feature` a feature of a requirement.
# [features]
# default = ["links"]
# links   = ["dep:lychee"]

# Variables templates and paths use; the target answers them.
# [vars.project]
# prompt = "Project name"

# Starter files, written once, together, when none of `unless` is there.
# [scaffolds.starter]
# unless = "README.md"

# Every file the profile manages, by its path in the target and under files/.
# [files."README.md"]
# scaffold = "starter"                        # written with its scaffold, then the target's
# template = true                             # rendered with the answers and `devset`
#
# [files.".editorconfig"]                     # owned: kept as the profile has it
#
# [files."Cargo.toml"]
# scope = "keys"                              # only the keys its payload defines
# when  = { exists = ["Cargo.toml"] }         # only where the target has one
"#;

/// A profile's README, `NAME` its name.
const PROFILE_README: &str = "# `NAME`\n\nWhat the profile sets up, and the features it offers.\n";

/// A collection's `collection.toml`, `NAME` its name.
const COLLECTION: &str = r#"[collection]
name        = "NAME"
description = "One line: what these profiles are for"
"#;

/// A collection's README, `NAME` its name.
const COLLECTION_README: &str = "# NAME\n\nProfiles for devset. A target takes one with:\n\n\
```sh\ndevset add NAME/example --git <this repository's URL> --tag <a release>\n```\n";

/// Writes a profile to author in `full`, named after it; returns what to do next, naming the
/// directory as `dir`.
///
/// # Errors
/// - [`Error::Name`], the directory's name is not a profile's.
/// - [`Error::Io`], a file cannot be written, or the directory already holds one of them.
pub(crate) fn profile(full: &Utf8Path, dir: &Utf8Path) -> Result<String, Error> {
    let name: ProfileName = full.file_name().unwrap_or_default().parse()?;
    write_profile(full, &name)?;
    Ok(format!(
        "list its files in {dir}/profile.toml and put them under {dir}/files/; a target takes it \
         with `devset add --path {dir}`"
    ))
}

/// Writes a collection of profiles to author in `full`, named after it, with one example
/// profile; returns what to do next, naming the directory as `dir`.
///
/// # Errors
/// As [`profile`].
pub(crate) fn collection(full: &Utf8Path, dir: &Utf8Path) -> Result<String, Error> {
    let name: SourceName = full.file_name().unwrap_or_default().parse()?;
    create(&full.join("collection.toml"), &COLLECTION.replace(NAME, name.as_str()))?;
    create(&full.join("README.md"), &COLLECTION_README.replace(NAME, name.as_str()))?;
    write_profile(&full.join("profiles/example"), &"example".parse()?)?;
    Ok(format!(
        "add profiles under {dir}/profiles/, each a directory with a profile.toml; `devset list \
         --path {dir}` shows them"
    ))
}

/// Writes the profile `name` in `dir`.
fn write_profile(dir: &Utf8Path, name: &ProfileName) -> Result<(), Error> {
    let manifest = PROFILE.replace(NAME, name.as_str()).replace(DEVSET, &release());
    create(&dir.join("profile.toml"), &manifest)?;
    create(&dir.join("README.md"), &PROFILE_README.replace(NAME, name.as_str()))?;
    create(&dir.join("files/.gitkeep"), "")
}

/// This build's release series, `major.minor`: what a profile written now works with.
fn release() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.');
    match (parts.next(), parts.next()) {
        (Some(major), Some(minor)) => format!("{major}.{minor}"),
        _ => version.to_owned(),
    }
}

/// Writes `text` to a new file at `path`, and the directories above it; never over one.
fn create(path: &Utf8Path, text: &str) -> Result<(), Error> {
    if let Some(dir) = path.parent() {
        fs_err::create_dir_all(dir)?;
    }
    let mut file = fs_err::OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(text.as_bytes())?;
    Ok(())
}
