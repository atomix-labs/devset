# Contributing

How to report a problem, and how to change devset: setting up a checkout, the
commit convention, what counts as breaking, the checks, and how the documents
are written. [ARCHITECTURE.md](ARCHITECTURE.md) says how devset is built.

## Reporting Issues

Open an issue with the template that fits: a **bug report** gives devset's
version, what you ran, what happened and what you expected, and how to reproduce
it; a **feature request** says what you want to do that devset does not let you.
A problem with a profile belongs in the repository that publishes it: for
atxp's, [atxp's issues](https://github.com/atomix-labs/atxp/issues).

A vulnerability is reported privately, as [SECURITY.md](SECURITY.md) says.

## A Checkout

devset applies the `rust` bundle of [atxp](https://github.com/atomix-labs/atxp)
to itself, so its tools, checks and CI are atxp's, pinned in `.devset/`.

```sh
./setup.sh      # the tools the lock pins, then the pinned nightly toolchain
just check      # every check, as CI runs them
just fix        # every fix
```

Development runs on the nightly that `rust-toolchain.toml` pins; every crate
also builds on the `rust-version` it declares, which `just check-msrv` checks.

## Pull Requests

Keep each pull request to one change: a feature, a fix, or a refactor, not a mix
of them.

Commits follow [Conventional Commits](https://www.conventionalcommits.org),
which `check-committed` holds every commit of a branch to:

```text
type(scope): subject
```

- The **type** is `feat`, `fix`, `refactor`, `docs`, `perf`, `test`, `build`,
  `ci`, `chore`, `style` or `revert`.
- The **scope** is the crate the commit changes, `devset-core` or `devset-cli`,
  or the part of the repository that is neither: `docs`, `ci` or `release`. A
  change across both crates has no scope.
- The **subject** is imperative, lower case, with no closing period: it is the
  line the changelog shows, so it says what changed for someone reading it
  there.
- A breaking change adds `!` after the scope, and a `BREAKING CHANGE:` footer
  saying what to do.

A change is **breaking** when someone who takes it must act. For devset, that is
a change to:

- a command or a flag, removed, renamed, or doing something else;
- output others parse: `status --json`, and the exit codes;
- a file people write, `profile.toml` or `.devset/config.toml`, where a file
  that worked no longer does;
- what `.devset/` records, where a target's state must be written again;
- `devset-core`'s public API.

A breaking change also adds its entry to
[BREAKING-CHANGES.md](BREAKING-CHANGES.md), under the release that will carry
it.

## Checks

```sh
just check         # formatting, lints, tests, the docs and the book, as CI runs them
just nightly       # the slower checks: each feature alone, advisories, the web's links
just fix-docs      # after changing a command's help or a schema: the manual's generated pages
```

The end-to-end tests compare each command's output with a snapshot in
`bin/devset-cli/tests/cli/snapshots/`. A change that means to change output
rewrites them:

```sh
SNAPSHOTS=overwrite cargo test --test cli
```

and the snapshots' diff is part of the review.

## Code

- The lint wall is atxp's, in the workspace `Cargo.toml`, with clippy's pedantic
  and restriction lints; a suppression names its reason.
- `devset-core` never prints or prompts. It returns errors whose messages state
  the problem and whose fields carry the rest; the command line's `help.rs`
  writes the next step.
- Every item has a doc comment, private ones included, saying what it is in a
  sentence the reader needs.
- A fix comes with the test that fails without it.

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

The manual is an mdBook in `docs/`; `just check-mdbook` builds it, and `just
check-lychee` checks every link in every document.
