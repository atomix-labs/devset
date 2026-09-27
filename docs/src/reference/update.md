# `devset update`

<!-- reference: written by `just fix-docs` from `devset update --help` -->

```text
Move sources to newer commits, or to another tag, merging local edits.

A source pinned to a tag stays there: `update` names the releases newer than it, and `--tag`,
`--branch` or `--rev` moves it.

Usage: devset update [OPTIONS] [NAME]

Arguments:
  [NAME]
          Only the source with this name, or the source of the layer with this name

Options:
  -h, --help
          Print help (see a summary with '-h')

Move the source:
      --tag <TAG>
          To this tag

      --branch <BRANCH>
          To this branch

      --rev <REV>
          To this commit, by its full id

      --dry-run
          Show what would change; write nothing

Variables:
      --var <NAME=VALUE>
          Answer a profile variable; repeatable

Global Options:
  -q, --quiet
          Print only results and errors

      --no-input
          Never prompt; fail with the flags to pass instead

      --no-color
          Never colour output

Examples:
  devset update                    every source
  devset update atxp               one source, or the source of one layer
  devset update atxp --tag <tag>   move a source to another release
  devset update --dry-run          what would change
```
