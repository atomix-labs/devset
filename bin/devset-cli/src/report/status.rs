//! `status`: the layers, then every managed file that needs doing, grouped by the command that
//! does it.

use core::fmt;
use std::io::{self, Write};

use anstyle::Style;
use clap_cargo::style::{GOOD, LITERAL, WARN};
use devset_core::Survey;
use devset_core::resolve::{Applied, Layer};
use devset_core::survey::Entry;

use super::json::StatusJson;
use super::{Standing, State, files, in_sync, suggestions};
use crate::shell::Shell;

/// A `status` section: its heading, that heading while an update is unfinished, its standings.
type Section = (&'static str, &'static str, fn(Standing) -> bool);

/// The sections of `status`, in order.
const SECTIONS: [Section; 5] = [
    (
        "Conflicts — resolve in .devset/conflicts/, then run `devset update --continue`:",
        "Conflicts — resolve in .devset/conflicts/, then run `devset update --continue`:",
        |s| s == Standing::Conflict,
    ),
    (
        "Drifted — `devset apply --force` restores:",
        "Drifted — once the update is finished, `devset apply --force` restores:",
        |s| matches!(s, Standing::Drifted(_)),
    ),
    ("Pending — `devset apply` will:", "Pending — `devset update --continue` will:", |s| {
        matches!(s, Standing::Pending(_))
    }),
    ("Local changes, kept:", "Local changes, kept:", |s| s == Standing::Local),
    ("In sync:", "In sync:", |s| s == Standing::InSync),
];

/// Prints `survey`: what needs doing, grouped by the command that does it.
pub(crate) fn status(shell: &Shell, survey: &Survey, json: bool, verbose: bool) -> io::Result<()> {
    let mut out = anstream::stdout().lock();
    if json {
        serde_json::to_writer_pretty(&mut out, &StatusJson::of(survey))?;
        return writeln!(out);
    }
    let resolved = survey.resolved();
    headings(&mut out, resolved.layers(), verbose)?;
    if verbose {
        writeln!(out, "  merge driver: {}", resolved.settings().driver)?;
        for (name, answer) in resolved.answers() {
            writeln!(out, "  {name} = {answer:?}")?;
        }
    }
    let rows: Vec<(&Entry, Standing)> =
        survey.entries().iter().map(|e| (e, Standing::of(e))).collect();
    let listed = |standing: Standing| verbose || standing != Standing::InSync;
    let width = rows
        .iter()
        .filter(|(_, s)| listed(*s))
        .map(|(e, _)| e.to_string().len())
        .max()
        .unwrap_or(0);
    let layered = resolved.layers().len() > 1;
    let unfinished = survey.unfinished();
    for (heading, while_unfinished, belongs) in SECTIONS {
        let heading = if unfinished { while_unfinished } else { heading };
        let members: Vec<_> = rows.iter().filter(|(_, s)| belongs(*s) && listed(*s)).collect();
        if members.is_empty() {
            continue;
        }
        writeln!(out, "\n{}", Emphasis(heading))?;
        for &&(entry, standing) in &members {
            let state = State::of(entry);
            let (style, path, name) = (state.style(), entry.path.as_str(), entry.to_string());
            // As strings, since a derived `Display` ignores the width the columns need.
            let state = state.to_string();
            let policy = entry.want.map_or_else(String::new, |want| want.policy.to_string());
            let action = match standing {
                Standing::Pending(change) | Standing::Drifted(change) => format!("  {change}"),
                Standing::Conflict => format!("  → .devset/conflicts/{path}"),
                Standing::Local | Standing::InSync => String::new(),
            };
            // A part is named after its layer already.
            let layer = resolved
                .provider(entry)
                .filter(|_| layered && entry.part.is_none())
                .map(|layer| format!("  ({layer})"));
            let layer = layer.unwrap_or_default();
            let row = format!(
                "    {style}{state:<9}{style:#}  {name:<width$}  {policy:<5}{action}{layer}"
            );
            writeln!(out, "{}", row.trim_end())?;
        }
    }
    if rows.iter().all(|(_, s)| *s == Standing::InSync) {
        writeln!(out, "\n{GOOD}{}{GOOD:#}", in_sync(files(survey)))?;
    }
    drop(out);
    suggestions(shell, resolved.suggestions())
}

/// Writes each layer's heading. Unless `verbose`, a required layer applied as it is goes on a
/// `requires` line under the configured layer that brings it in.
fn headings(out: &mut impl Write, layers: &[Layer], verbose: bool) -> io::Result<()> {
    let folded = |layer: &Layer| {
        !verbose && layer.required_by().is_some() && layer.applied() == Applied::Current
    };
    // The configured layer a required one comes from, however deep.
    let root = |layer: &Layer| {
        let mut at = layer;
        for _ in layers {
            let Some(by) = at.required_by() else { break };
            let Some(parent) = layers.iter().find(|l| l.meta().name == by) else {
                break;
            };
            at = parent;
        }
        at.meta().name.clone()
    };
    for layer in layers.iter().filter(|layer| !folded(layer)) {
        writeln!(out, "{}", Heading(layer))?;
        if layer.required_by().is_some() {
            continue;
        }
        let name = &layer.meta().name;
        let required: Vec<&str> = layers
            .iter()
            .filter(|l| folded(l) && root(l) == *name)
            .map(|l| l.meta().name.as_str())
            .collect();
        if !required.is_empty() {
            writeln!(out, "{}", wrapped("  requires ", &required))?;
        }
    }
    Ok(())
}

/// `lead` and then `names`, comma-separated and wrapped under the first, each name whole.
fn wrapped(lead: &str, names: &[&str]) -> String {
    let indent = " ".repeat(lead.len());
    let options = textwrap::Options::new(100)
        .initial_indent(lead)
        .subsequent_indent(&indent)
        .break_words(false)
        .word_splitter(textwrap::WordSplitter::NoHyphenation);
    textwrap::fill(&names.join(", "), options)
}

/// A section heading, commands in backticks styled as literals.
struct Emphasis<'a>(&'a str);

impl fmt::Display for Emphasis<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bold = Style::new().bold();
        for (i, part) in self.0.split('`').enumerate() {
            if i % 2 == 1 {
                f.write_str(&literal(part))?;
            } else {
                write!(f, "{bold}{part}{bold:#}")?;
            }
        }
        Ok(())
    }
}

/// `text` styled as a command, backticks kept so it stands out without colour too.
fn literal(text: &str) -> String {
    format!("{LITERAL}`{text}`{LITERAL:#}")
}

/// A layer's one-line summary: name, version, source, commit, and whether it is applied.
struct Heading<'a>(&'a Layer);

impl fmt::Display for Heading<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (layer, bold) = (self.0, Style::new().bold());
        write!(f, "{bold}{}{bold:#}", layer.meta().name)?;
        if let Some(version) = &layer.meta().version {
            write!(f, " {version}")?;
        }
        write!(f, "  {}", layer.source())?;
        if let Some(rev) = layer.rev() {
            write!(f, "  @{}", rev.short())?;
        }
        if let Some(by) = layer.required_by() {
            write!(f, "  {WARN}(required by {by}){WARN:#}")?;
        }
        match layer.applied() {
            Applied::Current => {},
            Applied::Changed => write!(f, "  {WARN}(changed since it was applied){WARN:#}")?,
            Applied::Never => write!(f, "  {WARN}(not applied yet){WARN:#}")?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::wrapped;

    #[test]
    fn names_wrap_whole() {
        let text = wrapped("  requires ", &["rust-toolchain"; 8]);
        assert!(text.lines().count() > 1, "a long list wraps: {text}");
        assert!(
            text.lines().all(|line| line.ends_with(',') || line.ends_with("toolchain")),
            "at a name's end: {text}"
        );
    }
}
