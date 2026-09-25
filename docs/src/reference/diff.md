# `devset diff`

<!-- reference: written by `just fix-docs` from `devset diff --help` -->

```text
Show, line by line, how files differ from the profile

Usage: devset diff [OPTIONS] [PATHS]...

Arguments:
  [PATHS]...  Only these files

Options:
  -h, --help  Print help

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset diff             every file that differs
  devset diff deny.toml   one file
```
