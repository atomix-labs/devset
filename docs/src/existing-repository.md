# An Existing Repository

A repository that has configuration of its own takes profiles as a new one does,
and loses nothing: devset keeps every file it finds, and shows how each differs
from its profile. This page is what adopting real repositories taught.

## Look First

```sh
devset add atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.13.0 --dry-run
```

`--dry-run` lists what the run would create, change or keep, and writes nothing.
Taking one layer at a time, and committing between, keeps each step small enough
to read.

## What devset Does with What Is There

- **A file a profile owns whole, already there**, is adopted: devset records the
  profile's version as the file's base and keeps the repository's file. `devset
  status` then shows it `edited`, with what differs. Keep it, and it stays
  drift; take the profile's with `devset apply --force`, which restores `owned`
  files only; or, when another layer should provide it, choose with
  [`from`](settings.md).
- **Keys a profile owns in a TOML, JSON or YAML file** are adopted one by one:
  the repository's values are kept, the keys it lacks are written, and every
  other key, comment and layout stays its own. `devset diff` shows each value
  the profile would change.
- **A block** is written at the end of a text file, a `.gitignore` or an
  `AGENTS.md`, and stays wherever you move it.
- **A scaffold** writes nothing where the repository has its own: a workspace
  already there is the workspace.

## What to Expect

The same few things come up in most repositories:

- **Recipe names that clash.** A collection with a recipe spine, as atxp's
  `just` profile is, owns names such as `check`, `test` and `nightly`. A
  repository's own recipe by one of those names clashes with it: rename it
  `<verb>-<name>`, as `test-unit`, and the verb runs it with the rest.
- **Workflows the profiles replace.** A repository's own CI, bump or docs
  workflows overlap the ones the profiles bring. Retire them, or keep a job the
  profiles have no counterpart for.
- **Exclusions the repository keeps.** A formatter's or a linter's own
  exclusions are the repository's keys, kept on adoption; add what the profiles'
  tools write, as `.just/**` or `CHANGELOG.md`, where a check trips on it.
- **Older copies of shared files.** A file the repository copied from the same
  place long ago, a skill or a workflow, shows as `edited`; `devset diff` shows
  whether the difference is worth keeping, and `apply --force` takes the
  profile's.
- **Checks that are new to it.** A spell checker, rustdoc over private items or
  a stricter lint finds real problems in code that was never held to them: fix
  them, or allow each by hand in the repository's own configuration, which the
  profile leaves to it.

## Done

`devset status --exit-code` passes once nothing has drifted: every `owned` file
is its profile's, and the repository's edits are in `merge` files, where they
are its own. Commit `.devset/` with the files, and [In CI](ci.md) keeps it that
way.
