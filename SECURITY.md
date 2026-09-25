# Security Policy

How to report a vulnerability in devset, and which releases are fixed.

## Supported Versions

The latest release only: a fix is released as a new version.

## Reporting a Vulnerability

Report it privately, through
[GitHub's security advisories](https://github.com/atomix-labs/devset/security/advisories/new),
not in a public issue.

## Scope

devset applies files from profiles that other people write, often from a
repository it fetches, so a profile is untrusted input. A vulnerability is a way
for a profile to make devset do more than write the files it declares inside the
target:

- write, or delete, outside the target, or into `.git` or `.devset`, through a
  path, a symlink, or a case or Unicode trick;
- run a program, including a merge driver the target did not name;
- read a file outside the target and the profile into the target;
- leak a credential git holds.

A merge driver the target names, and the tools a profile ships for the
repository to run, run with the user's own rights, and are outside this scope.
