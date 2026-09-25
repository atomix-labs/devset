---
name: writing-rustdoc
description: Use when writing, revising, tightening, or reviewing rustdoc (`//!`, `///`), `// SAFETY:` and `// ORDERING:` comments, `#[expect]` reasons, test/bench/example file headers, or a crate's `Cargo.toml` description and dependency comments in this Rust workspace; when a crate, module, trait, type, or `unsafe fn` is added or changed and its docs must be written or kept current; when asked to "document this", make a crate "docs.rs quality", "clean up the docs", or audit docs for restatement, filler, slop, widows, or missing Safety/Errors/Examples. Not for markdown outside Rust source (mdBook pages, READMEs, plans, notes).
---

# Writing Rustdoc

House style for rustdoc and code comments, learned from `lib/memory/wtx-init`,
`wtx-region` and `wtx-allocators`. The reader is a competent Rust engineer with
the code open beside the docs: tell them only what the code cannot (why it
exists, what it promises, what it assumes, what it costs) and link everything
else. Compactness is deleting restatement, never deleting content.

Load `references/style.md` and `references/templates.md` before writing
anything; `references/comments.md` before touching a `//` comment, an
`#[expect]`, a test, a bench, or a manifest; `references/exemplars.md` when
unsure how a rule looks on the page; `references/tooling.md` before the verify
step; `references/sources.md` when an external claim needs a citation.

## Non-Negotiables

1. **Nothing the code already says.** No summary that paraphrases the signature,
   no comment that narrates the next line, no `# Arguments` / `# Returns`, no
   module doc that lists its items, no doc on a `mod x;` line. If deleting a
   sentence loses nothing, delete it.
2. **Link, don't re-explain.** An item that has a home gets ``[`Name`]``; an
   external fact gets a reference definition at the bottom of the block. Never
   restate what the link target says.
3. **One sentence, one line.** The first paragraph is one sentence and fits one
   line when it can; a long colon-structured sentence may wrap. Body clauses
   hang off `:` and `;`. No em dash.
4. **Compact form.** `# Examples` (plural), `# Safety` / `# Errors` / `# Panics`
   with the content on the next line; `# Errors` as `[`Variant`], <condition>.`;
   backticks on every identifier.
5. **Each crate speaks for itself.** Link a dependency where a guarantee comes
   from it; never name a dependent, never re-explain a substrate concept, never
   editorialize (`generic`, `zero-cost`).
6. **No guessing.** A reason, contract, cost, or citation not found in the code,
   tests, manifest, design docs, or `git log` is asked for, never invented. See
   *Ask, don't guess*.
7. **Every gate, every time.** The formatters, the widow pass, then
   `scripts/doc-audit.sh <crate>` clean, before anything is reported. Read its
   output for `warning:` too.
8. **No process residue.** No `TODO`, phase or RC labels, PR talk, `# HOT`, rule
   IDs, or lint narration in a doc or comment.

## Workflow

1. **Read the whole crate.** `Cargo.toml`, `lib.rs`, every module, `tests/`,
   `benches/`, `examples/`, `tests/compile_fail/*.rs`, and any design doc under
   `docs/src/designs/` that names it. Note the voice already there and match it;
   do not import another crate's.
2. **Inventory the surface.** `python3 scripts/doc-inventory.py <crate-dir>`
   lists every item, its visibility, and which sections it has. For each
   `unsafe` item: the exact contract the code depends on. For each fallible
   item: the destination's state on `Err`. For each departure from the obvious
   design: its reason.
3. **Find the facts.** Reasons live in existing comments, tests (a test name is
   often the property), `git log -p -- <path>`, the design doc, the wrapped
   crate's own docs. Mark gaps.
4. **Ask if any gap remains** (protocol below). Do not write around it.
5. **Write, top down.** Crate → modules → items → fields and variants → `//
   SAFETY:` / `// ORDERING:` → `#[expect]` reasons → tests, fixtures, benches,
   examples → `Cargo.toml`. Use `references/templates.md`; check each block
   against the voice table below.
6. **Format, then tighten.** `just fix-rustfmt fix-dprint fix-taplo fix-rumdl`,
   then the widow pass: for every paragraph whose last line holds one to three
   words, cut words until it fits the line above, or move words between lines
   keeping each under 100 columns. `rustfmt` wraps an over-long line and never
   joins a short one; rewrite, then format again.
7. **Verify.** `scripts/doc-audit.sh <crate-dir>` (fmt check, rustdoc with
   private items, doctests, clippy, the mechanical cut list). Fix every finding;
   rerun until clean.
8. **Report** in a few lines: what was written where, the commands run with
   their status, and any question still open. Never summarize the docs
   themselves; the user reads them. Do not commit.

## Voice

Full table with the `not` column in `references/style.md` §1.

| item        | summary line                                                             | example                                                                               |
| ----------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| crate       | the pitch (`Cargo.toml` `description`), or `Topic: pitch.`               | Allocators over a byte range you own.                                                 |
| module      | one line: noun or gerund phrase, naming the key item                     | A bump cursor over one region.                                                        |
| fn / method | verb first, present tense, no subject                                    | Splits into the first `mid` bytes and the rest, or `None` when `mid` is past the end. |
| getter      | name the value: `The …`, `Bytes …`, `Where …`, `Whether …`, `How many …` | Bytes the span holds.                                                                 |
| constructor | the state it yields, or the door it is                                   | The range of `len` bytes at `start`.                                                  |
| type        | its role: noun phrase, or verb phrase if it acts                         | Drops the prefix a run wrote, unless the run completes.                               |
| trait       | `How to …`, or the role                                                  | How to build a run of `T` at a destination that already exists.                       |
| field       | noun phrase; invariant after a period                                    | How many are initialized. Zero once disarmed.                                         |
| error enum  | `Why …`                                                                  | Why an allocator could not make room.                                                 |
| variant     | the condition, as a fact                                                 | The region is shorter than the layout.                                                |
| const       | what it fixes, and why that value                                        | Slots the run writes into, and the one that refuses.                                  |
| macro       | verb first, like a fn                                                    | Builds a record straight into its destination.                                        |

Body prose says only: why it exists / when to use it (`Prefer [`x`] for …, so
…`), the contract (guarantee, order, state on failure), the reason for each
non-obvious choice as fact + consequence in one sentence, the cost. Properties
of a type go in bold-lead bullets (`- **Move-only.** …`). Reuse the crate's own
nouns and the house verbs (`references/style.md` §8): door, spend, refuse, lay,
hand out, carve, name, cover, peer, publish.

## Sections

````rust
/// Summary line.
///
/// Prose: why, when, contract, reason, cost.
///
/// # Safety
/// `dst` is aligned, writable, uninitialized for one `T`, and unaliased.
///
/// # Errors
/// - [`RegionError::TooSmall`], the region is shorter than `layout`.
/// - [`RegionError::Misaligned`], the base does not meet `layout`'s alignment.
///
/// # Examples
/// ```
/// <shortest example that shows why; ends in an assert with a fragment message>
/// ```
````

`# Errors` with one variant is one line (`[`RootError::Conflict`], a mapping is
already recorded.`); a propagated error names the state (`Whatever the source
reports, having written nothing.`); an identical contract delegates (`As
[`raw_try_init`].`) and says nothing else. An `unsafe trait`'s `# Safety` is the
impl's promise (``Ok`means …;`Err` means …`). Crate docs use task headings (`#
Building one value`), concept headings (`# Contention`), the fixed `# Types`
(role-grouped bullets, never one per export) and `# Crate features` (a table).
Examples: real domain types, `.expect("<why it cannot fail>")`, `?` closed by `#
Ok::<(), E>(())`, one `// SAFETY:` per `unsafe {}`, a closing `assert_eq!` whose
message continues the sentence.

## Cut List

Delete on sight; rewrites in `references/style.md` §4.

- Openers: `This function/struct …`, `A struct that …`, `Represents …`, `Returns
  …`, `Creates a new …`, `Used to …`, `Allows …`, `Provides …`. (`A range of …`,
  `The raw address.`, `This chunk's size …` are fine: it is article + the item's
  own kind that is banned.)
- Filler: `simply`, `just`, `basically`, `note that`, `in order to`, `it is
  important`, `please`, `will`, `various`, `etc.`, `for more information`, `see
  also`.
- Unverifiable: `powerful`, `simple`, `easy`, `efficient`, `robust`,
  `zero-cost`, `generic`, `venue-neutral`, `allocates nothing` (unless the code
  or a listing shows it).
- Restatement: parameter lists, return-type descriptions, a field's type, the
  derive list, the summary repeated in the body, a comment that reads like the
  line under it, a doc explaining a neighbouring `#[expect]`, a module doc
  enumerating members, `Entry point: …`.
- Structure: blank line after `# Safety`/`# Errors`/`# Panics`/`# Examples`; `#
  Example` singular; `# Arguments` / `# Returns` / `# Overview` / `# Usage` / `#
  Notes`; a heading over one sentence; a bullet list of two; a `# Types` bullet
  per export.
- Debris: `TODO`, `FIXME`, `(?)`, `should probably`, placeholder links, invented
  issue numbers, numbers without a run behind them, `RC10 phase B`, `chunk 3`,
  rule IDs, `# HOT`.
- Padding: a clause added to a fine one-liner (`One prepared signer per
  algorithm, each holding the only state its backend needs` says nothing the
  first half did not).

## Ask, Don't Guess

Write everything the code, tests, manifest, design docs, and history determine.
For the rest, stop and send one message before writing: a numbered list, each
entry naming the item, the sentence you would write with the gap marked, and the
fact you need. Never fill a gap with a plausible reason, a generic phrase, a
link you have not opened, or output you did not produce.

Ask when a departure needs a *reason* and none is on record; a `# Safety`
contract is not stated by the callee's `# Safety` or by the code; a claim about
codegen, allocation, or cost would be made without `cargo asm`/objdump/bench
output; an external claim needs a citation you cannot find; it is unclear
whether an item is surface, `#[doc(hidden)]` plumbing, or private; two docs, or
a doc and a test, disagree. Do not leave the gap as a placeholder: omit the
sentence and list it.

## Verify

```sh
just fix-rustfmt fix-dprint fix-taplo fix-rumdl     # or `cargo fmt -p <crate>` + `taplo fmt <crate>/Cargo.toml`
scripts/doc-audit.sh <crate-dir>                    # fmt check, cargo doc (private items too), doctests, clippy, doc-lint
scripts/doc-audit.sh <crate-dir> --lint-only --advisory   # the fast reread loop: cut list, widows, wrapped summaries
```

The audit builds under `--target $(rustc -vV | sed -n 's/^host: //p')`, as every
`just` recipe does, so it shares their cache; `--all-features` where the crate
has features. A crate in the justfile's `loom_pkgs`/`miri_pkgs` also gets `cargo
clippy … --config 'target."cfg(all())".rustflags=["--cfg","loom"]'`. Exact
flags, lints, and link tricks: `references/tooling.md`. Then read the rendered
page (`target/<host>/doc/<crate>/index.html`): every ``[`Name`]`` a link, no
heading over an empty paragraph, the crate page an introduction.

## Definition of Done

- Every `pub` item, every field, and every private item has a summary line
  carrying information beyond its name; every non-`lib.rs` file opens with a
  one-line `//!`.
- Every `unsafe fn`/`unsafe trait` has `# Safety`; every `Result`-returning item
  has `# Errors` in the house form; every `unsafe {}` and `unsafe impl` has a
  one-line `// SAFETY:`; every non-obvious atomic has `// ORDERING:`.
- Every public type a user constructs or drives, and every substantive public
  fn, has a runnable `# Examples`; trivial accessors have none, and a small type
  whose whole use sits in a neighbour's example needs none of its own.
- Crate docs: pitch, then either problem → shape → tasks or one paragraph → `#
  Types` → example; `# Crate features` if any; reference links last. `lib.rs`
  holds docs, attributes, `mod`, `pub use` and nothing else.
- `Cargo.toml`: `description` is the crate summary's pitch clause verbatim,
  first letter lowercased, trailing period, never naming itself a crate;
  dependencies under `# internal` / `# external`, which are the only comments
  the manifest carries (`writing-cargo-manifest`).
- Nothing on the cut list survives; no widow survives; `scripts/doc-audit.sh` is
  clean with no `warning:` in its output; nothing was committed.

## Red Flags

| thought                                                                    | reality                                                                                                                                                                                             |
| -------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| "The crate is already close, I'll fix the three real gaps"                 | The manifest, the summary shape, and the `# Errors` form are part of the surface. Run the inventory and the audit; they decide what is left.                                                        |
| "A blank line after `# Examples` is the rustdoc convention"                | Not here. Item sections put their content on the next line.                                                                                                                                         |
| "`[`Sign`] if the backend refuses` reads fine"                             | House form is `[`Variant`], <condition>.`; a `- ` list when several.                                                                                                                                |
| "Signing allocates nothing, that's obvious from the types"                 | A cost claim needs the code path or a listing behind it. Say what the code shows, or ask.                                                                                                           |
| "One more clause makes the module doc richer"                              | Richer means more facts, not more words. A one-liner that names the role is finished.                                                                                                               |
| "I'll leave the three-line summary, it's one thought"                      | The listing shows the first sentence. One sentence, blank `///`, then the rest.                                                                                                                     |
| "The user is in a hurry, `cargo fmt -p` is enough"                         | The formatters and `scripts/doc-audit.sh` take seconds on a warm cache. Run them.                                                                                                                   |
| "Under time pressure I'll skip reading the tests and history"              | There is no shorter path. The crate is read whole; the design doc and `git log` are opened for the gaps the inventory shows, not skimmed in full. A doc written without its facts costs more later. |
| "My rewrite of that comment reads more elegantly"                          | Elegance is not a fact. A terse existing line that states the fact is finished; a longer or figurative rewrite is a regression.                                                                     |
| "Every public type needs an example, so `Newtype` gets a tautological one" | A type whose whole use appears in a neighbour's example needs none. Examples show *why*, or they are noise.                                                                                         |
| "I'll add a `# Types` bullet for each export so nothing is missed"         | Bullets are roles, three to five; the module listing already names every export.                                                                                                                    |
| "`ignore` for now, the example needs a mapping"                            | `no_run`, or a `text` fence, or a smaller example. `ignore` is a defect.                                                                                                                            |
| "The dash reads better here"                                               | No em dash anywhere. A colon, semicolon, comma, or parenthesis, by what follows.                                                                                                                    |
| "I'll explain why the `#[expect]` is there in a comment"                   | The `reason` is its one home; a comment beside it is restatement.                                                                                                                                   |
| "This sibling crate's concept needs a paragraph here"                      | Link the defining crate. If the canonical doc is missing, write it there.                                                                                                                           |

## References

| file                       | load when                                                                                                                      |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `references/style.md`      | writing any summary or body: forms per item kind, compaction moves, the cut list with rewrites, links, words, house vocabulary |
| `references/templates.md`  | starting a crate page, module, item, error enum, test/bench/example file, manifest; drawing a diagram or table                 |
| `references/comments.md`   | writing `// SAFETY:`, `// ORDERING:`, inline `//`, `#[expect]` reasons, assertion and `expect` messages, tests, file headers   |
| `references/exemplars.md`  | unsure how a rule looks in practice: annotated excerpts from the three reference crates                                        |
| `references/tooling.md`    | the verify step: commands, the workspace's doc lints, cfg axes, links rustdoc cannot resolve, doctest attributes               |
| `references/sources.md`    | an external claim needs a citation: the Rust API Guidelines, RFC 1574, the rustdoc book, clippy, and what each backs           |
| `scripts/doc-audit.sh`     | the gate for one crate; `--lint-only --advisory` for the reread loop                                                           |
| `scripts/doc-lint.py`      | the cut list by machine, standalone                                                                                            |
| `scripts/doc-inventory.py` | the surface item by item, with sections present and gaps flagged                                                               |
