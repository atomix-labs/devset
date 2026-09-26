# Fails on a dependency no crate uses, by each tool the profile's features turn on, and names each
# tool that found one.
check-cargo-unused:
    #!/usr/bin/env bash
    set -euo pipefail
    failed=()
    cargo machete || failed+=(cargo-machete)
    cargo shear || failed+=(cargo-shear)
    (( ${#failed[@]} == 0 )) || { echo "failed: ${failed[*]}" >&2; exit 1; }

# Removes each dependency no crate uses, by each tool the profile's features turn on, and puts back
# the group markers a removal takes with it, where cargo-manifest is applied.
fix-cargo-unused:
    cargo machete --fix
    cargo shear --fix
    just fix-cargo-manifest
