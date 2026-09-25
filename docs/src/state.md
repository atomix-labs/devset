# State

Everything devset keeps about a target is in its `.devset/` directory. Commit
it: it is what lets a teammate, or CI, see exactly what you see, and what makes
`devset status` work on a fresh clone.

| Path             | Holds                                                        | In git  |
| ---------------- | ------------------------------------------------------------ | ------- |
| `config.toml`    | The layers, `[merge]` settings, and file overrides: yours    | commit  |
| `lock.toml`      | Each layer's commit and content digest                       | commit  |
| `answers.toml`   | The answers to the profiles' variables                       | commit  |
| `state.toml`     | What devset last wrote, per file and part: two digests each  | commit  |
| `base/`          | The bytes devset last wrote, one blob per content digest     | commit  |
| `.gitignore`     | devset's own ignore rules                                    | commit  |
| `.gitattributes` | Keeps `base/` out of diffs and line-ending conversion        | commit  |
| `conflicts/`     | An unfinished update: its sidecars, and what to take it back | ignored |
| `.lock`          | Held while devset writes, so two runs never interleave       | ignored |

## The Base

`base/` is what makes a real merge possible: the exact bytes devset last wrote
for each file, kept rather than reconstructed. Each blob is named by its own
digest, never by its path, so a profile's `.gitignore` or `.gitattributes` in
`base/` can never affect the rest, and identical files are stored once. A blob
nothing records any more is deleted when state is written.

A conflict in progress is your work in progress, as a rebase is, so `conflicts/`
stays out of version control.

## The Target's Own Files

devset never edits your `.gitignore` or `.gitattributes` for its own sake: its
rules are in `.devset/`, where git honours them, and where they are inert in a
target that is not a repository.

## Finding the Target

devset runs from anywhere inside a target: the target is the nearest directory,
upwards, that holds `.devset/`, found as Cargo finds `Cargo.toml`. Nothing needs
git, and each git worktree of a repository is a target of its own.

## The Cache

Git sources are fetched into a cache per machine, one bare repository per URL
under `$XDG_CACHE_HOME/devset/`, `~/.cache/devset/` by default, on Linux and
macOS alike, and never into a target. One fetch serves every target on the
machine, and a commit already there is never fetched again.
