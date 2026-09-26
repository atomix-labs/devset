"""The crates a release publishes to crates.io: every package of the workspace whose `publish` allows
crates.io.

Usage: crates-io.py check | publish

`check` packages each, and builds it from its package, as crates.io will. Between releases, while
crates.io has a crate at its version already, cargo would build its dependents against crates.io's
copy, not the working tree's, so each is packaged without the build; a release's check, its versions
new, builds them all. `publish` publishes each that crates.io does not have at its version,
dependencies first, so a release that stopped halfway publishes the rest when it runs again.
"""

import json
import subprocess
import sys
import urllib.error
import urllib.request

API = "https://crates.io/api/v1/crates"
# crates.io refuses a request without one that names who is asking.
USER_AGENT = "atxp crates-io (https://github.com/atomix-labs/atxp)"


def publishable():
    """The workspace's packages crates.io takes, as (name, version), in the workspace's order."""
    out = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
        capture_output=True,
        text=True,
        check=True,
    )
    # `publish` is null for any registry, or the registries allowed: `[]` is `publish = false`.
    return [
        (package["name"], package["version"])
        for package in json.loads(out.stdout)["packages"]
        if package["publish"] is None or "crates-io" in package["publish"]
    ]


def published(name, version):
    """Whether crates.io has `name` at `version`."""
    request = urllib.request.Request(f"{API}/{name}/{version}", headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=60):
            return True
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise


def cargo(verb, crates, *flags):
    """Runs `cargo <verb>` over `crates`, the lock as it is; cargo's exit code."""
    packages = [arg for name, _ in crates for arg in ("--package", name)]
    return subprocess.run(["cargo", verb, "--locked", *flags, *packages], check=False).returncode


def check():
    """Packages every publishable crate, and builds each from its package."""
    crates = publishable()
    if not crates:
        print("crates-io: the workspace publishes no crate")
        return 0
    # The working tree as it is: a check runs before its changes are committed.
    flags = ["--allow-dirty"]
    if any(published(name, version) for name, version in crates):
        print("crates-io: crates.io has these versions already; packaging without the build")
        flags.append("--no-verify")
    return cargo("package", crates, *flags)


def publish():
    """Publishes every publishable crate crates.io does not have yet."""
    crates = publishable()
    remaining = [(name, version) for name, version in crates if not published(name, version)]
    for name, version in crates:
        if (name, version) not in remaining:
            print(f"crates-io: {name} {version} is on crates.io already")
    # A publish is of commits alone, so cargo refuses a working tree with changes.
    return cargo("publish", remaining) if remaining else 0


if __name__ == "__main__":
    commands = {"check": check, "publish": publish}
    if len(sys.argv) != 2 or sys.argv[1] not in commands:
        sys.exit(__doc__)
    sys.exit(commands[sys.argv[1]]())
