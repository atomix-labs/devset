# `devset update`

<!-- reference: written by `just fix-docs` from `devset update --help` -->

```text
Move layers to what their refs name now, merging local edits

Usage: devset update [OPTIONS] [LAYER]

Arguments:
  [LAYER]  Only the layer whose profile has this name

Options:
      --continue  Install the conflicts resolved in .devset/conflicts/
      --abort     Take back the unfinished update: every file it wrote, the lock and the state
      --force     With --abort, also discard changes made since the update
      --dry-run   Show what would change; write nothing
  -h, --help      Print help

Variables:
      --var <NAME=VALUE>  Answer a profile variable; repeatable

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset update              every layer
  devset update rust         one layer, by its profile name
  devset update --continue   after resolving .devset/conflicts/
  devset update --abort      take back an update that conflicted
```
