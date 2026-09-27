# Features and Gates

A profile whose entries apply only when their gate holds: `rustfmt.toml` where
the repository has a `Cargo.toml`, and a book's `book.toml` when the `docs`
feature is on.
[Gates and Scaffolds](https://atomix-labs.github.io/devset/gates.html) and
[Composing Profiles](https://atomix-labs.github.io/devset/composing.html#features)
say what else a gate and a feature can do.

```text
profiles/base/profile.toml

[features]
docs = []

[files."rustfmt.toml"]
when = { exists = ["Cargo.toml"] }

[files."docs/book.toml"]
when = { features = ["docs"] }
```

Run these from this directory, or a copy of it:

```console
$ git init -q repo
$ cd repo
$ devset add team/base --path ../profiles
    Starting a target: .devset/config.toml
     Created .editorconfig
    Finished 1 change
$ devset explain base
team/base
    off: docs
$ devset explain rustfmt.toml
rustfmt.toml
    team/base  file  owned  off: Cargo.toml will not exist
        when Cargo.toml exists: no
```

The repository gains a `Cargo.toml`, and the gate holds on the next run; then it
turns the feature on:

```console
$ touch Cargo.toml
$ devset apply
     Created rustfmt.toml
    Finished 1 change
$ devset add team/base --features docs
      Adding feature docs to layer team/base
     Created docs/book.toml
    Finished 1 change
$ devset explain base
team/base  [docs]
    docs  <- target
```

Turning the feature off again takes `docs/book.toml` away, unless the repository
has edited it, in which case it is kept, and devset stops tracking it.
