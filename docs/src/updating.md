# Updating and Merging

This chapter covers what happens when a profile changes: how an update takes the
change, how devset merges it with the target's own edits, what happens when the
two conflict, and what becomes of a file no layer provides any more.

## Taking a Change

`.devset/lock.toml` pins each git layer to the commit its tag, branch or rev
named when it was applied. `apply`, on any machine, uses that commit; only an
update moves it:

```sh
devset update          # every layer, to what its ref names now
devset update rust     # one layer, by its profile's name
```

Changing a layer's `tag` in `.devset/config.toml` and running `devset apply`
does the same for that layer: a lock entry that no longer matches its layer is
resolved again.

A local directory has no history to pin, so it is read as it is now, by `apply`
and `update` alike. `status` marks such a layer when its files changed since it
was applied.

Every command that writes takes `--dry-run`, which says what it would do and
writes nothing.

## What an Update Does

For each file whose profile version changed, by its [policy](profiles.md):

| The target's file  | `owned`                         | `merge`           | `once`  |
| ------------------ | ------------------------------- | ----------------- | ------- |
| as devset wrote it | updated                         | updated           | nothing |
| edited             | kept, and reported as drift     | **merged**        | nothing |
| deleted            | the deletion kept, and reported | the deletion kept | nothing |

## Merging

A merge is three-way: the base, which is what devset last wrote and keeps in
`.devset/base/`; the target's version; and the profile's new one. By default a
built-in line merge makes it; a [merge driver](settings.md#merge-drivers) such
as [Mergiraf](https://mergiraf.org) can instead.

**A merged file is checked before it is written.** A line merge can report a
clean merge and still make a broken configuration. Given a local edit near the
top of a file and the profile's edit near its bottom, every line merger produces
this, cleanly:

```yaml
name: app
timeout: 30 # the target's edit
replicas: 1
port: 80
timeout: 60 # the profile's edit: the same key, twice
```

So a merged TOML, JSON or YAML file must parse, with no key twice, or the merge
is a conflict, however cleanly it merged. The format is read from the extension
(`.toml`, `.json` and `.jsonc`, `.yaml` and `.yml`); `validate` on the file's
entry names it, or turns the check off with `none`. JSON is read as JSON with
comments, since so many `.json` configurations have them.

## Conflicts

A conflict is written to `.devset/conflicts/<path>`, with conflict markers, and
**the working file is left untouched**, so a conflict never breaks the build.
The update exits 1 and names each conflict and why:

```console
$ devset update
    Fetching https://github.com/acme/profiles
  Conflicted deny.toml  conflicting changes
     Updated rustfmt.toml
    Finished 1 change, 1 conflict
help: resolve the files in .devset/conflicts/, then run `devset update --continue`;
      or take the update back with `devset update --abort`
```

Resolve it in the sidecar, which already holds every hunk that merged cleanly,
then continue:

```sh
$EDITOR .devset/conflicts/deny.toml    # fix the marked hunks
devset update --continue               # checks, installs, records the new base
```

`--continue` refuses while a sidecar still has markers or fails its check. It
installs every resolution at once, records the profile's version as each file's
new base, and removes the sidecars.

Until then the update is **unfinished**: `apply` and `update` refuse to start
another, `status` lists the conflicts first, and `status --exit-code` fails.

What else an update writes while a file conflicts is the target's choice,
`[merge] on-conflict` in [Settings](settings.md):

| `on-conflict`            | The rest of the update                                          |
| ------------------------ | --------------------------------------------------------------- |
| `apply-others` (default) | Written, and the lock moves; the conflicts wait                 |
| `apply-none`             | Withheld until every conflict is resolved, then written at once |

**Some files cannot merge.** A binary file conflicts, and its sidecar holds the
profile's version, to keep or to replace with yours. A file whose base is lost,
as when `.devset/base/` was never committed, merges without one, and always
waits for you.

## Taking an Update Back

`devset update --abort` takes back an unfinished update, as `git merge --abort`
does: every file it wrote, the lock and the state return to what they were
before it, and the sidecars go. A file changed since the update stops the abort,
so nothing you did since is lost; `--abort --force` discards those changes too.
`--abort --dry-run` says what it would restore.

## Files No Layer Provides

A file no layer provides any more, because a profile dropped it or a layer was
removed, is **dropped**, and `status` says so. The next `apply` takes it away
only if it is as devset wrote it:

| The dropped file     | `apply`                                     |
| -------------------- | ------------------------------------------- |
| as devset wrote it   | removes it, and directories it leaves empty |
| edited               | untracks it: the edit is the target's       |
| deleted              | untracks it                                 |
| written under `once` | untracks it: it became the target's         |

A dropped [part](parts.md) leaves its file the same way, its keys or its block
taken out, and a file it leaves empty goes. A key another layer's part now
covers stays, so a key that moves between profiles is never lost.
