# `devset status`

<!-- reference: written by `just fix-docs` from `devset status --help` -->

```text
Show where every managed file stands against the profile

Usage: devset status [OPTIONS]

Options:
      --exit-code  Exit 1 when `apply --force` would write a file, or an update is unfinished
      --json       Print JSON
  -v, --verbose    Also list files that match, and the settings in force
  -h, --help       Print help

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset status               what needs doing
  devset status -v            and everything in sync
  devset status --exit-code   in CI: fail on drift
```
