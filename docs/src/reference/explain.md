# `devset explain`

<!-- reference: written by `just fix-docs` from `devset explain --help` -->

```text
Show why a layer's features are on, or why a file is managed as it is.

For a layer, its features and who turned each on, and those it leaves off; for a file, each layer
that lists it, how it manages it, and its gates.

Usage: devset explain [OPTIONS] [NAME]

Arguments:
  [NAME]
          A layer, by its profile's name, or a file, from the current directory: `./name` for a file
          a layer's name matches

Options:
  -h, --help
          Print help (see a summary with '-h')

Global Options:
  -q, --quiet
          Print only results and errors

      --no-input
          Never prompt; fail with the flags to pass instead

      --no-color
          Never colour output

Examples:
  devset explain                  every layer, with its features and who turned each on
  devset explain mdbook           one layer, with the features it leaves off
  devset explain docs/book.toml   one file: each layer that lists it, and its gates
```
