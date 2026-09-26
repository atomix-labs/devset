# Schemas

devset publishes a JSON Schema for each file people write, so an editor can
complete and check them:

| File                  | Schema                                             |
| --------------------- | -------------------------------------------------- |
| `profile.toml`        | [`schema/profile.json`](schema/profile.json)       |
| `.devset/config.toml` | [`schema/config.json`](schema/config.json)         |
| `collection.toml`     | [`schema/collection.json`](schema/collection.json) |

Each is served beside this manual, at
`https://atomix-labs.github.io/devset/schema/<name>.json`, and `devset schema
profile`, `devset schema config` and `devset schema collection` print the ones a
build of devset knows.

## In an Editor

[Taplo](https://taplo.tamasfe.dev), and the editors built on it such as VS
Code's Even Better TOML, take a schema from a directive on a file's first line:

```toml
#:schema https://atomix-labs.github.io/devset/schema/profile.json
[profile]
name = "rust"
```

Or, for every profile in a repository, from `taplo.toml`:

```toml
[[rule]]
include = ["**/profile.toml"]
schema  = { path = "https://atomix-labs.github.io/devset/schema/profile.json" }
```
