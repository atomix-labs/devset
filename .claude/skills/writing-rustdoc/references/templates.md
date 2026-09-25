# Templates: Crate, Module, Item, File, Diagram, Table

Skeletons in the exemplar crates' shape. *Fixed* sections keep their exact name
across crates so a reader learns where to look once; *task* and *concept*
sections are named for what the reader does or the thing explained. Drop any
section with nothing to say; never pad one.

Contents: 1 Crate · 2 Module · 3 Items · 4 Errors · 5 Test, bench, example files
· 6 Manifest · 7 Diagrams · 8 Tables and bullets · 9 Section names

## 1 Crate (`lib.rs`)

Two shapes are in use. Both open with the pitch and close with the reference
links; both keep `lib.rs` to docs, `#![…]` attributes, `mod`, and `pub use`.

### 1.1 Problem-First (A Facade or a Mechanism with a Story)

````rust
//! <Topic>: <pitch, the manifest `description`>.
//!
//! <The problem, shown before it is named: a short example with one-line comments naming the
//! cost, then one paragraph on what goes wrong and the external fact that says so, cited.>
//!
//! <The shape: what the crate is, then each departure from the obvious design or from a wrapped
//! crate, as fact + consequence, one sentence each.>
//!
//! # <Task, e.g. Building one value>                 ← task, gerund
//! ```
//! <complete example of the primary path>
//! ```
//!
//! # <Task, e.g. Building many>
//! <One sentence naming and linking the entry points.>
//! ```
//! <example>
//! ```
//!
//! # <Concept, e.g. Pinned types / Contention / Ordering>   ← concept, noun
//! <Prose; a diagram (§7) if there is a flow, handoff, or state.>
//!
//! # What it compiles to                             ← fixed, only with real output in hand
//! <The exact call, the build (`release build, aarch64`), a `text` fence of the listing with
//! aligned `;` annotations, one paragraph on what it proves.>
//!
//! # Crate features                                  ← fixed, required if any feature exists
//! <One sentence on the defaults.>
//!
//! | feature   | enables                                                    |
//! | --------- | ---------------------------------------------------------- |
//! | `alloc`   | `InPlaceInit`, to build a `Box`/`Arc` in place             |
//!
//! [label]: https://…                                ← reference links, order of first use
````

### 1.2 Shape-First (A Vocabulary Crate: Several Types That Fit Together)

````rust
//! <Pitch.>
//!
//! <One paragraph: the central noun and what choosing it buys. A `text` shape diagram (§7.1) if
//! the parts relate.>
//!
//! # Types                                           ← fixed
//!
//! - **<Role.>** [`A`] is …; [`b`](A::b) narrows one, [`c`](A::c) divides one, and both spend
//!   what they take.
//! - **<Role.>** [`D`] is …; [`E`] is what a `b` yields.
//! - **Refusals.** [`AError`] when …; [`BError`] when ….
//!
//! ```
//! <one example of the primary path>
//! ```
````

`# Types` groups by *role*, three to five bullets, bold lead, each naming its
types in one clause with a link. It is never one bullet per export, and never
restates an item's own docs.

### 1.3 Attributes That Follow

```rust
#![no_std]
#![feature(non_exhaustive_omitted_patterns_lint, strict_provenance_lints)]
#![expect(
    unsafe_code,
    reason = "<the concrete unsafe this crate exists to do>"
)]
```

Ordering rule for the crate page: *why → what → how to use it → what it costs →
how to configure it*. Task sections go from the single, common case to the
plural or advanced one.

## 2 Module (`mod.rs` / `foo.rs`)

The default is one line:

```rust
//! <Noun or gerund phrase, naming the key item>.
```

`//! A bump cursor over one region.` · `//! Why an allocator refused.` · `//!
Driving an initializer at a destination.` · `//! `Arena`'s [`Allocator`] impl:
it carves, and never takes a cut back.`

A module that is itself a surface adds a contract paragraph and one example:

````rust
//! <Gerund phrase>: <what it gives, naming the key item>.
//!
//! <Contract: what is guaranteed, in what order, what remains on failure; the general entry
//! point against the specialized ones, all linked.>
//!
//! ```
//! <one example of the primary path>
//! ```
````

A module whose content is one type takes the type's role as its line and leaves
the type's docs primary. Never a `///` on the `mod foo;` declaration in
`lib.rs`; the file's `//!` is the one home.

## 3 Items

Order inside an item: summary · prose · `# Safety` · `# Errors` · `# Panics` ·
`# Examples`. Blank `///` line before each heading, none after. `# Examples`
always plural.

### 3.1 Function or Method

````rust
/// <Verb> <object> <at/into/from> <target>[, or `None` when <condition>].
///
/// <Why or when; the reason for a non-obvious choice; the cost.>
///
/// # Errors
/// <see §4>
///
/// # Examples
/// ```
/// <shortest example that shows the why>
/// ```
#[inline]
#[must_use]
pub fn …
````

Getters and constructors are one line with no example: `/// Bytes the span
holds.`, `/// The range of `len`bytes at`start`.`, `/// Wraps a raw address.`

### 3.2 Unsafe Function

````rust
/// <Verb> ….
///
/// # Safety
/// `dst` is <precondition>, <precondition>, and <precondition>[; and, unless <case>,
/// <conditional precondition>].
///
/// # Errors
/// <…>
///
/// # Examples
/// ```
/// // SAFETY: <the fact discharging the precondition>.
/// unsafe { … }
/// ```
pub unsafe fn …
````

The infallible twin of a fallible fn writes `# Safety` as `As [`try_version`].`
and drops `# Errors`. A method users should not call directly says so in the
summary and links the front door: `/// Writes them at `dst`. Prefer
[`raw_run_init`] / [`raw_try_run_init`] to calling this.`

### 3.3 Trait

```rust
/// How to <capability> [at/for <context>].            ← or the role, for a shape trait
///
/// <Why a trait; what implementors promise beyond the signatures; the one reason for its shape.>
///
/// # Safety                                           ← unsafe trait: the impl's promise
/// `Ok` means <state>; `Err` means <state>, and <what may then be done with the bytes>.
pub unsafe trait Name<T> {
    /// <A stored location: a pointer for [`Local`], an [`Offset`] for [`Shared`].>   ← assoc type
    type Stored: Copy;

    /// <What it fixes, and why that value.>            ← assoc const
    const MIN_REGION: Layout;

    /// <How many … / Whether … / The …>                ← getters
    fn len(&self) -> usize;

    /// <Verb> ….
    ///
    /// # Errors
    /// [`BindError`], <condition>, <condition>, or <condition>.
    fn open(region: &Region<Self::Storage>) -> Result<Self::Bound<'_>, BindError>;
}
```

A marker trait states the property an impl asserts and, for `unsafe trait`, what
that licenses: `/// Bytes returned to this allocator become available again.`
then `# Safety` then, if needed, the one place the marker may gate and where it
must not.

A trait method whose behaviour in one impl needs saying is documented *in that
impl*: `/// Bytes the free-lists do not hold, **the heap's own terminal sentinel
included**: …`.

### 3.4 Struct and Fields

````rust
/// <Role: noun phrase, or verb phrase for an active object>[: <the pitch clause>].
///
/// - **<Property.>** <Consequence, one sentence.>        ← three bullets, or prose for fewer
/// - **<Property.>** <…>
/// - <A property with no name, as prose.>
///
/// <Where the safe door is, linked.>
///
/// # Examples
/// ```
/// <…>
/// ```
pub struct Name<S> {
    /// <Noun phrase>.[ <Invariant.>]
    field: T,
    /// Fixes <the type parameters> by owning them.      ← PhantomData
    marker: PhantomData<S>,
}
````

Field docs are one line, private fields included
(`missing_docs_in_private_items` is denied). Adapter and return types: `/// What
[`builder`] builds.` and nothing more unless users construct them.

### 3.5 Enum and Variants

```rust
/// <Role>.
///
/// <If variants form a state machine or an ordering: the rule, or a diagram (§7.4).>
pub enum Name {
    /// <When it is produced, or what it carries.>
    Variant,
}
```

### 3.6 Type Alias and Constant

```rust
/// <The role>.
///
/// An **alias**, not a newtype: <the fact that decides it>.
pub type Offset = NonZeroU64;

/// <What it fixes>, and <why that value>.
pub const MIN_ALIGN: usize = 16;
```

### 3.7 Macro

````rust
/// <Verb> ….
///
/// <Grammar in one sentence: `name: value` writes a value, `name <- init` runs a nested
/// initializer, `name` takes the like-named field.> <Where the escape hatch is, linked.>
///
/// <A known wart, as fact + consequence, one sentence.>
///
/// ```
/// <…>
/// ```
#[macro_export]
macro_rules! name { … }
````

### 3.8 `#[doc(hidden)]`, Re-Exports, Deliberate Absences

```rust
#[doc(hidden)]
pub use ::pin_init as __pin_init;       // no doc; the `__` says "not yours"

/// What this crate takes from `pin-init` unchanged, globbed so an explicit import can shadow one
/// of its names.
mod borrowed {
    // `Zeroable`, `MaybeZeroable`, … are deliberately absent: <reason as fact + consequence>.
    pub use pin_init::{…};
}

// No `Reclaiming`: a bump cursor never steps back, so a block returned here is not served again.
```

A hidden module that a dependent's tests use gets a `//!` saying why it is
public and hidden: `//! Behind the `testing`feature: a plain`cfg(test)` module
is invisible across a crate boundary, and a dependent's tests count drops the
same way.`

## 4 Errors

`# Errors` names the variant, links it, and states the condition as a fact; the
destination's state when it matters. Four forms:

```rust
/// # Errors
/// [`RootError::Conflict`], a mapping is already recorded.                 ← one variant

/// # Errors
/// - [`RegionError::TooSmall`], the region is shorter than `layout`.       ← several variants
/// - [`RegionError::Misaligned`], the base does not meet `layout`'s alignment.

/// # Errors
/// [`BindError`], the region does not fit one, nothing is published yet, or another build laid
/// it out.                                                                 ← a whole enum, its cases listed

/// # Errors
/// Whatever the source reports, having dropped the prefix it had written.  ← propagated, with the state

/// # Errors
/// As [`open`](Self::open).                                                ← delegated
```

The error enum itself:

```rust
//! Why <the thing> did not work out[: <case>, or <case>].

/// Why <what could not happen>.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum FooError {
    /// <The condition, as a fact>[: <what to do about it>].
    #[error("foo error: <fragment with {fields} inline>")]
    Variant {
        /// <What the field carries.>
        need: usize,
    },
}
```

One `errors.rs` per crate. The message leads with the type's words (`region
error:`, `reserve error:`, `sign error:`), then a fragment; no trailing period;
fields by name. Variant docs are one line stating what it *is*; no value trivia.

## 5 Test, Bench, Example Files

```rust
//! <The proof, as a claim>: <what two parties share and what crosses between them>.
//!
//! <One paragraph on the one datum that crosses, or the shape the test exercises.>

#![feature(non_exhaustive_omitted_patterns_lint, strict_provenance_lints)]
#![cfg(not(any(loom, miri)))]
#![expect(
    clippy::tests_outside_test_module,
    reason = "an integration test's functions are top-level by construction"
)]
```

```rust
//! UI tests pinning the properties the type system carries. Regenerate snapshots with
//! `TRYBUILD=overwrite cargo test -p <crate> --test trybuild`.

#![feature(non_exhaustive_omitted_patterns_lint, strict_provenance_lints)]
// Neither a loom model nor miri drives a compiler or spawns a process.
#![cfg(not(any(loom, miri)))]

#[cfg(test)]
mod tests {
    /// Each fixture is a use <the shape> must refuse: <one clause per fixture>.
    #[test]
    fn ui() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/compile_fail/*.rs");
    }
}
```

```rust
//! <The property, and the unsoundness forgetting it would allow, in one or two lines.>   ← compile_fail fixture
```

```rust
//! What <the thing> costs: <arm one>, against <arm two>.
//!
//! <What the gap between the arms measures, and what the figures do and do not say.>

/// The isolated core the measuring thread runs on.
const CORE: u32 = 6;
```

```rust
//! <What the walk-through shows: one sentence.>
//!
//! Run with `cargo run -p <crate> --example <name>`.

#![expect(clippy::print_stdout, reason = "a demo binary reports its result on stdout")]
```

## 6 Manifest

Only the `description` is this skill's; the rest of the shape is
`writing-cargo-manifest`.

```toml
[package]
name        = "wtx-foo"
description = "<the crate summary's pitch clause verbatim, first letter lowercased, trailing period; never `the crate that …`>."

[dependencies]
# internal
wtx-assert = { workspace = true }
# external
zerocopy = { workspace = true }

[dev-dependencies]
# external
trybuild = { workspace = true }
```

## 7 Diagrams

A diagram earns its place when it shows a relation prose would have to
serialize: what is made of what, who talks to whom, where bytes live, which
state follows which. Never for a single call, a list, or a two-node arrow.
Inside a ` ```text ` fence; ≤ 12 lines; ≤ 90 columns after the `//! ` prefix;
annotations in an aligned right-hand column introduced by `──`; box-drawing
characters `─ │ ┌ ┐ └ ┘ ├ ┤ ┬ ┴ ▶ ▼`.

### 7.1 Shape (What a Crate Is Made Of)

```text
bytes you own ── a buffer, or a mapped segment
│
▼
Region        ── a span of bytes, yours to write
├──▶ Arena    ── bump a cursor, never frees
└──▶ Heap     ── segregated fit, coalescing
```

### 7.2 Layout (Bytes and Fields)

```text
dst ──▶ ┌──────────┬────────────────────┬──────────────────┐
        │ id: u64  │ near: Leg          │ tag: [u8; 32]    │
        └──────────┴────────────────────┴──────────────────┘
        +0         +8                   +24                +56
```

### 7.3 Sequence (Participants as Columns, Time Downward)

```text
writer                 Rcu<H>                 reader
   │                      │                      │
   │                      │◀──── read_with ─────▶│  borrow the published value
   │── store(new) ───────▶│                      │
   │◀──── Retired(old) ───│                      │  displaced, not yet freed
```

### 7.4 State (Boxes and Labeled Transitions)

```text
┌─────────┐  grew()  ┌─────────────┐  disarm()  ┌──────────┐
│ armed 0 │─────────▶│ armed n     │───────────▶│ disarmed │
└─────────┘          └──────┬──────┘            └──────────┘
                            │ drop on `Err` or panic
                            ▼
                    drop_in_place(data[..n])
```

### 7.5 Listing (Asm, Output)

```text
movi v0.2d, #0      ; zero the tag
stp  x1, x2, [x0]   ; id, near.px  -> dst, dst+8
```

Real output only, with the exact call and build named above the fence.

## 8 Tables and Bullets

| use      | when                                                                |
| -------- | ------------------------------------------------------------------- |
| table    | ≥ 3 rows sharing ≥ 2 attributes: features, variants, sources, modes |
| bullets  | ≥ 3 parallel fragments that are not a comparison and not a sequence |
| numbered | a sequence the reader performs, or an ordered protocol              |
| prose    | everything else, including any list of two                          |

Tables: header cells lowercase; pipes aligned by hand (`rustfmt` does not touch
markdown); code in backticks; no terminal periods; one sentence before the table
and no restatement after it.

```markdown
| feature | enables | ← # Crate
features | --------- | ------------------------------------------ |

| target | value | source | ← a fact per
platform | --------- | ----- | ------------------------------- |

| source | element built | cost | may refuse | ← # Choosing a
<noun> | --------- | ------------- | ------------- | ---------- |
```

Bullets: fragments carry no period; full sentences do. Parallel grammar across
items. A bold lead (`**Move-only.**`) when each bullet names a property. No
nested bullets deeper than one level.

## 9 Section Names

- *Task*: gerund, sentence case, what the reader is doing: `Building one value`,
  `Building many`, `Choosing a source`, `Allocating through Allocator Trait`.
- *Concept*: the noun: `Pinned types`, `Contention`, `Ordering`, `Lifecycle`,
  `Safety model`.
- *Fixed*, exact spelling: `Types`, `Crate features`, `What it compiles to`, and
  the item sections `Safety`, `Errors`, `Panics`, `Examples`.
- Never: `Overview`, `Introduction`, `Usage`, `Getting started`, `Notes`,
  `Miscellaneous`, `Implementation details`, `Arguments`, `Parameters`,
  `Returns`, `Example`, `HOT`.
