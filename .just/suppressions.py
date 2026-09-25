"""Prove every lint suppression still suppresses something.

`#[expect]` and ruff's `RUF100` report their own dead directives; the forms below do not, so neutralize each marker and re-run the tool: a finding that does not come back was not being suppressed.

    suppressions.py <repo>

Every tool runs from PATH, as mise puts it there.
"""

import re
import shutil
import subprocess
import sys
import tempfile
from collections import Counter
from fnmatch import fnmatch
from pathlib import Path

# Trailing, one rule. Cutting only the marker keeps line numbers, so findings still land.
ANSIBLE = re.compile(r"\s*#\s*noqa:\s*([a-z0-9\[\]-]+)\s*$")
# Its own line, above what it covers; one or more codes.
SHELL = re.compile(r"^\s*#\s*shellcheck\s+disable=(SC\d+(?:\s*,\s*SC\d+)*)")
# Its own line, above a node whose layout is deliberate. MULTILINE serves both search and match.
DPRINT = re.compile(r"^(\s*)//\s*dprint-ignore\s*$", re.MULTILINE)


def tracked(repo: Path, *globs: str) -> list[Path]:
    """Every file the tools check, as they select them: tracked or new, and not ignored; and in the tree
    still, as the index outlives a deletion not yet staged."""
    args = ["git", "ls-files", "--cached", "--others", "--exclude-standard", *globs]
    out = subprocess.run(args, cwd=repo, capture_output=True, text=True, check=True)
    return [repo / line for line in out.stdout.split() if (repo / line).is_file()]


def markers(paths: list[Path], pattern: re.Pattern, group: int = 1) -> Counter:
    """Every (path, rule) a suppression claims, counted."""
    found: Counter = Counter()
    for path in paths:
        for line in path.read_text().splitlines():
            match = pattern.search(line)
            if match:
                for rule in match.group(group).split(","):
                    found[(path, rule.strip())] += 1
    return found


def report(dead: list[str], live: int, tool: str) -> bool:
    if dead:
        print(f"  {tool}: {len(dead)} suppression(s) suppress nothing")
        for line in dead:
            print(f"    {line}")
        return False
    print(f"  {tool}: {live} suppression(s), every one load-bearing")
    return True


def check_ansible(repo: Path) -> bool:
    """Require each rule to fire again in the file that silenced it."""
    if not (repo / ".ansible-lint").is_file():
        return report([], 0, "ansible-lint")
    paths = tracked(repo, ".ansible/**/*.yml")
    claimed = markers(paths, ANSIBLE)
    if not claimed:
        return report([], 0, "ansible-lint")

    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        shutil.copytree(repo / ".ansible", work / ".ansible")
        shutil.copy(repo / ".ansible-lint", work / ".ansible-lint")
        for path in paths:
            target = work / path.relative_to(repo)
            target.write_text("\n".join(ANSIBLE.sub("", line) for line in path.read_text().splitlines()) + "\n")
        out = subprocess.run(
            [
                "ansible-lint",
                "--nocolor",
                "-c",
                "../.ansible-lint",
                "-f",
                "pep8",
                "playbooks/",
                "roles/",
            ],
            cwd=work / ".ansible",
            capture_output=True,
            text=True,
        )

    fired: Counter = Counter()
    for line in out.stdout.splitlines():
        hit = re.match(r"^(\S+?):(\d+):(?:\d+:)?\s*([a-z0-9\[\]-]+):", line)
        if hit:
            fired[(repo / ".ansible" / hit.group(1), hit.group(3))] += 1

    dead = [f"{path.relative_to(repo)}: `# noqa: {rule}` x{count}, but stripping it fires {fired[(path, rule)]}" for (path, rule), count in sorted(claimed.items()) if fired[(path, rule)] < count]
    return report(dead, sum(claimed.values()), "ansible-lint")


def check_shell(repo: Path) -> bool:
    """Require each code to fire again in the file that silenced it."""
    paths = tracked(repo, "*.sh")
    claimed = markers(paths, SHELL)
    if not claimed:
        return report([], 0, "shellcheck")

    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        for path in tracked(repo, "*.sh", "*.bash"):
            target = work / path.relative_to(repo)
            target.parent.mkdir(parents=True, exist_ok=True)
            # A bare `#` keeps the line, so reported numbers still match the source.
            target.write_text("\n".join("#" if SHELL.match(line) else line for line in path.read_text().splitlines()) + "\n")
        rel = [str(p.relative_to(repo)) for p in paths]
        out = subprocess.run(["shellcheck", "-x", "-f", "gcc", *rel], cwd=work, capture_output=True, text=True)

    fired: Counter = Counter()
    for line in out.stdout.splitlines():
        hit = re.match(r"^(\S+?):\d+:\d+:\s*\w+:.*\[(SC\d+)\]", line)
        if hit:
            fired[(repo / hit.group(1), hit.group(2))] += 1

    dead = [f"{path.relative_to(repo)}: `disable={code}` x{count}, but stripping it fires {fired[(path, code)]}" for (path, code), count in sorted(claimed.items()) if fired[(path, code)] < count]
    return report(dead, sum(claimed.values()), "shellcheck")


def check_dprint(repo: Path) -> bool:
    """One marker at a time, since a file may carry several; the file must then want reformatting."""
    paths = [p for p in tracked(repo, "*.json", "*.jsonc") if DPRINT.search(p.read_text())]
    sites = [(path, n) for path in paths for n, line in enumerate(path.read_text().splitlines()) if DPRINT.match(line)]
    if not sites:
        return report([], 0, "dprint")

    dead = []
    original = {path: path.read_text() for path, _ in sites}
    try:
        for path, index in sites:
            lines = original[path].splitlines()
            # Still a comment, so the marker's meaning goes and the line does not.
            lines[index] = DPRINT.match(lines[index]).group(1) + "//"
            path.write_text("\n".join(lines) + "\n")
            out = subprocess.run(
                ["dprint", "check", str(path.relative_to(repo))],
                cwd=repo,
                capture_output=True,
                text=True,
            )
            if out.returncode == 0:
                dead.append(f"{path.relative_to(repo)}:{index + 1}: the layout it protects is what dprint would write anyway")
            path.write_text(original[path])
    finally:
        for path, text in original.items():
            path.write_text(text)
    return report(dead, len(sites), "dprint")


def check_excludes(repo: Path) -> bool:
    """An exclude list is a suppression too, reporting even less: a path that matches nothing.

    Only the two tools fed an explicit file list are decidable; dprint walks the tree and its excludes name build-created directories, dead-looking on a clean checkout though they are not.
    """
    fed = {name: ("ignore", [p.relative_to(repo).as_posix() for p in tracked(repo, "*.yml", "*.yaml")]) for name in (".yamllint", ".yamllint.yaml", ".yamllint.yml")} | {
        ".ansible-lint": (
            "exclude_paths",
            [p.relative_to(repo).as_posix() for p in tracked(repo, ".ansible/playbooks/*", ".ansible/roles/*")],
        ),
    }
    dead, live = [], 0
    for name, (key, paths) in fed.items():
        if not (repo / name).is_file():
            continue
        text = (repo / name).read_text()
        block = re.search(rf"^{key}:\s*\|?\s*\n((?:[ \t]+.*\n|\n)*)", text, re.MULTILINE)
        if not block:
            continue
        for entry in re.findall(r"^\s*-?\s*(\S+)\s*$", block.group(1), re.MULTILINE):
            pattern = entry.rstrip("/")
            hit = any(fnmatch(path, pattern) or fnmatch(path, f"{pattern}/*") or path.startswith(f"{pattern}/") for path in paths)
            live += hit
            if not hit:
                dead.append(f"{name}: `{entry}` matches none of the {len(paths)} files the tool is given")
    return report(dead, live, "excludes")


def check_self_auditing(repo: Path) -> bool:
    """The self-reporting forms must keep the rule that makes them report."""
    if not (repo / "ruff.toml").is_file():
        return True
    ruff = (repo / "ruff.toml").read_text()
    if '"RUF100"' not in ruff and '"RUF"' not in ruff:
        print("  ruff: RUF100 is not selected, so a dead `# noqa` would go unreported")
        return False
    print("  ruff: RUF100 selected, so a dead `# noqa` reports itself")
    return True


def main() -> None:
    repo = Path(sys.argv[1]).resolve()
    ok = all(
        [
            check_self_auditing(repo),
            check_ansible(repo),
            check_shell(repo),
            check_dprint(repo),
            check_excludes(repo),
        ]
    )
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
