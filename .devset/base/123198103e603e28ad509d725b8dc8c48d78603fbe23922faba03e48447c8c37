# Builds every crate's documentation, private items included; a warning, such as a broken link, fails.
check-rustdoc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --document-private-items
