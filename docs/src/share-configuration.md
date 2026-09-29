# Share a Team's Configuration

A team keeps one collection of profiles for all its repositories, and each
repository takes it by tag. The collection can stand alone, or build on another
collection's profiles and add the team's own.

## Build on Another Collection

A profile requires another by name; with `git`, from another source. One tag
pins that whole source for every profile that requires it:

```toml
# profiles/house/profile.toml, in the team's collection
[profile]
name        = "house"
description = "Our repositories: atxp's rust bundle with the book, and our deploy workflow"
devset      = ">=0.5"

[requires]
rust = { git = "https://github.com/atomix-labs/atxp", tag = "v0.16.0", features = ["docs"] }

[files.".github/workflows/deploy.yml"]
```

A repository takes one layer, `devset add team/house --git <url> --tag <tag>`,
and gets every profile `rust` requires as a layer of its own, before `house`, so
the team's files come last and its choices stand. Moving atxp forward is one
change, in the team's collection, and a release of it.

## Where the Team Differs

Formatting, lints and CI are taste. Where the team's differs from a collection
it builds on, it has three ways, from the lightest:

- **Answer a variable**: each repository's answer, `--var line_width=120`, is
  kept in its `.devset/answers.toml`.
- **Turn a feature off**: `default-features = false` on the requirement, then
  the features the team wants.
- **Swap a profile**: require the profiles the team agrees with rather than the
  whole bundle, and ship a profile of its own for the concern it sees
  differently.

## Per Repository

Each repository still decides for itself: its answers, and in
`.devset/config.toml` the features on each layer, which layer provides a file
(`from`), and a file's policy, `once` to take a file over entirely.
[Settings](settings.md) has every override. A repository's own edits in `merge`
files and outside a profile's parts stay its own through every update.

## Rolling Out a Release

Each repository moves when it takes the new tag: `devset update team --tag
<release>`. A scheduled job can do it for each, and open a pull request with the
checks' verdict: [Automate Updates](automate-updates.md). To change many
repositories at once by hand, run devset from a tool such as
[multi-gitter](https://github.com/lindell/multi-gitter).
