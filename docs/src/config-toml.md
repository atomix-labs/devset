# `config.toml`

<!-- reference: written by `just fix-docs` from `docs/src/schema/config.json` -->

A target's `.devset/config.toml`: its sources, its layers, and its word on every
setting they carry. [Composing Profiles](composing.md) and
[Settings](settings.md) explain it.

Editors complete and check it from its schema, as [Schemas](schemas.md) says.

## `[sources]`

Where profiles come from, each named once. Each entry:

| Key      | Takes  | Meaning                                                                               |
| -------- | ------ | ------------------------------------------------------------------------------------- |
| `git`    | string | Repository URL.                                                                       |
| `tag`    | string | Tag to use; only with `git`.                                                          |
| `branch` | string | Branch to use; only with `git`.                                                       |
| `rev`    | string | Full commit id to use; only with `git`.                                               |
| `path`   | string | The profile's directory: relative to the target, or within the repository with `git`. |

## `[[layers]]`

Profiles to apply, in order. Each entry:

| Key                | Takes            | Meaning                                                                               |
| ------------------ | ---------------- | ------------------------------------------------------------------------------------- |
| `profile`          | string, required | The profile, `source/name`.                                                           |
| `features`         | list of strings  | Its features to turn on, beside its default ones.                                     |
| `default-features` | boolean          | Whether its default features are on; unless a profile that requires it turns them on. |

## `[merge]`

Merge settings; these override every layer's.

| Key           | Takes                          | Meaning                                                                                                                               |
| ------------- | ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| `on-conflict` | `apply-others` or `apply-none` | What a run does when a file conflicts.                                                                                                |
| `driver`      | string                         | A merge program, with `%O %A %B %P` substituted. From a profile it is only a suggestion: devset never runs a program a profile names. |

`on-conflict` takes:

- `apply-others`: write every other file and advance the lock; conflicts wait in
  `.devset/conflicts/`.
- `apply-none`: write nothing but the conflicts until every one is resolved.

## `[files."<path>"]`

Overrides of the layers' `[files]` entries. Each entry:

| Key        | Takes                            | Meaning                                            |
| ---------- | -------------------------------- | -------------------------------------------------- |
| `from`     | string                           | The layer that provides the file, when several do. |
| `policy`   | `owned`, `merge` or `once`       | How devset manages the file.                       |
| `validate` | `toml`, `json`, `yaml` or `none` | How a merged result is checked.                    |

`policy` takes:

- `owned`: the profile is authoritative; local edits are drift.
- `merge`: local edits are kept, and merged with the profile's.
- `once`: written once if absent; the target's from then on.

`validate` takes:

- `toml`: TOML.
- `json`: JSON with comments and trailing commas (JSONC), a superset of JSON.
- `yaml`: YAML.
- `none`: not checked.
