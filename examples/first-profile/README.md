# A First Profile

A collection of one profile, `base`, applied to a new repository: an
EditorConfig the profile owns, and a rustfmt configuration the repository may
add to.
[Getting Started](https://atomix-labs.github.io/devset/getting-started.html)
walks through the same steps.

```text
profiles/
  base/
    profile.toml     [files.".editorconfig"], and [files."rustfmt.toml"] under merge
    files/           the two files, as a repository should have them
```

Run these from this directory, or a copy of it:

```console
$ git init -q repo
$ cd repo
$ devset add team/base --path ../profiles
    Starting a target: .devset/config.toml
     Created .editorconfig
     Created rustfmt.toml
    Finished 2 changes
$ devset status
team/base

All 2 files match the profile.
```

Edit both files. The EditorConfig is the profile's, so its edit is drift; the
rustfmt configuration is under `merge`, so its edit is the repository's:

```console
$ echo 'indent_size = 2' >> .editorconfig
$ echo 'hard_tabs = false' >> rustfmt.toml
$ devset status
team/base

Drifted — `devset apply --force` restores:
    edited     .editorconfig  owned  restore

Local changes, kept:
    edited     rustfmt.toml   merge
$ devset apply --force
    Restored .editorconfig
    Finished 1 change
$ devset status
team/base

Local changes, kept:
    edited     rustfmt.toml  merge
```
