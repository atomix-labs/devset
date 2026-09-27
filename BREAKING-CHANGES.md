# Breaking Changes

A migration note for every breaking change, newest release first: what changed,
why, and what a user does about it. [CHANGELOG.md](CHANGELOG.md) lists every
change; this lists only those a user must act on.

## Summary

| Release | Change                                                              | Who acts                         |
| ------- | ------------------------------------------------------------------- | -------------------------------- |
| v0.4.0  | [`init` starts, `add` adds](#init-starts-add-adds)                  | scripts that run `new` or `init` |
| v0.4.0  | [`features` is `explain`](#features-is-explain)                     | scripts that run `features`      |
| v0.4.0  | [`apply` finishes a conflict](#apply-finishes-a-conflict)           | whoever resolves a conflict      |
| v0.4.0  | [`schema` is published](#schema-is-published)                       | editor setups that ran `schema`  |
| v0.4.0  | [The log is on `stderr`](#the-log-is-on-stderr)                     | scripts that read the log        |
| v0.3.0  | [A layer listed twice is refused](#a-layer-listed-twice-is-refused) | a target that lists one twice    |
| v0.2.0  | [Profiles are named, in sources](#profiles-are-named-in-sources)    | every target, every profile      |
| v0.2.0  | [`update` moves sources](#update-moves-sources)                     | scripts that run `update <name>` |
| v0.2.0  | [`devset-core`'s API](#the-api-of-devset-core)                      | callers of the library           |

## V0.4.0

devset has ten commands, each answering one question; three merged into others
and one moved its flags.

| Before                                 | After                                             |
| -------------------------------------- | ------------------------------------------------- |
| `devset new hello`                     | `devset init hello`                               |
| `devset new --profile lint`            | `devset init lint --profile`                      |
| `devset new --collection acme`         | `devset init acme --collection`                   |
| `devset new hello atxp/rust --git ...` | `devset init hello`, then `devset add` in `hello` |
| `devset init atxp/rust --git ...`      | `devset add atxp/rust --git ...`                  |
| `devset features [LAYER]`              | `devset explain [LAYER]`                          |
| `devset update --continue`             | `devset apply --continue`                         |
| `devset update --abort [--force]`      | `devset apply --abort [--force]`                  |
| `devset schema profile`                | the published schema, named by `#:schema`         |
| editing `tag` in `.devset/config.toml` | `devset update atxp --tag <tag>`, new in v0.4.0   |

### `init` Starts, `add` Adds

**What changed.** `devset new` is gone: `devset init [PATH]` makes the directory
a target, creating it if missing, and `--profile` or `--collection` make it a
profile or a collection to author. `init` takes no layer any more: `devset add`
adds every layer, the first too, and starts the target itself at a git
repository's top level or in an empty directory.

**Why.** `new` and `init` were two commands for one act, and the first layer
came from one command and every later one from another.

**What to do.** Replace `devset new <dir>` with `devset init <dir>`, and `devset
init <layer> ...` with `devset add <layer> ...`. In a directory that is neither
a repository's top level nor empty, run `devset init` before the first `add`.

### `features` Is `explain`

**What changed.** `devset features` is gone; `devset explain` with no argument
shows every layer's features, and `devset explain <layer>` one layer's, as
`explain <file>` shows why a file is managed.

**Why.** Both answered why something is as it is.

**What to do.** Replace `devset features` with `devset explain`. Where a layer
and a managed file share a name, `explain <name>` is the layer and `explain
./<name>` the file.

### `apply` Finishes a Conflict

**What changed.** `update --continue` and `update --abort` are `apply
--continue` and `apply --abort`.

**Why.** A conflict can come from `add`, `remove`, `apply` or `update`, which
all end in an apply; it is finished there.

**What to do.** After resolving `.devset/conflicts/`, run `devset apply
--continue`; to take the run back, `devset apply --abort`.

### `schema` Is Published

**What changed.** `devset schema` is gone. The schemas are at
`https://atomix-labs.github.io/devset/schema/<file>.json`, and every
`config.toml`, `profile.toml` and `collection.toml` devset writes names its
schema on its first line.

**Why.** An editor needs a URL, not a command to run.

**What to do.** Point an editor at the published schema, or add `#:schema
https://atomix-labs.github.io/devset/schema/profile.json` as the first line of a
file written before.

### The Log Is on `stderr`

**What changed.** What `add`, `remove`, `apply`, `update` and `init` did or
would do is printed on stderr; stdout carries what a command answers: `status`,
its JSON, `diff`, `explain`, `list` and `completions`.

**Why.** A script reading stdout read the log beside the answer; cargo, whose
log devset's follows, prints it on stderr.

**What to do.** Read `devset status --json` for what a script needs, or read
stderr for the log.

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
