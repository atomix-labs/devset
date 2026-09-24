//! Answering profile variables: prompted when interactive, defaulted otherwise.

use std::io;

use devset_core::profile::VarName;
use devset_core::resolve::Question;
use devset_core::{Cache, Error, Refresh, Resolved, Target, VarError, resolve};
use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;

use crate::shell::Shell;

/// Resolves `target` with `given` answered, asking for any other variable its layers declare.
pub(crate) fn resolved(
    shell: &Shell,
    target: &mut Target,
    cache: &Cache,
    refresh: Refresh<'_>,
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

/// Answers `questions` on `target`: by prompting, or by taking each default.
///
/// # Errors
/// [`VarError::Unanswered`], with the questions that have no default, devset may not prompt;
/// nothing is answered then.
fn answer(shell: &Shell, target: &mut Target, questions: Vec<Question>) -> Result<(), Error> {
    if !shell.interactive() {
        let missing: Vec<Question> = questions
            .iter()
            .filter(|q| q.default.is_none())
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(VarError::Unanswered { questions: missing }.into());
        }
    }
    let theme = ColorfulTheme::default();
    for Question {
        name,
        prompt,
        default,
    } in questions
    {
        let answer = match default {
            _ if shell.interactive() => {
                let input = Input::<String>::with_theme(&theme).with_prompt(prompt);
                let input = match default {
                    Some(default) => input.default(default),
                    None => input,
                };
                input.interact_text().map_err(io::Error::other)?
            }
            Some(default) => {
                let text = format!("answered {name} = \"{default}\", the default");
                shell.note(
                    &text,
                    Some(&format!("pass `--var {name}=…` to choose another")),
                )?;
                default
            }
            None => continue,
        };
        target.answer(name, answer);
    }
    Ok(())
}
