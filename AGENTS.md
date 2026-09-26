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

## Commands

```sh
just check                                 # everything CI runs; green before any push
just fix                                   # every formatter and fixer
SNAPSHOTS=overwrite cargo test --test cli  # rewrite the end-to-end snapshots a change meant to change
just fix-docs                              # after changing a command's help or a schema
```

## Rules

- **Only `commit` writes.** `resolve`, `survey` and `plan` decide; a change that
  writes earlier in the chain is wrong however it tests.
- **Output is the command line's.** `devset-core` returns errors whose fields
  carry what `bin/devset-cli/src/help.rs` needs to write the next step.
- **Snapshots are the review.** After `SNAPSHOTS=overwrite`, read every diff in
  `bin/devset-cli/tests/cli/snapshots/`; a snapshot holding devset's version
  matches it with `[..]`, or the next release breaks it.
- **Generated pages are generated.** `docs/src/reference/` and
  `docs/src/schema/` come from the build: change the source, then `just
  fix-docs`.
- **Breaking changes are named.** A change to a command, a flag, `status
  --json`, the exit codes, a file people write, what `.devset/` records, or
  `devset-core`'s public API adds `!` to its commit and its entry to
  BREAKING-CHANGES.md.
- **The house style.** Markdown wrapped at 80, title-case headings, no em
  dashes; every item documented, private ones too; a fix comes with the test
  that fails without it.
