# `devset remove`

<!-- reference: written by `just fix-docs` from `devset remove --help` -->

```text
Remove a layer: its unchanged files go, and edited ones stay, untracked

Usage: devset remove [OPTIONS] <LAYER>

Arguments:
  <LAYER>  The layer, by its profile's name

Options:
      --dry-run  Show what would change; write nothing
  -h, --help     Print help

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset remove mdbook             by its profile's name
  devset remove mdbook --dry-run   what would change
```
