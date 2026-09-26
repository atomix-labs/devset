# `devset features`

<!-- reference: written by `just fix-docs` from `devset features --help` -->

```text
Show each layer's features: which are on, and who turned them on

Usage: devset features [OPTIONS] [LAYER]

Arguments:
  [LAYER]  Only the layer whose profile has this name

Options:
  -h, --help  Print help

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset features          every layer
  devset features mdbook   one, with the features it leaves off
```
