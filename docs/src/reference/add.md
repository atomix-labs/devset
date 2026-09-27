# `devset add`

<!-- reference: written by `just fix-docs` from `devset add --help` -->

```text
Add a profile as a layer, or features to one, and apply it.

Where there is no target yet, `add` starts one: at a git repository's top level, or in an empty
directory; anywhere else, run `devset init` first.

Usage: devset add [OPTIONS] <LAYER|--git <GIT>|--path <PATH>>

Arguments:
  [LAYER]
          The profile, `source/profile`; with --git or --path, `profile` alone names it in the
          source they name

Options:
  -h, --help
          Print help (see a summary with '-h')

Source:
      --git <GIT>
          Git repository URL

      --tag <TAG>
          Tag to use

      --branch <BRANCH>
          Branch to use

      --rev <REV>
          Full commit id to use

      --path <PATH>
          A local directory; with --git, the directory in the repository its profiles are in

Features:
  -F, --features <FEATURES>
          Features to turn on, beside the default ones; comma-separated or repeated

      --no-default-features
          Leave the profile's default features off

      --default-features
          Turn a layer's default features back on

Variables:
      --var <NAME=VALUE>
          Answer a profile variable; repeatable

      --dry-run
          Show what would change; write nothing

Global Options:
  -q, --quiet
          Print only results and errors

      --no-input
          Never prompt; fail with the flags to pass instead

      --no-color
          Never colour output

Examples:
  devset add atxp/mdbook                      from a source the target names
  devset add atxp/mdbook --features katex     with features beside the defaults
  devset add atxp/mdbook --features mermaid   a feature, to a layer already applied
  devset add house/deploy --git git@github.com:acme/profiles --branch main
  devset add --path ../profiles/base          a source holding one profile
```
