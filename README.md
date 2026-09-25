# devset

[![CI][ci badge]][ci] [![Release][release badge]][releases]
[![License][license badge]][license]

[Manual] · [Changelog] · [Breaking Changes] · [Architecture] · [Contributing] ·
[Report a bug] · [Request a feature]

devset applies a versioned bundle of files to a directory, records exactly what
it wrote, and updates it later without destroying local edits.

Keep formatter settings, lint rules, dependency policy, editor settings and CI
workflows in one place, shared by every repository, while each keeps the local
content it needs. [atxp] is a collection of such bundles for Rust repositories.

## Install

A static binary for Linux, on x86-64 and Arm, and one for macOS on Apple
silicon, with every [release][releases]:

```sh
mise use -g github:atomix-labs/devset
```

[Getting Started][getting started] has the other ways, from an archive or from
source.

## Quick Start

A profile is a directory: `profile.toml`, which says how devset manages each
file, and `files/`, which holds them as a repository should have them.

```toml
# ../profiles/base/profile.toml
[profile]
name = "base"

[files.".editorconfig"]      # owned: the profile's, so an edit is drift

[files."deny.toml"]
policy = "merge"             # the repository's edits are kept, and merged
```

In a repository, apply it, and later take its changes:

```console
$ devset init --path ../profiles/base
     Created .editorconfig
     Created deny.toml
    Finished 2 changes
```

```sh
devset status --exit-code     # in CI: fails when a file has drifted
devset update                 # later: takes the profile's changes, merging your edits
```

## What It Does

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
- **Composes.** Profiles layer, and require one another, so a team builds on a
  shared collection instead of copying it.
- **Runs nothing.** A profile is data: devset never runs a program a profile
  names.

## Documentation

- [The manual][Manual]: getting started, profiles, parts, templates, composing,
  updating and merging, settings, state, CI, and every command.
- [ARCHITECTURE.md][Architecture]: how devset is built, from the crates to the
  chain every command runs.
- [CONTRIBUTING.md][Contributing]: how to change devset, the commit convention,
  and the checks.
- [RELEASE.md][Release]: how a release is cut, and what it ships.
- [CHANGELOG.md][Changelog]: what changed in each release, written by git-cliff
  from the commits.
- [BREAKING-CHANGES.md][Breaking Changes]: how to move across a breaking change.
- [SECURITY.md][Security]: how to report a vulnerability.

## Contributing

Issues and pull requests are welcome: read [CONTRIBUTING.md][Contributing]
first.

## License

MIT: see [LICENSE][license].

[atxp]: https://github.com/atomix-labs/atxp
[Manual]: https://atomix-labs.github.io/devset/
[getting started]: https://atomix-labs.github.io/devset/getting-started.html
[parts]: https://atomix-labs.github.io/devset/parts.html
[Changelog]: CHANGELOG.md
[Breaking Changes]: BREAKING-CHANGES.md
[Architecture]: ARCHITECTURE.md
[Contributing]: CONTRIBUTING.md
[Release]: RELEASE.md
[Security]: SECURITY.md
[Report a bug]: https://github.com/atomix-labs/devset/issues/new?template=bug_report.md
[Request a feature]: https://github.com/atomix-labs/devset/issues/new?template=feature_request.md
[ci]: https://github.com/atomix-labs/devset/actions/workflows/check.yml
[ci badge]: https://img.shields.io/github/actions/workflow/status/atomix-labs/devset/check.yml?branch=main&style=flat-square&logo=github&label=check
[releases]: https://github.com/atomix-labs/devset/releases
[release badge]: https://img.shields.io/github/v/release/atomix-labs/devset?style=flat-square&sort=semver
[license]: LICENSE
[license badge]: https://img.shields.io/github/license/atomix-labs/devset?style=flat-square
