# Part of a File

A profile that owns only what it needs of two files the repository already has:
the `[workspace.lints.rust]` keys of its `Cargo.toml`, and a block of its
`.gitignore`. Everything else in both files stays the repository's.
[Parts of a File](https://atomix-labs.github.io/devset/parts.html) says how each
merges.

```text
profiles/rust/     scope = "keys" for Cargo.toml, scope = "block" for .gitignore
repo/              a repository with a Cargo.toml and a .gitignore of its own
```

Run these from this directory, or a copy of it:

```console
$ cd repo
$ git init -q
$ devset add team/rust --path ../profiles
    Starting a target: .devset/config.toml
     Created .gitignore [block: rust]
     Created Cargo.toml [keys: rust]
    Finished 2 changes
$ cat Cargo.toml
[workspace]
members  = ["crates/*"]
resolver = "3"

[workspace.lints.rust]
unsafe_code = "forbid"
$ cat .gitignore
/local-notes

# >>> devset: rust >>>
/target
# <<< devset: rust <<<
$ devset explain Cargo.toml
Cargo.toml
    team/rust  keys  owned  applies
        unchanged
```

The repository's `[workspace]` table and its `/local-notes` line are its own:
devset never reports them, and an update never touches them. The block may move
anywhere in the file, and stays where it is put.
