# Gates and Scaffolds

A profile does real work: it scaffolds what a project lacks, wires up what it
has, and leaves alone what is not its business. Two things make that possible. A
**gate**, an entry's `when`, says when the entry applies. A **scaffold** is a
group of starter files, written once when the target has none of its own.

## Gates

An entry applies when every condition of its `when` holds:

```toml
[files."{{ book_dir }}/theme/katex.css"]
when = { features = ["katex"] }

[files."AGENTS.md"]
scope = "block"
when  = { exists = ["AGENTS.md"] }             # only where the project has one

[files."LICENSE-APACHE"]
when = { vars = { license = ["Apache-2.0", "MIT OR Apache-2.0"] } }
```

| Condition  | Holds when                                                              |
| ---------- | ----------------------------------------------------------------------- |
| `features` | every listed feature of this profile is on                              |
| `profiles` | every listed profile is active in the target                            |
| `vars`     | each listed variable of this profile is answered with one of its values |
| `exists`   | each path or glob will exist when the run is done                       |

**`exists` looks at the end of the run**, not the start: a path holds when it is
on disk and the run does not remove it, or when the run writes it, whole or as a
part; a directory holds when the run writes a file under it. So gates chain in
one run: one profile scaffolds `AGENTS.md`, and another's block attaches to it
in the same `apply`. A pattern with any of `*?[{` is a glob, `*` within a
directory and `**` across them, matched against the files no `.gitignore`
excludes, in a git repository or not; others are paths, and may use variables.

`exists` only ever turns entries on, so devset settles it the way that cannot
depend on order: every gated entry starts off, and each round turns on the ones
whose paths will exist, until no more do. An entry is never its own evidence:
its own path counts only as the disk and the other entries have it. The order of
the layers never matters.

There is no "if absent" condition. Content a project should get only when it has
none of its own is a scaffold, whose decision is recorded, so it never flips.

A gate is decided on disk. An `exists` on a file that is not committed decides
differently in CI, and `devset status --exit-code` shows that as drift.

## When a Gate Turns Off

An entry whose gate stops holding is released, as though its profile had stopped
shipping it: unchanged, it is removed, or a part taken out of its file; edited,
it is kept and untracked; a `once` file is untracked and left alone. `status`
and `--dry-run` say which condition decided:

```console
$ devset status
atxp/rust  (changed since it was applied)

Pending — `devset apply` will:
    dropped    book.toml         remove  (feature docs is off)
    dropped    guide.md          untrack  (feature docs is off)
```

`devset explain <path>` shows every layer that lists a file, its gates and how
each came out, and the decision of its scaffold:

```console
$ devset explain LICENSE-MIT
LICENSE-MIT
    house/project  file  owned  off: license is "Apache-2.0"
        when license is one of ["MIT", "MIT OR Apache-2.0"]: "Apache-2.0"
```

## Scaffolds

A scaffold is a named group of starter files, written together when none of its
sentinels is there:

```toml
[scaffolds.book]
unless = "{{ book_dir }}/book.toml"            # a path or a glob, or a list: any there means "found"

[files."{{ book_dir }}/book.toml"]
scaffold = "book"
template = true

[files."{{ book_dir }}/src/SUMMARY.md"]
scaffold = "book"
```

- **Decided once.** The first time the profile is applied, the group is
  `written` when none of its sentinels is there, and `found` when one is: the
  project has its own, and the group is left out. `state.toml` records the
  decision, and it holds: a sentinel that appears later changes nothing.
- **The target's once written.** A scaffold's files are `once`, so the project
  owns them: edit them, or delete them, and devset respects it.
- **Asked for again.** `devset apply --rescaffold book/book` writes the group's
  missing files again, whatever was decided, and adopts the ones that are there.
- **Where it was written.** `state.toml` records where each file went, so a
  changed answer that would move `{{ book_dir }}` moves no file a scaffold
  wrote.

A scaffold file may be a template, and gated: a file gated by a feature turned
on later is written then. A group is decided the first time one of its files
applies, so a group whose files are all gated off waits; one with no sentinel is
written then.

Scaffolds and [starters](parts.md#starters) are how a profile takes a project
from nothing: a workspace profile scaffolds `Cargo.toml`, and lint, release and
toolchain profiles splice their keys into it in the same run. Where the project
already has a `Cargo.toml`, the scaffold finds it, and the same keys go into the
project's own file.
