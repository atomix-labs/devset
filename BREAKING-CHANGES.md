# Breaking Changes

A migration note for every breaking change, newest release first: what changed,
why, and what a user does about it. [CHANGELOG.md](CHANGELOG.md) lists every
change; this lists only those a user must act on.

## Summary

| Release | Change                                                              | Who acts                         |
| ------- | ------------------------------------------------------------------- | -------------------------------- |
| v0.3.0  | [A layer listed twice is refused](#a-layer-listed-twice-is-refused) | a target that lists one twice    |
| v0.2.0  | [Profiles are named, in sources](#profiles-are-named-in-sources)    | every target, every profile      |
| v0.2.0  | [`update` moves sources](#update-moves-sources)                     | scripts that run `update <name>` |
| v0.2.0  | [`devset-core`'s API](#the-api-of-devset-core)                      | callers of the library           |

## V0.3.0

### A Layer Listed Twice Is Refused

**What changed.** devset refuses a `.devset/config.toml` whose `[[layers]]` name
one profile twice; it used to take the features of both without a word.

**Why.** A layer is named by its profile, so two entries for one profile, each
with its own features or `default-features`, say two things of one layer.

**What to do.** Keep one `[[layers]]` entry for the profile, with the features
of both. `devset add <source>/<profile> --features <feature>` now turns a
feature on in the one entry.

## V0.2.0

### Profiles Are Named, in Sources

**What changed.** A target names each source once, in `[sources]`, and each
layer by `source/profile`; a source's profiles are found by name wherever they
are in it. A profile requires others by name, in `[requires]`, not by path in
`[profile] requires`, and may offer features, gates and scaffolds. devset 0.2
reads profiles written for it, and refuses a `.devset/config.toml` written for
0.1, whose layers named locations.

**Why.** Names make profiles composable as crates are: requirements that turn
features on, a collection free to regroup its profiles, and gates and scaffolds
that let one profile scaffold a project from nothing or wire up one that exists.

**What to do.** Take a release of each source written for devset 0.2 (for atxp,
0.4.0 or later), then write `.devset/config.toml` again, one source for each
repository or directory the layers named:

```toml
# devset 0.1
[[layers]]
git  = "https://github.com/atomix-labs/atxp"
tag  = "v0.3.2"
path = "profiles/rust"

[[layers]]
path = "../house/deploy"
```

```toml
# devset 0.2
[sources]
atxp  = { git = "https://github.com/atomix-labs/atxp", tag = "v0.4.0" }
house = { path = "../house" }

[[layers]]
profile = "atxp/rust"

[[layers]]
profile = "house/deploy"
```

A layer is its profile's `name`, not its directory. Keep `[merge]` and `[files]`
as they are. Then run `devset apply`: `state.toml` is read as it is, so every
file devset wrote is still known as its, and `lock.toml` is resolved anew and
written as version 2.

A profile you author moves `requires` out of `[profile]` into a table keyed by
name. A sibling is named alone; another source takes its `git` fields:

```toml
# devset 0.1
[profile]
name     = "rust"
requires = ["../rustfmt", { git = "https://github.com/acme/profiles", tag = "v2.0.0", path = "deny" }]
```

```toml
# devset 0.2
[profile]
name   = "rust"
devset = ">=0.2"

[requires]
rustfmt = {}
deny    = { git = "https://github.com/acme/profiles", tag = "v2.0.0" }
```

### `update` Moves Sources

**What changed.** `devset update <name>` moves a source: the one named so in
`[sources]`, or the one the layer named so comes from, with every profile of it.

**Why.** Every profile of a source is at one commit, so a source moves as one.

**What to do.** Nothing, to move a layer's source; name the source to be plain
about it: `devset update atxp`.

### The API of `devset-core`

**What changed.** The library follows the model: `Config` holds `sources` and
`LayerSpec`s; `Target::add_source`, `add_layer(LayerSpec)` and
`remove_layer(&ProfileName)` replace the source-keyed calls; names are types in
`devset_core::name`; `Refresh::Only` replaces `Refresh::Layer`; errors gain the
variants the model needs and lose `NotAProfile`, `TooDeep` and `Escapes`.

**What to do.** Follow the compiler; the crate's documentation shows the chain
with a named source.
