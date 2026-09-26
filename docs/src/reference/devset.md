# `devset`

<!-- reference: written by `just fix-docs` from `devset --help` -->

```text
Apply versioned file bundles to a directory, and update them without losing local edits

Usage: devset [OPTIONS] <COMMAND>

Commands:
  new          Start a target in a new directory, or a profile or a collection to author
  init         Start a target in the current directory, with a first layer if given
  add          Add a profile as a layer, or features to a layer, and apply it
  remove       Remove a layer, or features from one: unchanged files go, edited ones stay
  status       Show where every managed file stands against the profile
  diff         Show, line by line, how files differ from the profile
  apply        Apply the pinned profile without destroying local edits
  update       Move sources to what their refs name now, merging local edits
  features     Show each layer's features: which are on, and who turned them on
  explain      Show why a file is managed as it is: each layer that lists it, and its gates
  list         List the profiles a source holds, with their features
  schema       Print the JSON Schema of a devset file, for editor completion
  completions  Print a shell completion script
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset new hello atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.4.0
  devset add atxp/mdbook --features katex
  devset status
  devset update

Manual: https://atomix-labs.github.io/devset/
```
