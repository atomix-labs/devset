# Lints every crate, target and feature; a warning fails.
check-rust-clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Applies clippy's suggestions.
fix-rust-clippy:
    cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged
