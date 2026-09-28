# A New Rust Project

A collection's bundle sets a whole repository up at once. This page takes
[atxp](https://github.com/atomix-labs/atxp)'s `rust` bundle, a collection of
profiles provided by Atomix Labs: a Cargo workspace with its first crate, the
pinned toolchain, formatting and lints, dependency policy, tests, the changelog,
CI and the weekly bump. Any other collection's bundle is taken the same way.

## Apply the Bundle

In an empty directory, or a new repository's top level:

```sh
mkdir hello && cd hello && git init
devset add atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.13.2 --var repository=you/hello
```

`add` starts the target, names the source `atxp`, pins its tag, and applies the
bundle and every profile it requires, each a layer of its own. Each variable the
profiles declare takes its default, and devset names them all in one note;
`--var name=value` answers one otherwise, now or later. A variable with no
default, as `repository` is, is asked for in a terminal, and with `--no-input`,
the form for a script, it is an error naming the flag to pass.

## See What It Wrote

```sh
devset status            # every file, and whether it matches its profile
devset explain rust      # the bundle's features, and which are off
devset explain Cargo.toml
```

`explain` of a file names each layer with a part of it, the part and the policy.
What the repository lacked, a profile wrote once, as a scaffold: the workspace
and its first crate, the changelog, the justfile's start. Those are the
repository's from now on; the rest stays in step with the profiles.

## Set up and Check

The profiles pin their tools with mise and hang their checks from `just`.
`setup.sh`, which the bundle writes, installs mise at its pinned version,
checked against the release's checksum, then every tool the lock pins, then runs
`just setup`; run again, it keeps what is in place:

```sh
./setup.sh
just check
```

`just check` runs every check the profiles define, as CI runs them, and names
each that fails. `just --list` shows every recipe: `fix`, `test`, `bump` and the
rest.

## Turn on More

The bundle's features add what a project may want, none on by default:

```sh
devset add atxp/rust --features docs,publish
```

`devset explain rust` lists them. A feature turned off later takes its files
with it, unless the repository has edited them. And any profile of the
collection is a layer of its own: `devset add atxp/vhs` records a demo for the
README.

## Commit

Commit everything, `.devset/` included: it records what devset wrote, so a
teammate, or CI, sees the same state. From here, [In CI](ci.md) makes drift fail
a build, and [Update and Resolve Conflicts](resolving.md) takes the collection's
next release.
