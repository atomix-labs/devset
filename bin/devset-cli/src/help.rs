//! The next step to suggest for an [`Error`], in the terms of this command line.

use devset_core::name::ProfileName;
use devset_core::{Error, MergeError, ProfileError, RelPath, SourceError, TargetError, VarError};

use crate::words::{list, nearest};

/// The next step after `error`, when there is one to suggest.
pub(crate) fn help(error: &Error) -> Option<String> {
    match error {
        Error::Source(error) => source(error),
        Error::Profile(error) => profile(error),
        Error::Target(error) => target(error),
        Error::Var(error) => var(error),
        Error::Merge(error) => merge(error),
        Error::Io(_) | Error::Parse(_) | Error::Path(_) | Error::Name(_) | _ => None,
    }
}

/// [`help`] for a [`SourceError`].
fn source(error: &SourceError) -> Option<String> {
    match error {
        SourceError::NoGit => Some("install git, or use a local source with `--path`".into()),
        SourceError::NoRef { url, reference } => Some(match reference.split_once(' ') {
            Some(("branch", _)) => format!("list its branches with `git ls-remote --heads {url}`"),
            Some(("tag", _)) => format!("list its tags with `git ls-remote --tags {url}`"),
            _ => "name a branch or a tag with `--branch` or `--tag`".into(),
        }),
        SourceError::NoCommit { .. } => {
            Some("check the commit id, or follow a branch or a tag instead".into())
        },
        SourceError::Credentials { .. } => Some(
            "give git credentials for it: a credential helper (for GitHub, `gh auth setup-git`), \
             or an SSH key in ssh-agent"
                .into(),
        ),
        SourceError::NoLocation
        | SourceError::EmptyPath
        | SourceError::RefWithoutGit
        | SourceError::TwoRefs
        | SourceError::Url { .. }
        | SourceError::Rev { .. }
        | SourceError::Path(_)
        | SourceError::NotAFile { .. }
        | SourceError::Git { .. }
        | _ => None,
    }
}

/// [`help`] for a [`ProfileError`].
fn profile(error: &ProfileError) -> Option<String> {
    match error {
        ProfileError::NoProfiles { .. } => Some(
            "a source holds profiles, each a directory with a profile.toml; `devset new \
             --profile <dir>` starts one"
                .into(),
        ),
        ProfileError::NotInSource { name, profiles, .. } => {
            Some(nearest(name, profiles).map_or_else(
                || {
                    format!(
                        "it holds {}; `devset list` shows them, and `devset update <source>` \
                         takes a newer commit of the source",
                        list(profiles)
                    )
                },
                |near| format!("did you mean `{near}`?"),
            ))
        },
        ProfileError::SameNameInSource { .. } => {
            Some("rename one of them: a name tells profiles apart in a source".into())
        },
        ProfileError::Incompatible { .. } => {
            Some("upgrade devset, or pin the source to an older version".into())
        },
        ProfileError::MissingPayload { path, .. } => {
            Some(format!("add files/{path} to the profile, or remove what lists it"))
        },
        ProfileError::UnknownFeature { feature, known, .. } => {
            Some(match (nearest(feature, known), known.as_slice()) {
                (Some(near), _) => format!("did you mean `{near}`?"),
                (None, []) => "the profile declares no features".to_owned(),
                (None, _) => format!("its features are {}", list(known)),
            })
        },
        ProfileError::SamePath { .. } => {
            Some("answer the variables so the paths differ, or list the file once".into())
        },
        ProfileError::Collision { path, layers } | ProfileError::Starters { path, layers } => {
            Some(pick(path, layers))
        },
        ProfileError::Scopes { path, layers } => {
            let names: Vec<ProfileName> = layers.iter().map(|(layer, _)| layer.clone()).collect();
            Some(pick(path, &names))
        },
        ProfileError::Overlap { path, first, second, .. } => {
            Some(pick(path, &[first.clone(), second.clone()]))
        },
        ProfileError::NoKeys { path } => Some(format!(
            "name the format it is written in, or own it another way:\n    [files.\"{path}\"]\n    validate = \"toml\"   # or scope = \"file\", or scope = \"block\""
        )),
        ProfileError::NoComment { path } => Some(format!(
            "name the syntax on its entry in profile.toml:\n    [files.\"{path}\"]\n    comment = \"#\""
        )),
        ProfileError::NotProvided { from, layers, .. } => {
            let near = nearest(from, layers).map(|near| format!("did you mean `{near}`? "));
            Some(format!("{}it is provided by {}", near.unwrap_or_default(), list(layers)))
        },
        ProfileError::FoldCollision { .. } => Some("rename one of them in the profile".into()),
        ProfileError::Cycle { .. } => Some("drop one requirement to break the cycle".into()),
        ProfileError::Diverged { .. } => {
            Some("use the source at one ref everywhere: move the older one forward".into())
        },
        ProfileError::SameName { .. } => Some(
            "a layer's name picks it in `from`, `remove` and `update`: rename one profile, or \
             remove one layer with `devset remove`"
                .into(),
        ),
        ProfileError::Setting { key, .. } => {
            Some(format!("decide it in .devset/config.toml by setting {key}"))
        },
        ProfileError::Manifest { .. } | ProfileError::Pattern { .. } | _ => None,
    }
}

/// The `from` that settles which of `layers` provides `path`.
fn pick(path: &RelPath, layers: &[ProfileName]) -> String {
    let options: Vec<String> = layers.iter().map(|layer| format!("\"{layer}\"")).collect();
    let options = options.join("   # or ");
    format!("choose one in .devset/config.toml:\n    [files.\"{path}\"]\n    from = {options}")
}

/// [`help`] for a [`TargetError`].
fn target(error: &TargetError) -> Option<String> {
    match error {
        TargetError::NotFound { .. } => Some(
            "start one: `devset init`, or `devset new <dir>`, with a first layer if you like"
                .into(),
        ),
        TargetError::Nested { root, .. } => Some(format!("run devset from {root}")),
        TargetError::OldConfig => Some(
            "name each source once in [sources], and each layer as `profile = \"source/name\"`; \
             BREAKING-CHANGES.md in devset's repository shows how"
                .into(),
        ),
        TargetError::NoSuchSource { name, sources, .. } => {
            Some(match (nearest(name, sources), sources.as_slice()) {
                (Some(near), _) => format!("did you mean `{near}`?"),
                (None, []) => {
                    "the target names no source: add one with `--git <url>` or `--path <dir>`"
                        .into()
                },
                (None, _) => format!("the sources are {}", list(sources)),
            })
        },
        TargetError::SourceExists { name, .. } => Some(format!(
            "name the new one otherwise: `devset add <name>/<profile>`, not `{name}/<profile>`"
        )),
        TargetError::WhichSource { profile, sources } => Some(match sources.as_slice() {
            [] => format!(
                "the target names no source: `devset add <source>/{profile} --git <url>`, or \
                 `--path <dir>`"
            ),
            _ => format!("the sources are {}", list(sources)),
        }),
        TargetError::Ambiguous { profiles, .. } => {
            Some(format!("its profiles are {}", list(profiles)))
        },
        TargetError::NoSuchLayer { name, names } => {
            Some(match (nearest(name, names), names.as_slice()) {
                (Some(near), _) => format!("did you mean `{near}`?"),
                (None, []) => "the target has no layers; add one with `devset add`".to_owned(),
                (None, _) => format!("the layers and sources are {}", list(names)),
            })
        },
        TargetError::NoSuchScaffold { name, scaffolds } => {
            Some(match (nearest(name, scaffolds), scaffolds.as_slice()) {
                (Some(near), _) => format!("did you mean `{near}`?"),
                (None, []) => "no active profile has a scaffold".to_owned(),
                (None, _) => format!("the scaffolds are {}", list(scaffolds)),
            })
        },
        TargetError::NotManaged { path, managed } => Some(nearest(path, managed).map_or_else(
            || "`devset status -v` lists the managed files".to_owned(),
            |near| format!("did you mean `{near}`?"),
        )),
        TargetError::StaleOverride { path, provided } => Some(nearest(path, provided).map_or_else(
            || format!("remove its [files.\"{path}\"] table"),
            |near| format!("did you mean `{near}`?"),
        )),
        TargetError::NotAFile { path, .. } => Some(format!(
            "devset manages regular files: move {path} aside, and `devset apply` writes it again"
        )),
        TargetError::Required { by, .. } => Some(format!("remove {by}, which brings it in")),
        TargetError::Busy => Some("wait for it to finish".into()),
        TargetError::Concurrent => Some("run the command again".into()),
        TargetError::LayerTwice { .. } => {
            Some("keep one [[layers]] entry for it, with the features of both".into())
        },
        TargetError::DuplicateLayer { .. } | _ => None,
    }
}

/// [`help`] for a [`VarError`].
fn var(error: &VarError) -> Option<String> {
    let did_you_mean = |name: &str, declared: &[_]| {
        nearest(name, declared).map(|near| format!("did you mean `{near}`?"))
    };
    match error {
        VarError::Unknown { name, declared } => Some(
            did_you_mean(name.as_str(), declared).unwrap_or_else(|| match declared.as_slice() {
                [] => "no profile declares any variables".to_owned(),
                _ => format!("the variables are {}", list(declared)),
            }),
        ),
        VarError::Unanswered { questions } => {
            let flags: Vec<String> =
                questions.iter().map(|q| format!("--var {}=…", q.name)).collect();
            Some(format!("answer with `{}`", flags.join(" ")))
        },
        VarError::Undeclared { name, declared, .. } => Some(
            did_you_mean(name, declared)
                .unwrap_or_else(|| format!("declare it in the profile: [vars.{name}]")),
        ),
        VarError::Reserved => Some("name the variable otherwise".into()),
        VarError::Name { .. } | VarError::Template { .. } | _ => None,
    }
}

/// [`help`] for a [`MergeError`].
fn merge(error: &MergeError) -> Option<String> {
    match error {
        MergeError::NoDriver { .. } => Some(
            "install it, or remove `driver` from .devset/config.toml to use the built-in merge"
                .into(),
        ),
        MergeError::Unresolved { .. } => Some(
            "resolve them in .devset/conflicts/, then run `devset update --continue`;\n\
             or take the update back with `devset update --abort`"
                .into(),
        ),
        MergeError::Unmerged { .. } | MergeError::Invalid { .. } => {
            Some("fix it, then run `devset update --continue` again".into())
        },
        MergeError::ChangedSince { .. } => {
            Some("keep what you need from them, then run `devset update --abort --force`".into())
        },
        MergeError::CorruptUndo { .. } => Some(
            "restore that file by hand, or delete .devset/conflicts/ to keep what the update wrote"
                .into(),
        ),
        MergeError::Driver { .. }
        | MergeError::Run { .. }
        | MergeError::NothingToContinue
        | MergeError::NothingToAbort
        | _ => None,
    }
}
