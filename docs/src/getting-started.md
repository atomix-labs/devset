# Getting Started

This chapter installs devset, applies a first profile to a repository, and shows
how devset treats an edit to a file it manages.

## Install

Every release has a static binary for Linux, on x86-64 and Arm, and one for
macOS on Apple silicon. With [mise](https://mise.jdx.dev):

```sh
mise use -g github:atomix-labs/devset
```

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), which takes
the same archive:

```sh
cargo binstall devset-cli
```

Or download the archive for your machine from the
[releases](https://github.com/atomix-labs/devset/releases), check it against the
`.sha256` beside it, and put `devset` on your `PATH`.

From source, with Rust 1.98 or later, from
[crates.io](https://crates.io/crates/devset-cli):

```sh
cargo install --locked devset-cli
```

The package is `devset-cli`; the binary it installs is `devset`.

A profile in a git repository needs `git` on your `PATH`; a profile in a local
directory needs nothing else.

## A First Profile

A profile is a directory holding `profile.toml`, which lists the files it
manages, and `files/`, which holds them as the target should have them. Make one
beside a repository:

```text
profiles/base/
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

Then, in the repository, add it as a layer:

```console
$ devset init --path ../profiles/base
     Created .editorconfig
     Created rustfmt.toml
    Finished 2 changes
```

devset wrote both files, and recorded what it wrote in `.devset/`. Commit
`.devset/` with the files: it is what lets a teammate, or CI, see the same
thing.

## Where Things Stand

`devset status` compares every managed file with the profile, and says what
would change it:

```console
$ devset status
base  ../profiles/base

All 2 files match the profile.
```

Edit both files, and ask again:

```console
$ devset status
base  ../profiles/base

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

## A Profile in Git

A team keeps its profiles in a git repository, and a target pins a tag of it:

```sh
devset init --git https://github.com/acme/profiles --tag v1.4.0 --path rust
```

`.devset/lock.toml` records the commit the tag named. Every later `apply`, on
any machine, uses that commit; `devset update` moves to what the tag, or a
branch, names now, and merges your edits with what changed.

## Next

- [Profiles](profiles.md) says what a profile can declare.
- [Updating and Merging](updating.md) says what happens when a profile changes.
- [In CI](ci.md) makes drift fail a build.
