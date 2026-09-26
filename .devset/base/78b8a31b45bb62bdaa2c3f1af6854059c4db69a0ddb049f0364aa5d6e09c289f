# Packages every crate crates.io would publish, and builds each from its package, as crates.io will.
check-cargo-publish:
    mise exec -- python3 .just/cargo-publish.py check

# Publishes every crate crates.io does not have at its version yet, dependencies first.
publish-cargo-publish:
    mise exec -- python3 .just/cargo-publish.py publish
