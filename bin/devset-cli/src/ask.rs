//! Answering profile variables: each default taken, and a variable with none asked in a terminal.

use std::io;

use devset_core::profile::VarName;
use devset_core::resolve::Question;
use devset_core::{Cache, Error, Refresh, Resolved, Target, VarError, resolve};
use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;

use crate::shell::Shell;

/// Resolves `target` with `given` answered, asking for any other variable its layers declare.
pub(crate) fn resolved(
    shell: &Shell, target: &mut Target, cache: &Cache, refresh: Refresh<'_>,
    given: Vec<(VarName, String)>,
) -> Result<Resolved, Error> {
    for (name, value) in given {
        target.answer(name, value);
    }
    loop {
        let error = match resolve(target, cache, refresh) {
            Ok(resolved) => return Ok(resolved),
            Err(error) => error,
        };
        let Error::Var(VarError::Unanswered { questions }) = error else {
            return Err(error);
        };
        answer(shell, target, questions)?;
    }
}

/// Answers `questions` on `target`: each that has a default takes it, named in one note; each that
/// has none is asked, in a terminal.
///
/// # Errors
/// [`VarError::Unanswered`], with the questions that have no default, where devset may not prompt;
/// nothing is answered then.
fn answer(shell: &Shell, target: &mut Target, questions: Vec<Question>) -> Result<(), Error> {
    let (defaulted, open): (Vec<Question>, Vec<Question>) =
        questions.into_iter().partition(|question| question.default.is_some());
    if !open.is_empty() && !shell.interactive() {
        return Err(VarError::Unanswered { questions: open }.into());
    }
    if !defaulted.is_empty() {
        let help = "`.devset/answers.toml` keeps each answer; `--var <name>=<value>` changes one";
        shell.note(&taken(&defaulted), Some(help))?;
    }
    for Question { name, default, .. } in defaulted {
        if let Some(default) = default {
            target.answer(name, default);
        }
    }
    let theme = ColorfulTheme::default();
    for Question { name, prompt, .. } in open {
        let input = Input::<String>::with_theme(&theme).with_prompt(prompt);
        target.answer(name, input.interact_text().map_err(io::Error::other)?);
    }
    Ok(())
}

/// What taking the defaults of `defaulted` answered: the one, with its value, or how many, by name.
fn taken(defaulted: &[Question]) -> String {
    let default = |question: &Question| question.default.clone().unwrap_or_default();
    match defaulted {
        [one] => format!("answered {} = \"{}\", its default", one.name, default(one)),
        many => {
            let names: Vec<String> =
                many.iter().map(|question| question.name.to_string()).collect();
            format!("answered {} variables with their defaults: {}", many.len(), names.join(", "))
        },
    }
}
