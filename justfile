# devset's own recipes; the block below them is the `just` profile's.

# Checks the manual's command reference and schemas are what this checkout's devset prints.
check-docs:
    mise exec -- python3 scripts/docs.py check

# Writes the manual's command reference and schemas from this checkout's devset.
fix-docs:
    mise exec -- python3 scripts/docs.py fix

# Points the manual's pinned install at v$RELEASE_VERSION.
release-docs:
    #!/usr/bin/env bash
    set -euo pipefail
    : "${RELEASE_VERSION:?set it: RELEASE_VERSION=x.y.z just release}"
    sed -i.bak -E "s|(github:atomix-labs/devset@)[0-9]+\.[0-9]+\.[0-9]+|\1$RELEASE_VERSION|" docs/src/ci.md
    rm docs/src/ci.md.bak

# >>> devset: just >>>
# Each atom's recipes, where the atom is applied.
import? '.just/actionlint.just'
import? '.just/ansible-lint.just'
import? '.just/cargo-binaries.just'
import? '.just/cargo-bump.just'
import? '.just/cargo-deny.just'
import? '.just/cargo-hack.just'
import? '.just/cargo-machete.just'
import? '.just/cargo-shear.just'
import? '.just/cargo-workspace-lints.just'
import? '.just/clippy.just'
import? '.just/committed.just'
import? '.just/conftest.just'
import? '.just/dprint.just'
import? '.just/git-cliff.just'
import? '.just/lints-nightly.just'
import? '.just/lychee.just'
import? '.just/manifest-lint.just'
import? '.just/mdbook.just'
import? '.just/mise.just'
import? '.just/msrv.just'
import? '.just/nextest.just'
import? '.just/profile-pins.just'
import? '.just/ruff.just'
import? '.just/rumdl.just'
import? '.just/rust-toolchain.just'
import? '.just/rustdoc.just'
import? '.just/rustfmt.just'
import? '.just/rustup.just'
import? '.just/shellcheck.just'
import? '.just/suppressions.just'
import? '.just/taplo.just'
import? '.just/typos.just'
import? '.just/yamllint.just'
import? '.just/zizmor.just'

# Runs every `check-*` recipe, as CI does, and names each that fails.
check: (_each "check")

# Runs every `fix-*` recipe.
fix: (_each "fix")

# Runs every `bump-*` recipe: each moves what its atom pins, and reports to $BUMP_REPORT_DIR.
bump: (_each "bump")

# Runs every `nightly-*` recipe: the checks too slow for every change.
nightly: (_each "nightly")

# Runs every `setup-*` recipe: what a checkout needs before it builds. `mise bootstrap` runs it.
setup: (_each "setup")

# Runs every `host-*` recipe: the machine's own setup, whose steps may ask for sudo.
host: (_each "host")

# Runs every `release-*` recipe for $RELEASE_VERSION: each writes what a release needs.
release: (_each "release")

# Runs every `package-*` recipe: what a release ships, built for this machine into dist/.
package: (_each "package")

# Runs every recipe named `<verb>-*`, and names each that fails.
_each verb:
    #!/usr/bin/env bash
    set -uo pipefail
    failed=()
    for recipe in $(just --justfile '{{ justfile() }}' --summary); do
        [[ $recipe == {{ verb }}-* ]] || continue
        just --justfile '{{ justfile() }}' "$recipe" || failed+=("$recipe")
    done
    if (( ${#failed[@]} )); then
        echo "failed: ${failed[*]}" >&2
        exit 1
    fi
# <<< devset: just <<<
