# `devset`

<!-- reference: written by `just fix-docs` from `devset --help` -->

```text
Apply versioned file bundles to a directory, and update them without losing local edits

Usage: devset [OPTIONS] <COMMAND>

Commands:
  init         Add a profile as a layer, and apply it
  remove       Remove a layer: its unchanged files go, and edited ones stay, untracked
  status       Show where every managed file stands against the profile
  diff         Show, line by line, how files differ from the profile
  apply        Apply the pinned profile without destroying local edits
  update       Move layers to what their refs name now, merging local edits
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
  devset init --git https://github.com/acme/profiles --tag v1.4.0 --path rust
  devset status
  devset update

Manual: https://atomix-labs.github.io/devset/
```
