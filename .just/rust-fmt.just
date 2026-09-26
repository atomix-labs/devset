# Checks that every crate is formatted.
check-rust-fmt:
    cargo fmt --all --check

# Formats every crate.
fix-rust-fmt:
    cargo fmt --all
