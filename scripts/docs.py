"""The manual's generated pages: the command reference, from each `devset <command> --help`, and
the JSON schemas, from `devset schema`.

Usage: docs.py check | fix

`fix` writes them from this checkout's devset. `check` fails, naming each page, when one is not
what this checkout's devset prints, or the book's summary leaves a command's page out.
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path

SRC = Path("docs/src")
REFERENCE = SRC / "reference"
SCHEMA = SRC / "schema"
SUMMARY = SRC / "SUMMARY.md"
SCHEMAS = ("profile", "config")
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


def schemas():
    """Each schema, by path, as devset prints it."""
    return {SCHEMA / f"{name}.json": devset("schema", name) for name in SCHEMAS}


def leftovers(written):
    """Reference pages written here once that no command has now."""
    generated = (path for path in REFERENCE.glob("*.md") if MARKER in path.read_text())
    return sorted(path for path in generated if path not in written)


def stale(path, text):
    """Whether `path` is missing, or holds other than `text`."""
    return not path.is_file() or path.read_text() != text


def stale_schema(path, text):
    """Whether `path` is missing, or holds another schema than `text`: dprint lays it out."""
    return not path.is_file() or json.loads(path.read_text()) != json.loads(text)


def check():
    """Names every page that is stale, and fails if any is."""
    written, printed = pages(), schemas()
    outdated = [path for path, text in written.items() if stale(path, text)]
    outdated += [path for path, text in printed.items() if stale_schema(path, text)]
    outdated += leftovers(written)
    summary = SUMMARY.read_text()
    unlisted = [path for path in written if f"]({path.relative_to(SRC)})" not in summary]
    for path in outdated:
        print(f"{path}: stale; run `just fix-docs`", file=sys.stderr)
    for path in unlisted:
        print(f"{SUMMARY}: no chapter links {path.relative_to(SRC)}", file=sys.stderr)
    return 1 if outdated or unlisted else 0


def fix():
    """Writes every page, then lays the schemas out as dprint does."""
    written, printed = pages(), schemas()
    for path in leftovers(written):
        path.unlink()
    for path, text in {**written, **printed}.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    subprocess.run(["dprint", "fmt", *map(str, printed)], check=True)
    return 0


if __name__ == "__main__":
    match sys.argv[1:]:
        case ["check"]:
            sys.exit(check())
        case ["fix"]:
            sys.exit(fix())
        case _:
            sys.exit(__doc__)
