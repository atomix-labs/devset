# `devset`

<!-- reference: written by `just fix-docs` from `devset --help` -->

```text
Apply versioned file bundles to a directory, and update them without losing local edits

Usage: devset [OPTIONS] <COMMAND>

Commands:
  init         Make a directory a target, or a profile or a collection to author
  add          Add a profile as a layer, or features to one, and apply it
  remove       Remove a layer, or features from one: unchanged files go, edited ones stay
  apply        Apply the pinned profiles without destroying local edits
  update       Move sources to newer commits, or to another tag, merging local edits
  status       Show where every managed file stands against the profile
  diff         Show, line by line, how files differ from the profile
  explain      Show why a layer's features are on, or why a file is managed as it is
  list         List the profiles a source holds, with their features
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
  devset add atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.20.0
  devset add atxp/mdbook --features katex
  devset status
  devset update --dry-run

Manual: https://atomix-labs.github.io/devset/
```
