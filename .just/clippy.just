# Lints every crate, target and feature; a warning fails.
check-clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Applies clippy's suggestions.
fix-clippy:
    cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged
