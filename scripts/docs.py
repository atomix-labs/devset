"""The manual's generated pages, the command reference from each `devset <command> --help` and the
file references from the published schemas; the release of atxp every document names, which is
the one devset applies; and the devset a profile in the manual asks for, this release's series.

Usage: docs.py check | fix

`fix` writes the pages from this checkout's devset, after naming atxp's release as the pin in every
document, the help and the skeleton `init` writes, and devset's series in every profile the manual
shows. `check` fails, naming each file, when a page is not what this checkout's devset prints, the
book's summary leaves a command's page out, a file names another release of atxp than the one
`.devset/config.toml` pins, or a profile asks for another devset than this release's series.
"""

import json
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
# The release of atxp a demo moves to, which is the one devset applies; where it starts is older on
# purpose, so the demo has a release to move to.
UPDATE = re.compile(r"(devset update atxp --tag )(v\d+\.\d+\.\d+)")
DEMOS = Path("docs/demo")
# The devset a profile asks for, `devset = ">=<major>.<minor>"`: in the manual, this release's
# series, as `devset init --profile` writes it.
FLOOR = re.compile(r'(\bdevset\s*=\s*">=)(\d+\.\d+)(?=")')
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


# Each file a schema describes: its schema, its page, and the page's opening, which links the pages
# that explain it.
FILES = {
    "profile": (
        "profile-toml.md",
        "`profile.toml`",
        "A profile's manifest: who it is, what it builds on, its features and variables, and every file it manages. [Profiles](profiles.md) explains it, and [Write a Profile](write-a-profile.md) writes one.",
    ),
    "config": (
        "config-toml.md",
        "`config.toml`",
        "A target's `.devset/config.toml`: its sources, its layers, and its word on every setting they carry. [Composing Profiles](composing.md) and [Settings](settings.md) explain it.",
    ),
    "collection": (
        "collection-toml.md",
        "`collection.toml`",
        "What a source says of itself, at its root. [Publish a Collection](publish-a-collection.md) explains it.",
    ),
}
# How a map-valued key of a file is written, by its key: each entry's header.
ENTRIES = {
    "requires": "[requires]",
    "vars": "[vars.<name>]",
    "scaffolds": "[scaffolds.<name>]",
    "files": '[files."<path>"]',
    "sources": "[sources]",
    "layers": "[[layers]]",
}


# The words an enum value's description may start with that begin a sentence, not a name: the value
# list says them after a colon, lowercased.
PROSE = {"a", "an", "all", "each", "every", "local", "no", "not", "one", "the", "write", "written"}


def plural(kind):
    """`kind`, a type as a row says it, of more than one."""
    return f"lists{kind[4:]}" if kind.startswith("list ") else f"{kind}s"


def text(node):
    """A schema node's description, its paragraphs joined for a table's cell."""
    return " ".join((node.get("description") or "").split()).replace("|", "\\|")


class Schema:
    """A published schema, read into the page that documents its file."""

    def __init__(self, path):
        self.root = json.loads(path.read_text())
        self.defs = self.root.get("$defs", {})
        self.nested = []

    def resolve(self, node):
        """`node`, followed through a `$ref`, and through `anyOf` with `null`, as the key being
        optional."""
        if "$ref" in node:
            return self.defs[node["$ref"].rsplit("/", 1)[1]]
        some = [n for n in node.get("anyOf", []) if n.get("type") != "null"]
        if len(some) == 1 and len(node.get("anyOf", [])) == 2:
            return self.resolve(some[0])
        return node

    def kind(self, name, node):
        """What a key takes, for its row: a type, the values of an enum, or a table below."""
        node = self.resolve(node)
        if "oneOf" in node and all("const" in n for n in node["oneOf"]):
            values = [f"`{n['const']}`" for n in node["oneOf"]]
            return ", ".join(values[:-1]) + " or " + values[-1] if len(values) > 1 else values[0]
        if "anyOf" in node:
            return " or ".join(self.kind(name, n) for n in node["anyOf"])
        kind = node.get("type")
        if isinstance(kind, list):
            kind = next(k for k in kind if k != "null")
        if kind == "array":
            return f"list of {plural(self.kind(name, node.get('items', {})))}"
        if kind == "object" and "properties" in node:
            self.nested.append((name, node))
            return f"table, [below](#{name.replace('-', '')})"
        if kind == "object":
            return f"table of {plural(self.kind(name, node.get('additionalProperties', {})))}"
        return {"integer": "number"}.get(kind, kind or "any")

    def values(self, node):
        """An enum's values, each with what it means; `None` for any other node."""
        node = self.resolve(node)
        if "oneOf" not in node or not all("const" in n for n in node["oneOf"]):
            return None
        said = []
        for value in node["oneOf"]:
            meaning = re.sub(r"^`[^`]+`: ", "", text(value)).rstrip(".")
            first = meaning.split(" ", 1)[0]
            if first.lower() in PROSE:
                meaning = meaning[0].lower() + meaning[1:]
            said.append(f"- `{value['const']}`: {meaning}.")
        return said

    def meaning(self, node):
        """What a key means, and its default."""
        said = text(node)
        default = node.get("default")
        if default not in (None, [], {}, ""):
            shown = f"`{default if isinstance(default, str) else json.dumps(default)}`"
            # A description that already names its default is not told it again.
            if shown not in said:
                said += f" Default {shown}."
        return said.strip()

    def table(self, node):
        """The rows of an object's keys, then what each value of an enum means."""
        required = set(node.get("required", []))
        rows = ["| Key | Takes | Meaning |", "| --- | --- | --- |"]
        enums = []
        for name, child in node.get("properties", {}).items():
            takes = self.kind(name, child) + (", required" if name in required else "")
            rows.append(f"| `{name}` | {takes} | {self.meaning(child)} |")
            if (values := self.values(child)) is not None:
                enums += ["", f"`{name}` takes:", "", *values]
        return rows + enums

    def page(self, title, opening, source):
        """The whole page."""
        out = [f"# {title}", "", f"{MARKER} from `{source}` -->", "", opening, ""]
        out += [
            "Editors complete and check it from its schema, as [Schemas](schemas.md) says.",
            "",
        ]
        for name, child in self.root.get("properties", {}).items():
            header = ENTRIES.get(name, f"[{name}]")
            out += [f"## `{header}`", ""]
            node = self.resolve(child)
            entry = node.get("additionalProperties") or node.get("items")
            if entry is not None and "properties" in self.resolve(entry):
                out += [text(child) + " Each entry:", "", *self.table(self.resolve(entry)), ""]
            elif "properties" in node:
                out += [text(child) or text(node), "", *self.table(node), ""]
            else:
                out += [f"{text(child)} {text(node)}".strip(), ""]
            while self.nested:
                nested, table = self.nested.pop(0)
                out += [f"### `{nested}`", "", text(table), "", *self.table(table), ""]
        return "\n".join(out).rstrip() + "\n"


def formatted(path, text):
    """`text` as dprint formats a page at `path`, so the check compares what the formatter keeps."""
    return subprocess.run(["dprint", "fmt", "--stdin", str(path)], input=text, capture_output=True, text=True, check=True).stdout


def schemas():
    """Every file reference, by path, from the schemas `docs/src/schema/` publishes."""
    written = {}
    for name, (page_name, title, opening) in FILES.items():
        source = SRC / "schema" / f"{name}.json"
        path = SRC / page_name
        written[path] = formatted(path, Schema(source).page(title, opening, source))
    return written


def pin():
    """The release of atxp this repository applies, from its own `.devset/config.toml`."""
    return tomllib.loads(Path(".devset/config.toml").read_text())["sources"]["atxp"]["tag"]


def series():
    """This release's series, `major.minor`, from the workspace's version."""
    version = tomllib.loads(Path("Cargo.toml").read_text())["workspace"]["package"]["version"]
    return ".".join(version.split(".")[:2])


def naming():
    """Each file that names a release, with how to say what it names, the pattern that finds it
    there, and the release it must be: atxp's pin, or devset's series."""
    atxp, floor = pin(), series()
    manual = sorted(SRC.rglob("*.md"))
    files = [(Path(name), "atxp {}", ATXP, atxp) for name in NAMING]
    files += [(path, "atxp {}", ATXP, atxp) for path in manual]
    files += [(path, "atxp {}", UPDATE, atxp) for path in sorted(DEMOS.rglob("*.tape"))]
    return files + [(path, "devset >={}", FLOOR, floor) for path in [Path("README.md"), *manual]]


def misnamed():
    """Each file naming another release than it must, with what it names, the releases, the
    pattern that finds them, and the release it must name."""
    found = {}
    for path, what, pattern, want in naming():
        names = {match.group(2) for match in pattern.finditer(path.read_text())} - {want}
        if names:
            found.setdefault(path, []).append((what, sorted(names), pattern, want))
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
    written = pages() | schemas()
    outdated = [path for path, text in written.items() if stale(path, text)]
    outdated += leftovers(written)
    summary = SUMMARY.read_text()
    unlisted = [path for path in written if f"]({path.relative_to(SRC)})" not in summary]
    for path in outdated:
        print(f"{path}: stale; run `just fix-docs`", file=sys.stderr)
    for path in unlisted:
        print(f"{SUMMARY}: no chapter links {path.relative_to(SRC)}", file=sys.stderr)
    named = misnamed()
    for path, wrong in named.items():
        for what, names, _, want in wrong:
            named_now = ", ".join(what.format(name) for name in names)
            print(f"{path}: names {named_now}, not {what.format(want)}; run `just fix-docs`", file=sys.stderr)
    return 1 if outdated or unlisted or named else 0


def fix():
    """Names the pinned release of atxp and devset's series everywhere, then writes every page, and
    removes the pages of commands there are no more."""
    for path, wrong in misnamed().items():
        text = path.read_text()
        for _, _, pattern, want in wrong:
            text = pattern.sub(lambda match, want=want: match.group(1) + want, text)
        path.write_text(text)
    written = pages() | schemas()
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
