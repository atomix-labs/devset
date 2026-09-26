# devset

[![CI][ci badge]][ci] [![Release][release badge]][releases]
[![crates.io][crates badge]][crates] [![License][license badge]][license]

[Manual] · [Changelog] · [Breaking Changes] · [Architecture] · [Contributing] ·
[Report a bug] · [Request a feature]

devset applies versioned bundles of files to a directory, records exactly what
it wrote, and updates them later without destroying local edits.

Keep formatter settings, lint rules, dependency policy, editor settings, CI
workflows and documentation scaffolds in one place, shared by every repository,
while each keeps the local content it needs. Profiles compose as crates do, with
requirements and features, and do real work: they scaffold what a project lacks,
wire up what it has, and keep their parts in sync. [atxp] is a collection of
them for Rust repositories.

## Install

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh
```

A static binary for Linux, on x86-64 and Arm, and one for macOS on Apple
silicon, checked against its release's checksum and build attestation. The same
binaries, or a build from [crates.io][crates]:

```sh
mise use -g github:atomix-labs/devset   # the release's binary
cargo binstall devset-cli               # the same binary, through cargo-binstall
cargo install --locked devset-cli       # built from crates.io, with Rust 1.98 or later
```

The package is `devset-cli`; the binary is `devset`.
[Getting Started][getting started] has the rest.

## Quick Start

A target names its sources once, and applies profiles from them by name, with
the features it wants:

```sh
devset new hello atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.4.0 --features docs
cd hello
devset add atxp/mdbook --features katex
```

A profile is a directory in a source: `profile.toml`, which says how devset
manages each file, and when, and `files/`, which holds them as a repository
should have them.

```toml
[profile]
name = "base"

[features]
default = ["deny"]
deny    = []

[files.".editorconfig"]      # owned: the profile's, so an edit is drift

[files."deny.toml"]
policy = "merge"             # the repository's edits are kept, and merged
when   = { features = ["deny"] }
```

Then, in CI and later:

```sh
devset status --exit-code     # in CI: fails when a file has drifted
devset update                 # later: takes the sources' changes, merging your edits
```

## What It Does

- **Composes like crates.** Sources are named once; profiles require others by
  name, and offer features that unify across every requirer, as Cargo's do.
- **Does real work.** A scaffold writes starter files once, where the project
  has none of its own; a gate applies an entry only when a feature is on, a
  profile is there, or a file will exist; a starter and every profile's part of
  a file are written in one run.
- **Tracks what it wrote.** `.devset/` records every file's bytes, so devset
  tells a local edit from a profile's change, and a reformatted file from an
  edited one.
- **Keeps what is local.** Each file has a policy: the profile's, merged with
  the repository's edits, or written once and left alone. A profile may own only
  [part of a file][parts]: some keys of a `Cargo.toml`, or a block of a
  `.gitignore`.
- **Merges safely.** An update merges three ways against the bytes devset wrote,
  checks that a merged TOML, JSON or YAML file still parses with no duplicate
  key, and leaves a conflict beside the file, never in it.
- **Runs nothing.** A profile is data: devset never runs a program a profile
  names.

## Documentation

- [The manual][Manual]: getting started, profiles, parts, templates, gates and
  scaffolds, composing and designing profiles, updating and merging, settings,
  state, CI, and every command.
- [ARCHITECTURE.md][Architecture]: how devset is built, from the crates to the
  chain every command runs.
- [CONTRIBUTING.md][Contributing]: how to change devset, the commit convention,
  and the checks.
- [RELEASE.md][Release]: how a release is cut, and what it ships.
- [CHANGELOG.md][Changelog]: what changed in each release, written by git-cliff
  from the commits.
- [BREAKING-CHANGES.md][Breaking Changes]: how to move across a breaking change.
- [SECURITY.md][Security]: how to report a vulnerability.
- [devset-core's API][docs.rs]: the library behind the command line, on docs.rs.

## Contributing

Issues and pull requests are welcome: read [CONTRIBUTING.md][Contributing]
first.

## License

MIT: see [LICENSE][license].

[atxp]: https://github.com/atomix-labs/atxp
[Manual]: https://atomix-labs.github.io/devset/
[getting started]: https://atomix-labs.github.io/devset/getting-started.html
[parts]: https://atomix-labs.github.io/devset/parts.html
[Changelog]: https://github.com/atomix-labs/devset/blob/main/CHANGELOG.md
[Breaking Changes]: https://github.com/atomix-labs/devset/blob/main/BREAKING-CHANGES.md
[Architecture]: https://github.com/atomix-labs/devset/blob/main/ARCHITECTURE.md
[Contributing]: https://github.com/atomix-labs/devset/blob/main/CONTRIBUTING.md
[Release]: https://github.com/atomix-labs/devset/blob/main/RELEASE.md
[Security]: https://github.com/atomix-labs/devset/blob/main/SECURITY.md
[Report a bug]: https://github.com/atomix-labs/devset/issues/new?template=bug_report.md
[Request a feature]: https://github.com/atomix-labs/devset/issues/new?template=feature_request.md
[ci]: https://github.com/atomix-labs/devset/actions/workflows/check.yml
[ci badge]: https://img.shields.io/github/actions/workflow/status/atomix-labs/devset/check.yml?branch=main&style=flat-square&logo=github&label=check
[releases]: https://github.com/atomix-labs/devset/releases
[release badge]: https://img.shields.io/github/v/release/atomix-labs/devset?style=flat-square&sort=semver
[crates]: https://crates.io/crates/devset-cli
[crates badge]: https://img.shields.io/crates/v/devset-cli?style=flat-square
[docs.rs]: https://docs.rs/devset-core
[license]: https://github.com/atomix-labs/devset/blob/main/LICENSE
[license badge]: https://img.shields.io/github/license/atomix-labs/devset?style=flat-square
