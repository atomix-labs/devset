# Getting Started

This tutorial applies a first profile to a repository, and shows how devset
treats an edit to a file it manages.

## Install

[Install](install.md) has every way to install devset; the shortest:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh
```

## A First Target

A target is a directory devset manages. `devset init` makes one, in the
directory you name, created if missing, or in the one you are in:

```console
$ devset init hello
     Created hello/.devset/config.toml
help: add a layer: `devset add <source>/<profile> --git <url>`, or `--path <dir>`
```

`.devset/config.toml` starts as a commented skeleton: where profiles come from,
and which to apply. `init` runs no git and needs no source; a target with no
layer is a target all the same. At a git repository's top level, or in an empty
directory, `init` can wait: the first `devset add` starts the target itself.

## A First Profile

A profile is a directory holding `profile.toml`, which lists the files it
manages, and `files/`, which holds them as the target should have them. Profiles
live in a source, a directory of them; make one beside a repository:

```text
profiles/
  base/
    profile.toml
    files/
      .editorconfig
      rustfmt.toml
```

```toml
# profiles/base/profile.toml
[profile]
name = "base"

[files.".editorconfig"]

[files."rustfmt.toml"]
policy = "merge"
```

`devset init <dir> --profile` writes a profile's skeleton, its manifest a
commented tour of what it can say. Then, in the repository, add it as a layer,
naming the source `house`:

```console
$ devset add house/base --path ../profiles
     Created .editorconfig
     Created rustfmt.toml
    Finished 2 changes
```

devset named the source in `.devset/config.toml`, applied `house/base`, and
recorded what it wrote in `.devset/`. Commit `.devset/` with the files: it is
what lets a teammate, or CI, see the same thing.

```toml
[sources]
house = { path = "../profiles" }

[[layers]]
profile = "house/base"
```

## Where Things Stand

`devset status` compares every managed file with the profile, and says what
would change it:

```console
$ devset status
house/base

All 2 files match the profile.
```

Edit both files, and ask again:

```console
$ devset status
house/base

Drifted — `devset apply --force` restores:
    edited     .editorconfig  owned  restore

Local changes, kept:
    edited     rustfmt.toml   merge
```

`.editorconfig` has the default policy, `owned`: the profile is authoritative,
and the edit is drift. `devset apply` still leaves it alone, since devset never
destroys an edit unless told to; `devset apply --force` restores it.
`rustfmt.toml` is `merge`: the edit is the repository's, kept, and merged with
the profile's own changes when the profile changes.

A file whose only change is whitespace at the end of its lines or of the file,
its line endings or a BOM is `cosmetic`, not edited: an editor that trims
whitespace or converts line endings never makes drift.

## A Source in Git

A team keeps its profiles in a git repository, and a target pins a tag of it:

```sh
devset add acme/rust --git https://github.com/acme/profiles --tag v1.4.0 --features docs
devset add acme/book                       # the source is named now: a name is enough
```

`.devset/lock.toml` records the commit the tag named. Every later `apply`, on
any machine, uses that commit; `devset update` moves to what the tag, or a
branch, names now, and merges your edits with what changed. `devset list acme`
shows every profile the source holds, and its features.

## Next

- [Write a Profile](write-a-profile.md) goes further: parts of a file, variables
  and features.
- [A New Rust Project](new-project.md) and
  [An Existing Repository](existing-repository.md) apply a whole collection.
- [In CI](ci.md) makes drift fail a build.
- [`examples/`](https://github.com/atomix-labs/devset/tree/main/examples) holds
  each step as an example that runs.
