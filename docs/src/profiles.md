# Profiles

A profile is a directory in a source: `profile.toml` at its root, and `files/`,
which mirrors the target. This chapter covers what `profile.toml` declares and
how devset treats each file; the [schema](schemas.md) lists every field, and
[Designing Profiles](designing.md) says how to cut a concern into profiles,
features and variables.

```text
rust/
  profile.toml
  README.md                  # the profile's own documentation; never applied
  files/
    rustfmt.toml
    deny.toml
    .github/workflows/ci.yml
```

```toml
[profile]
name        = "rust"
version     = "1.4.0"
description = "Formatting, lints and CI for a Rust repository"
devset      = ">=0.2"

[files."rustfmt.toml"]
[files.".github/workflows/ci.yml"]

[files."deny.toml"]
policy = "merge"
```

## The Profile Table

| Field         | Meaning                                                                             |
| ------------- | ----------------------------------------------------------------------------------- |
| `name`        | Required. Names the profile: in its source, among a target's layers, and in output. |
| `version`     | For people; a git source is pinned by its commit, not by this.                      |
| `description` | One line, for people; `devset list` shows it.                                       |
| `devset`      | The devset versions the profile works with, as `>=0.2`; any other is refused.       |

Beside `[profile]`, a manifest may declare `[requires]` and `[features]`
([Composing Profiles](composing.md)), `[vars]` ([Templates](templates.md)),
`[scaffolds]` ([Gates and Scaffolds](gates.md)), and `[merge]`
([Settings](settings.md)).

## Files

`[files."<path>"]` declares that devset manages `files/<path>`, at `<path>` in
the target. A file under `files/` that no entry declares is not applied, and an
entry whose file is missing is an error. A path may use variables, as `"{{
book_dir }}/book.toml"`: its payload is under `files/` as written, and the
target's answer decides where it goes ([Templates](templates.md)). An entry's
fields:

| Field        | Meaning                                                           | Chapter                             |
| ------------ | ----------------------------------------------------------------- | ----------------------------------- |
| `policy`     | How local edits are treated: `owned`, `merge` or `once`.          | below                               |
| `scope`      | How much of the file the profile owns: `file`, `keys` or `block`. | [Parts of a File](parts.md)         |
| `comment`    | The comment syntax of a block's markers.                          | [Parts of a File](parts.md)         |
| `starter`    | For a part, the file the target's starts from when it is absent.  | [Parts of a File](parts.md)         |
| `template`   | Render the file with the answers and the graph.                   | [Templates](templates.md)           |
| `when`       | When the entry applies: features, profiles, variables, paths.     | [Gates and Scaffolds](gates.md)     |
| `scaffold`   | The scaffold group the file is a starter of.                      | [Gates and Scaffolds](gates.md)     |
| `executable` | Write the file with mode 755.                                     | below                               |
| `validate`   | How a merged file is checked: `toml`, `json`, `yaml` or `none`.   | [Updating and Merging](updating.md) |

## Policies

| Policy  | Meaning                                                           |
| ------- | ----------------------------------------------------------------- |
| `owned` | The default. The profile is authoritative; local edits are drift. |
| `merge` | Local edits are kept, and merged with the profile's changes.      |
| `once`  | Written if absent; the target's from then on.                     |

What `devset apply` does with each file, by what is on disk:

| On disk                                | `apply`                                                   | `apply --force` |
| -------------------------------------- | --------------------------------------------------------- | --------------- |
| absent, never written                  | write it                                                  | write it        |
| present, never written                 | **adopt** it: record the profile's version, keep the file | overwrite it    |
| `unchanged`: as devset wrote it        | write it, if the profile's version changed                | the same        |
| `cosmetic`: changed in whitespace only | write it, if the profile's version changed                | the same        |
| `edited`                               | keep it; `status` reports it                              | restore it      |
| `missing`: deleted                     | respect the deletion; `status` reports it                 | restore it      |

`--force` touches `owned` files only. A `once` file is written only when it is
absent and was never written; after that it is the target's. A file of a
[scaffold](gates.md#scaffolds) is `once`. A `merge` file is kept like an `owned`
one, except that when the profile's version changes, an update merges the two.

**Adoption** is what makes a first `init` into an existing repository safe:
devset records the profile's version as the file's base and keeps the file, so
`status` shows how the repository's file differs from the profile, and nothing
is lost.

**Whitespace at the ends is never an edit.** devset compares text in a canonical
form, without a BOM, the whitespace that ends a line, CRLF line endings, or
blank lines at the end of the file, so an editor or formatter that changes only
those never makes drift, and `apply` does not rewrite the file for them. A
binary file is compared byte for byte.

## Scripts

A file marked `executable = true` is written with mode 755 wherever devset
writes it, so a script a profile ships runs as `./setup.sh`, and git records it
executable. The mode is not part of what devset records: changing it is not
drift.

## Paths devset Refuses

A profile's paths are relative, and stay inside the target. devset refuses a
profile with a path under `.git` or `.devset`, compared without case, since on a
case-insensitive filesystem `.GIT/config` is `.git/config`; or with two paths
that name one file on such a filesystem, as `README.md` and `readme.md` do. In
the target, a managed path that has become a symlink or a directory is an error
devset reports: it never writes through a symlink.
