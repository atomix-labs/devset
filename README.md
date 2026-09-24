# devset

devset applies a versioned bundle of files into a directory, records exactly what it wrote, and
updates it later without destroying local edits.

Keep formatter settings, lint rules, dependency policy, editor config and CI workflows in one place,
shared by every repository, while each keeps the local content it legitimately needs.

## Install

From a checkout; `rustup` installs the pinned toolchain on first use:

```sh
cargo install --path bin/devset
```

The build runs on any machine of its architecture: `.cargo/config.toml` sets the CPU floors every
realistic machine meets (x86-64-v2, CRC32 on Arm, the first Apple silicon), so a build cached across
machines, as CI runners share one, never holds an instruction another lacks. A build tuned to this
machine sets `RUSTFLAGS="-C target-cpu=native"`, which replaces the floors.

Git sources need `git` on `PATH`. Local directories, and targets that are not repositories, need
nothing else.

## Profiles

A profile is a directory: `profile.toml`, and `files/` mirroring the target.

```text
rust/
  profile.toml
  README.md                  # the profile's own docs; never applied
  files/
    rustfmt.toml
    deny.toml
    .github/workflows/ci.yml
```

```toml
[profile]
name    = "rust"
version = "1.4.0"
devset  = ">=0.1"

[files."rustfmt.toml"]
[files.".github/workflows/ci.yml"]

[files."deny.toml"]
policy = "merge"
```

| Policy  | Meaning                                                           |
| ------- | ----------------------------------------------------------------- |
| `owned` | The default. The profile is authoritative; local edits are drift. |
| `merge` | Local edits are kept.                                             |
| `once`  | Written if absent; the target's from then on.                     |

## Parts of a File

A profile may own part of a file the repository otherwise owns: the lint tables of a `Cargo.toml`, a
few keys of `.vscode/settings.json`, one stack's lines in a `.gitignore`. `scope` on its entry says
how much:

| Scope   | The profile owns                            | For                                      |
| ------- | ------------------------------------------- | ---------------------------------------- |
| `file`  | The default. The whole file.                | Configs with one owner                   |
| `keys`  | Every key its payload defines               | TOML, JSON and JSONC; YAML, experimental |
| `block` | The lines between two comments that mark it | `.gitignore`, `CODEOWNERS`, READMEs      |

```toml
# profile.toml
[files."Cargo.toml"]
scope = "keys"
```

```toml
# files/Cargo.toml: a partial document
[workspace.lints.clippy]
pedantic    = { level = "deny", priority = -1 }
unwrap_used = "deny"
```

Every other key is the repository's, including keys it adds to the same tables. Values are compared
after parsing, so reformatting is never drift, and only the keys that change are rewritten: comments
and layout elsewhere survive. A file the part creates is the payload as written. A file that already
holds some of the keys is adopted key by key, its values kept and the missing keys written. An
update merges key by key, and a conflict marks only the keys both sides changed.

A block is marked in the file's own comment syntax, and named after its profile:

```gitignore
/deployments/local/*

# >>> devset: rust >>>
/target
# <<< devset: rust <<<
```

A new block goes at the end of the file, and stays wherever you move it. Where devset does not know
a file's comment syntax, `comment` names it: `#`, `//`, `--`, `;`, `%`, `/*` or `<!--`.

Several layers may own parts of one file, in one scope, so long as no two own the same key. With
every part written, the file must still parse, with no duplicate keys; if it does not, it waits as a
conflict.

## Building on Profiles

```toml
[profile]
name     = "rust"
requires = [
    "../rustfmt",                                                            # beside this one
    { git = "https://github.com/acme/profiles", tag = "v2.0.0", path = "deny" },
]
```

A string is a profile beside this one, from the same source at the same revision; a table is a git
source, written as a layer is. Each required profile becomes a layer of its own, before the profile
that requires it, and `status` shows what brought it in. A profile with only `requires` is a bundle.

## Usage

```sh
devset init --git https://github.com/acme/profiles --tag v1.4.0 --path rust
devset status
devset apply
```

| Command                                                               | Does                                                             |
| --------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `devset init --path <dir>`                                            | Add a local profile as a layer, and apply it                     |
| `devset init --git <url> [--tag \| --branch \| --rev] [--path <dir>]` | Add a git profile, pinned in the lock                            |
| `devset remove <name>`                                                | Remove a layer; its files stay, no longer tracked                |
| `devset status [--exit-code] [--json] [-v]`                           | Show where every managed file stands against the profile         |
| `devset diff [<path>…]`                                               | Show, line by line, how files differ from the profile            |
| `devset apply [--force]`                                              | Apply without destroying edits; `--force` restores `owned` files |
| `devset update [<name>]`                                              | Move layers to what their refs name now, merging local edits     |
| `devset update --continue`                                            | Install the conflicts resolved in `.devset/conflicts/`           |
| `devset update --abort [--force]`                                     | Take back an update that conflicted                              |
| `devset schema <profile\|config>`                                     | JSON Schema, for editor completion                               |
| `devset completions <shell>`                                          | Shell completions: bash, zsh, fish, elvish, powershell           |

`init`, `apply` and `update` take `--var NAME=VALUE`; everything that writes takes `--dry-run`. A
layer is named by its profile's `name`.

devset never destroys bytes without `--force`. A file that already exists when a profile is first
applied is adopted, not overwritten: `status` then shows how it differs from the profile.
Reformatting (whitespace, line endings, a BOM) is never mistaken for an edit.

Global flags: `-q` for results and errors only, `--no-input` to never prompt, and `--no-color`
(`NO_COLOR` works too).

## In CI

```yaml
- run: devset status --exit-code
```

It fails exactly when `devset apply --force` would change a file, or an update is unfinished. Under
GitHub Actions, drift is also annotated on the pull request's files, and summarized on the job page.
git never prompts for credentials there: give it a credential helper or an SSH key.

## Templates

A profile file marked `template = true` is rendered with the target's answers:

```toml
# profile.toml
[vars.author]
prompt = "Author name"

[vars.target_cpu]
default = "x86-64-v2"

[files."Cargo.toml"]
policy   = "once"
template = true
```

```toml
# files/Cargo.toml
[package]
authors = ["{{ author }}"]
```

devset asks for anything unanswered when run in a terminal; elsewhere, pass `--var author=Ada`.
Answers are kept in `.devset/answers.toml`, which you commit, and changing one updates the files
that use it.

## Scripts

A profile file marked `executable = true` is written with mode 755 wherever devset writes it, so a
script it ships runs as `./setup.sh`, and git records it executable. The mode is not part of what
devset records: changing it is not drift.

## Merging

`update` merges a `merge` file's local edits with the profile's new version, then checks the result
parses with no duplicate keys: TOML, YAML, and JSON with comments, by extension. A conflict, or a
merge that fails that check, waits in `.devset/conflicts/<path>` with the working file untouched:

```sh
devset update                          # exit 1: deny.toml conflicts
$EDITOR .devset/conflicts/deny.toml    # fix the marked hunks
devset update --continue               # checks, installs, records the new base
```

Until then the update is unfinished, and `apply` and `update` refuse to start another. `devset
update --abort` takes it back instead: every file it wrote, the lock and the state return to what
they were before it. Files changed since the update stop an abort, unless `--force` discards those
changes too.

A binary file never merges: its sidecar holds the profile's version, to keep or replace with yours.
A file whose recorded base is lost merges without one, and always waits for you.

## Settings

A profile sets defaults; `.devset/config.toml` overrides them, and a layer's `[files]` entries too:

```toml
[merge]
on-conflict = "apply-none"   # default "apply-others": apply clean files anyway
driver      = "mergiraf merge --git %O %A %B -p %P"   # default: a built-in line merge

[files."deny.toml"]
policy = "once"

[files."rustfmt.toml"]
from = "base"                # the one layer that provides it, when several do
```

A driver runs without a shell, on git's placeholders. A profile may suggest one; devset tells you,
but never runs a program a profile names.

## State

`.devset/` holds everything devset keeps; commit it. `config.toml` lists your layers, `lock.toml`
pins each to an exact revision, and `state.toml` with `base/` records what devset last wrote. An
unfinished update lives in `conflicts/`, which stays out of version control. devset never edits your
`.gitignore` or `.gitattributes`: its own rules live in `.devset/`.

When no layer provides a file, or a part, any more, devset removes it if it is as devset wrote it: a
part leaves its file, and a file left empty goes. One you edited, or one written under `once`, stays
in place, untracked.

## Develop

devset applies the [devset-profiles](https://github.com/atomix-labs/devset-profiles) collection to
itself: its `rust` bundle, with rustfmt's unstable options, as `devset` builds on a pinned nightly.
`./setup.sh` readies a checkout: the tools the lock pins, then the toolchain. `just check` runs
every check, as CI does, and `just fix` every fix.

## Out of Scope

Installing tools: ship a `mise.toml`. Running tasks: ship a `justfile`. Changing many repositories
at once: run devset from `multi-gitter`. devset never executes code from a profile.

## License

MIT.
