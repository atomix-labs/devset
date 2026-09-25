# Tooling: Gates, Lints, Rustdoc Mechanics

Contents: 1 Commands · 2 Workspace lints that shape docs · 3 Features and cfg
axes · 4 Links rustdoc cannot resolve · 5 Rustdoc and doctest attributes · 6
Rendered check

## 1 Commands

From the workspace root. `scripts/doc-audit.sh <crate-dir>` runs the whole
ladder and then the heuristic lint; the raw steps, in order, each clean before
the next:

```sh
just fix-rustfmt fix-dprint fix-taplo fix-rumdl            # rewraps, never tightens
host=$(rustc -vV | sed -n 's/^host: //p')                  # every `just` recipe builds under --target, so share its cache
RUSTDOCFLAGS="-D warnings" cargo doc -p <crate> --no-deps --target "$host" [--all-features] --document-private-items
cargo test -p <crate> --doc --target "$host" [--all-features]
cargo clippy -p <crate> --all-targets --target "$host"
python3 scripts/doc-lint.py <crate-dir>                    # the cut list, mechanically
```

`--all-features` where the manifest has a `[features]` table. The rustdoc lints
below are `deny` through `[workspace.lints]`; `-D warnings` holds the rest to
the same bar, as `just check-rustdoc` does.

The build documents private items, as `just check-rustdoc` does: it is where a
private doc's ``[`SLOTS`]`` link is checked, and it surfaces link hygiene a
public build never sees (a private `mod init` making ``[`init!`](crate::init)``
ambiguous, or an explicit target on a private field's link being redundant).

`just fix-rustfmt fix-taplo` format the whole tree; when only one crate changed,
`cargo fmt -p <crate>` and `taplo fmt <crate>/Cargo.toml` are the same two
formatters scoped down. What `rustfmt` does to docs (`rustfmt.toml`:
`wrap_comments`, `comment_width = 100`, `format_code_in_doc_comments`): it wraps
a line over 100 columns and pushes the tail down, which is where widows come
from; it never joins a short line to the next, so a paragraph rebalanced by hand
stays as written while every line is under 100; and it reformats the Rust inside
a doc fence, so write examples as `rustfmt` would.

For a crate named in the justfile's `loom_pkgs` or `miri_pkgs`, its docs must
also compile on that axis, since a `cfg`-gated item a doc links may vanish
there:

```sh
cargo clippy -p <crate> --lib --tests --config 'target."cfg(all())".rustflags=["--cfg","loom"]'
```

Snapshots a doc change can move: `TRYBUILD=overwrite cargo test -p <crate>
--test trybuild` regenerates `tests/compile_fail/*.stderr`; never hand-edit one.

`just check-clippy` and `just check-rustdoc`, and `just check` over everything,
are the whole-tree gates; run them before a PR, not per edit.

## 2 Workspace Lints That Shape Docs

All set to `deny` in the root `Cargo.toml`; every crate inherits them with
`[lints] workspace = true`. Do not add lint attributes to a crate to satisfy
this skill.

| lint                                                                                  | fires on                                                                                                                                                | so                                                                                                        |
| ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `missing_docs`                                                                        | an undocumented `pub` item                                                                                                                              | every public item has a summary line                                                                      |
| `clippy::missing_docs_in_private_items` (restriction)                                 | an undocumented private item, field, const, or test helper                                                                                              | private items too; make the line carry information, or link (`See [`SLOTS`].`)                            |
| `clippy::missing_safety_doc` / `missing_errors_doc` / `missing_panics_doc` (pedantic) | `unsafe fn` / `Result` / `panic!` path without its section                                                                                              | the section standard                                                                                      |
| `clippy::unnecessary_safety_doc`                                                      | `# Safety` on a safe fn                                                                                                                                 | delete it                                                                                                 |
| `clippy::undocumented_unsafe_blocks` (restriction)                                    | `unsafe {}` or `unsafe impl` without `// SAFETY:` above it (above the statement or above its attributes, both accepted)                                 | `comments.md` §1                                                                                          |
| `clippy::unnecessary_safety_comment`                                                  | `// SAFETY:` on safe code                                                                                                                               | delete it                                                                                                 |
| `clippy::multiple_unsafe_ops_per_block`                                               | two `unsafe` ops in one block                                                                                                                           | split; one comment per obligation                                                                         |
| `clippy::too_long_first_doc_paragraph` (nursery)                                      | a first paragraph over 200 rendered characters (hardcoded, not configurable) on an exported module-page item; impl members and private items are exempt | one sentence, blank `///`, then the body; the house asks this everywhere, the lint only gates the surface |
| `clippy::doc_markdown` (pedantic)                                                     | an identifier or `CamelCase` word without backticks                                                                                                     | backticks on every identifier                                                                             |
| `clippy::doc_lazy_continuation`, `doc_overindented_list_items`                        | a list item's continuation line mis-indented                                                                                                            | indent continuation lines to the item's text                                                              |
| `clippy::empty_line_after_doc_comments`                                               | a blank line between a doc block and its item                                                                                                           | keep the doc attached                                                                                     |
| `clippy::allow_attributes`, `allow_attributes_without_reason`                         | `#[allow]`, or an `#[expect]` without a reason                                                                                                          | `#[expect(lint, reason = "…")]` only                                                                      |
| `clippy::missing_assert_message`                                                      | `assert!` without a message                                                                                                                             | a fragment naming the property (`comments.md` §5)                                                         |
| `rustdoc::broken_intra_doc_links`                                                     | ``[`Name`]`` that does not resolve; an ambiguous `Foo`                                                                                                  | fix the path; disambiguate with `fn@`/`struct@`/`Foo()`                                                   |
| `rustdoc::private_intra_doc_links`                                                    | a public doc linking a private item                                                                                                                     | link the public front door                                                                                |
| `rustdoc::redundant_explicit_links`                                                   | ``[`Foo`](Foo)``                                                                                                                                        | ``[`Foo`]``                                                                                               |
| `rustdoc::bare_urls`                                                                  | a URL not in `<…>` or a link                                                                                                                            | a reference definition at the bottom                                                                      |
| `rustdoc::unescaped_backticks`, `invalid_html_tags`                                   | a stray `` ` ``; `<T>` read as HTML                                                                                                                     | write `` `<T>` `` in backticks                                                                            |
| `rustdoc::invalid_rust_codeblocks`, `invalid_codeblock_attributes`                    | a fence that does not parse; a misspelled attribute                                                                                                     | fix the example; `text` for non-Rust                                                                      |

`unfulfilled_lint_expectations` is a *warning*: read gate output for `warning:`,
not only for errors, or a dead `#[expect]` ships.

## 3 Features and Cfg Axes

Twenty crates carry `[features]`. Features are explained once, in the crate
docs' `# Crate features` table; never in item prose (the item is either there or
not for the reader's build). `cargo hack --each-feature clippy` (`just
nightly-cargo-hack`) is the nightly per-feature pass.

An item behind `#[cfg(target_os = …)]` is named in plain backticks on shared
surfaces; an intra-doc link to it breaks on the other target. An item behind
`cfg(loom)`/`cfg(miri)` likewise.

`publish = false` workspace-wide: `[package.metadata.docs.rs]`,
`#![cfg_attr(docsrs, …)]` and `doc_cfg` badges do not apply here.

## 4 Links Rustdoc Cannot Resolve

| target                                     | write                                                                                                               |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- |
| an item of this crate or a dependency      | ``[`Name`]``, ``[`m`](Self::m)``, ``[`Layout`](core::alloc::Layout)``                                               |
| a path used several times in one block     | a reference definition: `[`Shared`]: crate::Shared`                                                                 |
| a workspace crate that is not a dependency | relative HTML path: `[`SeqLock`]: ../wtx_sync/seqlock/struct.SeqLock.html`                                          |
| an unstable / impl-restricted std item     | URL: `[`AtomicPrimitive`]: https://doc.rust-lang.org/std/sync/atomic/trait.AtomicPrimitive.html`                    |
| a third-party crate or one of its items    | URL: `[loom]: https://docs.rs/loom`, `[`UnsafeCell`]: https://docs.rs/loom/latest/loom/cell/struct.UnsafeCell.html` |
| an issue, RFC, paper                       | URL with a stable short label: `[rust#125632]: https://github.com/rust-lang/rust/issues/125632`                     |
| a `cfg`-gated variant on a shared surface  | plain `` `Variant` ``, no link                                                                                      |

A ``[`Foo`]`` with a matching reference definition uses the URL and skips
intra-doc resolution, so it links cleanly and stays code-formatted. Never demote
an unresolvable link to a bare code span when a page for it exists.

A name shared by a private module and a public macro or fn (`mod init` and
`init!`) is ambiguous only when private items are documented, which `just
check-rustdoc` and the audit both do: write the disambiguator from the start,
``[`init!`](crate::init!)``, ``[`foo`](fn@crate::foo)``,
``[`init`](mod@crate::init)``.

## 5 Rustdoc and Doctest Attributes

| attribute / fence           | use here                                                                                                                                             |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `#[doc(hidden)]`            | plumbing a macro needs `pub` (`pub use ::pin_init as __pin_init`), a test-only module behind a feature; no doc on it, the `__` name says "not yours" |
| `pub(crate)` / `pub(super)` | the first tool for keeping an item off the surface; `#[doc(hidden)]` only for what must be `pub`                                                     |
| `#[doc(inline)]`            | a `pub use` of the crate's own item so it lists with its siblings; never on a third-party re-export                                                  |
| `#[doc(alias = "…")]`       | a name readers search for that the item is not called (`alias = "memset"`)                                                                           |
| ` ``` `                     | compiled and run: the default for every example                                                                                                      |
| `no_run`                    | compiled, not run: I/O, a process, a mapping                                                                                                         |
| `should_panic`              | documents a `# Panics` condition (rare: `panic` is denied in library code)                                                                           |
| `compile_fail`              | a type-level guarantee shown in docs; prefer a `trybuild` fixture for the error text                                                                 |
| `text`                      | not Rust: diagrams, listings, invocations                                                                                                            |
| `ignore`                    | never                                                                                                                                                |
| `# ` line prefix            | compiled, hidden: `# Ok::<(), E>(())`, `# install(…)?;`, `# let _ = boxed;`                                                                          |
| `#![feature(…)]` first line | when the example needs a nightly feature (`allocator_api`)                                                                                           |

## 6 Rendered Check

The per-crate build lands at `target/<host>/doc/<crate_snake>/index.html`; `just
docs api` builds the merged workspace API into `target/doc/`. Read the crate
page top to bottom in the HTML (no browser on the box: `sed 's/<[^>]*>//g'` over
the file, or the `.md` a `--output-format markdown` build would give) and check:

1. The crate page reads as an introduction; the sidebar shows the task headings.
2. Every ``[`Name`]`` became a link; no literal `` [` `` survives (``grep -c
   '\[``' index.html``). A link into a sibling workspace crate renders as plain
   code, not a link, until that crate is documented into the
   same ``target/doc`` (``just docs api` does all of them); that is not a defect
   of the doc, and no lint fires for it.
3. Diagrams and listings sit in `text` fences, aligned; table pipes align in the
   source.
4. Each summary is complete in the module listing: one sentence, no trailing
   fragment.
5. No heading is followed by an empty paragraph; no `# Example` singular.
