# devset's own recipes; the block below them is the `just` profile's.

# Checks the manual's command reference and schemas are what this checkout's devset prints.
check-docs:
    mise exec -- python3 scripts/docs.py check

# Writes the manual's command reference and schemas from this checkout's devset.
fix-docs:
    mise exec -- python3 scripts/docs.py fix

# Puts the installer, which the manual serves too, among what a release attaches.
package-installer:
    mkdir -p dist
    cp docs/src/install.sh dist/install.sh

# Installs the latest release and a pinned one with the installer, into a scratch directory, and
# takes one away again; the network's, so nightly.
nightly-installer:
    #!/usr/bin/env bash
    set -euo pipefail
    dir=$(mktemp -d)
    trap 'rm -rf "$dir"' EXIT
    sh docs/src/install.sh -b "$dir"
    "$dir/devset" --version
    sh docs/src/install.sh -v 0.1.3 -b "$dir"
    [[ $("$dir/devset" --version) == "devset 0.1.3" ]]
    sh docs/src/install.sh --uninstall -b "$dir"
    [[ ! -e $dir/devset ]]

# Points the manual's pinned install at v$RELEASE_VERSION.
release-docs:
    #!/usr/bin/env bash
    set -euo pipefail
    : "${RELEASE_VERSION:?set it: RELEASE_VERSION=x.y.z just release}"
    sed -i.bak -E "s|(github:atomix-labs/devset@)[0-9]+\.[0-9]+\.[0-9]+|\1$RELEASE_VERSION|" docs/src/ci.md
    rm docs/src/ci.md.bak

# >>> devset: just >>>
# Each active profile's recipes.
import? '.just/agents.just'
import? '.just/cargo-binaries.just'
import? '.just/cargo-bump.just'
import? '.just/cargo-deny.just'
import? '.just/cargo-hack.just'
import? '.just/cargo-manifest.just'
import? '.just/cargo-nextest.just'
import? '.just/cargo-profiles.just'
import? '.just/cargo-publish.just'
import? '.just/cargo-unused.just'
import? '.just/cargo-workspace.just'
import? '.just/devset.just'
import? '.just/dprint.just'
import? '.just/editorconfig.just'
import? '.just/git-attributes.just'
import? '.just/git-changelog.just'
import? '.just/git-commits.just'
import? '.just/git-ignore.just'
import? '.just/github-automation.just'
import? '.just/github-bump.just'
import? '.just/github-ci.just'
import? '.just/github-dependabot.just'
import? '.just/github-nightly.just'
import? '.just/github-release.just'
import? '.just/github-templates.just'
import? '.just/github-watch.just'
import? '.just/github-workflow-lint.just'
import? '.just/just.just'
import? '.just/lychee.just'
import? '.just/markdown.just'
import? '.just/mdbook.just'
import? '.just/mise.just'
import? '.just/project.just'
import? '.just/python.just'
import? '.just/rust.just'
import? '.just/rust-clippy.just'
import? '.just/rust-doc.just'
import? '.just/rust-fmt.just'
import? '.just/rust-lints.just'
import? '.just/rust-msrv.just'
import? '.just/rust-toolchain.just'
import? '.just/setup.just'
import? '.just/shell.just'
import? '.just/spelling.just'
import? '.just/suppressions.just'
import? '.just/toml.just'
import? '.just/vhs.just'
import? '.just/vscode.just'
import? '.just/yaml.just'

# Runs every `check-*` recipe, as CI does, and names each that fails.
check: (_each "check")

# Runs every `fix-*` recipe.
fix: (_each "fix")

# Runs every `bump-*` recipe: each moves what its profile pins, and reports to $BUMP_REPORT_DIR.
bump: (_each "bump")

# Runs every `nightly-*` recipe: the checks too slow for every change.
nightly: (_each "nightly")

# Runs every `test-*` recipe: the suites too slow for `just check`, which CI runs beside it.
test: (_each "test")

# Runs every `setup-*` recipe: what a checkout needs before it builds. `mise bootstrap` runs it.
setup: (_each "setup")

# Runs every `host-*` recipe: the machine's own setup, whose steps may ask for sudo.
host: (_each "host")

# Runs every `release-*` recipe for $RELEASE_VERSION: each writes what a release needs.
release: (_each "release")

# Runs every `package-*` recipe: what a release ships, built for this machine into dist/.
package: (_each "package")

# Runs every `publish-*` recipe: what a release puts in a registry, once the release is out.
publish: (_each "publish")

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
