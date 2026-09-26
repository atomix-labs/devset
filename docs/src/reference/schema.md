# `devset schema`

<!-- reference: written by `just fix-docs` from `devset schema --help` -->

```text
Print the JSON Schema of a devset file, for editor completion

Usage: devset schema [OPTIONS] <FILE>

Arguments:
  <FILE>
          Which file

          Possible values:
          - profile:    A profile's `profile.toml`
          - config:     A target's `.devset/config.toml`
          - collection: A source's `collection.toml`

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
```
