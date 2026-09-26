# In CI

One command makes drift fail a build:

```sh
devset status --exit-code
```

It exits 1 exactly when `devset apply --force` would write a file, or an update
is unfinished: an `owned` file edited, deleted or never recorded; a file a
profile added and the target has not applied; a conflict waiting in
`.devset/conflicts/`. A local edit that is the target's to make, to a `merge` or
a `once` file, never fails it, and neither does a change of whitespace alone.

## Installing devset

Pin devset in the repository's [mise](https://mise.jdx.dev) configuration, so
every machine and every job runs the same version:

```sh
mise use github:atomix-labs/devset@0.2.0
```

Then a GitHub Actions job installs it with the rest of the repository's tools:

```yaml
jobs:
  devset:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: jdx/mise-action@v4
      - run: devset status --exit-code
```

## GitHub Actions

Under GitHub Actions, devset also annotates each file that needs doing on the
pull request, as an error where the gate fails and a warning where `apply` would
write, and adds a table of them to the job's summary page:

```text
::error title=devset,file=a.toml::a.toml is edited; `devset apply --force` would restore it
::warning title=devset,file=c.toml::c.toml is new; `devset apply` would create it
```

## Credentials

A git source's repository is fetched with the job's own `git`, so its credential
helpers, SSH keys and `insteadOf` rules apply as they do to any `git fetch`.
devset never lets git prompt where there is no terminal to answer, so a private
repository needs a credential helper or an SSH key set up before devset runs.

## Status as JSON

`devset status --json` prints the survey for scripts:

| Field         | Holds                                                                                                                                                                                           |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `layers`      | Each layer, in order: `name`, `profile` as `source/name`, `features`, `configured`, `required_by`, `version`, `source`, `rev`, `digest`, and `applied`, one of `current`, `changed` and `never` |
| `files`       | Each managed file or part: `path`, `scope`, `part`, `layer`, `policy`, `state`, what `apply` and `apply --force` would do to it, `apply` and `force`, and `gate`, why its gate released it      |
| `settings`    | `on-conflict`, and the merge `driver`: `builtin`, or its command line                                                                                                                           |
| `answers`     | Every variable's answer                                                                                                                                                                         |
| `suggestions` | Merge drivers a layer suggests that the target has not set                                                                                                                                      |
| `drifted`     | Whether `apply --force` would write a file                                                                                                                                                      |
| `unfinished`  | Whether an update is unfinished                                                                                                                                                                 |

A file's `state` is one of `unchanged`, `cosmetic`, `edited`, `missing`, `new`,
`untracked`, `dropped` and `conflict`. What `apply` would do is one of `create`,
`adopt`, `update`, `restore`, `overwrite`, `merge`, `record`, `untrack`,
`remove` and `conflict`, or `null` for nothing. `status --exit-code` fails when
`drifted` or `unfinished` is true.
