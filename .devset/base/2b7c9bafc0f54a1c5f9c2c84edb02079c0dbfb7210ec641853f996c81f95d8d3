# Comments: Everything That Is Not Rustdoc

`// SAFETY:`, `// ORDERING:`, inline `//`, `#[expect]` reasons, assertion and
`expect` messages, tests, fixtures, benches, examples, and file headers. Same
reader as the docs: the code is open, so a comment says only what the code
cannot.

Contents: 1 `// SAFETY:` · 2 `// ORDERING:` · 3 Inline `//` · 4 `#[expect]` · 5
Messages · 6 Tests · 7 File headers · 8 Manifest

## 1 `// SAFETY:`

Required above every `unsafe {}` block and every `unsafe impl`
(`undocumented_unsafe_blocks` is denied); directly above the statement, or above
its attributes when it has them (`#[expect(…)]` then `// SAFETY:` then `unsafe
impl` reads best, and both placements are accepted). One `unsafe` operation per
block (`multiple_unsafe_ops_per_block` is denied), so each comment discharges
one obligation. Never on safe code (`unnecessary_safety_comment` is denied).

One line, stating the *fact* that discharges the obligation. Three sources of
fact:

| source                | pattern                                     | example                                                                                       |
| --------------------- | ------------------------------------------- | --------------------------------------------------------------------------------------------- |
| the caller's contract | `the caller upholds <item>'s contract[, …]` | `// SAFETY: the caller upholds `__init`'s contract, including pinning unless `I` cancels it.` |
| the preceding step    | `<what just happened>, so <permission>`     | `// SAFETY: the initializer reported success, so the guard may count it.`                     |
| an invariant          | `<invariant>[; <invariant>]`                | `// SAFETY: the `index`th of the `len` slots the caller promised, still fresh.`               |

Chaining, when the fact was stated a few lines up: `// SAFETY: as above.`, `//
SAFETY: as `base`.`, `// SAFETY: as `read_at`, and nothing else reaches the
field while the lock is held.`

On an `unsafe impl`, state why every obligation of the trait holds: `// SAFETY:
one copy initializes every element, and a `Copy` type cannot refuse or panic.`
`// SAFETY: the only mutable state is the atomic cursor, so handles never
alias.` (`Send`), then `// SAFETY: see the `Send` impl.` (`Sync`).

Never: restate the operation (`// SAFETY: write to the pointer`), open with
`This is safe because`, say `trust me`, or explain what `unsafe` means. Name the
hazard the code rests on, not a proxy for it (*torn-read bit-pattern validity*,
not *reachability*).

Rationale that is not a safety argument goes on its own `//` line above,
separated by a bare `//`:

```rust
// Built where it lands, so nothing is moved in; on `Err` it wrote nothing.
//
// SAFETY: a fresh unaliased slot for one `T`, and `Init` cancels the pinning duty.
unsafe { PinInit::__init(init, slot) }?;
```

A `# Safety` section on an `unsafe fn` and the `// SAFETY:` inside it are
different sentences: the section is the caller's obligation, the comment is why
this body's `unsafe` op is discharged by it (`// SAFETY: the caller
upholds `raw_try_init`'s contract; the initializer cannot fail.`).

## 2 `// ORDERING:`

Above an atomic operation whose `Ordering` is not the obvious one, or once at
the top of a function that uses one ordering throughout. States the ordering and
what it publishes or does not:

```rust
// ORDERING: Relaxed throughout. The word hands out disjoint ranges and publishes nothing; whatever
// a caller lays in the bytes it took, it releases itself.
```

`// ORDERING: Relaxed, as `carve`: the cursor hands out ranges and publishes
nothing.` chains like `// SAFETY:` does.

## 3 Inline `//`

Only where the code cannot say it: a reason, an invariant, a non-obvious
consequence, a deliberate absence. Capitalized, ends with a period, one fact per
comment; `rustfmt` wraps at 100 columns, so a second line is a second fact or
nothing.

| use                    | example                                                                                             |
| ---------------------- | --------------------------------------------------------------------------------------------------- |
| a non-obvious choice   | `// A mask, not a remainder: `align()` is a power of two, but only a divide would prove it.`        |
| a phase label          | `// Commit: `chunk`shrinks to`need` and turns in use, in one store.`                                |
| a deliberate absence   | `// No `Reclaiming`: a bump cursor never steps back, so a block returned here is not served again.` |
| a deliberate omission  | `// `Zeroable` … are deliberately absent: <reason as fact + consequence>.`                          |
| a `#![feature]` entry  | `// `impl_restriction`: the settle is sealed by the payload it indexes, which a module cannot say.` |
| a `cfg` that looks odd | `// Neither a loom model nor miri drives a compiler or spawns a process.`                           |
| a debug tripwire       | `// A double-claim (unprovable in types) arrives free; a tripwire, coalescing can hide it.`         |

Delete any comment a rename would make redundant, and consider the rename. Never
narrate the next line, never explain what a lint wants, never cite a rule ID or
a phase.

An existing comment that states its fact in one terse line is finished.
Rewriting it longer, or into a figure of speech (`the type system catching up`),
is a regression even when it reads well: keep the fact, drop the flourish.
`// `hmac::sign`computes into a stack`Tag`, no heap.` beats two lines saying the
same thing more elegantly.

## 4 `#[expect(lint, reason = "…")]`

`expect`, never `allow` (`allow_attributes` is denied): a stale suppression then
warns. The reason is the *cause*, lowercase, no terminal period, one clause; it
names the concrete thing the lint misreads, never the consequence (`"otherwise
clippy complains"`) or the lint's name.

```rust
#[expect(clippy::mem_forget, reason = "pin-init disarms its field guards this way")]
#[expect(clippy::indexing_slicing, reason = "a `Bucket` is below `NUM_BUCKETS`, this array's length")]
#[expect(clippy::indexing_slicing, reason = "as `index`")]
```

Crate-level `unsafe_code` names the concrete unsafe and its obligation:

```rust
#![expect(
    unsafe_code,
    reason = "writing a value straight into raw destination bytes is this crate's whole purpose"
)]
```

A file whose role excuses a lint says so once at the top, in the same voice:
`#![expect(clippy::print_stdout, reason = "a demo binary reports its result on
stdout")]`, `#![expect(clippy::tests_outside_test_module, reason = "an
integration test's functions are top-level by construction")]`.

No comment ever explains an `#[expect]`; the reason is its one home. If the only
unsafe in a crate is a derived `unsafe impl`, the crate-level expect is
unfulfilled: delete it.

## 5 Messages

Every message is a lowercase fragment, no terminal period, saying what is true
or what went wrong, never what the code is doing.

| site                                 | form                                            | example                                                               |
| ------------------------------------ | ----------------------------------------------- | --------------------------------------------------------------------- |
| `#[assert(cond => "…")]`             | the property, as a claim                        | `size == size_of::<usize>() => "an address, and nothing else"`        |
| `debug_assert!(cond, "…")`           | the violation, as a fact about the input        | `"a pointer outside the recorded mapping"`                            |
| `.expect("…")` in tests and examples | why it cannot fail here                         | `expect("the slack absorbs the pad")`, `expect("8 bytes, 8-aligned")` |
| `assert_eq!(a, b, "…")` in tests     | the property pinned, continuing the sentence    | `"the word survives a decode"`, then `"and the value an encode"`      |
| `#[error("…")]` on a variant         | `<type words> error: <fragment>`, fields inline | `"region error: needs {need} bytes, region holds {have}"`             |
| `#[cfg_attr(miri, ignore = "…")]`    | what Miri cannot do here                        | `ignore = "mmap is unsupported under Miri"`                           |
| `panic!`/`unreachable!` (tests only) | the impossible state                            | `panic!("a one-byte layout is not too small: {other:?}")`             |

Assertion messages in a test read as one running argument: `"to itself"`, `"and
forwards"`, `"never backwards"`. Not every assertion needs one; add it where the
*why* is not the expression.

## 6 Tests

- A `#[test]` fn name is the property, as a snake_case sentence, article first:
  `a_slice_becomes_a_region_over_its_own_bytes`,
  `an_each_that_gives_up_drops_the_prefix_exactly_once`. It carries no doc
  comment.
- A `//` comment above a test says what it pins and why, when the name cannot:
  `// The bug this closes aligned the offset, so it was correct only when the
  base already was.`
- Helpers, fixtures, and test-local constants get one summary line stating their
  role in the proof: `/// A layout that is valid by construction.`, `/// Slots
  the run writes into, and the one that refuses.`; a constant that pairs with
  another links it: `/// See [`SLOTS`].`
- A test module opens with `//!` only for setup notes (how to regenerate, why a
  `cfg` excludes a runner). Test modules carry `#[cfg(test)] #[cfg(not(loom))]`;
  a loom model is `#[cfg(test)] #[cfg(loom)] mod model` with a `// Run with …`
  comment and one paragraph on what the model can and cannot see.
- Inline comments in tests explain a non-obvious *setup*, never the assertion.
- `#[deny(unused_unsafe)]` on a test turns "this door is `unsafe`" into a
  compile-time property; say so in the test's `///`.
- A test-only `unsafe fn` helper carries `# Safety` like any other.

## 7 File Headers

Every non-`lib.rs` file opens with `//!`. One line by default; a contract
paragraph and one example only for a module that is itself a surface.

| file                      | `//!` says                                                                                                                                       | then                                                                                                                                                               |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| module                    | the role, naming the key item                                                                                                                    |                                                                                                                                                                    |
| `errors.rs`               | `Why <the thing> did not work out[: <the two cases>].`                                                                                           |                                                                                                                                                                    |
| `testing.rs`              | `What the in-crate tests share.` or the fixture's role                                                                                           |                                                                                                                                                                    |
| `tests/<name>.rs`         | the proof, as a claim, and the one datum that crosses                                                                                            | `#![cfg(not(any(loom, miri)))]`, the file-level `#![expect]`s the role needs                                                                                       |
| `tests/trybuild.rs`       | `UI tests pinning the properties the type system carries. Regenerate snapshots with `TRYBUILD=overwrite cargo test -p <crate> --test trybuild`.` | `// Neither a loom model nor miri drives a compiler or spawns a process.` + `#![cfg(not(any(loom, miri)))]`; the `ui` test's `///` lists what each fixture refuses |
| `tests/compile_fail/*.rs` | one or two lines: the unsoundness the compile failure prevents                                                                                   | nothing about the harness                                                                                                                                          |
| `benches/<name>.rs`       | what is measured and what the gap between arms means                                                                                             | every `const` documented: `/// The isolated core the measuring thread runs on.`                                                                                    |
| `examples/<name>.rs`      | what the walk-through shows, then `Run with `cargo run -p <crate> --example <name>`.`                                                            | `#![expect(clippy::print_stdout, reason = "a demo binary reports its result on stdout")]`                                                                          |
| a bin's `main.rs`         | the operator's view: what it does, how it is driven, a `text` fence of invocations                                                               |                                                                                                                                                                    |

## 8 Manifest

- `description` is the crate summary's pitch clause verbatim, first letter
  lowercased (acronyms and names keep their case: `REST`, `FIX`, `Binance`),
  trailing period, an imperative or noun phrase that never names the crate
  itself (`the one crate that …`, `a library for …`): `description = "build a
  value where it will live, never moved there."`, `description = "allocators
  over a byte range you own."`
- Dependencies sit under `# internal` / `# external` comments, `# internal`
  first (taplo sorts within each group). Those two are the *only* comments a
  manifest carries: no trailing gloss on a dependency, no paragraph above a
  table. A dependency whose purpose is not its name is named in the crate docs
  where its guarantee is used, not beside the entry.
- A feature that is not self-explanatory is explained once, in the crate docs'
  `# Crate features` table, never in the manifest.
- Everything else about a manifest — shape, inheritance, features, targets, the
  gates — is `writing-cargo-manifest`.
