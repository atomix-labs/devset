"""Moves each source a target names by tag to its newest release past the cooldown, then applies.

Usage: devset.py bump [<report>]

A source on github.com moves to its newest release three days old or older, never backwards; any
other source is reported as unchecked. After a move, `devset update` merges; a conflict takes the
move back, `devset update --abort`, and is reported.
"""

import datetime
import json
import os
import re
import subprocess
import sys
import tomllib
import urllib.request
from pathlib import Path

CONFIG = Path(".devset/config.toml")
COOLDOWN = datetime.timedelta(days=3)
GITHUB = re.compile(r"^https://github\.com/([\w.-]+)/([\w.-]+?)(?:\.git)?/?$")
VERSION = re.compile(r"^v?(\d+(?:\.\d+)*)$")


def key(tag):
    """`tag`'s version as a tuple to compare; `None` for a tag that is no version."""
    match = VERSION.match(tag)
    return tuple(int(part) for part in match.group(1).split(".")) if match else None


def releases(owner, repo):
    """The repository's releases, as GitHub lists them."""
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "devset-bump"}
    if token := os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN"):
        headers["Authorization"] = f"Bearer {token}"
    url = f"https://api.github.com/repos/{owner}/{repo}/releases?per_page=30"
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers)) as response:
        return json.load(response)


def newest(owner, repo, have):
    """The newest release tag past the cooldown that is later than `have`, if any."""
    cutoff = datetime.datetime.now(datetime.UTC) - COOLDOWN
    aged = [
        release["tag_name"]
        for release in releases(owner, repo)
        if not release["draft"]
        and not release["prerelease"]
        and key(release["tag_name"])
        and datetime.datetime.fromisoformat(release["published_at"]) <= cutoff
    ]
    best = max(aged, key=key, default=None)
    return best if best and key(best) > key(have) else None


def retag(text, name, tag):
    """`text`, the config, with source `name`'s tag set to `tag`."""
    line = re.compile(rf'^(\s*{re.escape(name)}\s*=\s*\{{[^}}]*\btag\s*=\s*")[^"]+(")', re.M)
    return line.sub(rf"\g<1>{tag}\g<2>", text, count=1)


def report_text(sections):
    """The report: each non-empty section as a list."""
    body = "".join(
        f"{title} ({len(items)}):\n\n" + "".join(f"- {item}\n" for item in items) + "\n"
        for title, items in sections
        if items
    )
    return "### devset sources\n\n" + (body or "Nothing to move.\n")


def bump(report):
    """Moves every source it can, applies, and reports."""
    text = CONFIG.read_text()
    moved, unchecked, failed = [], [], []
    for name, spec in tomllib.loads(text).get("sources", {}).items():
        if "tag" not in spec:
            continue
        match = GITHUB.match(spec.get("git", ""))
        if not match or not key(spec["tag"]):
            unchecked.append(f"`{name}`")
            continue
        try:
            want = newest(*match.groups(), spec["tag"])
        except OSError as error:
            failed.append(f"`{name}`: {error}")
            continue
        if want:
            text = retag(text, name, want)
            moved.append(f"`{name}` {spec['tag']} -> {want}")
    if moved:
        CONFIG.write_text(text)
        if subprocess.run(["devset", "--no-input", "update"], check=False).returncode:
            subprocess.run(["devset", "--no-input", "update", "--abort"], check=False)
            failed.append("`devset update` conflicted, and was taken back: run it by hand")
    out = report_text([("Moved", moved), ("Not on GitHub", unchecked), ("Could not move", failed)])
    if report:
        Path(report).write_text(out)
    else:
        print(out, end="")
    return 1 if failed else 0


def main(args):
    match args:
        case ["bump", *report] if len(report) <= 1:
            return bump(report[0] if report else None)
    print(__doc__.strip().splitlines()[2], file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
