# Settings

A profile sets defaults; the target decides. `.devset/config.toml` holds the
target's decisions, beside its layers: how an update handles a conflict, which
merge driver runs, and overrides of the layers' file entries.

```toml
[merge]
on-conflict = "apply-none"                              # default "apply-others"
driver      = "mergiraf merge --git %O %A %B -p %P"     # default: the built-in line merge

[files."deny.toml"]
policy = "once"

[files."rustfmt.toml"]
from = "base"               # the one layer that provides it, when several do
```

## Precedence

A profile's `[merge]` table sets defaults for the target. Where layers set a
setting differently, devset does not choose between them: it refuses until the
target decides, naming the setting.

```console
$ devset init --path ../q
error: p and q set merge.on-conflict differently
  |
  = help: decide it in .devset/config.toml by setting merge.on-conflict
```

The target's `[merge]` overrides every layer's, and its `[files."<path>"]`
tables override the layers' entries for that path:

| Field      | Overrides                                                          |
| ---------- | ------------------------------------------------------------------ |
| `policy`   | The file's policy: `owned`, `merge` or `once`.                     |
| `validate` | How a merged result is checked: `toml`, `json`, `yaml` or `none`.  |
| `from`     | The layer that provides the file, when several do: a layer's name. |

An override of a path no layer provides is an error, so a stale one never
lingers; `devset remove` takes out the overrides of the layer it removes.

## Conflicts

`on-conflict` says what an update does when a file conflicts: `apply-others`,
the default, writes every other file and moves the lock, and `apply-none` writes
nothing but the conflicts until every one is resolved.
[Updating and Merging](updating.md#conflicts) has the whole story.

## Merge Drivers

`driver` names a program that merges instead of the built-in line merge, as
git's merge drivers do, with git's placeholders:

| Placeholder | Is                                                           |
| ----------- | ------------------------------------------------------------ |
| `%O`        | The base: what devset last wrote                             |
| `%A`        | The target's version, and where the driver leaves its result |
| `%B`        | The profile's new version                                    |
| `%P`        | The file's path in the target                                |
| `%%`        | A literal `%`                                                |

Exit zero is a clean merge; any other exit, a conflict whose result is `%A`.
Either way, devset then checks the result as it checks its own merges.

The line is split into arguments as a shell would split it, then run **without a
shell**: no quoting pitfalls, and no injection. Each version is written to a
scratch directory under the file's own name, so a driver can tell the language
from the extension, and `%A` is always a scratch copy: no driver, however it
behaves, touches the target's file.

A profile may suggest a driver in its `[merge]` table. devset tells you, and
never runs it: **devset never runs a program a profile names.** To use it, set
it in `.devset/config.toml`.
