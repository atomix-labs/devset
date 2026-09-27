# Write a Profile

A profile is a directory: `profile.toml`, which says how devset manages each
file and when, and `files/`, which holds them as a target should have them. This
guide writes one, tries it on a scratch repository, and grows it a step at a
time. [Getting Started](getting-started.md) is the shorter first pass.

## Start

```sh
devset init profiles/lint --profile
```

`init --profile` writes `profile.toml`, a commented tour of what a manifest can
say, a README, and an empty `files/`. Put a file in `files/` at the path a
target should have it, and name it in the manifest:

```toml
[profile]
name        = "lint"
description = "The team's EditorConfig and rustfmt settings"
devset      = ">=0.4"

[files.".editorconfig"]

[files."rustfmt.toml"]
policy = "merge"
```

## Try It

A scratch repository takes the profile straight from its directory, so every
change is one `apply` away:

```sh
mkdir scratch && cd scratch && git init
devset add lint --path ../profiles/lint
devset status
```

A local source is read as it is now: edit the profile, run `devset apply`, and
the scratch repository takes the change, merging it with anything edited there.

## Choose a Policy per File

- `owned`, the default: the profile's; an edit is drift, which `status` reports
  and `apply --force` restores.
- `merge`: the target's edits are kept, and merged with the profile's changes.
- `once`: written where absent, then the target's.

[Profiles](profiles.md#policies) has what `apply` does with each.

## Own Part of a File

Where a target keeps its own content in a file, own only what the profile needs:
the keys its payload defines, or a marked block.

```toml
[files."Cargo.toml"]
scope = "keys"               # the [workspace.lints] the payload holds, no more

[files.".gitignore"]
scope = "block"              # a block between devset's markers
```

[Parts of a File](parts.md) says how each merges and what a starter is.

## Ask the Target

A variable is a value each target chooses, and a template uses it:

```toml
[vars.line_width]
prompt  = "Line width"
default = "100"

[files."rustfmt.toml"]
template = true              # files/rustfmt.toml holds max_width = {{ line_width }}
```

[Templates](templates.md) has the language and what else a template can read.

## Offer Features

A feature turns on files, requirements or other features; a gate says when an
entry applies:

```toml
[features]
default = ["deny"]
deny    = []

[files."deny.toml"]
when = { features = ["deny"] }
```

`devset explain lint` in the scratch repository shows the features and who
turned each on. [Gates and Scaffolds](gates.md) and
[Composing Profiles](composing.md) have the rest, and
[`examples/`](https://github.com/atomix-labs/devset/tree/main/examples) has each
step as an example that runs.

## Share It

Put the profile in a git repository beside others, and tag a release:
[Publish a Collection](publish-a-collection.md). [Design Profiles](designing.md)
says how to cut profiles so they compose.
