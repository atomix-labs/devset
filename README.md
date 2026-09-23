# devset

devset applies a versioned bundle of files into a directory, records exactly
what it wrote, and updates it later without destroying local edits.

Keep formatter settings, lint rules, dependency policy, editor config and CI
workflows in one place, shared by every repository, while each keeps the local
content it legitimately needs.

## Install

From a checkout, with Rust 1.89 or later:

```sh
cargo install --path bin/devset
```

Git sources need `git` on `PATH`. Local directories, and targets that are not
repositories, need nothing else.

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
| `devset status [--exit-code] [--json] [-v]`                           | Compare every managed file with the profile                      |
| `devset apply [--force] [--dry-run]`                                  | Apply without destroying edits; `--force` restores `owned` files |
| `devset update [<name>] [--dry-run]`                                  | Move layers to what their refs name now, merging local edits     |
| `devset update --continue`                                            | Install conflicts resolved in `.devset/conflicts/`               |
| `devset schema <profile\|config>`                                     | JSON Schema, for editor completion                               |

`init`, `apply` and `update` take `--var NAME=VALUE`, and `--dry-run`.

devset never destroys bytes without `--force`. A file that already exists when a
profile is first applied is adopted, not overwritten: `status` then shows how it
differs from the profile. Reformatting (whitespace, line endings, a BOM) is
never mistaken for an edit.

Global flags: `-q` for results and errors only, `--no-input` to never prompt,
and `--no-color` (`NO_COLOR` works too).

## In CI

```yaml
- run: devset status --exit-code
```

It fails exactly when `devset apply --force` would change a file. Under GitHub
Actions, drift is also annotated on the pull request's files, and summarized on
the job page.

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

devset asks for anything unanswered when run in a terminal; elsewhere, pass
`--var author=Ada`. Answers are kept in `.devset/answers.toml`, which you
commit, and changing one updates the files that use it.

## Merging

`update` merges a `merge` file's local edits with the profile's new version,
then checks the result parses with no duplicate keys: TOML, YAML, and JSON with
comments, by extension. A conflict, or a merge that fails that check, waits in
`.devset/conflicts/<path>` with the working file untouched:

```sh
devset update                          # exit 1: deny.toml conflicts
$EDITOR .devset/conflicts/deny.toml    # fix the marked hunks
devset update --continue               # checks, installs, records the new base
```

Deleting `.devset/conflicts/` abandons the update.

## Settings

A profile sets defaults; `.devset/config.toml` overrides them, and a layer's
`[files]` entries too:

```toml
[merge]
on-conflict = "apply-none"   # default "apply-others": apply clean files anyway
driver      = "mergiraf merge --git %O %A %B -p %P"   # default: a built-in line merge

[files."deny.toml"]
policy = "once"

[files."rustfmt.toml"]
from = "base"                # when several layers provide it, which one wins
```

A driver runs without a shell, on git's placeholders. A profile may suggest one;
devset tells you, but never runs a program a profile names.

## State

`.devset/` holds everything devset keeps; commit it. `config.toml` lists your
layers, `lock.toml` pins each to an exact revision, and `state.toml` with
`base/` records what devset last wrote. devset never edits your `.gitignore` or
`.gitattributes`: its own rules live in `.devset/`.

## Out of Scope

Installing tools: ship a `mise.toml`. Running tasks: ship a `justfile`. Changing
many repositories at once: run devset from `multi-gitter`. devset never executes
code from a profile.

## License

MIT or Apache-2.0, at your option.
