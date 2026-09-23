//! The next step to suggest for an [`Error`], in the terms of this command line.

use core::fmt::Display;

use devset_core::{Error, MergeError, ProfileError, SourceError, TargetError, VarError};

/// The next step after `error`, when there is one to suggest.
pub(crate) fn help(error: &Error) -> Option<String> {
    match error {
        Error::Source(error) => source(error),
        Error::Profile(error) => profile(error),
        Error::Target(error) => target(error),
        Error::Var(error) => var(error),
        Error::Merge(error) => merge(error),
        Error::Io(_) | Error::Parse(_) | Error::Path(_) | _ => None,
    }
}

/// [`help`] for a [`SourceError`].
fn source(error: &SourceError) -> Option<String> {
    match error {
        SourceError::NoGit => Some("install git, or use a local profile with `--path`".into()),
        SourceError::NoLocation
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
        ProfileError::Incompatible { .. } => {
            Some("upgrade devset, or pin the profile to an older version".into())
        },
        ProfileError::Collision { path, layers } => {
            let options: Vec<String> = layers.iter().map(|layer| format!("\"{layer}\"")).collect();
            let options = options.join("   # or ");
            Some(format!(
                "choose one in .devset/config.toml:\n    [files.\"{path}\"]\n    from = {options}"
            ))
        },
        ProfileError::NotProvided { from, layers, .. } => {
            let near = nearest(from, layers).map(|near| format!("did you mean `{near}`? "));
            Some(format!("{}it is provided by {}", near.unwrap_or_default(), list(layers)))
        },
        ProfileError::Setting { key, .. } => {
            Some(format!("decide it in .devset/config.toml by setting {key}"))
        },
        ProfileError::NotAProfile { .. }
        | ProfileError::MissingPayload { .. }
        | ProfileError::FoldCollision { .. }
        | _ => None,
    }
}

/// [`help`] for a [`TargetError`].
fn target(error: &TargetError) -> Option<String> {
    match error {
        TargetError::NotFound { .. } => {
            Some("start one: `devset init --path <profile>` or `devset init --git <url>`".into())
        },
        TargetError::Nested { root, .. } => Some(format!("run devset from {root}")),
        TargetError::NoSuchLayer { name, layers } => Some(nearest(name, layers).map_or_else(
            || format!("the layers are {}", list(layers)),
            |near| format!("did you mean `{near}`?"),
        )),
        TargetError::StaleOverride { path, provided } => Some(nearest(path, provided).map_or_else(
            || format!("remove its [files.\"{path}\"] table"),
            |near| format!("did you mean `{near}`?"),
        )),
        TargetError::CorruptBase { .. } => {
            Some("restore .devset/base/ from version control".into())
        },
        TargetError::Busy => Some("wait for it to finish".into()),
        TargetError::Concurrent => Some("run the command again".into()),
        TargetError::DuplicateLayer { .. } | TargetError::NotAFile { .. } | _ => None,
    }
}

/// [`help`] for a [`VarError`].
fn var(error: &VarError) -> Option<String> {
    match error {
        VarError::Unknown { name, declared } => Some(nearest(name, declared).map_or_else(
            || match declared.as_slice() {
                [] => "no profile declares any variables".to_owned(),
                _ => format!("the variables are {}", list(declared)),
            },
            |near| format!("did you mean `{near}`?"),
        )),
        VarError::Unanswered { questions } => {
            let flags: Vec<String> =
                questions.iter().map(|q| format!("--var {}=…", q.name)).collect();
            Some(format!("answer with `{}`", flags.join(" ")))
        },
        VarError::Name { .. } | VarError::Template { .. } | _ => None,
    }
}

/// [`help`] for a [`MergeError`].
fn merge(error: &MergeError) -> Option<String> {
    match error {
        MergeError::Unresolved { .. } => Some(
            "resolve them in .devset/conflicts/, then run `devset update --continue`;\n\
             or delete .devset/conflicts/ to abandon the update"
                .into(),
        ),
        MergeError::Unmerged { .. } | MergeError::Invalid { .. } => {
            Some("fix it, then run `devset update --continue` again".into())
        },
        MergeError::Driver { .. } | MergeError::Run { .. } | MergeError::NothingToContinue | _ => {
            None
        },
    }
}

/// The candidate `needle` was most likely meant to be.
fn nearest<T: AsRef<str>>(needle: impl AsRef<str>, candidates: &[T]) -> Option<&str> {
    candidates
        .iter()
        .map(|candidate| {
            (strsim::jaro_winkler(needle.as_ref(), candidate.as_ref()), candidate.as_ref())
        })
        .filter(|&(score, _)| score > 0.8)
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, candidate)| candidate)
}

/// `items`, comma-separated, with `and` before the last.
fn list<T: Display>(items: &[T]) -> String {
    let items: Vec<String> = items.iter().map(ToString::to_string).collect();
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => items.concat(),
    }
}
