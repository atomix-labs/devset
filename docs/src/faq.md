# FAQ

The questions people ask of tools like devset, and devset's answers.

## Can I Use devset in a Repository It Did Not Create?

Yes. `devset add` into an existing repository adopts what it finds: a file a
profile owns whole is kept, its differences shown as drift; keys a profile owns
are merged into the repository's file one by one; a block goes at the end of a
text file. Nothing is overwritten unless you say so, and `--dry-run` shows the
run first. [An Existing Repository](existing-repository.md) goes through it.

## Two Profiles Both Need the Same File. What Happens?

Each can own a part of it: different keys of a TOML, JSON or YAML file, or a
block each of a text file, and devset writes both. Where two profiles provide
the same whole file, or the same key, devset refuses to guess: it stops and
names them, and the repository chooses one with `from`.
[Composing Profiles](composing.md#layers) and [Settings](settings.md) say how.

## I Edited a File devset Manages. Will an Update Clobber It?

No. What happens depends on the file's policy. Under `merge`, the edit is the
repository's: an update merges the profile's change with it, three ways, against
what devset last wrote. Under `owned`, the edit is drift: `devset status`
reports it, an update leaves the file alone, and only `devset apply --force`
restores the profile's version. A `once` file is the repository's from the first
write. A repository can change a file's policy for itself, as `once` to take a
file over. [Updating and Merging](updating.md) has the table.

## Can I See What an Update Would Do First?

Yes. `devset update --dry-run` names the newer releases, and `devset update
<source> --tag <release> --dry-run` says what taking one would change, writing
nothing; `devset diff` shows, file by file, how the repository differs from its
profiles now. Every command that writes takes `--dry-run`.

## An Update Went Wrong. How Do I Undo It?

While a run is unfinished, because a file conflicted, `devset apply --abort`
takes it back: every file it wrote, the lock and the state return to what they
were. A finished update is an ordinary change in the working tree: review it
with `git diff`, and discard it as any change is, or move back with `devset
update <source> --tag <the old tag>`.
[Update and Resolve Conflicts](resolving.md#taking-a-run-back)

## Why Is a Conflict Not Written into the File?

Because a file with conflict markers in it breaks whatever reads it: a
formatter, a linter, a build, a CI job. devset writes the conflict to
`.devset/conflicts/<path>`, with markers, and leaves the working file as it was,
so the repository keeps working while you resolve it. `devset apply --continue`
installs the resolution, and checks a TOML, JSON or YAML file still parses.
[Update and Resolve Conflicts](resolving.md#resolving-a-conflict)

## Does Applying a Profile Run Its Code?

No. A profile is data: files, and when to write them. devset runs no hook, task
or script a profile ships, and installs no tool, so taking a profile needs no
more trust than reading its files. What other tools do in hooks, a profile does
with files: it ships a script, as atxp's `setup.sh` is, for the repository to
run when it chooses. A merge driver a profile suggests is only a suggestion,
which the repository must name itself.

## I Deleted a Managed File, or a Profile Stopped Shipping One. What Now?

A file you deleted stays deleted: `devset status` reports it, and `devset apply
--force` brings it back. A file a profile no longer ships is taken away if it is
as devset wrote it, and kept, no longer tracked, if the repository edited it.
[Updating and Merging](updating.md#files-no-layer-provides)

## Why Is This File Managed, and by Which Profile?

`devset explain <file>` names every layer that lists it, the part each owns, its
policy and its gates, and whether a scaffold wrote it. `devset explain <layer>`
shows a layer's features and who turned each on. `devset status -v` lists every
file, those that match too.

## Is Reformatting a Managed File Drift?

Not for what devset owns in part: keys are compared after parsing, so a
reformatted TOML, JSON or YAML file is unchanged. A whole file is compared as
text, except whitespace at the ends of lines, line endings, a BOM and blank
lines at the end, which are never edits, so CRLF against LF is no change; any
other reformatting of an `owned` file is drift. A profile that ships files as
its formatter leaves them never meets it.

## How Do I Keep Many Repositories in Step?

Keep the shared configuration in one collection, and have each repository take
it by tag. A scheduled job in each moves to the newest release and opens a pull
request, as atxp's weekly bump does; [Automate Updates](automate-updates.md) has
the steps, and [Share a Team's Configuration](share-configuration.md) the
layout. In CI, `--no-input` takes every default and fails on a question with
none; `devset status --exit-code` fails on drift; and an update that conflicts
exits 1, for the job to take back with `devset apply --abort` and report.
[Output and Exit Codes](reference/output.md) lists every code.

## Can a Repository Skip One File a Profile Ships?

Yes, in `.devset/config.toml`: `policy = "once"` on that file makes it the
repository's after its first write, and `from` picks another layer's version
where several provide it. A profile's feature that brings the file may also be
turned off. [Settings](settings.md)

## Does devset Need the Network?

Only to fetch a git source it has not fetched before, or to update one. It keeps
each source in its cache, and `.devset/lock.toml` pins the commit, so `apply`
and `status` work offline once a source is fetched. A local directory source
needs nothing.

## How Do I Stop Using Devset?

Delete `.devset/`, and whatever runs devset: every file stays as it is.
[Stop Using devset](stop-using.md)

## My Question Is Not Here

Ask it in [Discussions](https://github.com/atomix-labs/devset/discussions),
under Q&A, where the answer helps the next person too.
