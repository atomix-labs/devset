# `devset remove`

<!-- reference: written by `just fix-docs` from `devset remove --help` -->

```text
Remove a layer, or features from one: unchanged files go, edited ones stay

Usage: devset remove [OPTIONS] <LAYER>

Arguments:
  <LAYER>  The layer, by its profile's name

Options:
      --dry-run  Show what would change; write nothing
  -h, --help     Print help

Features:
  -F, --features <FEATURES>  Only these features, which the layer lists; the layer stays.
                             Comma-separated or repeated

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset remove mdbook                      by its profile's name
  devset remove mdbook --features mermaid   a feature, keeping the layer
  devset remove mdbook --dry-run            what would change
```
