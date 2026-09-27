# Publish a Collection

A collection is a git repository of profiles that other repositories take by
tag. Anyone can keep one, a person, a team or a company, for their own tastes
and conventions; [atxp](https://github.com/atomix-labs/atxp), provided by Atomix
Labs, is one.

## Lay It Out

```sh
devset init profiles-repo --collection
```

`init --collection` writes `collection.toml`, a README and a first profile under
`profiles/`:

```text
profiles-repo/
  collection.toml          [collection] name and description
  README.md
  profiles/
    example/
      profile.toml
      README.md
      files/
```

The collection's `name` is what a target calls the source unless it chooses
another: `devset add acme/lint --git <url>` and `devset add lint --git <url>`
both name it `acme` when `collection.toml` says `name = "acme"`. Profiles may
sit in folders of their own under `profiles/`, grouped by concern; a profile is
found by its `name`, wherever it is. `devset list --path profiles-repo` shows
what a target would see.

## Release It

A tag is a release: every profile of the collection at one commit. Semantic
versions tell a target what a move costs:

- a **patch** changes nothing a target must act on;
- a **minor** release, while the major is 0, may break a target, and says how in
  a note of migration;
- a **major** release, from 1.0, is the break.

```sh
git tag -s v0.1.0 && git push origin v0.1.0
```

A target pins the tag, and `devset update` names each newer release, so a tag
once pushed stays: move forward with a new one.

## Say What It Holds

Each profile's README says what it does and why; a collection's README lists
them. atxp's `devset-collection` profile writes both from the manifests, a
catalog in the collection's README and a section of facts in each profile's, and
fails its check when either is stale.

## Let People Find It

Tag the repository with the GitHub topic `devset-collection`, so
[a search](https://github.com/topics/devset-collection) finds it. Say in the
README what taste the profiles hold, and how to take one:

```sh
devset add acme/lint --git https://github.com/acme/profiles --tag v0.1.0
```
