---
name: writing-cargo-manifest
description: Use when creating a crate or writing, reviewing, cleaning or auditing any `Cargo.toml` in this workspace; when adding, removing, or re-pinning a dependency; when adding or reshaping a `[features]` table, or deciding whether something should be a feature, an optional dependency, or a separate crate; when splitting a proc-macro crate into its shim and impl halves; when declaring a `[[bench]]`, `[[bin]]`, `[[example]]` or `[lib]` target; when `just lint` reports a `manifest-lint.py` finding, or `just deps` reports an unused or misplaced dependency; when std leaks into a `no_std` crate. Not for the workspace root's profiles or lint wall, which change only on deliberate request.
---

# Writing Cargo Manifests

House shape for every `Cargo.toml` under this workspace. `taplo` settles layout
(alignment, and alphabetical order inside each dependency group) and
`scripts/dev/manifest-lint.py` settles the text around the values. Neither can
decide what belongs in the crate at all, which is most of this page.

A manifest here is a *declaration*, not a document: it says what the crate is,
what it may reach, and what a consumer may switch on. Everything explanatory
lives in the crate docs.

## Non-Negotiables

1. **No comments.** `# external` and `# internal` are the only ones, only inside
   a dependency table, `# external` first, each written once, bare. A reason
   worth recording goes in the crate docs' `# Crate features` table or the
   target file's `//!` header.
2. **Internal dependencies inherit.** `{ workspace = true }`, never a version,
   never a `path`. The sole exception is the macro ladder below.
3. **Every `[features]` table opens with `default`**, even when it is `[]`.
4. **`[lints] workspace = true`**, in every crate.
5. **`[package]` is `name`, `description`, then the six inherited keys** as
   `<key>.workspace = true`, in the order below.
6. **A version, a feature list, or `default-features` lives in
   `[workspace.dependencies]`** and nowhere else.

## The Shape

Tables in this order; every one but `[package]` and `[dependencies]` is written
only when it has something to say. The `cargo-machete` / `cargo-shear` pair
appears only to silence a scanner that cannot see a use, `[lib]` only to set a
non-default key.

```toml
[package]
name                   = "wtx-thing"
description            = "what the crate is, as its crate-doc pitch clause."
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
authors.workspace      = true
publish.workspace      = true

[package.metadata.cargo-machete]
ignored = ["proc-macro2"]
[package.metadata.cargo-shear]
ignored = ["proc-macro2"]

[features]
default = []

[lints]
workspace = true

[lib]
proc-macro = true

[dependencies]
# external
zerocopy = { workspace = true }
# internal
wtx-region = { workspace = true }

[dev-dependencies]
# external
trybuild = { workspace = true }
# internal
wtx-os = { workspace = true }

[target.'cfg(loom)'.dependencies]
# external
loom = { workspace = true }
```

`description` is the crate summary's pitch clause verbatim, first letter
lowercased, trailing period, never naming itself a crate — see
`writing-rustdoc`. Foundation crates (`lib/core`, `lib/memory`, `lib/sync`,
`lib/utils`, `lib/messaging`, and every `benches/`, `examples/`, `tests/` crate)
open lowercase; domain and service crates open capitalized.

Never written here: `keywords`, `categories`, `readme`, `repository`,
`homepage`, `documentation`, `build`, `crate-type`, `autobins` and friends.
Nothing is published, so the crates.io metadata earns nothing; a `build.rs` is
auto-detected and stays std-only so it needs no `[build-dependencies]`.

## Dependencies

A member may add exactly two keys to an inherited dependency, `features` and
`optional`, in that order after `workspace = true`:

```toml
wtx-time       = { workspace = true, features = ["std"] }
wtx-layouthash = { workspace = true, optional = true }
```

Everything else — the version, `default-features`, the baseline feature set —
belongs to the workspace entry, so one edit moves every crate. Two entries under
one `# external` header are alphabetized by `taplo`; the `=` column resets at
each group marker.

**The macro ladder is the one path-dep exception.** A proc-macro crate is always
a pair under the parent's `macros/`: a `proc-macro = true` shim, and a plain lib
holding the token logic so it can be unit-tested. The shim reaches its impl
half, and the parent reaches the shim, by relative path, because neither is in
`[workspace.dependencies]`:

```toml
# lib/utils/wtx-assert/Cargo.toml
wtx-assert-macro = { path = "macros/macro" }
# lib/utils/wtx-assert/macros/macro/Cargo.toml
wtx-assert-macro-impl = { path = "../macro-impl" }
```

A shim that is *optional* cannot do this — an optional dep must be resolvable by
name — so it goes in `[workspace.dependencies]` like any other internal crate
and is inherited. `wtx-atomics-derive` is the only one.

The rule the gate checks is the mechanical form of both: **a crate named in
`[workspace.dependencies]` is always inherited; a `path` is allowed only to a
directory inside the crate's own tree.**

### Defaults and the Std Leak

Defaults are the main way `std` reaches a `no_std` crate, and features unify as
a *union* across everything one cargo invocation builds. Measured in this
workspace: `wtx-tags` inherits `serde` with `default-features = false` and gets
no `std`; `wtx-instrument-db` inherits it without, and gets `default` and `std`,
and under `cargo build --workspace` that union reaches both.

So: put `default-features = false` on the **workspace** entry of any external
crate a `no_std` member touches, rather than on each member. A member override
does work on this toolchain (upstream documents it as ignored; it is not), but
it only protects the member that remembers to write it.

## Features

### What Earns One

Start from what the crate is *for* and put that in the crate with no feature at
all. A feature is then justified by one of exactly three things: **a dependency
not everyone should pay for**, **a capability the target may not have** (`std`,
a syscall surface), or **a hook only tests need** (`wtx-init`'s `testing`).

It is not justified by taste, by a second way of doing the crate's one job, or
by "someone might not want this". If the switch changes *what the crate is*, it
is a second crate — Cargo's own first advice for features that fight each other
is to split the package, and splitting costs nothing here where every crate is a
path dep.

Each feature is permanent (removing one is a breaking change), is another edge
any sibling crate can flip on for everyone, and multiplies `just
nightly-cargo-hack`, which runs `cargo hack --each-feature` over the workspace.
Three features that each buy something beat eight that hedge.

### Additive, Always

Enabling a feature only ever *adds*. Never a `no-std`, `no-alloc`, or
`disable-x` feature: union semantics mean an unrelated crate switching it on
subtracts functionality from yours, and you cannot opt out. Features that cannot
both be on are a defect, not a design — split the crate. Name the thing, not the
switch: `std`, not `use-std` or `with-std`.

### Std Is the Opt-In

Over half the lib crates are `#![no_std]`, and every one of them writes it
unconditionally rather than `cfg_attr`-ing it on: with the bare `core` prelude
always in force, a std use site has to be `cfg`-gated deliberately instead of
compiling by accident the day something unifies `std` on. `extern crate alloc;`
follows immediately where the crate allocates.

`std` is therefore an opt-in feature and is **never** in `default`. When a
dependency has its own, forward explicitly — `std = ["wtx-time/std"]`, or
`dep?/std` when that dependency is optional. Std-only by nature, and so carrying
no `std` feature at all: `lib/pal` (it owns the syscalls), `lib/net`,
`lib/service`, `lib/strategy`, `wtx-microbench`, and every `macros/` crate.

### `default`

Always present, first key, and `[]` unless the crate is genuinely unusable
without something — `wtx-time` defaults to `calendar`, `wtx-atomics` to
`derive`. Keep it small: a consumer can only escape a default by writing
`default-features = false`, and every crate in the graph must remember to.

### Writing One

```toml
[features]
default = []

ecs   = ["dep:bevy_ecs"]
serde = ["dep:serde", "wtx-primitives/serde", "wtx-tags/serde", "wtx-time/serde"]
std   = ["dep:rustix"]
```

`dep:` on every optional dependency, always. Without it Cargo invents a public
feature named after the crate, which makes swapping that crate a breaking
change. Forwarded dependency features follow the `dep:` entries.

A feature that is not self-explanatory is explained once, in the crate docs' `#
Crate features` table — never in the manifest.

## Targets

Declared only for a non-default key; `src/lib.rs`, `src/main.rs`, `benches/*.rs`
and `tests/*.rs` are found without help.

| written                                    | when                                                                                          |
| ------------------------------------------ | --------------------------------------------------------------------------------------------- |
| `[lib] proc-macro = true`                  | the shim half of a macro pair                                                                 |
| `[lib] bench = false`                      | the crate has standalone measurement `main`s                                                  |
| `[[bench]] harness = false`                | every bench here: they pin cores and own their loop, so libtest's harness would eat the flags |
| `[[bin]] name`, `path`                     | a second binary, or a cross-process test's child                                              |
| `[[example]]`/`[[test]] required-features` | the target needs a feature the crate does not default to                                      |

## Gates

| command                                                                       | decides                                                                                                                                |
| ----------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `just fix-taplo` / `just check-taplo`                                         | `taplo` over every `Cargo.toml`: alignment, and alphabetical order within each group                                                   |
| `just check-clippy`, `just check-manifest-lint`                               | clippy over every target and feature, then `.just/manifest-lint.py`: the shape rules above                                             |
| `just check-cargo-deny`, `just check-cargo-machete`, `just check-cargo-shear` | `cargo deny` (advisories, licenses, bans, sources) + `cargo machete` + `cargo shear`: **unused and misplaced dependencies**            |
| `just nightly-cargo-hack`                                                     | `cargo hack --each-feature --workspace clippy`: every feature still compiles alone. Minutes, so nightly on main, not per-PR            |
| `just check`                                                                  | the whole PR gate, every `check-*` recipe; `just nightly` adds the slow ones: the feature sweep, and the repository's own (loom, Miri) |

`python3 .just/manifest-lint.py [<path>...]` runs on one crate for the edit
loop. To answer *why* a feature is on, `cargo tree -e features -i <crate>` names
who turned it on; add `-p <crate>` to ask within one member rather than the
union.

`cargo machete` is fast and imprecise, `cargo shear` also catches a dependency
in the wrong section — they disagree, which is why `just check` runs both. When
one is wrong because a dep is reached only through a macro expansion, silence it
with the `[package.metadata.cargo-machete]` / `cargo-shear` pair, both of them,
and nothing else.

## Red Flags

| thought                                                                | reality                                                                            |
| ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| "This dependency's purpose isn't obvious, a short `#` gloss will help" | No comments. If it needs saying, the crate docs say it.                            |
| "A paragraph above `[[bench]]` explains why it isn't criterion"        | Same rule. That reason belongs in the bench's `//!` header.                        |
| "The crate has no features worth defaulting, so I'll omit `default`"   | `default = []` is the declaration that there are none. Write it.                   |
| "I'll add `default-features = false` in my crate to keep std out"      | It protects only your crate. Put it on the workspace entry.                        |
| "`no-std` as a feature is clearer for this crate"                      | Features only add. A subtractive feature breaks every consumer that never asked.   |
| "Two features that conflict, I'll add a `compile_error!`"              | That is the fallback after splitting the crate is rejected, not the design.        |
| "It's an optional dep, the implicit feature is fine"                   | `dep:` always: the implicit one puts the crate's name in your public API.          |
| "This new switch is small, one more feature won't hurt"                | It is permanent, it unifies across the graph, and it multiplies the feature sweep. |
| "I'll pin the version here, it's only used by this crate"              | Until it is used by two. Versions live in `[workspace.dependencies]`.              |
| "`taplo fmt` passed, the manifest is clean"                            | `taplo` never sees a comment. Run `just check-manifest-lint`.                      |

## References

`references/sources.md` — the external canon behind each rule, what it says, and
the two places measurement here disagrees with it.
