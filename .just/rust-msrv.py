"""The oldest rust-version a workspace's crates declare, from `cargo metadata` on stdin.

Exits 1, naming the problem, when no crate declares one.
"""

import json
import sys


def main():
    packages = json.load(sys.stdin)["packages"]
    declared = [package["rust_version"] for package in packages if package.get("rust_version")]
    if not declared:
        sys.exit("no crate declares a rust-version, so there is none to build on")
    print(min(declared, key=lambda version: tuple(int(part) for part in version.split("."))))


if __name__ == "__main__":
    main()
