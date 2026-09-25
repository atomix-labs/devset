"""Hold every crate manifest to the shape the workspace reads them in.

taplo settles layout: alignment, and alphabetical order inside each dependency group. What it cannot
reach is the text around the values, because a JSON schema validates a parsed tree and a comment is
not in one. Those rules are checked here, on the lines rather than the values.

    manifest-lint.py [<path>...]   # default: every tracked Cargo.toml, vendored crates aside
"""

import re
import subprocess
import sys
from pathlib import Path

# A header, an entry, and the two group markers: the whole grammar a manifest is read with here.
TABLE = re.compile(r"^\[+([^\]]+)\]+\s*$")
ENTRY = re.compile(r'^\s*((?:[\w.-]+|"[^"]+")(?:\.[\w-]+)*)\s*=')
GROUPS = ("# external", "# internal")

# `[package]`, in the order every crate writes it. `name` and `description` are the crate's own; the
# rest say `<key>.workspace = true` and say nothing else.
PACKAGE_ORDER = [
    "name",
    "description",
    "version",
    "edition",
    "rust-version",
    "license",
    "authors",
    "publish",
]
INHERITED = {"version", "edition", "rust-version", "license", "authors", "publish"}

DEP_KINDS = {"dependencies", "dev-dependencies", "build-dependencies"}


def tracked(repo: Path) -> list[Path]:
    """Every manifest git knows and the tree still has: the index outlives a staged deletion."""
    out = subprocess.run(["git", "ls-files", "*Cargo.toml"], cwd=repo, capture_output=True, text=True, check=True)
    # A vendored crate is upstream's, under a `vendor/` directory anywhere in the tree.
    return [repo / line for line in out.stdout.split() if "vendor" not in Path(line).parts[:-1] and (repo / line).is_file()]


def internal_names(paths: list[Path]) -> set:
    """Every package the workspace builds: what `# internal` is allowed to name."""
    names = set()
    for path in paths:
        for line in path.read_text().splitlines():
            match = re.match(r'^name\s*=\s*"([^"]+)"', line)
            if match:
                names.add(match.group(1))
                break
    return names


def workspace_deps(root: Path) -> set:
    """The `[workspace.dependencies]` keys: the internal deps a crate must inherit rather than path to."""
    names, inside = set(), False
    for line in (root / "Cargo.toml").read_text().splitlines():
        table = TABLE.match(line)
        if table:
            inside = table.group(1) == "workspace.dependencies"
        elif inside:
            entry = ENTRY.match(line)
            if entry:
                names.add(entry.group(1))
    return names


def tables(path: Path) -> list:
    """(header, [(lineno, text)]) per table, the preamble under a header of `""`."""
    out, current = [("", [])], None
    for lineno, line in enumerate(path.read_text().splitlines(), 1):
        table = TABLE.match(line)
        if table:
            current = (table.group(1), [])
            out.append(current)
        else:
            (current or out[0])[1].append((lineno, line))
    return out


def kind(header: str) -> str:
    """The dependency kind a header names, `[target.'cfg(…)'.dev-dependencies]` included."""
    tail = header.rsplit(".", 1)[-1]
    return tail if tail in DEP_KINDS and not header.startswith("workspace.lints") else ""


def bare(line: str) -> str:
    """The line with every quoted string blanked, so a `#` inside a value is not read as a comment."""
    return re.sub(r'"[^"]*"|\'[^\']*\'', '""', line)


def check_comments(path: Path, header: str, body: list, found: list) -> None:
    for lineno, line in body:
        text = line.strip()
        if text.startswith("#"):
            if not (kind(header) or header == "workspace.dependencies") or text not in GROUPS:
                found.append((path, lineno, f"comment: `{text[:60]}`"))
        elif ENTRY.match(line) and "#" in bare(line):
            found.append((path, lineno, "comment: trailing, on an entry"))


def check_groups(path: Path, header: str, body: list, internal: set, found: list) -> None:
    """Both markers, `# external` first, each once, and every entry under the one that names it."""
    seen, group = [], None
    entries = {"# internal": [], "# external": []}
    for lineno, line in body:
        text = line.strip()
        if text in GROUPS:
            if text in seen:
                found.append((path, lineno, f"group: `{text}` written twice"))
            seen.append(text)
            group = text
        elif ENTRY.match(line):
            if group is None:
                found.append((path, lineno, "group: an entry before any `# internal` / `# external`"))
            else:
                entries[group].append((lineno, ENTRY.match(line).group(1)))
    if seen == ["# internal", "# external"]:
        found.append((path, body[0][0], "group: `# internal` before `# external`"))
    for lineno, name in entries["# internal"]:
        if name not in internal:
            found.append((path, lineno, f"group: `{name}` is external, under `# internal`"))
    for lineno, name in entries["# external"]:
        if name in internal:
            found.append((path, lineno, f"group: `{name}` is internal, under `# external`"))
    for marker in GROUPS:
        if entries[marker] and marker not in seen:
            found.append((path, body[0][0], f"group: entries with no `{marker}` header"))


def owned(crate: Path, target: str) -> bool:
    """Whether a path dep stays inside the crate's own tree, a `macros/` sibling included."""
    roots = [crate, crate.parent] if crate.parent.name == "macros" else [crate]
    resolved = (crate / target).resolve()
    return any(resolved.is_relative_to(root.resolve()) for root in roots)


def check_inherit(path: Path, body: list, shared: set, internal: set, found: list) -> None:
    """An internal dep the workspace already pins is inherited; a path dep stays in the crate's tree."""
    for lineno, line in body:
        entry = ENTRY.match(line)
        if not entry or entry.group(1) not in internal:
            continue
        name, value = entry.group(1), line.split("=", 1)[1]
        target = re.search(r'path\s*=\s*"([^"]+)"', value)
        if target and name in shared:
            found.append((path, lineno, f"dep: `{name}` is in [workspace.dependencies]; inherit it"))
        elif target and not owned(path.parent, target.group(1)):
            found.append((path, lineno, f"dep: `{name}` paths outside the crate's own tree"))
        elif not target and "workspace" not in bare(value):
            found.append((path, lineno, f"dep: `{name}` pins a version; use `{{ workspace = true }}`"))


def check_package(path: Path, body: list, found: list) -> None:
    keys = [ENTRY.match(line).group(1) for _, line in body if ENTRY.match(line)]
    stems = [key.split(".")[0] for key in keys]
    for lineno, line in body:
        entry = ENTRY.match(line)
        if entry and entry.group(1) in INHERITED:
            found.append((path, lineno, f"package: write `{entry.group(1)}.workspace = true`"))
    if "description" not in stems:
        found.append((path, body[0][0] if body else 1, "package: no `description`"))
    wanted = [key for key in PACKAGE_ORDER if key in stems]
    if [key for key in stems if key in PACKAGE_ORDER] != wanted:
        found.append((path, body[0][0], f"package: key order; want {' '.join(wanted)}"))


def check(path: Path, repo: Path, shared: set, internal: set) -> list:
    found: list = []
    text = path.read_text()
    nested = re.search(r"^\[workspace\]", text, re.M) is not None  # its own workspace: it inherits nothing to check
    seen = set()
    for header, body in tables(path):
        seen.add(header)
        check_comments(path, header, body, found)
        if kind(header) or header == "workspace.dependencies":
            check_groups(path, header, body, internal, found)
            if not nested:
                check_inherit(path, body, shared, internal, found)
        elif header == "package" and not nested:
            check_package(path, body, found)
        elif header == "features":
            keys = [ENTRY.match(line).group(1) for _, line in body if ENTRY.match(line)]
            if keys and keys[0] != "default":
                found.append((path, body[0][0], "features: `default` is missing or is not first"))
    if not nested and not any(header.split(".")[0] == "lints" for header in seen):
        found.append((path, 1, "lints: no `[lints] workspace = true`"))
    return sorted(found, key=lambda item: item[1])


def main() -> int:
    repo = Path(subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip())
    every = tracked(repo)
    paths = [Path(arg).resolve() for arg in sys.argv[1:]] or [path for path in every if path != repo / "Cargo.toml"]
    for path in paths:
        if not path.is_file():
            print(f"error: no manifest at {path}", file=sys.stderr)
            return 2
    shared, internal = workspace_deps(repo), internal_names(every)

    found = [item for path in paths for item in check(path, repo, shared, internal)]
    for path, lineno, message in found:
        print(f"{path.relative_to(repo)}:{lineno}  {message}")
    print(f"  manifests: {len(found)} finding(s) over {len(paths)} manifest(s)")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
