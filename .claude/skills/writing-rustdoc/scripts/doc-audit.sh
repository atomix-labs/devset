#!/usr/bin/env bash
# The gate for one crate's docs: format check, rustdoc (private items too), doctests, clippy,
# then the mechanical cut-list pass. Every step must be clean; the first failure stops the run.
#
# Usage: doc-audit.sh <crate-dir> [--lint-only] [--advisory]
#   --lint-only   skip the cargo steps (fast reread loop)
#   --advisory    also print widows and wrapped summaries
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
usage() { sed -n '2,7p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }
case "${1:-}" in
    -h|--help) usage; exit 0 ;;
    "") usage >&2; exit 2 ;;
esac
crate=$1
shift
lint_only=false; advisory=()
for arg in "$@"; do
    case "$arg" in
        --lint-only) lint_only=true ;;
        --advisory) advisory=(--advisory) ;;
        *) echo "doc-audit: unknown argument $arg" >&2; exit 2 ;;
    esac
done
[ -f "$crate/Cargo.toml" ] || { echo "doc-audit: $crate has no Cargo.toml" >&2; exit 2; }
name=$(sed -n 's/^name *= *"\([^"]*\)".*/\1/p' "$crate/Cargo.toml" | head -1)
root=$(cd "$crate" && cargo locate-project --workspace --message-format plain | xargs dirname)
cd "$root"

if ! $lint_only; then
    host=$(rustc -vV | sed -n 's/^host: //p')
    features=()
    grep -q '^\[features\]' "$crate/Cargo.toml" && features=(--all-features)
    step() { echo "== $*"; "$@"; }
    step cargo fmt -p "$name" -- --check
    # As `just check-rustdoc` builds it: private items documented, and any warning fatal.
    RUSTDOCFLAGS="-D warnings" step cargo doc -p "$name" --no-deps --target "$host" "${features[@]}" \
        --document-private-items
    step cargo test -p "$name" --doc --target "$host" "${features[@]}"
    step cargo clippy -p "$name" --all-targets --target "$host" "${features[@]}"
    # A dead `#[expect]` is a warning, so it does not fail clippy on its own.
    if cargo clippy -p "$name" --all-targets --target "$host" "${features[@]}" 2>&1 | grep -q 'unfulfilled_lint_expectations'; then
        echo "doc-audit: an #[expect] no longer fires" >&2; exit 1
    fi
fi
echo "== doc-lint"
python3 "$here/doc-lint.py" "$crate" "${advisory[@]}"
