# `devset new`

<!-- reference: written by `just fix-docs` from `devset new --help` -->

```text
Start a target in a new directory, or a profile or a collection to author

Usage: devset new [OPTIONS] <DIR> [LAYER]

Arguments:
  <DIR>    The directory to create it in
  [LAYER]  The profile, `source/profile`; with --git or --path, `profile` alone names it in the
           source they name

Options:
      --profile     Create a profile to author, not a target
      --collection  Create a collection of profiles to publish, not a target
  -h, --help        Print help

Source:
      --git <GIT>        Git repository URL
      --tag <TAG>        Tag to use
      --branch <BRANCH>  Branch to use
      --rev <REV>        Full commit id to use
      --path <PATH>      A local directory; with --git, the directory in the repository its profiles
                         are in

Features:
  -F, --features <FEATURES>  Features to turn on, beside the default ones; comma-separated or
                             repeated
      --no-default-features  Leave the profile's default features off

Variables:
      --var <NAME=VALUE>  Answer a profile variable; repeatable
      --dry-run           Show what would change; write nothing

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset new hello                    a target: hello/.devset/config.toml, to add layers to
  devset new hello atxp/rust --git https://github.com/atomix-labs/atxp --tag v0.4.0
  devset new --profile my-lint        a profile: profile.toml, files/ and a README
  devset new --collection acme        a source of profiles: collection.toml and profiles/
```
