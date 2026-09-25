# Output and Exit Codes

What devset prints, where, and what its exit code means; the same for every
command.

## Where Output Goes

- **stdout** has the result: `status` and its JSON, `diff`, `schema`,
  `completions`, and the log of what a command did or would do.
- **stderr** has everything else: errors, notes, a `help:` line with the next
  step, prompts, and a `Fetching <url>` line before devset reaches a remote.

So `devset status | grep edited` sees only the status, and a reader that stops
early, as `devset status | head` does, is not an error.

The log reads as cargo's does: a verb aligned on the right, past tense for what
happened, and a `Finished` summary. A dry run says `Would` instead, and ends
`nothing written`.

```text
   Created mise.toml
    Merged clippy.toml
Conflicted deny.toml  merged, but invalid: duplicate key at line 56
  Finished 2 changes, 1 conflict
```

## Errors

An error says what is wrong, then what fixes it. A file that does not parse is
shown with the offending line marked; a mistyped layer, variable or path gets a
did-you-mean, or else a list of what exists.

```text
error: no layer is named teem
  |
  = help: did you mean `team`?
```

## Global Options

| Option          | Does                                                                        |
| --------------- | --------------------------------------------------------------------------- |
| `-q`, `--quiet` | Print only results, conflicts and errors: no log, no notes, no fetch lines. |
| `--no-input`    | Never prompt; fail with the flags to pass instead.                          |
| `--no-color`    | Never colour output. Setting `NO_COLOR` does the same.                      |

devset prompts only when both stdin and stderr are terminals, and git may ask
for credentials only when devset may prompt.

## Exit Codes

| Code | Means                                                                                                  |
| ---- | ------------------------------------------------------------------------------------------------------ |
| `0`  | Success.                                                                                               |
| `1`  | A merge conflicted; or, under `status --exit-code`, the target has drifted or an update is unfinished. |
| `2`  | An error, invalid arguments included.                                                                  |
