---
name: editing-cargo-manifests
description: Use when creating a crate, or writing, reviewing or cleaning any `Cargo.toml` in the workspace; when adding, removing or re-pinning a dependency; when adding or reshaping a `[features]` table, or deciding between a feature, an optional dependency and a separate crate; when declaring a `[[bench]]`, `[[bin]]`, `[[example]]` or `[lib]` target; when the manifest check or the unused-dependency check reports a finding.
---

# Editing Cargo Manifests

A manifest is a declaration: what the crate is, what it may reach, and what a
consumer may switch on. Explanation lives in the crate's docs. taplo settles the
layout, and `.just/cargo-manifest.py` the text around the values; neither
decides what belongs in the crate, which is most of this skill.

`references/sources.md` holds the Cargo documentation behind each rule.

## Rules

1. **Every version lives in `[workspace.dependencies]`**, external and internal
   alike, with its `default-features` and its baseline features. A member writes
   `name = { workspace = true }`, adding at most `features` and `optional`, in
   that order. One edit then moves every crate.
2. **An internal crate is listed in `[workspace.dependencies]` with its path and
   version**, under `# internal`, and every member inherits it; a member never
   writes a `path`.
3. **No comments but the group markers**: `# external`, then `# internal`, each
   once, inside a dependency table. A reason worth recording goes in the crate's
   docs.
4. **`[package]` reads `name`, `description`, then the inherited keys** as
   `<key>.workspace = true`: `version`, `edition`, `rust-version`, `license`,
   `authors`, then `publish` or `repository` where the workspace sets them.
5. **Every crate has `[lints] workspace = true`**, so no crate leaves the shared
   lints without anyone deciding it should.
6. **Every `[features]` table opens with `default`**, even when it is `[]`.

## Features

A feature is justified by exactly one of three things: a dependency not every
user should pay for, a capability the target may not have (`std`, a syscall), or
a hook only tests need. A second way of doing the crate's one job is not a
feature; a switch that changes what the crate is makes a second crate.

- **Features only add.** Cargo unifies them across the whole build, so a
  `no-std` or `disable-x` feature that one crate turns on takes something away
  from every other. Name the thing, `std`, not the switch, `use-std`.
- **`dep:` on every optional dependency**, `serde = ["dep:serde"]`; without it
  Cargo makes the dependency's name a public feature.
- **Forward a dependency's feature explicitly**, `std = ["other/std"]`, or
  `other?/std` where `other` is optional.
- **Defaults stay small**, since turning them off is a consumer's only way out.
  An external crate a `no_std` member uses gets `default-features = false` on
  its workspace entry, not on each member.
- **Each feature is permanent**: removing one breaks every user who names it.

## Targets

`src/lib.rs`, `src/main.rs`, `tests/*.rs`, `benches/*.rs` and `examples/*.rs`
need no table. Declare one only for a key that is not the default: a proc-macro
`[lib]`, a bench with `harness = false`, a second `[[bin]]`, or
`required-features` on a target that needs a feature the crate does not default
to.

## Steps

1. **Add the crate**: `crates/<name>/Cargo.toml` in the shape below, then add it
   to git: the manifest check reads only the manifests git tracks.
2. **Add a dependency**: its entry in `[workspace.dependencies]`, under its
   group, then `{ workspace = true }` in the member.
3. **Format**: `just fix`, which puts each dependency under its group, then
   aligns and orders each group.
4. **Check**, below.

```toml
[package]
name                   = "tiles-core"
description            = "the grid a board is laid out on, and the moves across it."
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true

[features]
default = []

[lints]
workspace = true

[dependencies]
# external
serde = { workspace = true, features = ["derive"] }
# internal
tiles-geometry = { workspace = true }
```

## Checks

- `just check-cargo-manifest`: the shape above, and every crate on the
  workspace's lints.
- `just check-toml`: taplo's layout, each dependency group in alphabetical
  order.
- `just check-cargo-unused`: a dependency nothing uses, or one in the wrong
  table. Where one is reached only through a macro, silence both scanners with
  `[package.metadata.cargo-machete]` and `[package.metadata.cargo-shear]`.
- `just check-cargo-deny`: advisories, licences, bans and sources.
- `just nightly-cargo-hack`: every feature builds alone.
- `just check`: all of them. `cargo tree -e features -i <crate>` says who turned
  a feature on.

## What Not to Do

- Do not pin a version in a member, even for a crate only one member uses: it
  will not stay the only one.
- Do not write a `path` in a member, or a comment to explain a dependency.
- Do not add a subtractive feature, or two features that cannot both be on.
- Do not remove an unused workspace entry that a check flags without asking
  whether a crate is about to use it.
- Do not trust `taplo` alone: it never sees a comment. Run the manifest check.
