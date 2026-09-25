# Composing Profiles

A target applies a list of profiles, its **layers**, and a profile may require
others. This is how a team builds on a shared collection instead of copying it:
its own profile requires the collection's, and adds what is the team's.

## Layers

`.devset/config.toml` lists a target's layers, in order; `devset init` adds one,
and `devset remove` takes one away.

```toml
[[layers]]
path = "../profiles/base"            # a local directory, relative to the target

[[layers]]
git  = "https://github.com/acme/profiles"
tag  = "v1.4.0"                      # or branch = "main", or rev = "<commit>"
path = "rust"                        # the profile's directory in the repository
```

A source uses Cargo's git-dependency fields. `path` alone is a directory,
relative to the target; with `git`, it is a directory inside the repository.
`tag`, `branch` and `rev` go only with `git`, and at most one of them; with
none, the repository's default branch. A `git` URL may be a local path, as
`../profiles.git`, relative to the target too.

A layer is named by its profile's `name`, which is what `devset update <name>`
and `devset remove <name>` take.

## When Layers Collide

Two layers that provide the same file collide, and devset refuses to guess which
one wins:

```console
$ devset init --path ../rust
error: fmt.toml is provided by base and rust
  |
  = help: choose one in .devset/config.toml:
              [files."fmt.toml"]
              from = "base"   # or "rust"
```

`from` is the target's choice; [Settings](settings.md) has the rest of what the
target decides. Layers that own [parts](parts.md) of one file share it without a
choice, so long as no two own the same key.

## Profiles That Require Profiles

`requires` in `[profile]` names the profiles one builds on:

```toml
[profile]
name     = "rust"
requires = [
    "../rustfmt",                                                            # beside this one
    { git = "https://github.com/acme/profiles", tag = "v2.0.0", path = "deny" },
]
```

A string is a profile beside this one, a path relative to it, read from the same
source at the same commit: one tag pins a whole collection. A table is a git
source, written as a layer is.

Each required profile becomes a layer of its own, before the profile that
requires it, so the requirer's files come last and its choices stand. A profile
required twice, from the same source, is one layer. A profile with nothing but
`requires` is a **bundle**.

`status` folds each required layer into a `requires` line under the layer that
brings it in; `devset status -v` lists them all, each marked with what required
it. A required layer is not the target's to name: `update` and `remove` take the
configured layer that brings it in.

devset refuses requirements that cannot hold: a cycle, two required profiles
that provide the same file, and a requirement table without `git`.
