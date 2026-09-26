# Getting Started

This chapter installs devset, applies a first profile to a repository, and shows
how devset treats an edit to a file it manages.

## Install

On Linux, on x86-64 or Arm, and on macOS on Apple silicon:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh
```

The script downloads the latest release's static binary for the machine, checks
it against the release's checksum, and, when the GitHub CLI is installed, its
build attestation; then it puts `devset` in `~/.local/bin`. It edits no shell
file, and says so when that directory is not on your `PATH`. Its options:

| Option               | Does                                                                  |
| -------------------- | --------------------------------------------------------------------- |
| `-v <version>`       | Installs that release, as `0.2.0`, rather than the latest.            |
| `-b <dir>`           | Installs into `<dir>`, rather than `$XDG_BIN_HOME` or `~/.local/bin`. |
| `--uninstall`        | Removes the binary it would install.                                  |
| `DEVSET_VERSION`     | The environment's way to say `-v`.                                    |
| `DEVSET_INSTALL_DIR` | The environment's way to say `-b`.                                    |
| `DEVSET_NO_ATTEST=1` | Skips the attestation check.                                          |

Pass options after `sh -s --`:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh -s -- -v 0.2.0 -b /usr/local/bin
```

The same binaries, other ways:

```sh
mise use -g github:atomix-labs/devset    # with mise
cargo binstall devset-cli                # with cargo-binstall
cargo install --locked devset-cli        # from source, with Rust 1.98 or later
```

Or download the archive for your machine from the
[releases](https://github.com/atomix-labs/devset/releases), check it against the
`.sha256` beside it, and put `devset` on your `PATH`. The package is
`devset-cli`; the binary it installs is `devset`.

A source in a git repository needs `git` on your `PATH`; a local one needs
nothing else.

## A First Target

A target is a directory devset manages. `devset new` starts one in a new
directory, `devset init` in the one you are in:

```console
$ devset new hello
     Created hello/.devset/config.toml
help: add a layer: `devset add <source>/<profile> --git <url>`, or `--path <dir>`
```

`.devset/config.toml` starts as a commented skeleton: where profiles come from,
and which to apply. Neither command runs git or needs a source; a target with no
layer is a target all the same.

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

`devset new --profile <dir>` writes a profile's skeleton, its manifest a
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

- [Profiles](profiles.md) says what a profile can declare.
- [Composing Profiles](composing.md) covers sources, requirements and features.
- [Updating and Merging](updating.md) says what happens when a profile changes.
- [In CI](ci.md) makes drift fail a build.
