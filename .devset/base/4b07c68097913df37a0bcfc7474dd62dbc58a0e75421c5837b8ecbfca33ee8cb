---
name: writing-rustdoc
description: Use when writing, revising or reviewing rustdoc (`//!`, `///`), `// SAFETY:` and `// ORDERING:` comments, `#[expect]` reasons, or a crate's `Cargo.toml` description; when a crate, module, type, trait or `unsafe fn` is added or changed and its docs must follow; when asked to document a crate, make it docs.rs ready, or audit docs for restatement, filler or missing Safety, Errors and Examples sections. Not for Markdown outside Rust source.
---

# Writing Rustdoc

The reader is a Rust engineer with the code open beside the docs. Tell them only
what the code cannot: why an item exists, what it promises, what it assumes,
what it costs; and link the rest. Compact means no restatement, never less
content.

Load `references/style.md` and `references/templates.md` before writing;
`references/comments.md` before a `//` comment, an `#[expect]`, a test or a
manifest; `references/exemplars.md` to see a rule on the page;
`references/tooling.md` before the checks.

## Rules

1. **Only what the code, its tests, its manifest or its history say.** A sign
   convention, a unit, an ordering, a cost, an atomicity or durability promise
   that the code does not enforce or state is a guess, however plausible. Leave
   the sentence out and ask (step 4); never fill the gap.
2. **Nothing the code already says.** No summary that paraphrases the signature,
   no `# Arguments` or `# Returns`, no module doc listing its items, no comment
   narrating the next line. A sentence whose deletion loses nothing goes.
3. **Link, do not re-explain.** An item with a home is ``[`Name`]``; an external
   fact is a reference definition at the bottom of the block.
4. **The summary is one sentence**, on one line where it fits, in the form the
   item's kind takes (`references/style.md` §1): a function starts with a verb,
   a getter names the value, an error enum says why, a variant states the
   condition.
5. **Sections in the compact form**: the Examples, Errors, Panics and Safety
   sections start their content on the line after the heading, and every
   identifier is in backticks. An entry of an Errors section names the variant,
   then the condition, as the example below the rules shows.
6. **Every `unsafe` item carries its contract**: `# Safety` on an `unsafe fn` or
   trait, and a one-line `// SAFETY:` on each `unsafe` block and impl. Each
   atomic that is not `SeqCst` carries an `// ORDERING:` line.
7. **The manifest's `description` is the crate summary's pitch clause**, first
   letter lowercased, with a trailing period, never naming itself a crate. A
   crate to publish needs one: crates.io refuses a manifest without.
8. **No process residue**: no `TODO`, no ticket or phase names, no rule ids, no
   lint narration, no em dash.

```rust
/// Parses `input` as a port number.
///
/// # Errors
/// - [`PortError::Empty`], `input` is empty.
/// - [`PortError::Range`], the number does not fit a `u16`.
```

## Steps

1. **Read the whole crate**: its manifest, every module, `tests/`, `benches/`,
   `examples/`. Match the voice already there.
2. **Inventory the surface**: `scripts/doc-inventory.py`, run on the crate's
   directory, lists every item, its visibility and the sections it has.
3. **Find the facts**: existing comments, test names (often the property), the
   history (`git log -p -- <path>`), and the design documents that name it.
4. **Ask once for every gap left**, before writing: one message, a numbered
   list, each entry naming the item, the sentence with the gap marked, and the
   fact needed. Without an answer, write everything else and leave those
   sentences out.
5. **Write top down**: crate, modules, items, fields and variants, `// SAFETY:`
   and `// ORDERING:`, `#[expect]` reasons, tests and examples, then the
   manifest's `description`.
6. **Format, then tighten**: `just fix`, then for each paragraph whose last line
   holds one to three words, cut or move words until it does not.
7. **Check**, below, until everything passes.
8. **Report** in a few lines: what was written where, each command and its
   result, each question still open.

## Checks

```sh
just check-rust-doc                                     # rustdoc, private items included, warnings denied
python3 .just/rust-doc.py <crate-dir> --advisory        # the cut list, widows, summaries that wrap
.claude/skills/writing-rustdoc/scripts/doc-audit.sh <crate-dir>   # both, with formatting, doctests and clippy
```

Then read the rendered page, `target/<host>/doc/<crate>/index.html`: every
``[`Name`]`` a link, no heading over an empty paragraph, the crate page an
introduction a newcomer can start from.

Done means:

- Every public item, field and variant has a summary that says more than its
  name; every file but `lib.rs` opens with a one-line `//!`.
- Every `unsafe` item, block and impl, and every fallible item, has its section
  or comment in the compact form.
- Every public type a user builds or drives, and every substantial function, has
  a runnable `# Examples` that ends in an assertion; a trivial accessor has
  none.
- The crate page gives the pitch, then the problem or the shape, the tasks, and
  `# Crate features` where there are any.
- `description` is set; nothing on the cut list survives; the checks pass.

## What Not to Do

| Thought                                                  | Instead                                                                   |
| -------------------------------------------------------- | ------------------------------------------------------------------------- |
| "A negative offset probably means earlier; I'll say so"  | The code does not say it. Leave it out, and ask.                          |
| "The description is out of scope for docs"               | It is the crate's pitch on crates.io. Write it, or ask for it.            |
| "A blank line after `# Examples` is the convention"      | Not here: a section's content starts on the next line.                    |
| "One more clause makes the summary richer"               | Richer means more facts. A one-liner that names the role is finished.     |
| "`ignore` for now; the example needs setup"              | `no_run`, a `text` fence, or a smaller example. `ignore` is a defect.     |
| "Every export gets a `# Types` bullet"                   | Bullets are roles, three to five; the listing already names every export. |
| "I'll explain the `#[expect]` in a comment beside it"    | Its `reason` is its one home.                                             |
| "This signing path allocates nothing; the types show it" | A cost needs the code path or a measurement behind it. Say what shows it. |
| "The checks are slow; `cargo fmt` is enough"             | Run the checks above; they take seconds on a warm cache.                  |
