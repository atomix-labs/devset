"""A release's notes: the repository's .github/release-notes.md, with the release's changes in it.

Usage: notes.py <tag> <owner/name> | --unreleased <owner/name>

`{version}` and `{tag}` in the file take the release's, and its `<!-- changes -->` line takes the
changes: a callout where one is breaking, linking the release's migration in BREAKING-CHANGES.md,
then git-cliff's section for the tag without its heading, which the release's title says already.
With no such file, the notes are the changes alone. `--unreleased` renders what the next release's
notes would be, to check the file.
"""

import re
import subprocess
import sys
from pathlib import Path

TEMPLATE = Path(".github/release-notes.md")
MARKER = "<!-- changes -->"
MIGRATIONS = Path("BREAKING-CHANGES.md")
# How the changelog's body marks a breaking change.
BREAKING = "**breaking**"


def changes(tag):
    """git-cliff's section for `tag`, or the unreleased one, without its heading."""
    which = ["--unreleased"] if tag is None else ["--latest"]
    text = subprocess.run(
        ["git", "cliff", *which, "--strip", "all"], stdout=subprocess.PIPE, text=True, check=True
    ).stdout
    lines = text.strip().splitlines()
    if lines and lines[0].startswith("## "):
        lines = lines[1:]
    return "\n".join(lines).strip()


def callout(tag, repo):
    """The warning a breaking release opens with, linking its migration where there is one."""
    version = tag.removeprefix("v")
    target = f"https://github.com/{repo}/blob/{tag}/{MIGRATIONS}"
    heading = re.compile(rf"^## v{re.escape(version)}\s*$", re.MULTILINE | re.IGNORECASE)
    if MIGRATIONS.is_file() and heading.search(MIGRATIONS.read_text()):
        target += "#v" + version.replace(".", "")
    return f"> [!WARNING]\n> This release has breaking changes: [{MIGRATIONS}]({target}) says what to do."


def notes(tag, repo):
    """The notes for `tag` of `repo`, or for its next release where `tag` is None."""
    shown = tag or "vX.Y.Z"
    body = changes(tag)
    if BREAKING in body:
        body = f"{callout(shown, repo)}\n\n{body}"
    if not TEMPLATE.is_file():
        return body + "\n"
    text = TEMPLATE.read_text()
    if MARKER not in text:
        sys.exit(f"{TEMPLATE}: no `{MARKER}` line, where the release's changes go")
    text = text.replace("{version}", shown.removeprefix("v")).replace("{tag}", shown)
    return text.replace(MARKER, body).strip() + "\n"


if __name__ == "__main__":
    match sys.argv[1:]:
        case ["--unreleased", repo]:
            notes(None, repo)
        case [tag, repo]:
            sys.stdout.write(notes(tag, repo))
        case _:
            sys.exit(__doc__)
