//! Terminal output: cargo-style status lines on stdout; diagnostics and progress on stderr.

use core::fmt::Display;
use std::io::{self, IsTerminal, Write};

use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
use anstyle::Style;
use clap_cargo::style::GOOD;
use devset_core::Error;
use devset_core::source::Fetch;

use crate::help;

/// Columns a status verb is right-aligned in, as cargo does.
const VERB: usize = 12;

/// Where devset writes, and how much.
#[derive(Debug)]
pub(crate) struct Shell {
    /// Only results and errors.
    quiet: bool,
    /// Whether devset may prompt.
    interactive: bool,
}

impl Shell {
    /// A shell that prompts unless `no_input`, or stdin or stderr is not a terminal.
    pub(crate) fn new(quiet: bool, no_input: bool) -> Self {
        let interactive = !no_input && io::stdin().is_terminal() && io::stderr().is_terminal();
        Self { quiet, interactive }
    }

    /// Whether devset may prompt.
    pub(crate) const fn interactive(&self) -> bool {
        self.interactive
    }

    /// A status line, `verb` in `style` and then `message`, unless quiet.
    pub(crate) fn status(&self, verb: &str, style: Style, message: impl Display) -> io::Result<()> {
        if self.quiet { Ok(()) } else { self.always(verb, style, message) }
    }

    /// A status line even when quiet: for what needs attention.
    #[expect(clippy::unused_self, reason = "all output goes through the shell")]
    pub(crate) fn always(&self, verb: &str, style: Style, message: impl Display) -> io::Result<()> {
        writeln!(anstream::stdout(), "{style}{verb:>VERB$}{style:#} {message}")
    }

    /// A `note:` on stderr, with a `help:` if given, unless quiet.
    pub(crate) fn note(&self, text: &str, help: Option<&str>) -> io::Result<()> {
        let title = Level::NOTE.secondary_title(text);
        let group = match help {
            Some(help) => title.element(Level::HELP.message(help)),
            None => Group::with_title(title),
        };
        self.diagnose(&[group])
    }

    /// A `warning:` on stderr, with a `help:` if given, unless quiet.
    pub(crate) fn warn(&self, text: &str, help: Option<&str>) -> io::Result<()> {
        let title = Level::WARNING.secondary_title(text);
        let group = match help {
            Some(help) => title.element(Level::HELP.message(help)),
            None => Group::with_title(title),
        };
        self.diagnose(&[group])
    }

    /// A `help:` on stderr, unless quiet.
    pub(crate) fn help(&self, text: &str) -> io::Result<()> {
        self.diagnose(&[Group::with_title(Level::HELP.secondary_title(text))])
    }

    /// `error` as an `error:`, with its help, on stderr; a parse error points into its file.
    #[expect(clippy::unused_self, reason = "all output goes through the shell")]
    pub(crate) fn error(&self, error: &Error) -> io::Result<()> {
        let renderer = Renderer::styled();
        if let Error::Parse(parse) = error
            && let Some(span) = parse.span.clone()
        {
            let snippet = Snippet::source(parse.text.as_str())
                .path(parse.file.as_str())
                .annotation(AnnotationKind::Primary.span(span));
            let report = Level::ERROR.primary_title(parse.message.as_str()).element(snippet);
            return writeln!(anstream::stderr(), "{}", renderer.render(&[report]));
        }
        let message = error.to_string();
        let help = help::help(error);
        let title = Level::ERROR.primary_title(message.as_str());
        let report = match &help {
            Some(help) => title.element(Level::HELP.message(help.as_str())),
            None => Group::with_title(title),
        };
        writeln!(anstream::stderr(), "{}", renderer.render(&[report]))
    }

    /// A [`Cache::on_fetch`](devset_core::Cache::on_fetch) hook: each fetch on stderr.
    ///
    /// A plain line, never an animation, since git may ask for credentials on the same terminal
    /// and a redrawn line would hide its question; nothing when quiet.
    pub(crate) fn fetches(&self) -> impl Fn(Fetch<'_>) + Send + Sync + 'static {
        let quiet = self.quiet;
        move |event| {
            if let (false, Fetch::Start(url)) = (quiet, event) {
                // Progress only: a failed write here leaves the fetch, and its result, unchanged.
                drop(writeln!(anstream::stderr(), "{GOOD}{:>VERB$}{GOOD:#} {url}", "Fetching"));
            }
        }
    }

    /// Renders `report` on stderr, unless quiet.
    fn diagnose(&self, report: &[Group<'_>]) -> io::Result<()> {
        if self.quiet {
            Ok(())
        } else {
            writeln!(anstream::stderr(), "{}", Renderer::styled().render(report))
        }
    }
}
