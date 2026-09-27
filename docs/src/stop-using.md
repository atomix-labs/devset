# Stop Using devset

Every file devset wrote is an ordinary file in the repository. Stopping takes
nothing away: the files stay as they are, and only devset's own records go.

## Keep Every File

```sh
rm -r .devset
```

Then remove what runs devset:

- a CI step running `devset status` or `devset update`;
- a recipe or script that runs it, as atxp's `check-devset`, and a pin of devset
  in `mise.toml`;
- a scheduled update job, if the repository has one.

A block a profile owned stays in its file between devset's markers, which are
comments: delete the two marker lines, or leave them. Every tool the profiles
pinned stays pinned where the repository's own configuration names it.

## Drop One Layer

`devset remove <layer>` is for the other case: dropping a profile's files while
devset stays. It takes away each file the layer wrote that is still as devset
wrote it, keeps an edited one and stops tracking it, and takes the layer's keys
or block out of a file it shared. `devset remove <layer> --dry-run` says which.

## Coming Back

A repository that stopped can start again with `devset add`: every file there is
adopted, as [An Existing Repository](existing-repository.md) says, and nothing
is lost.
