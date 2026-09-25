# `devset init`

<!-- reference: written by `just fix-docs` from `devset init --help` -->

```text
Add a profile as a layer, and apply it

Usage: devset init [OPTIONS]

Options:
      --dry-run  Show what would change; write nothing
  -h, --help     Print help

Source:
      --git <GIT>        Git repository URL
      --tag <TAG>        Tag to use
      --branch <BRANCH>  Branch to use
      --rev <REV>        Full commit id to use
      --path <PATH>      Profile directory: local, or within the repository with --git

Variables:
      --var <NAME=VALUE>  Answer a profile variable; repeatable

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset init --path ../profiles/base
  devset init --git https://github.com/acme/profiles --tag v1.4.0 --path rust
  devset init --git git@github.com:acme/profiles --branch main --var author=Ada
```
