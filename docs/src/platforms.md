# Platforms and Rust Versions

Where devset's releases run, where it builds from source, and which Rust it
builds with.

## Releases

Each release attaches an archive for each platform below, built on that platform
by the release workflow, with its `.sha256` and its build attestation.

| Platform               | Archive's target             | Notes                            |
| ---------------------- | ---------------------------- | -------------------------------- |
| Linux on x86-64        | `x86_64-unknown-linux-musl`  | static: runs on any distribution |
| Linux on Arm64         | `aarch64-unknown-linux-musl` | static: runs on any distribution |
| macOS on Apple silicon | `aarch64-apple-darwin`       |                                  |

These are the platforms the installer, mise and cargo-binstall take a release
for. CI runs every check on Linux; the macOS archive is built, and attested, on
macOS.

## From Source

Elsewhere, `cargo install --locked devset-cli` builds devset where Rust and git
run, such as macOS on Intel. Windows is not tested yet: a report of what works
there is welcome, as an issue.

## Rust Versions

Every crate declares its minimum Rust as `rust-version`, now 1.98, and `just
check-rust-msrv` builds every crate on that version, with every target and
feature, on every change. The minimum rises only when a newer Rust brings
something devset uses, in a minor release whose changelog says so; a patch
release never raises it. Developing devset takes the nightly its
`rust-toolchain.toml` pins, which `./setup.sh` installs.

## What devset Runs

A source in a git repository needs `git` on your `PATH`; a local one needs
nothing else. devset runs no other program, and none a profile names.
