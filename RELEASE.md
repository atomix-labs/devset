# Releasing

How a release of devset is cut: what its version says, what it ships, and the
steps from the default branch to a published release.
[CHANGELOG.md](CHANGELOG.md) is its record.

## Versions

A tag versions both crates at once, `vX.Y.Z` under semantic versioning. While
the major version is 0, a minor release may break a user who takes it, and a
patch release never does. [CONTRIBUTING.md](CONTRIBUTING.md) says what counts as
breaking.

## What a Release Ships

A GitHub Release, with the release's section of the changelog as its notes and
the `devset` binary for each platform, archived as
`devset-<version>-<target>.tar.xz` with a `.sha256` beside it:

| Target                       | Built on           | Linked      |
| ---------------------------- | ------------------ | ----------- |
| `x86_64-unknown-linux-musl`  | `ubuntu-latest`    | statically  |
| `aarch64-unknown-linux-musl` | `ubuntu-24.04-arm` | statically  |
| `aarch64-apple-darwin`       | `macos-latest`     | dynamically |

Each is built for the CPU floor `.cargo/config.toml` sets, which every realistic
machine of its architecture meets: x86-64-v2, CRC32 on Arm, and the first Apple
silicon. devset is not published to crates.io yet.

## Steps

1. Check that every breaking commit since the last tag has its entry in
   [BREAKING-CHANGES.md](BREAKING-CHANGES.md), under the version about to be
   tagged.
2. Write what the release changes:

   ```sh
   RELEASE_VERSION=x.y.z just release
   ```

   Every `release-*` recipe runs: `release-git-cliff` writes `CHANGELOG.md` from
   the commits since the last tag, and `release-cargo-bump` sets both crates'
   version, and the lock's.
3. Read the new section; fix a commit's subject by rewording the commit, not the
   file.
4. Commit it as `chore(release): vx.y.z`, which the changelog leaves out; sign
   the tag, `git tag -s vx.y.z`; push the branch and the tag.
5. The tag starts `release.yml`: `just package` builds the archives on each
   platform, then the GitHub Release is published with the notes and every
   archive attached.
6. Download an archive, check it against its `.sha256`, and run `devset
   --version`.
