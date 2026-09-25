# `devset apply`

<!-- reference: written by `just fix-docs` from `devset apply --help` -->

```text
Apply the pinned profile without destroying local edits

Usage: devset apply [OPTIONS]

Options:
      --force    Also restore `owned` files that were edited, deleted or never recorded
      --dry-run  Show what would change; write nothing
  -h, --help     Print help

Variables:
      --var <NAME=VALUE>  Answer a profile variable; repeatable

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset apply --dry-run   what would change
  devset apply --force     also restore drifted owned files
```
