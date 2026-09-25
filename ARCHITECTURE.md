# Architecture

How devset is built: its two crates, the chain every command runs, the rules the
chain keeps, where each part of it lives, and how it is tested. The [manual]
says what devset does; [CONTRIBUTING.md](CONTRIBUTING.md) says how to change it.

## Crates

| Crate         | Path              | Job                                                                    |
| ------------- | ----------------- | ---------------------------------------------------------------------- |
| `devset-core` | `lib/devset-core` | Everything devset decides and writes: profiles, targets, merges, state |
| `devset-cli`  | `bin/devset-cli`  | The `devset` binary: arguments, prompts, the report, and exit codes    |

The line between them is output. `devset-core` never prints and never prompts:
it returns what happened, and errors that carry what the command line needs to
explain them. `devset-cli` owns every sentence a person reads, so only it knows
its own flags, and the library serves another caller as well as it serves the
binary.

## The Chain

Every command is a prefix of one chain, and only its last step writes:

```text
resolve   the layers, fetched at their locked commits or refreshed, composed and answered
survey    every managed path: as the profile wants it, as recorded, and as on disk
plan      what committing does to each path, every merge made; nothing written yet
commit    every file, blob and record, written atomically, state last
```

| Step      | Takes                            | Gives      | Module       |
| --------- | -------------------------------- | ---------- | ------------ |
| `resolve` | a `Target`, a `Cache`, `Refresh` | `Resolved` | `resolve.rs` |
| `survey`  | `Resolved`, the `Target`         | `Survey`   | `survey.rs`  |
| `plan`    | `Survey`, a `Mode`, the `Target` | `Plan`     | `plan.rs`    |
| `commit`  | `Plan`, the `Target`             | its steps  | `commit.rs`  |

`status` and `diff` stop at `survey`; a dry run stops at `plan`; `apply`,
`update` and `init` run it all. `Refresh` says which layers move (none, one, or
all) and `Mode` what `plan` may do: never destroy bytes, restore `owned` files
too, or install resolved conflicts. `Rollback`, beside the chain, takes back an
unfinished update from what `commit` saved before it wrote.

## Rules

These hold everywhere, and a change that breaks one is wrong however it tests.

- **Decide everything before writing anything.** `plan` makes every decision,
  merges included, and `commit` only carries them out, so there is never a
  half-applied target. `commit` writes each file atomically, to a temporary file
  in its own directory, synced, then renamed over it; writes `state.toml` last;
  and holds `.devset/.lock` throughout, so two runs never interleave.
- **Never destroy bytes without `--force`.** Plain `apply` never overwrites an
  edit, or a file devset has no record of; a file adopted is recorded, not
  written.
- **Nothing is resolved silently.** Two layers that provide one file, layers
  that disagree on a setting, a conflict, drift: each is reported, and the
  target decides.
- **Store the bytes.** The merge base is what devset wrote, kept in
  `.devset/base/`, never reconstructed from an old version of the profile.
- **Check every merge.** A merged TOML, JSON or YAML file must parse with no
  duplicate key, or it is a conflict, however cleanly the lines merged.
- **Run no code from a profile.** A profile is data. A merge driver runs only
  when the target names it, and then without a shell.
- **Use the user's own `git`.** Git sources are fetched by running `git`, so
  credentials, proxies and `insteadOf` rules behave as they do for the user.

## `devset-core`

| Module     | Holds                                                                  |
| ---------- | ---------------------------------------------------------------------- |
| `profile`  | What `profile.toml` declares: files, variables, requirements, settings |
| `target`   | The directory devset manages, and what its `.devset/` records          |
| `source`   | Where a layer comes from, a directory or a git commit, and the cache   |
| `git`      | Git sources, read through `git` into a bare repository per URL         |
| `resolve`  | The layers fetched, composed and answered into one `Resolved` profile  |
| `vars`     | Template variables: declared, answered, and rendered into payloads     |
| `settings` | Settings a profile defaults and a target overrides                     |
| `survey`   | The target compared with its resolved profile, path by path            |
| `digest`   | Content digests, exact and canonical, and file fingerprints            |
| `plan`     | What committing does to each path, merges included                     |
| `merge`    | Three-way merges, built in or through a configured driver              |
| `format`   | Checking a merged file: it parses, with no duplicate key               |
| `part`     | Parts of a file: keys in TOML, JSON and YAML, and marked blocks        |
| `commit`   | Carrying out a plan: files, blobs and records, state last              |
| `rollback` | Taking back an unfinished update                                       |
| `path`     | Managed paths, checked to stay inside their root on every filesystem   |
| `tree`     | File sets held in one buffer                                           |
| `errors`   | Why a command did not go through, one type per domain                  |

`profile`, `target`, `source`, `resolve`, `survey` and `plan` are public, with
the chain's functions and types at the crate root; the rest is internal.

### Errors

`Error` has one variant per domain: `Path`, `Source`, `Profile`, `Target`, `Var`
and `Merge`, with `Parse` for a file that does not parse and `Io`. Each error's
message states the problem, and its fields carry the rest: the candidates for a
did-you-mean, the layers that collide, the span of a parse error. The command
line turns them into its `help:` line; the library never writes one.

## `devset-cli`

| Module   | Holds                                                                           |
| -------- | ------------------------------------------------------------------------------- |
| `main`   | The entry point: each command's handler, and its exit code                      |
| `cli`    | The command line: every command and argument, and its help                      |
| `ask`    | Answering variables: prompted in a terminal, defaulted otherwise                |
| `report` | What happened and where things stand: the apply log, `status`, its JSON, `diff` |
| `github` | Under GitHub Actions: annotations on the pull request, and a job summary        |
| `help`   | The next step to suggest for each error                                         |
| `shell`  | Terminal output: status lines, notes and errors, quiet or not                   |
| `words`  | Phrasing the report and the help share: counts, lists, near misses              |

Output follows cargo's conventions, with the crates cargo uses: the result on
stdout, everything else on stderr, verbs aligned on the right in cargo's
colours, and errors rendered by `annotate-snippets` as rustc renders its own.
[Output and Exit Codes][output] in the manual is what a user can rely on.

What others parse is an interface: `status --json`, the exit codes, and the two
files people write, whose schemas `devset schema` prints. A change to any of
them is breaking, as [CONTRIBUTING.md](CONTRIBUTING.md) says.

## Tests

- **Unit tests** sit beside the code they test: merges, parts, digests, paths,
  plans, rollbacks. The canonical digest has property tests.
- **End-to-end tests** run the `devset` binary, as one test binary in
  `bin/devset-cli/tests/cli/`, a file per area. Each test runs in a `Sandbox`
  with its own `HOME`, cache and git configuration, and checks its log, the
  commands with their output and exit codes, against a snapshot in `snapshots/`.
  `SNAPSHOTS=overwrite` rewrites the snapshots a change meant to change; the
  diff is then the review.
- **The crate's documentation** runs the chain in `lib.rs`'s example.
- **The manual's generated pages**, the command reference and the schemas, are
  checked against the build by `just check-docs`.

[manual]: https://atomix-labs.github.io/devset/
[output]: https://atomix-labs.github.io/devset/reference/output.html
