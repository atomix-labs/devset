# Local Edits Kept

A repository adds a line of its own to a file the profile manages under `merge`;
then the profile changes the file too. devset merges the two, three ways,
against what it wrote, and both changes are there.

```text
profiles/base/     the profile as the repository first takes it
next/base/         its next version: max_width moves from 100 to 120
```

Run these from this directory, or a copy of it:

```console
$ git init -q repo
$ cd repo
$ devset add team/base --path ../profiles
    Starting a target: .devset/config.toml
     Created rustfmt.toml
    Finished 1 change
$ echo 'imports_granularity = "Module"' >> rustfmt.toml
```

The profile's maintainers release its next version; here, copying it in stands
for that. A local source is read as it is, so `status` sees the change at once:

```console
$ cp -r ../next/. ../profiles/
$ devset status
team/base  (changed since it was applied)

Pending — `devset apply` will:
    edited     rustfmt.toml  merge  merge
$ devset apply
      Merged rustfmt.toml
    Finished 1 change
$ cat rustfmt.toml
edition    = "2024"
max_width  = 120
tab_spaces = 4
imports_granularity = "Module"
```

A source in git moves with `devset update` instead, and merges the same way.
Where the profile and the repository change the same lines, the merge is a
conflict, written beside the file in `.devset/conflicts/`, and the file is left
as it was:
[Update and Resolve Conflicts](https://atomix-labs.github.io/devset/resolving.html).
