# Install

devset is one static binary, for Linux on x86-64 and Arm, and for macOS on Apple
silicon. The installer takes the latest release:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh
```

It downloads the release's archive for the machine, checks it against the
release's checksum, and, when the GitHub CLI is installed, its build
attestation; then it puts `devset` in `~/.local/bin`. It edits no shell file,
and says so when that directory is not on your `PATH`.

## Options

| Option               | Does                                                                  |
| -------------------- | --------------------------------------------------------------------- |
| `-v <version>`       | Installs that release, as `0.5.2`, rather than the latest.            |
| `-b <dir>`           | Installs into `<dir>`, rather than `$XDG_BIN_HOME` or `~/.local/bin`. |
| `--uninstall`        | Removes the binary it would install.                                  |
| `DEVSET_VERSION`     | The environment's way to say `-v`.                                    |
| `DEVSET_INSTALL_DIR` | The environment's way to say `-b`.                                    |
| `DEVSET_NO_ATTEST=1` | Skips the attestation check.                                          |

Pass options after `sh -s --`:

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh -s -- -v 0.5.2 -b /usr/local/bin
```

## Other Ways

| You use        | Install                                 | Upgrade                                  | Remove                                    |
| -------------- | --------------------------------------- | ---------------------------------------- | ----------------------------------------- |
| mise           | `mise use -g github:atomix-labs/devset` | `mise upgrade github:atomix-labs/devset` | `mise unuse -g github:atomix-labs/devset` |
| cargo-binstall | `cargo binstall devset-cli`             | the same command                         | `cargo uninstall devset-cli`              |
| cargo          | `cargo install --locked devset-cli`     | the same command                         | `cargo uninstall devset-cli`              |

The package is `devset-cli`, and the binary `devset`; `cargo install` builds it
with Rust 1.98 or later. In a repository, pin devset as its other tools are: a
`mise.toml` naming `"github:atomix-labs/devset" = "0.5.2"` gives every machine
and CI job the same one, and moving the pin upgrades them all.

## Upgrade and Remove

The installer upgrades as it installs: run the line again for the latest
release, or with `-v <version>` for another. `--uninstall` removes the binary it
would install, from the same `-b` directory if you gave one. The table above has
the other ways'.

Removing the binary leaves devset's cache of fetched sources, under
`$XDG_CACHE_HOME/devset/` or `~/.cache/devset/`, which is safe to delete, and
any completion script you wrote. A repository devset manages keeps working
without it: [Stop Using devset](stop-using.md) says what to delete there.

## Checking a Download

Each release's archives are on its
[release page](https://github.com/atomix-labs/devset/releases), each with a
`.sha256` beside it, and each attested by the workflow that built it. To check
one by hand:

```sh
sha256sum -c devset-0.5.2-x86_64-unknown-linux-musl.tar.xz.sha256
gh attestation verify devset-0.5.2-x86_64-unknown-linux-musl.tar.xz --repo atomix-labs/devset
```

## Shell Completions

`devset completions <shell>` prints a completion script for bash, elvish, fish,
PowerShell or zsh:

```sh
devset completions bash > ~/.local/share/bash-completion/completions/devset
devset completions zsh > ~/.zfunc/_devset
devset completions fish > ~/.config/fish/completions/devset.fish
```

## What It Needs

[Platforms and Rust Versions](platforms.md) lists where the releases run and
what builds devset from source.

A source in a git repository needs `git` on your `PATH`; a local one needs
nothing else. devset installs no other tool, and runs none a profile names.
