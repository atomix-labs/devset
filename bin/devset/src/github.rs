//! GitHub Actions: drift annotated on the pull request, and a job summary.

use std::env;
use std::io::{self, Write};

use devset_core::plan::Step;
use devset_core::{RelPath, Survey, Target};
use github_actions::{CommandWithProperties, SummaryError, append_job_summary};

use crate::report::{Change, Standing, Wrote, files, in_sync, state};

/// Whether devset runs in a GitHub Actions job.
fn active() -> bool {
    env::var("GITHUB_ACTIONS").is_ok_and(|value| value == "true")
}

/// `path` relative to the workspace, which is what GitHub annotates.
fn file(target: &Target, path: &RelPath) -> String {
    let full = path.under(target.root());
    let relative = env::var("GITHUB_WORKSPACE")
        .ok()
        .and_then(|workspace| full.strip_prefix(workspace).ok().map(ToString::to_string));
    relative.unwrap_or_else(|| path.to_string())
}

/// One annotation: `level` is `error` or `warning`.
fn annotate(level: &str, file: &str, message: &str) -> io::Result<()> {
    let command = CommandWithProperties {
        command: level,
        value: message,
        title: Some("devset"),
        file: Some(file),
        ..Default::default()
    };
    writeln!(io::stdout(), "{command}")
}

/// Annotates what `status` found, and summarizes it for the job.
pub(crate) fn status(survey: &Survey, target: &Target) -> io::Result<()> {
    if !active() {
        return Ok(());
    }
    let mut rows = Vec::new();
    let apply = if survey.unfinished() {
        "`devset update --continue`"
    } else {
        "`devset apply`"
    };
    for entry in survey.entries() {
        let path = &entry.path;
        let (level, next) = match Standing::of(entry) {
            Standing::Conflict => (
                "error",
                "resolve `.devset/conflicts/`, then run `devset update --continue`".to_owned(),
            ),
            Standing::Drifted(change) => (
                "error",
                format!("`devset apply --force` would {} it", change.verb()),
            ),
            Standing::Pending(change) => ("warning", format!("{apply} would {} it", change.verb())),
            Standing::Local | Standing::InSync => continue,
        };
        let state = state(entry);
        annotate(
            level,
            &file(target, path),
            &format!("{entry} is {state}; {next}"),
        )?;
        rows.push(format!("| `{entry}` | {state} | {next} |"));
    }
    let summary = if rows.is_empty() {
        format!("### devset\n\n{}\n", in_sync(files(survey)))
    } else {
        format!(
            "### devset\n\n| File | State | Next |\n| --- | --- | --- |\n{}\n",
            rows.join("\n")
        )
    };
    summarize(&summary)
}

/// Annotates the conflicts of an update, which wrote as `wrote` says.
pub(crate) fn conflicts(steps: &[Step], target: &Target, wrote: Wrote) -> io::Result<()> {
    if !active() {
        return Ok(());
    }
    for step in steps {
        if Change::of(&step.entry, step.action) == Some(Change::Conflict) {
            let (entry, path) = (&step.entry, &step.entry.path);
            let why = step.note.as_deref().unwrap_or("conflicting changes");
            let message = match wrote {
                Wrote::Nothing { .. } => format!("{entry} would conflict: {why}"),
                Wrote::All | Wrote::Conflicts => {
                    format!("{entry} conflicted: {why}; resolve .devset/conflicts/{path}")
                }
            };
            annotate("error", &file(target, path), &message)?;
        }
    }
    Ok(())
}

/// Appends `markdown` to the job summary, when the job has one.
fn summarize(markdown: &str) -> io::Result<()> {
    match append_job_summary(markdown) {
        Ok(()) | Err(SummaryError::VarError(_)) => Ok(()),
        Err(SummaryError::FileError(e)) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(SummaryError::FileError(e)) => Err(e),
    }
}
