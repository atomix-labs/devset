# `profile.toml`

<!-- reference: written by `just fix-docs` from `docs/src/schema/profile.json` -->

A profile's manifest: who it is, what it builds on, its features and variables,
and every file it manages. [Profiles](profiles.md) explains it, and
[Write a Profile](write-a-profile.md) writes one.

Editors complete and check it from its schema, as [Schemas](schemas.md) says.

## `[profile]`

Identity.

| Key           | Takes            | Meaning                                                                       |
| ------------- | ---------------- | ----------------------------------------------------------------------------- |
| `name`        | string, required | Names the profile: in its source, among the layers, in output, and in `from`. |
| `version`     | string           | For humans; a git source is pinned by commit.                                 |
| `description` | string           | One line for humans.                                                          |
| `devset`      | string           | The devset versions the profile works with.                                   |

## `[requires]`

Profiles this one builds on, by name: each a layer before it. Each entry:

| Key                | Takes           | Meaning                                                               |
| ------------------ | --------------- | --------------------------------------------------------------------- |
| `features`         | list of strings | Its features to turn on.                                              |
| `default-features` | boolean         | Whether its default features are on; `true` unless set.               |
| `optional`         | boolean         | Whether it applies only when a feature activates it. Default `false`. |
| `git`              | string          | The repository of another source.                                     |
| `tag`              | string          | Its tag; only with `git`.                                             |
| `branch`           | string          | Its branch; only with `git`.                                          |
| `rev`              | string          | Its commit; only with `git`.                                          |
| `path`             | string          | The directory in the repository its profiles are in; only with `git`. |

## `[features]`

What a target or a requirer may turn on. The `[features]` table: `default`, and
every feature with what it turns on.

## `[vars.<name>]`

Variables templates and paths use; the target answers them. Each entry:

| Key       | Takes  | Meaning                         |
| --------- | ------ | ------------------------------- |
| `prompt`  | string | What to ask; the name if unset. |
| `default` | string | The answer offered.             |

## `[merge]`

Merge defaults.

| Key           | Takes                          | Meaning                                                                                                                               |
| ------------- | ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| `on-conflict` | `apply-others` or `apply-none` | What a run does when a file conflicts.                                                                                                |
| `driver`      | string                         | A merge program, with `%O %A %B %P` substituted. From a profile it is only a suggestion: devset never runs a program a profile names. |

`on-conflict` takes:

- `apply-others`: write every other file and advance the lock; conflicts wait in
  `.devset/conflicts/`.
- `apply-none`: write nothing but the conflicts until every one is resolved.

## `[scaffolds.<name>]`

Groups of starter files, each written once, when none of its sentinels is there.
Each entry:

| Key      | Takes                     | Meaning                                                                                                                                                                                 |
| -------- | ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unless` | string or list of strings | Paths or globs of which any, there before the run, means the target has its own: the group is not written. A string or a list; none writes the group when the profile is first applied. |

## `[files."<path>"]`

Managed files, by path in the target, as found under `files/`; a path may use
variables. Each entry:

| Key          | Takes                                     | Meaning                                                                                                |
| ------------ | ----------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `policy`     | `owned`, `merge` or `once`                | How devset manages the file; `owned` unless set, and `once` in a scaffold.                             |
| `validate`   | `toml`, `json`, `yaml` or `none`          | How a merged result is checked; inferred from the extension unless set.                                |
| `template`   | boolean                                   | Whether the file is a template, rendered with the answers and the `devset` object. Default `false`.    |
| `scope`      | `file`, `keys` or `block`                 | How much of the file devset owns: all of it, the keys the payload defines, or a block. Default `file`. |
| `comment`    | `#`, `//`, `--`, `;`, `%`, `/*` or `<!--` | The comment syntax of a block's markers, where devset does not know the file type's.                   |
| `executable` | boolean                                   | Whether devset writes the file executable, mode 755, as a script it ships must be. Default `false`.    |
| `scaffold`   | string                                    | The scaffold group the file belongs to, written with it.                                               |
| `starter`    | string                                    | For a part, a file under `files/` the target's file starts from when it is absent.                     |
| `when`       | table, [below](#when)                     | When the entry applies; always, unless set.                                                            |

`policy` takes:

- `owned`: the profile is authoritative; local edits are drift.
- `merge`: local edits are kept, and merged with the profile's.
- `once`: written once if absent; the target's from then on.

`validate` takes:

- `toml`: TOML.
- `json`: JSON with comments and trailing commas (JSONC), a superset of JSON.
- `yaml`: YAML.
- `none`: not checked.

`scope` takes:

- `file`: the whole file.
- `keys`: every leaf the payload, a partial document, defines; the file's other
  keys are the target's.
- `block`: one comment-marked block, named after the profile; the file's other
  lines are the target's.

`comment` takes:

- `#`: shells, TOML, YAML, Python, `.gitignore`.
- `//`: C, Rust, JavaScript, JSONC.
- `--`: SQL, Lua, Haskell.
- `;`: INI, Lisp, assembly.
- `%`: TeX, Erlang, MATLAB.
- `/*`: CSS.
- `<!--`: Markdown, HTML, XML.

### `when`

When an entry applies: every condition holds.

| Key        | Takes                     | Meaning                                                                                        |
| ---------- | ------------------------- | ---------------------------------------------------------------------------------------------- |
| `features` | list of strings           | Features of this profile that are on.                                                          |
| `profiles` | list of strings           | Profiles active in the target.                                                                 |
| `vars`     | table of lists of strings | Variables of this profile, each answered with one of its values.                               |
| `exists`   | list of strings           | Paths or globs that will exist when the run finishes, whole files or parts; may use variables. |
