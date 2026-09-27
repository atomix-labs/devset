# Agents

What an agent working in this repository needs: what devset is, where each fact
lives, the commands, and the rules a change keeps. It links rather than repeats;
read the linked document before changing what it covers.

## The Repository

devset applies versioned bundles of files to a directory and updates them
without losing local edits. Two crates: `lib/devset-core`, which decides and
writes everything and never prints, and `bin/devset-cli`, the `devset` binary,
which owns every sentence a person reads.

| For                                                 | Read                                       |
| --------------------------------------------------- | ------------------------------------------ |
| The chain, the run in order, the modules, the rules | [ARCHITECTURE.md](ARCHITECTURE.md)         |
| Commits, what is breaking, the checks, the writing  | [CONTRIBUTING.md](CONTRIBUTING.md)         |
| What devset does, for users                         | the manual, `docs/src/`                    |
| Cutting a release                                   | [RELEASE.md](RELEASE.md)                   |
| Migrating across a breaking change                  | [BREAKING-CHANGES.md](BREAKING-CHANGES.md) |

<!-- >>> devset: agents >>> -->

## Before You Commit

Run `just check`: CI runs the same checks, and names each that fails. `just fix`
fixes what a formatter or linter can, and `just --list` shows every recipe.

## Managed Files

Profiles, applied by devset, manage some of the files here. `devset status`
names each, and whether a local change to it is kept or is drift; `devset
explain <file>` says which profile owns what in it. What a profile owns changes
with the profile, on `devset update`. Never edit `.devset/`.

<!-- <<< devset: agents <<< -->

## Commands

```sh
SNAPSHOTS=overwrite cargo test --workspace  # rewrite the snapshots and schemas a change meant to change
just fix-docs                               # after changing a command's help
```

## Rules

- **Only `commit` writes.** `resolve`, `survey` and `plan` decide; a change that
  writes earlier in the chain is wrong however it tests.
- **Output is the command line's.** `devset-core` returns errors whose fields
  carry what `bin/devset-cli/src/help.rs` needs to write the next step.
- **Snapshots are the review.** After `SNAPSHOTS=overwrite`, read every diff in
  `bin/devset-cli/tests/cli/snapshots/`; a snapshot holding devset's version
  matches it with `[..]`, or the next release breaks it.
- **Generated pages are generated.** `docs/src/reference/` comes from the help:
  change `cli.rs`, then `just fix-docs`. `docs/src/schema/` comes from
  devset-core's types: change them, then `SNAPSHOTS=overwrite cargo test -p
  devset-core --test schemas`.
- **Breaking changes are named.** A change to a command, a flag, `status
  --json`, the exit codes, a file people write, what `.devset/` records, or
  `devset-core`'s public API adds `!` to its commit and its entry to
  BREAKING-CHANGES.md.
- **The house style.** Markdown wrapped at 80, title-case headings, no em
  dashes; every item documented, private ones too; a fix comes with the test
  that fails without it.

<!-- >>> devset: cargo-deny >>> -->

## Dependencies

`just check-cargo-deny` holds every dependency to `deny.toml`: its advisories,
its licence, its source, and the bans. A failure names a choice for the
maintainer, between a newer version, another crate, and an exception with its
reason: ask before adding an exception or allowing another licence.

<!-- <<< devset: cargo-deny <<< -->

<!-- >>> devset: git-commits >>> -->

## Commits

`just check-git-commits` holds every commit of a branch to Conventional Commits:
`type(scope): subject`, the subject imperative and lower case, with no closing
period, since it is the line the changelog shows. A breaking change adds `!`
after the scope, and a footer that starts `BREAKING CHANGE:` and says what to
do.

<!-- <<< devset: git-commits <<< -->

<!-- >>> devset: mdbook >>> -->

## The Book

`just check-mdbook` lints the book, builds it and runs its examples. Its pages
are Markdown under the `src/` of its directory, each listed in `SUMMARY.md`, and
a preview rebuilds on every save:

```sh
mdbook serve docs
```

<!-- <<< devset: mdbook <<< -->
