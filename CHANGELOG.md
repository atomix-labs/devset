# Changelog

Every release, newest first, written by [git-cliff](https://git-cliff.org) from the commits.
[BREAKING-CHANGES.md](BREAKING-CHANGES.md) says how to move across a breaking change.

## [0.2.0](https://github.com/atomix-labs/devset/releases/tag/v0.2.0) - 2026-09-26

### Features

- [9a70f9c](https://github.com/atomix-labs/devset/commit/9a70f9c328ffc3e6363ddacd8f67cc0b6170ce66) *(release)* Serve a one-line installer with the manual, and attach it to releases
- [2bc2eee](https://github.com/atomix-labs/devset/commit/2bc2eee75972e46e501e3b7e2ed044484b368bf7) Compose profiles as crates **breaking**

### Documentation

- [0f235c2](https://github.com/atomix-labs/devset/commit/0f235c214ebe89c12456f80039382fa856cecbd2) Write the repository's documents for profiles as crates
- [20e6ef4](https://github.com/atomix-labs/devset/commit/20e6ef40465dedccf98596d8131e0115771c47ad) Cover sources, features, gates and scaffolds in the manual

### Miscellaneous

- [08618eb](https://github.com/atomix-labs/devset/commit/08618ebf54cc345ad5f9f2a9552682fa760ff2b6) Take atxp v0.3.3, attesting release archives

**Full Changelog**: <https://github.com/atomix-labs/devset/compare/v0.1.3...v0.2.0>

## [0.1.3](https://github.com/atomix-labs/devset/releases/tag/v0.1.3) - 2026-09-26

### Documentation

- [523df14](https://github.com/atomix-labs/devset/commit/523df14b1ce4cbbdccd9c9f229cd77f10f15cc2a) Link the README's documents in full, as crates.io resolves links from the crate

### Miscellaneous

- [13f52ca](https://github.com/atomix-labs/devset/commit/13f52cac28aac10867de3f94b7960d96a556f844) *(devset-cli)* Drop the homepage its documentation link repeats
- [7502786](https://github.com/atomix-labs/devset/commit/750278600426903db5634552a4cdc55562254c2f) Take atxp v0.3.2

**Full Changelog**: <https://github.com/atomix-labs/devset/compare/v0.1.2...v0.1.3>

## [0.1.2](https://github.com/atomix-labs/devset/releases/tag/v0.1.2) - 2026-09-25

### Features

- [e25315a](https://github.com/atomix-labs/devset/commit/e25315a2c39de3603b3fe3fc6265a03fa0764141) *(devset-cli)* Let cargo-binstall install the release's binary

### Documentation

- [79fed9f](https://github.com/atomix-labs/devset/commit/79fed9f7218145c76dbd5715f1aafb04868bbc79) Install from crates.io, and say how a release publishes
- [2ebd5b9](https://github.com/atomix-labs/devset/commit/2ebd5b934764fa78b7bd0f41336b2828e6b84a34) *(release)* Run the checks before tagging
- [e2fb5d4](https://github.com/atomix-labs/devset/commit/e2fb5d4e1916b86b6b34f16fd48595c398b722e0) *(release)* Keep the manual's pinned install at the latest release

### Miscellaneous

- [c3e6add](https://github.com/atomix-labs/devset/commit/c3e6addde05456af51501b70d7499284c6c50777) Give each crate its licence text, homepage and categories
- [6c77236](https://github.com/atomix-labs/devset/commit/6c772362c935e6795ff572dd25773743de133e71) Take atxp v0.3.1, which publishes the crates to crates.io
- [e24616b](https://github.com/atomix-labs/devset/commit/e24616b39277476447d03fcd762283b57e7f37f1) Take atxp v0.2.2: one mise cache for CI, a longer download timeout
- [2dd821a](https://github.com/atomix-labs/devset/commit/2dd821a5bf61407361c05ed2a94fef83be84470f) *(devset-cli)* Match any devset version in the requirement refusal

**Full Changelog**: <https://github.com/atomix-labs/devset/compare/v0.1.1...v0.1.2>

## [0.1.1](https://github.com/atomix-labs/devset/releases/tag/v0.1.1) - 2026-09-25

### Bug Fixes

- [c74d078](https://github.com/atomix-labs/devset/commit/c74d07862606f27af3febfab1cbb6ceacbe6af4b) *(devset-cli)* Name the binary devset in --version

**Full Changelog**: <https://github.com/atomix-labs/devset/compare/v0.1.0...v0.1.1>

## [0.1.0](https://github.com/atomix-labs/devset/releases/tag/v0.1.0) - 2026-09-25

### Features

- [f1c1430](https://github.com/atomix-labs/devset/commit/f1c1430439e4ad607b415c8eb55870ec013d1bdf) *(devset-cli)* Name the manual in --help
- [01bbdf9](https://github.com/atomix-labs/devset/commit/01bbdf9c4c06060a6a2857bbdf52d476ef8dac53) Build on stable Rust

### Bug Fixes

- [c182bdf](https://github.com/atomix-labs/devset/commit/c182bdf0f9687de579185961c3a889338d1170f1) *(devset-cli)* Say what remove does with a layer's files
- [3ff590d](https://github.com/atomix-labs/devset/commit/3ff590db9f57ffdd4a7c270f43e4981646ad1686) *(devset-cli)* Wrap the requires line at whole names
- [ffa4cdb](https://github.com/atomix-labs/devset/commit/ffa4cdb65dc9681f2c5638a553d5afbef49a77c4) *(devset-core)* Write YAML values as the payload writes them
- [056f968](https://github.com/atomix-labs/devset/commit/056f968a3a2f42aff91271945bcc6f313cff01e2) *(ci)* Install the toolchain before mise builds any tool
- [6db3b23](https://github.com/atomix-labs/devset/commit/6db3b23c21dace0538838531e32eca68ad2830ab) *(devset-core)* Write the keys a profile adds in the payload's layout

### Refactor

- [2f3f3cf](https://github.com/atomix-labs/devset/commit/2f3f3cf10d638f386b1f7c97e31fba717d598496) *(devset-cli)* Split the report by what it reports
- [0bd3272](https://github.com/atomix-labs/devset/commit/0bd3272b60e839be38285a7a2fcc2d7424e382b6) *(devset-cli)* Move the command line into cli.rs
- [30f7cc2](https://github.com/atomix-labs/devset/commit/30f7cc284ed9f070f9084682700d7bef4b7b6986) Name the CLI crate devset-cli
- [b04cac3](https://github.com/atomix-labs/devset/commit/b04cac3a8b1ac71ee5546f8d1ace8e8b1d7b743a) *(devset-core)* Write files atomically through camino-tempfile

### Documentation

- [535af61](https://github.com/atomix-labs/devset/commit/535af61b31a3290bf9684443db02e7ec6584218b) Give each reader a document of their own
- [5318363](https://github.com/atomix-labs/devset/commit/5318363780313643f84f32c564a92b0e93f1f37b) Write the manual, its reference generated from the build
- [458c63f](https://github.com/atomix-labs/devset/commit/458c63fbd7b1c3798d772d432247c2daf2af6da8) *(devset-core)* Give the library its own README

### Miscellaneous

- [303a630](https://github.com/atomix-labs/devset/commit/303a63072bd714af6b58551a1bea6169248e8960) Take atxp v0.2.1, with mdbook and lychee
- [cda0d29](https://github.com/atomix-labs/devset/commit/cda0d29be1b7b3fb6f71876d17abc21f98d00cc1) *(devset-cli)* Split the end-to-end tests by area
- [4c9b6ab](https://github.com/atomix-labs/devset/commit/4c9b6ab0ded5f3019b1fd10ddf5e9ddfd1089ece) Take atxp v0.2.0, with msrv and cargo-binaries
- [cd3af67](https://github.com/atomix-labs/devset/commit/cd3af67f0a1fbc5dfd83c85331ec97e7de661dd0) Take atxp from its v0.1.0 tag
- [81910b8](https://github.com/atomix-labs/devset/commit/81910b8ec3927a8745af89113361e3bfc80d2082) Format with atxp's rustfmt, taplo and dprint
- [11b2829](https://github.com/atomix-labs/devset/commit/11b2829bae84f2603e381a720ca97c4e06ce522f) Take atxp's rust bundle
