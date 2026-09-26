# Composing Profiles

A target applies profiles from sources, and a profile builds on others and
offers features, as a crate builds on crates and offers features. This is how a
team builds on a shared collection instead of copying it: its own profiles
require the collection's, turn on what they need, and add what is the team's.

## Sources

`.devset/config.toml` names each source once, and applies profiles from them as
layers, by `source/profile`:

```toml
[sources]
atxp  = { git = "https://github.com/atomix-labs/atxp", tag = "v0.4.0" }
house = { path = "../house-profiles" }

[[layers]]
profile  = "atxp/rust"
features = ["docs", "agents"]

[[layers]]
profile          = "house/deploy"
default-features = false
```

A source uses Cargo's git-dependency fields. `path` alone is a directory,
relative to the target; with `git`, it is a directory inside the repository that
holds the profiles. `tag`, `branch` and `rev` go only with `git`, and at most
one of them; with none, the repository's default branch. A `git` URL may be a
local path, as `../profiles.git`, relative to the target too.

A source's profiles are every `profile.toml` in it, found by their `name`
wherever they are, so a collection may group its profiles as it likes, and move
them, without breaking a target. Names are unique in a source, which devset
checks when it reads one; a `profile.toml` inside another profile's `files/` is
that profile's payload. A source may describe itself in `collection.toml`:

```toml
[collection]
name        = "atxp"                                   # the name a target gives it unless it chooses another
description = "Profiles for Rust repositories"
```

`devset add` names a source as it adds it: `devset add atxp/rust --git <url>`
names it `atxp`; without a name, after its `collection.toml`, or else its
directory. A profile named alone, as `devset add book`, is in the target's only
source. `devset list` shows each source's profiles, their descriptions and their
features.

## Layers

Each active profile is a **layer**: the target's own, and every profile they
require. A layer is named by its profile's name, which is what `from`, `devset
remove` and `devset features` take; so two active profiles may not share one,
whichever sources they come from. Layers apply in order, each requirement before
the profile that requires it.

Two layers that provide the same file collide, and devset refuses to guess which
one wins:

```console
$ devset add house/rust
error: fmt.toml is provided by base and rust
  |
  = help: choose one in .devset/config.toml:
              [files."fmt.toml"]
              from = "base"   # or "rust"
```

`from` is the target's choice; [Settings](settings.md) has the rest of what the
target decides. Layers that own [parts](parts.md) of one file share it without a
choice, so long as no two own the same key, and one layer's whole `once` file
may [start](parts.md#starters) a file the others own parts of.

## Requirements

`[requires]` names the profiles one builds on, by name:

```toml
[requires]
just      = {}                                         # in this profile's source
lychee    = { optional = true }                        # only when a feature activates it
github-ci = { features = ["pages"], default-features = false }
deny      = { git = "https://github.com/acme/profiles", tag = "v2.0.0" }
```

A requirement is in the requirer's own source, at the same commit, unless it
names another with `git`: one tag pins a whole collection. Each is a layer of
its own, before the profile that requires it, so the requirer's files come last
and its choices stand. A profile required from two places is one layer. A
profile with nothing but requirements is a **bundle**.

`status` folds each required layer, applied as it is, into a `requires` line
under the layer that brings it in; `devset status -v` lists them all. A required
layer is not the target's to remove: `devset remove` takes the configured one
that brings it in.

## Features

`[features]` declares what a target, or a profile that requires this one, may
turn on:

```toml
[features]
default = ["katex", "links"]
katex   = []                       # files gated on it: when = { features = ["katex"] }
api     = ["katex"]                # another feature of this profile
pages   = ["github-ci/pages"]      # a feature of a requirement, which activates it too
links   = ["dep:lychee"]           # an optional requirement, activated
agents  = ["mdbook?/agents"]       # a requirement's feature, only if the target has it anyway
```

Features follow Cargo's rules:

- **Additive.** A feature adds files, parts, requirements and features. It never
  removes or replaces: a choice between alternatives is a
  [variable](templates.md).
- **Unified.** A profile is one layer, whoever asks for it: the features the
  target and every requirer turn on are unioned, and devset expands the graph
  until nothing more turns on.
- **Default.** `default` lists the features on unless the layer, or a requirer,
  sets `default-features = false`. When a layer switches them off and a requirer
  turns them on, they are on, and devset warns and names the requirer.
- **Named.** An unknown feature is an error, with a near match.

`devset features` shows each layer's features, and who turned each on; `devset
features <layer>` adds the ones it leaves off. The lock records them.

```console
$ devset features
atxp/ci  [pages]  (required by mdbook)
    pages  <- mdbook/pages
atxp/mdbook  [api, katex, pages]  (required by rust)
    api    <- rust
    katex  <- default, mdbook/api
    pages  <- rust/docs
atxp/rust  [docs]
    docs  <- target
```

devset refuses a graph that cannot hold: a cycle of requirements, a profile
named twice from two sources, one source at two refs, and two required profiles
that provide the same file.
