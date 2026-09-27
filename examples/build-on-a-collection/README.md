# Build on a Collection

A team's collection that builds on another's: its `house` profile requires
`lint` from `upstream`, pinned by a tag, and adds a workflow of the team's own.
`upstream` stands for any collection another person or team keeps, such as
[atxp](https://github.com/atomix-labs/atxp); here it is a local git repository.
[Share a Team's Configuration](https://atomix-labs.github.io/devset/share-configuration.html)
says more.

```toml
# team/profiles/house/profile.toml
[requires]
lint = { git = "../upstream", tag = "v1.0.0" }

[files.".github/workflows/deploy.yml"]
```

Run these from this directory, or a copy of it. The first four make `upstream` a
git repository with a release, as a published collection is:

```console
$ git -C upstream init -q
$ git -C upstream add -A
$ git -C upstream commit -qm 'lint 1.0.0'
$ git -C upstream tag v1.0.0
$ git init -q repo
$ cd repo
$ devset add team/house --path ../team --var line_width=120
    Starting a target: .devset/config.toml
    Fetching ../upstream
     Created .editorconfig
     Created .github/workflows/deploy.yml
     Created rustfmt.toml
    Finished 3 changes
$ devset status
team/house
  requires lint

All 3 files match the profile.
$ cat rustfmt.toml
max_width = 120
```

The repository took one layer, and got `lint` as a layer of its own, before
`house`, answering its `line_width`. Moving `upstream` forward is one change to
the tag in the team's `house`, and a release of the team's collection.
