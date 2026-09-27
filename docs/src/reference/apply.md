# `devset apply`

<!-- reference: written by `just fix-docs` from `devset apply --help` -->

```text
Apply the pinned profiles without destroying local edits.

A run that conflicted, of any command, is finished with `--continue`, once the files in
.devset/conflicts/ are resolved, or taken back with `--abort`.

Usage: devset apply [OPTIONS]

Options:
      --force
          Also restore `owned` files that were edited, deleted or never recorded; with --abort, also
          discard changes made since the conflicted run

      --rescaffold <PROFILE/GROUP>
          Write the missing files of a scaffold again, `profile/group`; repeatable

      --continue
          Install the conflicts resolved in .devset/conflicts/

      --abort
          Take back the run that conflicted: every file it wrote, the lock and the state

      --dry-run
          Show what would change; write nothing

  -h, --help
          Print help (see a summary with '-h')

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
  devset apply --dry-run                 what would change
  devset apply --force                   also restore drifted owned files
  devset apply --rescaffold mdbook/book  write a scaffold's missing files again
  devset apply --continue                after resolving .devset/conflicts/
  devset apply --abort                   take back what the conflicted run wrote
```
