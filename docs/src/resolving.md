# Update and Resolve Conflicts

How to take a profile's changes, what to do when one meets an edit of the
target's own, and how to take a run back. [Updating and Merging](updating.md)
says what devset does underneath.

## Taking a Change

`.devset/lock.toml` pins each git source to the commit its tag, branch or rev
named when it was applied, so every profile of a source is at one commit.
`apply`, on any machine, uses that commit; only an update moves it:

```sh
devset update          # every source, to what its ref names now
devset update atxp     # one source, by its name
devset update rust     # the source of one layer, by its profile's name
```

A source pinned to a tag stays there until you move it, and `update` names the
releases newer than it, with the command that takes the newest:

```console
$ devset update
    Fetching https://github.com/atomix-labs/atxp
    Finished up to date
note: atxp is pinned to v0.6.2; newer: v0.7.0
  |
  = help: take the newest: `devset update atxp --tag v0.7.0`
$ devset update atxp --tag v0.7.0
```

`--tag`, `--branch` and `--rev` move a source in `.devset/config.toml`, keeping
how the file writes it, and apply what the new ref holds; `update --dry-run`
names the newer releases without writing. Editing the ref by hand and running
`devset apply` does the same: a lock entry that no longer matches its source is
resolved again.

A local directory has no history to pin, so it is read as it is now, by `apply`
and `update` alike. `status` marks such a layer when its files changed since it
was applied.

Every command that writes takes `--dry-run`, which says what it would do and
writes nothing.

## Resolving a Conflict

A conflict is written to `.devset/conflicts/<path>`, with conflict markers, and
**the working file is left untouched**, so a conflict never breaks the build.
The update exits 1 and names each conflict and why:

```console
$ devset update
    Fetching https://github.com/acme/profiles
  Conflicted deny.toml  conflicting changes
     Updated rustfmt.toml
    Finished 1 change, 1 conflict
help: resolve the files in .devset/conflicts/, then run `devset apply --continue`;
      or take it back with `devset apply --abort`
```

Resolve it in the sidecar, which already holds every hunk that merged cleanly,
then continue:

```sh
$EDITOR .devset/conflicts/deny.toml    # fix the marked hunks
devset apply --continue                # checks, installs, records the new base
```

`--continue` refuses while a sidecar still has markers or fails its check. It
installs every resolution at once, records the profile's version as each file's
new base, and removes the sidecars.

Until then the run is **unfinished**: every command that writes refuses to start
another, `status` lists the conflicts first, and `status --exit-code` fails. A
conflict can come from any of them, `add`, `remove` or `apply` as well as
`update`, and is finished the same way.

## Taking a Run Back

`devset apply --abort` takes back the run that conflicted, as `git merge
--abort` does: every file it wrote, the lock and the state return to what they
were before it, and the sidecars go. A file changed since the run stops the
abort, so nothing you did since is lost; `--abort --force` discards those
changes too. `--abort --dry-run` says what it would restore.
