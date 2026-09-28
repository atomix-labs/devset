# Contributing

How to report a problem, and how to change devset: setting up a checkout, how a
pull request lands, what counts as breaking, the checks, the command line's
style, and how the documents are written. [ARCHITECTURE.md](ARCHITECTURE.md)
says how devset is built.

## Reporting Issues

Open an issue with the form that fits: a **bug report** gives devset's and git's
versions, what you ran, what happened and what you expected, how to reproduce
it, and `devset status -v`; a **feature request** says what you want to do that
devset does not let you. A question, or an idea to talk over first, goes to
[Discussions](https://github.com/atomix-labs/devset/discussions). A problem with
a profile belongs in the repository that publishes it: for atxp's,
[atxp's issues](https://github.com/atomix-labs/atxp/issues).

A vulnerability is reported privately, as [SECURITY.md](SECURITY.md) says.

<!-- >>> devset: setup >>> -->

## Getting Started

`./setup.sh` readies a machine to work on the repository: it installs mise,
pinned and checked against its release's sha256, then every tool the repository
pins, at the version its lock records, and runs `just setup`. It needs git, curl
and bash, installs into your home directory without sudo, and `--dry-run` says
what it would do. Fork the repository, then:

```sh
git clone https://github.com/<you>/devset.git
cd devset
./setup.sh
```

Or clone and set up in one line:

```sh
curl -fsSL https://atomix-labs.github.io/atxp/setup.sh | bash -s -- github.com/atomix-labs/devset
```

The first run takes a few minutes; run it again after pulling, and it installs
only what moved. `./setup.sh --activate` adds mise to your shell, so the tools
are on `PATH` in every new one; without it, `mise exec -- just check` runs them.

<!-- <<< devset: setup <<< -->

devset applies the `rust` bundle of [atxp](https://github.com/atomix-labs/atxp)
to itself, with the features and the profiles beside it that
`.devset/config.toml` names, so its tools, checks and CI are atxp's, pinned in
`.devset/`. Development runs on the nightly that `rust-toolchain.toml` pins,
which `./setup.sh` installs; every crate also builds on the `rust-version` it
declares, which `just check-rust-msrv` checks. `install.sh` installs devset to
use, not to develop.

## Pull Requests

Keep each pull request to one change: a feature, a fix, or a refactor, not a mix
of them. Every change to `main` is a pull request, merged once its checks pass:
`check`, every check recipe, and `title`, its title.

<!-- >>> devset: git-commits >>> -->

## Commits

A pull request lands squashed, as one commit its title names, so its **title**
follows [Conventional Commits](https://www.conventionalcommits.org), which CI
checks on every edit:

```text
type(scope): subject
```

- The **type** is `feat`, `fix`, `refactor`, `docs`, `perf`, `test`, `build`,
  `ci`, `chore`, `style` or `revert`.
- The **subject** is imperative, lower case, with no closing period: it is the
  line the changelog shows.
- A breaking change adds `!` after the scope, and its description says what to
  do.

The commits on your branch are yours to shape; `just check-git-commits` checks
them against the same rules, for a branch that reads well in review.

<!-- <<< devset: git-commits <<< -->

A commit's scope is the crate it changes, `devset-core` or `devset-cli`, or the
part of the repository that is neither: `docs`, `ci` or `release`. A change
across both crates has no scope.

A change is **breaking** when someone who takes it must act. For devset, that is
a change to:

- a command or a flag, removed, renamed, or doing something else;
- output others parse: `status --json`, and the exit codes;
- a file people write, `profile.toml`, `collection.toml` or
  `.devset/config.toml`, where a file that worked no longer does;
- what `.devset/` records, where a target's state must be written again;
- `devset-core`'s public API.

A breaking change also adds its entry to
[BREAKING-CHANGES.md](BREAKING-CHANGES.md), under the release that will carry
it.

<!-- >>> devset: project >>> -->

## Checks

Run `just check` before you open a pull request: CI runs the same checks, and
names each that fails. `just fix` fixes what a formatter or linter can, and
`just --list` shows every recipe.

<!-- <<< devset: project <<< -->

```sh
just check         # formatting, lints, tests, the docs and the book, as CI runs them
just nightly       # the slower checks: each feature alone, advisories, the web's links
just fix-docs      # after changing a command's help: the manual's reference, and every atxp tag named
```

The end-to-end tests compare each command's output with a snapshot in
`bin/devset-cli/tests/cli/snapshots/`, and a test in devset-core compares the
manual's JSON schemas, `docs/src/schema/`, with the types they describe. A
change that means to change either rewrites them:

```sh
SNAPSHOTS=overwrite cargo test --workspace
```

and the diff is part of the review.

## Code

- The lint wall is atxp's, in the workspace `Cargo.toml`, with clippy's pedantic
  and restriction lints; a suppression names its reason.
- `devset-core` never prints or prompts. It returns errors whose messages state
  the problem and whose fields carry the rest; the command line's `help.rs`
  writes the next step.
- Every item has a doc comment, private ones included, saying what it is in a
  sentence the reader needs.
- A fix comes with the test that fails without it.

### Command-Line Style

Every message devset prints keeps these rules; a change that adds one is read
against them.

- A command's answer goes to stdout; its log, notes, warnings, errors and
  prompts go to stderr.
- Output reads correctly without colour: a command or a path in a message is in
  backticks even where it is also coloured. `NO_COLOR`, `CLICOLOR_FORCE` and
  `--no-color` are honoured.
- An error states the problem in one line. What to do goes on a `help:` line,
  with the command to run, ready to copy.
- The log reads as cargo's: a verb aligned on the right, past tense for what
  happened, `Would` in a dry run, and a `Finished` summary.
- A command exists for a question no other command answers. A flag changes how a
  command answers, never what it answers.

## Writing

Every document here, the manual included, keeps these rules.

- A document opens with one paragraph saying what it is for.
- A fact lives in one document; the others link to it.
- Explanation is plain statement, in the present tense; a step to follow is an
  imperative.
- Headings are in title case, one H1 a file.
- Markdown is wrapped at 80, with no em dashes.
- Identifiers are in backticks.
- A long document collects its link targets at the bottom.
- A generated region is marked by a comment that names what writes it.

The manual is an mdBook in `docs/`; `just check-mdbook` builds it and checks the
links of the built book, and `just check-lychee` checks every other document's.
