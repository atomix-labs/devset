"""The manual's generated pages, the command reference from each `devset <command> --help`; and
the release of atxp every document names, which is the one devset applies.

Usage: docs.py check | fix

`fix` writes the pages from this checkout's devset, after naming atxp's release as the pin in every
document, the help and the skeleton `init` writes. `check` fails, naming each file, when a page is
not what this checkout's devset prints, the book's summary leaves a command's page out, or a file
names another release of atxp than the one `.devset/config.toml` pins.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

import tomllib

SRC = Path("docs/src")
REFERENCE = SRC / "reference"
SUMMARY = SRC / "SUMMARY.md"
# A release of atxp a file names: its URL, then on the same line `--tag v...` or `tag = "v..."`.
ATXP = re.compile(r'(atomix-labs/atxp\b[^\n]*?(?:--tag |tag = \\?"))(v\d+\.\d+\.\d+)')
# What names atxp's release besides the manual: the README, the help, and the skeleton `init` writes.
NAMING = ("README.md", "bin/devset-cli/src/cli.rs", "bin/devset-cli/src/main.rs", "lib/devset-core/src/target.rs")
# Marks a reference page as written here; a page without it is written by hand.
MARKER = "<!-- reference: written by `just fix-docs`"
# devset's own help lists its commands under this heading, two spaces in, up to a blank line.
COMMANDS = re.compile(r"^Commands:\n((?:  .*\n)+)", re.MULTILINE)


def devset(*args):
    """What this checkout's devset prints for `args`: uncoloured, 100 columns wide."""
    env = {**os.environ, "NO_COLOR": "1", "COLUMNS": "100"}
    command = ["cargo", "run", "--quiet", "--locked", "--package", "devset-cli", "--", *args]
    return subprocess.run(command, env=env, capture_output=True, text=True, check=True).stdout


def commands(top):
    """The commands `top`, devset's own help, lists; `help` aside."""
    names = [line.split()[0] for line in COMMANDS.search(top).group(1).splitlines()]
    return [name for name in names if name != "help"]


def page(name, text):
    """The reference page of `name`, `devset` or `devset <command>`, whose help is `text`."""
    return f"# `{name}`\n\n{MARKER} from `{name} --help` -->\n\n```text\n{text.rstrip()}\n```\n"


def pages():
    """Every reference page, by path: devset's own, then each command's."""
    top = devset("--help")
    written = {REFERENCE / "devset.md": page("devset", top)}
    for command in commands(top):
        written[REFERENCE / f"{command}.md"] = page(f"devset {command}", devset(command, "--help"))
    return written


def pin():
    """The release of atxp this repository applies, from its own `.devset/config.toml`."""
    return tomllib.loads(Path(".devset/config.toml").read_text())["sources"]["atxp"]["tag"]


def misnamed(want):
    """Each file naming a release of atxp other than `want`, with the releases it names."""
    found = {}
    for path in [Path(name) for name in NAMING] + sorted(SRC.rglob("*.md")):
        tags = {match.group(2) for match in ATXP.finditer(path.read_text())} - {want}
        if tags:
            found[path] = sorted(tags)
    return found


def leftovers(written):
    """Reference pages written here once that no command has now."""
    generated = (path for path in REFERENCE.glob("*.md") if MARKER in path.read_text())
    return sorted(path for path in generated if path not in written)


def stale(path, text):
    """Whether `path` is missing, or holds other than `text`."""
    return not path.is_file() or path.read_text() != text


def check():
    """Names every page that is stale, and fails if any is."""
    written = pages()
    outdated = [path for path, text in written.items() if stale(path, text)]
    outdated += leftovers(written)
    summary = SUMMARY.read_text()
    unlisted = [path for path in written if f"]({path.relative_to(SRC)})" not in summary]
    for path in outdated:
        print(f"{path}: stale; run `just fix-docs`", file=sys.stderr)
    for path in unlisted:
        print(f"{SUMMARY}: no chapter links {path.relative_to(SRC)}", file=sys.stderr)
    want = pin()
    named = misnamed(want)
    for path, tags in named.items():
        print(f"{path}: names atxp {', '.join(tags)}, not {want}; run `just fix-docs`", file=sys.stderr)
    return 1 if outdated or unlisted or named else 0


def fix():
    """Names the pinned release of atxp everywhere, then writes every page, and removes the pages
    of commands there are no more."""
    want = pin()
    for path in misnamed(want):
        path.write_text(ATXP.sub(lambda match: match.group(1) + want, path.read_text()))
    written = pages()
    for path in leftovers(written):
        path.unlink()
    for path, text in written.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    return 0


if __name__ == "__main__":
    match sys.argv[1:]:
        case ["check"]:
            sys.exit(check())
        case ["fix"]:
            sys.exit(fix())
        case _:
            sys.exit(__doc__)
