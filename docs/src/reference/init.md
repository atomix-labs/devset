# `devset init`

<!-- reference: written by `just fix-docs` from `devset init --help` -->

```text
Make a directory a target, or a profile or a collection to author

Usage: devset init [OPTIONS] [PATH]

Arguments:
  [PATH]  The directory, created if missing; this one if not given

Options:
      --profile     Make it a profile to author, not a target
      --collection  Make it a collection of profiles to publish, not a target
      --dry-run     Show what would change; write nothing
  -h, --help        Print help

Global Options:
  -q, --quiet     Print only results and errors
      --no-input  Never prompt; fail with the flags to pass instead
      --no-color  Never colour output

Examples:
  devset init                        this directory, a target to add layers to
  devset init hello                  a new directory, hello/, as a target
  devset init my-lint --profile      a profile: profile.toml, files/ and a README
  devset init acme --collection      a source of profiles: collection.toml and profiles/
```
