//! What `init --profile` and `init --collection` write: a profile, or a source of them, to
//! author, its manifest a commented tour of what it can say.

use std::io::Write as _;

use camino::{Utf8Path, Utf8PathBuf};
use devset_core::name::{ProfileName, SourceName};
use devset_core::{Error, TargetError};

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

/// What `init --profile` or `init --collection` wrote.
pub(crate) struct Authored {
    /// What it is, and its name: `profile my-lint`.
    pub(crate) what: String,
    /// What to do next.
    pub(crate) next: String,
}

/// Writes a profile to author in `full`, named after it; what to do next names the directory as
/// `dir`.
///
/// # Errors
/// - [`Error::Name`], the directory's name is not a profile's.
/// - [`TargetError::Occupied`], one of its files is there already; nothing is written.
/// - [`Error::Io`], a file cannot be written.
pub(crate) fn profile(full: &Utf8Path, dir: &Utf8Path) -> Result<Authored, Error> {
    let name: ProfileName = full.file_name().unwrap_or_default().parse()?;
    write(full, dir, &profile_files(Utf8Path::new(""), &name))?;
    let path = if here(dir) { "<its path>".to_owned() } else { dir.to_string() };
    Ok(Authored {
        what: format!("profile {name}"),
        next: format!(
            "list its files in {} and put them under {}; a target takes it with `devset add \
             --path {path}`",
            at(dir, "profile.toml"),
            at(dir, "files/"),
        ),
    })
}

/// Writes a collection of profiles to author in `full`, named after it, with one example
/// profile; what to do next names the directory as `dir`.
///
/// # Errors
/// As [`profile`].
pub(crate) fn collection(full: &Utf8Path, dir: &Utf8Path) -> Result<Authored, Error> {
    let name: SourceName = full.file_name().unwrap_or_default().parse()?;
    let mut files = vec![
        (Utf8PathBuf::from("collection.toml"), COLLECTION.replace(NAME, name.as_str())),
        (Utf8PathBuf::from("README.md"), COLLECTION_README.replace(NAME, name.as_str())),
    ];
    files.extend(profile_files(Utf8Path::new("profiles/example"), &"example".parse()?));
    write(full, dir, &files)?;
    Ok(Authored {
        what: format!("collection {name}"),
        next: format!(
            "add profiles under {}, each a directory with a profile.toml; `devset list --path \
             {dir}` shows them",
            at(dir, "profiles/"),
        ),
    })
}

/// The files of the profile `name`, under `under`.
fn profile_files(under: &Utf8Path, name: &ProfileName) -> Vec<(Utf8PathBuf, String)> {
    let manifest = PROFILE.replace(NAME, name.as_str()).replace(DEVSET, &release());
    vec![
        (under.join("profile.toml"), manifest),
        (under.join("README.md"), PROFILE_README.replace(NAME, name.as_str())),
        (under.join("files/.gitkeep"), String::new()),
    ]
}

/// Whether `dir` is the directory devset runs in.
fn here(dir: &Utf8Path) -> bool {
    dir == Utf8Path::new(".")
}

/// `rel` in `dir`, as a message names it: `rel` alone when `dir` is the directory devset runs in.
fn at(dir: &Utf8Path, rel: &str) -> Utf8PathBuf {
    if here(dir) { Utf8PathBuf::from(rel) } else { dir.join(rel) }
}

/// Writes each of `files` in `full`, named from `dir` in an error: none of them if one is there.
fn write(full: &Utf8Path, dir: &Utf8Path, files: &[(Utf8PathBuf, String)]) -> Result<(), Error> {
    if let Some((path, _)) = files.iter().find(|(path, _)| full.join(path).exists()) {
        return Err(TargetError::Occupied { path: at(dir, path.as_str()) }.into());
    }
    for (path, text) in files {
        create(&full.join(path), text)?;
    }
    Ok(())
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
