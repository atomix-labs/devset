# `devset list`

<!-- reference: written by `just fix-docs` from `devset list --help` -->

```text
List the profiles a source holds, with their features

Usage: devset list [OPTIONS] [SOURCE]

Arguments:
  [SOURCE]  A source the target names

Options:
  -h, --help  Print help

Source:
      --git <GIT>        Git repository URL
      --tag <TAG>        Tag to use
      --branch <BRANCH>  Branch to use
      --rev <REV>        Full commit id to use
      --path <PATH>      A local directory; with --git, the directory in the repository its profiles
                         are in

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset list                                   every source the target names
  devset list atxp                              one of them
  devset list --git https://github.com/atomix-labs/atxp --tag v0.4.0
```
