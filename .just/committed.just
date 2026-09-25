# Checks every commit the branch adds to the default branch: Conventional Commits, an allowed type.
check-committed:
    #!/usr/bin/env bash
    set -euo pipefail
    git rev-parse -q --verify HEAD > /dev/null || exit 0
    base=$(git rev-parse -q --verify "origin/${GITHUB_BASE_REF:-HEAD}" || true)
    [[ -n $base ]] || exit 0
    # A pull request is checked out as a merge commit: its own commits are the second parent's.
    head=HEAD
    if git rev-parse -q --verify HEAD^2 > /dev/null; then head=HEAD^2; fi
    range="$(git merge-base "$base" "$head")..$head"
    [[ -n $(git rev-list "$range") ]] || exit 0
    committed "$range"
