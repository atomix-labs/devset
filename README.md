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

| With           | Install                                 | Update           |
| -------------- | --------------------------------------- | ---------------- |
| mise           | `mise use -g github:atomix-labs/devset` | `mise upgrade`   |
| cargo-binstall | `cargo binstall devset-cli`             | the same command |
| cargo          | `cargo install --locked devset-cli`     | the same command |

The package is `devset-cli`, and the binary `devset`; `cargo install` builds it
with Rust 1.98 or later.

## Quick Start

A new Rust project, from atxp's bundle, set up and checked:

```sh
mkdir hello && cd hello && git init
devset add atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.8.0 --var repository=you/hello
./setup.sh && just check
```

A repository that has configuration of its own takes profiles the same way. What
it lacks, a profile writes once and leaves to it; what it has, a profile that
owns part of a file joins, taking only its keys or its block. `--dry-run` shows
what would change first:

```sh
devset add atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.8.0 --dry-run
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

## How It Compares

cookiecutter starts a project from a template and leaves it there; copier and
cruft also carry a template's later changes into the project. devset is built
for that second part: a repository takes many small profiles, from one source or
several, each owning whole files, a file's keys, or a block of it, and stays
current release by release, with drift from them failing CI. A profile is data,
so taking one runs nothing; projen, by contrast, writes its files from code, and
owns them whole.

## Documentation

| I want to                                   | Read                                    |
| ------------------------------------------- | --------------------------------------- |
| try devset on a repository                  | [Getting Started]                       |
| write a profile                             | [Profiles], [Designing Profiles]        |
| share profiles across repositories          | [Composing Profiles]                    |
| take a newer release, or resolve a conflict | [Updating and Merging]                  |
| check for drift in CI                       | [In CI]                                 |
| look a command up                           | [Command Reference]                     |
| use the library                             | [devset-core on docs.rs][docs.rs]       |
| know how devset is built                    | [ARCHITECTURE.md][Architecture]         |
| move across a breaking change               | [BREAKING-CHANGES.md][Breaking Changes] |
| report a vulnerability                      | [SECURITY.md][Security]                 |

## Contributing

Issues and pull requests are welcome: read [CONTRIBUTING.md][Contributing]
first. [Report a bug] or [request a feature].

## License

MIT: see [LICENSE][license].

[atxp]: https://github.com/atomix-labs/atxp
[Getting Started]: https://atomix-labs.github.io/devset/getting-started.html
[Profiles]: https://atomix-labs.github.io/devset/profiles.html
[Designing Profiles]: https://atomix-labs.github.io/devset/designing.html
[Composing Profiles]: https://atomix-labs.github.io/devset/composing.html
[Updating and Merging]: https://atomix-labs.github.io/devset/updating.html
[In CI]: https://atomix-labs.github.io/devset/ci.html
[Command Reference]: https://atomix-labs.github.io/devset/reference/devset.html
[Breaking Changes]: https://github.com/atomix-labs/devset/blob/main/BREAKING-CHANGES.md
[Architecture]: https://github.com/atomix-labs/devset/blob/main/ARCHITECTURE.md
[Contributing]: https://github.com/atomix-labs/devset/blob/main/CONTRIBUTING.md
[Security]: https://github.com/atomix-labs/devset/blob/main/SECURITY.md
[Report a bug]: https://github.com/atomix-labs/devset/issues/new?template=bug_report.md
[request a feature]: https://github.com/atomix-labs/devset/issues/new?template=feature_request.md
[docs.rs]: https://docs.rs/devset-core
[license]: https://github.com/atomix-labs/devset/blob/main/LICENSE
