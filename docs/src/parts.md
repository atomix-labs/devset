# Parts of a File

A profile may own part of a file the target otherwise owns: the lint tables of a
`Cargo.toml`, a few keys of `.vscode/settings.json`, one stack's lines in a
`.gitignore`. `scope` on the file's entry says how much:

| Scope   | The profile owns                            | For                                      |
| ------- | ------------------------------------------- | ---------------------------------------- |
| `file`  | The default. The whole file.                | Configurations with one owner            |
| `keys`  | Every key its payload defines               | TOML, JSON and JSONC; YAML, experimental |
| `block` | The lines between two comments that mark it | `.gitignore`, `CODEOWNERS`, READMEs      |

`status` names a part by its file and its profile, as `Cargo.toml [keys: rust]`.

## Keys

The payload of a `keys` part is a partial document: the keys the profile owns,
and nothing else.

```toml
# profile.toml
[files."Cargo.toml"]
scope = "keys"
```

```toml
# files/Cargo.toml
[workspace.lints.clippy]
pedantic    = { level = "deny", priority = -1 }
unwrap_used = "deny"
```

The profile owns each **leaf** its payload defines. In TOML, a leaf is any value
but a `[table]`: an inline table, as `pedantic` is here, and an array of tables
are leaves whole. In JSON and YAML, a leaf is any value but an object. Here the
leaves are `workspace.lints.clippy.pedantic` and
`workspace.lints.clippy.unwrap_used`. Every other key is the target's, including
keys it adds to the same tables.

- **Values are compared after parsing**, so reformatting is never drift, and
  only the keys that change are rewritten: comments and layout elsewhere in the
  file survive.
- **A key the profile adds or changes** is written as its payload writes it, an
  array of tables with its comments; a file the part creates is the payload as
  written.
- **A file that already holds some of the keys** is adopted key by key: its
  values are kept, and the keys it lacks are written.
- **An update merges key by key**, and a conflict marks only the keys both sides
  changed.

YAML is experimental: only the first document of a stream is managed.

## Blocks

A block is the lines between two comments, in the file's own comment syntax,
named after the profile:

```gitignore
/deployments/local/*

# >>> devset: rust >>>
/target
# <<< devset: rust <<<
```

A new block goes at the end of the file, and stays wherever you move it. devset
knows the comment syntax of common files by name, as `.gitignore`, `justfile`
and `Dockerfile`, or by extension; where it does not, `comment` on the entry
names it: `#`, `//`, `--`, `;`, `%`, `/*` or `<!--`. The last two close their
markers, as `<!-- >>> devset: badges >>> -->`.

## Several Parts in One File

Several layers may own parts of one file, in one scope, so long as no two own
the same key. With every part written, the file must still parse, with no
duplicate keys; if it does not, the change waits as a conflict, and the file is
left as it was.

## Starters

A part of a file the target does not have yet starts it. With nothing else, the
file holds the part alone; a **starter** gives it the rest, once:

- **A whole `once` file** of another layer, or a
  [scaffold](gates.md#scaffolds)'s file, on the same path starts the file, and
  every layer's part is spliced into it in the same run. One profile scaffolds
  `Cargo.toml`, others add their lint tables and release profile to it, and the
  file is written once, whole.
- **A part's own starter**, `starter` on its entry, names a file under `files/`
  the target's file starts from when it is absent:

  ```toml
  [files."docs/book.toml"]
  scope   = "keys"
  starter = "book.starter.toml"   # files/book.starter.toml: the rest of a new book.toml
  ```

When the file is there, the parts go into it as it is, and no starter is used. A
starter is `once`: the target's as soon as it is written, so what the parts
change in it is never an edit, and a starter written again, by `--rescaffold`,
takes every part with it. A file has one starter: two, whole or a part's own,
collide, and `from` names the layer whose starter it is, every layer's part
staying.
