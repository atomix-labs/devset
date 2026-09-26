#!/usr/bin/env bash
# Claude Code's Stop hook: the agent stops once `just check` passes on what it changed. A tree that
# matches HEAD has nothing to check. Otherwise `just check` runs, and its failures, on stderr with
# exit 2, keep the agent at work. Once it has blocked, and nothing changed since, the agent may
# stop, so a failure it cannot fix never loops.
set -uo pipefail

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
[[ -n $(git status --porcelain 2> /dev/null) ]] || exit 0
state=$(git rev-parse --git-path claude-check)
tree=$({
    git diff HEAD
    git ls-files -z --others --exclude-standard | xargs -0 -r sha256sum
} 2> /dev/null | sha256sum)
if [[ -f $state && $(< "$state") == "$tree" ]]; then
    rm -f "$state"
    exit 0
fi
if output=$(just check 2>&1); then
    rm -f "$state"
    exit 0
fi
printf '%s\n' "$tree" > "$state"
printf '%s\n' "$output" | tail -n 40 >&2
echo "just check fails: fix what it names, then stop." >&2
exit 2
