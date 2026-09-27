# Automate Updates

A repository pinned to a collection's tag stays there until it moves, so a
release does nothing until someone takes it. A scheduled job takes it for you:
it moves the pin, runs the checks, and opens a pull request with their verdict.

## What the Job Does

1. **Find the newest release.** `devset update --dry-run` names every release
   newer than the pin, and the command that takes the newest. A script finds the
   tag itself:

   ```sh
   latest=$(git ls-remote --tags --refs --sort=-version:refname https://github.com/acme/profiles 'v*' \
       | head -n 1 | sed 's|.*refs/tags/||')
   ```

2. **Take it.** `devset update acme --tag "$latest"` moves the pin in
   `.devset/config.toml` and merges every change with the repository's edits.
3. **Take a conflict back.** An update that conflicts exits 1 and leaves the
   conflict in `.devset/conflicts/`; `devset apply --abort` returns the
   repository to where it was, and the job reports the release for a person to
   take by hand, as [Update and Resolve Conflicts](resolving.md) shows.
4. **Check.** Run the repository's checks, so the pull request says whether the
   release passes.
5. **Open a pull request** with the changes, the release it moved to, and the
   checks' verdict.

A cooldown, taking a release only once it is a few days old, leaves time for a
bad one to be followed by its fix.

## With Atxp

atxp's `github-bump` profile is such a job, for every repository that applies
it: once a week, its `bump-devset` recipe moves each source pinned by tag on
github.com to its newest release three days old or older, and takes a conflict
back; the same run moves the repository's other pins, runs `just check`, and
commits the result as one signed commit, to a branch, a pull request, or a merge
once green, as the repository chooses.

## By Hand, Across Repositories

To move many repositories at once, run the same steps in each from a tool such
as [multi-gitter](https://github.com/lindell/multi-gitter), which opens a pull
request per repository.
