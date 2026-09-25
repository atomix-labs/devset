# Builds every crate on the oldest rust-version one declares, every target and feature included.
check-msrv:
    #!/usr/bin/env bash
    set -euo pipefail
    version=$(cargo metadata --no-deps --format-version 1 | mise exec -- python3 .just/msrv.py)
    rustup toolchain install "$version" --profile minimal --no-self-update
    cargo "+$version" check --workspace --all-targets --all-features --locked
