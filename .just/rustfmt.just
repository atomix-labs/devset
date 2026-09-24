# Checks that every crate is formatted.
check-rustfmt:
    cargo fmt --all --check

# Formats every crate.
fix-rustfmt:
    cargo fmt --all
