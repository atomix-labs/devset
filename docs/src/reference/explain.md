# `devset explain`

<!-- reference: written by `just fix-docs` from `devset explain --help` -->

```text
Show why a file is managed as it is: each layer that lists it, and its gates

Usage: devset explain [OPTIONS] <PATH>

Arguments:
  <PATH>  The file, from the current directory

Options:
  -h, --help  Print help

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset explain docs/book.toml
```
