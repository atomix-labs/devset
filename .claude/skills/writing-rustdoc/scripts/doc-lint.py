#!/usr/bin/env python3
"""Mechanical pass over a crate's docs and comments: the cut list, checked by machine.

Usage: doc-lint.py <crate-dir> [--advisory] [--quiet]

Reads every .rs file under src/, tests/, benches/, examples/ and the Cargo.toml. Prints one line per
finding as `path:line: [tag] message`. Exit 1 if any error-class finding, else 0. Advisory findings
(widows, wrapped summaries) print only with --advisory. A finding is a prompt to reread the line,
not an order: the exemplar crates carry a handful.
"""

import re
import sys
from pathlib import Path

DOC = re.compile(r"^\s*(///|//!)( ?)(.*)$")
COMMENT = re.compile(r"^\s*(//[/!]?)( ?)(.*)$")

WEAK_OPENER = re.compile(
    r"^(This (function|method|fn|struct|type|trait|enum|module|crate|macro|impl|is|does|returns|"
    r"creates|provides|represents|allows|holds|contains|wraps|implements|defines) |Returns? |"
    r"Creates? (a |an |the )?new|Represents? |Used (to|for|by) |Helper |Wrapper |Provides? |"
    r"Allows? |Gets? |Sets? |A (struct|trait|function|fn|method|type|enum|module|helper|wrapper|"
    r"marker|utility|simple|generic) |An (enum|implementation|abstraction) |"
    r"The (struct|trait|function|type|enum|module) (that|which|for) )",
)
ITEM_SECTION = re.compile(r"^# (Safety|Errors|Panics|Examples)\s*$")
SENTENCE_END = re.compile(r"[.!?]\s+(?=[A-Z`\[*])")
FILLER = re.compile(
    r"\b(simply|basically|essentially|just|note that|in order to|it is important|please|etc\.|"
    r"and so on|various|for more (information|details)|see also|make sure|should probably)\b",
    re.I,
)
MARKETING = re.compile(
    r"\b(powerful|easy to use|user[- ]friendly|efficient|flexible|robust|convenient|zero[- ]cost|"
    r"venue[- ]neutral|blazingly|seamless(ly)?|elegant(ly)?|leverag(e|es|ing))\b",
    re.I,
)
PROCESS = re.compile(r"\b(TODO|FIXME|XXX|HACK|phase [0-9A-Za-z]+|RC[0-9]+\b|chunk [0-9]+)\b")
RULE_ID = re.compile(r"\b(STR|STY|UNS|API|ERG|DOC|PTN|BLD|TST|BCH|PRF|GIT|CLD|MNT|OVR|POR|MIN)-[A-Z]+-[0-9]+\b")
BAD_HEADING = re.compile(r"^# (Example|Arguments?|Parameters?|Returns?|HOT|Overview|Introduction|Usage|Notes?|Getting started|Implementation details)\s*$")
BAD_ERRORS_PROSE = re.compile(r"^(Returns?|Fails?|Errors?) |\bif\b")
SAFETY_RESTATE = re.compile(r"SAFETY:\s*(this is safe|safe because|it is safe|trust)", re.I)
WTX_NAME = re.compile(r"\bwtx[-_][a-z][a-z0-9_-]*")


def crate_meta(crate: Path):
    manifest = (crate / "Cargo.toml").read_text()
    name = re.search(r'^name\s*=\s*"([^"]+)"', manifest, re.M)
    deps = set(re.findall(r"^(wtx-[a-z0-9-]+)\s*=", manifest, re.M))
    return (name.group(1) if name else crate.name), deps, manifest


def paragraphs(block):
    """Split a doc block (list of text lines) into paragraphs outside code fences."""
    out, para, fence = [], [], False
    for text in block:
        if text.strip().startswith("```"):
            fence = not fence
            if para:
                out.append(para)
                para = []
            continue
        if fence:
            continue
        if text.strip() == "" or text.startswith(("# ", "|", "- ", "* ")) or re.match(r"^\[[^\]]+\]:\s", text) or re.match(r"^\d+\. ", text):
            if para:
                out.append(para)
                para = []
            continue
        para.append(text)
    if para:
        out.append(para)
    return out


def lint_file(path: Path, me: str, deps: set, findings):
    lines = path.read_text().splitlines()
    in_src = "src" in path.parts
    if in_src and path.name != "lib.rs":
        first = next((raw for raw in lines if raw.strip()), "")
        if not first.startswith("//!"):
            findings.append(
                (
                    path,
                    1,
                    "error",
                    "no `//!` header: every module file opens with one line on its role",
                )
            )
    i, n = 0, len(lines)
    fence = False
    while i < n:
        line = lines[i]
        m = DOC.match(line)
        if not m:
            c = COMMENT.match(line)
            if c and not fence:
                text = c.group(3)
                if "—" in text:
                    findings.append((path, i + 1, "error", "em dash: rewrite with `:` `;` `,` or a parenthesis"))
                if PROCESS.search(text):
                    findings.append((path, i + 1, "error", "dev-process marker in a comment"))
                if RULE_ID.search(text):
                    findings.append((path, i + 1, "error", "convention rule ID in a comment"))
                if SAFETY_RESTATE.search(text):
                    findings.append(
                        (
                            path,
                            i + 1,
                            "error",
                            "`// SAFETY:` asserts safety instead of naming the fact",
                        )
                    )
            if re.match(r"^\s*(pub(\([a-z]+\))? )?mod [a-z_]+;", line) and i > 0 and DOC.match(lines[i - 1]) and path.name == "lib.rs":
                findings.append(
                    (
                        path,
                        i,
                        "error",
                        "doc on a `mod x;` line: the file's own `//!` is the one home",
                    )
                )
            i += 1
            continue
        # a doc block: gather it
        prefix = m.group(1)
        start = i
        block = []
        while i < n:
            mm = re.match(r"^\s*" + re.escape(prefix) + r"( ?)(.*)$", lines[i])
            if not mm:
                break
            block.append(mm.group(2))
            i += 1
        item = lines[i] if i < n else ""
        check_block(path, start, block, item, me, deps, findings)


def check_block(path, start, block, item, me, deps, findings):
    fence = False
    for k, text in enumerate(block):
        ln = start + k + 1
        stripped = text.strip()
        if stripped.startswith("```"):
            fence = not fence
            if stripped == "```ignore" or stripped.startswith("```ignore"):
                findings.append((path, ln, "error", "`ignore` doctest: fix it, `no_run` it, or make it `text`"))
            continue
        if fence:
            if re.search(r"\.unwrap\(\)", text):
                findings.append(
                    (
                        path,
                        ln,
                        "error",
                        '`unwrap()` in an example: `.expect("<why it cannot fail>")` or `?`',
                    )
                )
            continue
        if "—" in text:
            findings.append((path, ln, "error", "em dash: rewrite with `:` `;` `,` or a parenthesis"))
        if BAD_HEADING.match(stripped):
            findings.append(
                (
                    path,
                    ln,
                    "error",
                    f"heading `{stripped}`: not a house section (`# Examples` plural; no Arguments/Returns/HOT)",
                )
            )
        if ITEM_SECTION.match(stripped) and k + 1 < len(block) and block[k + 1].strip() == "":
            findings.append(
                (
                    path,
                    ln,
                    "error",
                    f"blank line after `{stripped}`: the content starts on the next line",
                )
            )
        if stripped == "# Errors" and k + 1 < len(block):
            nxt = block[k + 1].strip()
            if BAD_ERRORS_PROSE.search(nxt) and not nxt.startswith(("As [", "Whatever")):
                findings.append(
                    (
                        path,
                        ln + 1,
                        "error",
                        "`# Errors` as prose: `[`Variant`], <condition>.` (a `- ` list when several)",
                    )
                )
        if FILLER.search(text):
            findings.append((path, ln, "error", f"filler: `{FILLER.search(text).group(0)}`"))
        if MARKETING.search(text):
            findings.append((path, ln, "error", f"unverifiable adjective: `{MARKETING.search(text).group(0)}`"))
        if PROCESS.search(text):
            findings.append((path, ln, "error", "dev-process marker in a doc"))
        if RULE_ID.search(text):
            findings.append((path, ln, "error", "convention rule ID in a doc"))
        for name in WTX_NAME.findall(text):
            norm = name.replace("_", "-")
            if norm != me and norm not in deps and not norm.startswith(me):
                findings.append(
                    (
                        path,
                        ln,
                        "error",
                        f"names `{name}`, neither this crate nor a dependency: a crate speaks only for itself",
                    )
                )
    # summary shape
    ps = paragraphs(block)
    if ps:
        first = ps[0]
        head = first[0].strip()
        if WEAK_OPENER.match(head):
            findings.append((path, start + 1, "error", f"weak opener: `{head[:60]}`"))
        sentences = len(SENTENCE_END.findall(" ".join(first))) + 1
        if sentences >= 3 or len(first) >= 4:
            public = item.lstrip().startswith("pub") or "//!" in "".join(block[:0]) or block is None
            findings.append(
                (
                    path,
                    start + 1,
                    "error" if public else "advisory",
                    "first paragraph is a body: one sentence, a blank `///`, then the rest",
                )
            )
        elif len(first) >= 2:
            findings.append(
                (
                    path,
                    start + 1,
                    "advisory",
                    f"summary wraps to {len(first)} lines: fine for one colon-structured sentence, else split",
                )
            )
        if head and not head.endswith((".", ":", "`", ")")) and not head.startswith(("#", "|", "-", "[")) and len(first) == 1 and not item.strip().startswith("#[error"):
            findings.append((path, start + 1, "advisory", "summary does not end with a period"))
    for p in ps:
        if len(p) > 1 and len(p[-1].split()) <= 3:
            findings.append(
                (
                    path,
                    start + block.index(p[-1]) + 1,
                    "advisory",
                    f"widow: `{p[-1].strip()}`; cut a few words or rebalance",
                )
            )


def main(argv):
    if len(argv) < 2 or argv[1] in ("-h", "--help"):
        print(__doc__.strip())
        return 0
    crate = Path(argv[1]).resolve()
    advisory = "--advisory" in argv
    quiet = "--quiet" in argv
    if not (crate / "Cargo.toml").exists():
        print(f"doc-lint: {crate} has no Cargo.toml", file=sys.stderr)
        return 2
    me, deps, manifest = crate_meta(crate)
    findings = []
    desc = re.search(r'^description\s*=\s*"([^"]*)"', manifest, re.M)
    if desc:
        d = desc.group(1)
        if d and (d[0].isupper() or not d.endswith(".")):
            findings.append(
                (
                    crate / "Cargo.toml",
                    1,
                    "error",
                    "description: lowercase pitch clause with a trailing period",
                )
            )
    if re.search(r"^\[dependencies\]", manifest, re.M) and "# internal" not in manifest and "# external" not in manifest:
        findings.append(
            (
                crate / "Cargo.toml",
                1,
                "error",
                "dependencies carry no `# internal` / `# external` group comments",
            )
        )
    for sub in ("src", "tests", "benches", "examples"):
        d = crate / sub
        if d.is_dir():
            for f in sorted(d.rglob("*.rs")):
                lint_file(f, me, deps, findings)
    errors = 0
    for path, ln, kind, msg in findings:
        if kind == "advisory" and not advisory:
            continue
        if kind == "error":
            errors += 1
        if not quiet or kind == "error":
            try:
                rel = path.relative_to(Path.cwd())
            except ValueError:
                rel = path
            print(f"{rel}:{ln}: [{kind}] {msg}")
    adv = sum(1 for f in findings if f[2] == "advisory")
    print(f"doc-lint {me}: {errors} error(s), {adv} advisory" + ("" if advisory else " (show with --advisory)"))
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
