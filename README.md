<!-- >>> devset: project >>> -->
<!-- dprint-ignore-start -->

<h1 align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/atomix-labs/devset/main/docs/src/media/logo-dark.svg">
    <img alt="devset" src="https://raw.githubusercontent.com/atomix-labs/devset/main/docs/src/media/logo-light.svg" height="56">
  </picture>
</h1>

<p align="center">Share your repositories' configuration, and keep it in sync without losing local edits.</p>

<p align="center">
  <a href="https://github.com/atomix-labs/devset/actions/workflows/check.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/atomix-labs/devset/check.yml?branch=main&amp;style=flat-square&amp;label=check"></a>
  <a href="https://crates.io/crates/devset-core"><img alt="crates.io" src="https://img.shields.io/crates/v/devset-core?style=flat-square"></a>
  <a href="https://docs.rs/devset-core"><img alt="docs.rs" src="https://img.shields.io/docsrs/devset-core?style=flat-square"></a>
  <a href="https://atomix-labs.github.io/devset/"><img alt="Book" src="https://img.shields.io/badge/book-read-blue?style=flat-square"></a>
  <a href="https://github.com/atomix-labs/devset"><img alt="managed with devset" src="https://img.shields.io/badge/managed_with-devset-0969da?style=flat-square&amp;logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHRpdGxlPmRldnNldDwvdGl0bGU+PHBhdGggZmlsbD0iI2YwZjZmYyIgZD0ibTE2IDMgMTMgNi41TDE2IDE2IDMgOS41WiIvPjxwYXRoIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2YwZjZmYyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIiBzdHJva2Utd2lkdGg9IjIuNSIgZD0ibTMgMTYgMTMgNi41TDI5IDE2TTMgMjIuNSAxNiAyOWwxMy02LjUiLz48L3N2Zz4K"></a>
</p>

<!-- dprint-ignore-end -->
<!-- <<< devset: project <<< -->

<!-- dprint-ignore-start -->

<p align="center">
  <a href="https://atomix-labs.github.io/devset/getting-started.html">Getting Started</a> ·
  <a href="https://atomix-labs.github.io/devset/">Manual</a> ·
  <a href="https://github.com/atomix-labs/atxp">Profiles</a> ·
  <a href="https://github.com/atomix-labs/devset/blob/main/CHANGELOG.md">Changelog</a>
</p>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/atomix-labs/devset/main/docs/src/media/demo-dark.gif">
    <img alt="devset adds a profile from an old release, the repository adds a line of its own to typos.toml, and devset update names the newer releases and moves to one: the profile's change comes in, and the line is still there" src="https://raw.githubusercontent.com/atomix-labs/devset/main/docs/src/media/demo-light.gif" width="720">
  </picture>
</p>

<!-- dprint-ignore-end -->

A profile is a folder of configuration in a git repository: formatter settings,
lint rules, CI workflows, whatever your repositories share. devset applies it to
a repository, records what it wrote, and later brings in the profile's changes
while keeping the repository's own edits. [atxp], provided by Atomix Labs, is a
collection to start from; anyone can keep a collection of their own the same
way.

## Install

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh
```

A static binary for Linux, on x86-64 and Arm, or for macOS on Apple silicon,
checked against its release's checksum and build attestation. Or:

| You use        | Install                                 | Upgrade                                  | Uninstall                                 |
| -------------- | --------------------------------------- | ---------------------------------------- | ----------------------------------------- |
| the installer  | the line above                          | the same line again                      | the line with `sh -s -- --uninstall`      |
| mise           | `mise use -g github:atomix-labs/devset` | `mise upgrade github:atomix-labs/devset` | `mise unuse -g github:atomix-labs/devset` |
| cargo-binstall | `cargo binstall devset-cli`             | the same command                         | `cargo uninstall devset-cli`              |
| cargo          | `cargo install --locked devset-cli`     | the same command                         | `cargo uninstall devset-cli`              |

The package is `devset-cli`, and the binary `devset`; `cargo install` builds it
with Rust 1.98 or later. [Install] has the installer's options, checking a
download, the shell completions, and what uninstalling leaves.

## Quick Start

A new Rust project, from atxp's bundle, set up and checked:

```sh
mkdir hello && cd hello && git init
devset add atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.20.0 --var repository=you/hello
./setup.sh && just check
```

A repository that has configuration of its own takes profiles the same way. What
it lacks, a profile writes once and leaves to it; what it has, a profile that
owns part of a file joins, taking only its keys or its block. `--dry-run` shows
what would change first:

```sh
devset add atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.20.0 --dry-run
```

In CI, fail the build when a file has drifted from its profile:

```yaml
- run: devset status --exit-code
```

Later, take the profiles' changes:

```sh
devset update --dry-run              # names the newer releases of each source
devset update atxp --tag <release>   # takes one, merging your edits
```

## Why devset

- **It keeps your edits.** An update merges three ways against what devset
  wrote; where the profile and the repository changed the same lines, it stops,
  and leaves the conflict beside the file for you to resolve, never in it.
- **It owns only its part of a file.** Some keys of a `Cargo.toml`, a block of a
  `.gitignore`: the rest of the file stays the repository's.
- **Profiles compose like crates.** A profile requires others by name, and
  offers features that unify across everything that requires it, as Cargo's do.
- **Drift fails CI.** `.devset/` records what devset wrote, so `devset status`
  tells a local edit from a profile's change, and a reformatted file from an
  edited one.
- **It never runs a profile's code.** A profile is data: files, and when to
  write them.

## Profiles

[atxp], provided by Atomix Labs, holds profiles for any repository and for Rust
ones, one concern each, with a bundle that takes them: use them, build on them,
or contribute. Or write your own: a profile is a directory with a
`profile.toml`, which says how devset manages each file and when, and `files/`,
which holds them as a repository should have them.

```toml
[profile]
name = "base"

[files.".editorconfig"]      # owned: the profile's, so an edit is drift

[files."deny.toml"]
policy = "merge"             # the repository's edits are kept, and merged
```

Profiles do real work beside copying files: a scaffold writes starter files
where a project has none of its own, and a gate applies an entry only when a
feature is on, another profile is applied, or a file exists. Put them in a git
repository, and any repository takes one with `devset add base --git <url> --tag
<tag>`. [Designing Profiles] has the craft of it.

Collections others publish carry the GitHub topic
[`devset-collection`](https://github.com/topics/devset-collection). A repository
devset manages may say so with
[![managed with devset](https://img.shields.io/badge/managed_with-devset-0969da?style=flat-square&logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAzMiAzMiI+PHRpdGxlPmRldnNldDwvdGl0bGU+PHBhdGggZmlsbD0iI2YwZjZmYyIgZD0ibTE2IDMgMTMgNi41TDE2IDE2IDMgOS41WiIvPjxwYXRoIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2YwZjZmYyIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBzdHJva2UtbGluZWpvaW49InJvdW5kIiBzdHJva2Utd2lkdGg9IjIuNSIgZD0ibTMgMTYgMTMgNi41TDI5IDE2TTMgMjIuNSAxNiAyOWwxMy02LjUiLz48L3N2Zz4K)](https://github.com/atomix-labs/devset),
which
[Getting Started](https://atomix-labs.github.io/devset/getting-started.html#show-it)
gives to copy.

## How It Compares

cookiecutter starts a project from a template and leaves it there; copier and
cruft also carry a template's later changes into the project. devset is built
for that second part: a repository takes many small profiles, from one source or
several, each owning whole files, a file's keys, or a block of it, and stays
current release by release, with drift from them failing CI. A profile is data,
so taking one runs nothing; projen, by contrast, writes its files from code, and
owns them whole. [Comparison] sets devset beside copier, cruft, projen, chezmoi
and cargo-generate, row by row.

## Documentation

| I want to                                   | Read                                    |
| ------------------------------------------- | --------------------------------------- |
| try devset on a repository                  | [Getting Started]                       |
| start a new Rust project                    | [A New Rust Project]                    |
| adopt a repository that has its own setup   | [An Existing Repository]                |
| write a profile                             | [Write a Profile]                       |
| share profiles across repositories          | [Share a Team's Configuration]          |
| publish a collection for others             | [Publish a Collection]                  |
| take a newer release, or resolve a conflict | [Update and Resolve Conflicts]          |
| check for drift in CI                       | [In CI]                                 |
| see how devset differs from similar tools   | [Comparison]                            |
| look a command or a file's keys up          | [Command Reference], [`profile.toml`]   |
| run the examples                            | [`examples/`][examples]                 |
| use the library                             | [devset-core on docs.rs][docs.rs]       |
| know how devset is built                    | [ARCHITECTURE.md][Architecture]         |
| move across a breaking change               | [BREAKING-CHANGES.md][Breaking Changes] |
| report a vulnerability                      | [SECURITY.md][Security]                 |

## Contributing

Issues and pull requests are welcome. To work on devset, fork it, clone your
fork, and run `./setup.sh`, which installs mise, every tool devset pins and the
nightly Rust toolchain; `install.sh` is for using devset, not for developing it.

```sh
git clone https://github.com/<you>/devset.git && cd devset
./setup.sh
just check    # formatting, lints, tests and the docs, as CI runs them
```

[CONTRIBUTING.md][Contributing] has the rest: how a pull request lands, the
tests and snapshots, and the style. [Report a bug], [request a feature], or ask
a question in [Discussions].

## License

MIT: see [LICENSE][license].

[atxp]: https://github.com/atomix-labs/atxp
[Getting Started]: https://atomix-labs.github.io/devset/getting-started.html
[A New Rust Project]: https://atomix-labs.github.io/devset/new-project.html
[An Existing Repository]: https://atomix-labs.github.io/devset/existing-repository.html
[Write a Profile]: https://atomix-labs.github.io/devset/write-a-profile.html
[Designing Profiles]: https://atomix-labs.github.io/devset/designing.html
[Share a Team's Configuration]: https://atomix-labs.github.io/devset/share-configuration.html
[Publish a Collection]: https://atomix-labs.github.io/devset/publish-a-collection.html
[Update and Resolve Conflicts]: https://atomix-labs.github.io/devset/resolving.html
[In CI]: https://atomix-labs.github.io/devset/ci.html
[Comparison]: https://atomix-labs.github.io/devset/comparison.html
[Command Reference]: https://atomix-labs.github.io/devset/reference/devset.html
[`profile.toml`]: https://atomix-labs.github.io/devset/profile-toml.html
[examples]: https://github.com/atomix-labs/devset/tree/main/examples
[Breaking Changes]: https://github.com/atomix-labs/devset/blob/main/BREAKING-CHANGES.md
[Architecture]: https://github.com/atomix-labs/devset/blob/main/ARCHITECTURE.md
[Contributing]: https://github.com/atomix-labs/devset/blob/main/CONTRIBUTING.md
[Security]: https://github.com/atomix-labs/devset/blob/main/SECURITY.md
[Discussions]: https://github.com/atomix-labs/devset/discussions
[Install]: https://atomix-labs.github.io/devset/install.html
[Report a bug]: https://github.com/atomix-labs/devset/issues/new?template=bug_report.yml
[request a feature]: https://github.com/atomix-labs/devset/issues/new?template=feature_request.yml
[docs.rs]: https://docs.rs/devset-core
[license]: https://github.com/atomix-labs/devset/blob/main/LICENSE
