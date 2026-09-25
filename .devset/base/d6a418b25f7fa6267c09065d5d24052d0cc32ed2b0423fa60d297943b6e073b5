# Packages every crate crates.io would publish, and builds each from its package, as crates.io will.
check-crates-io:
    mise exec -- python3 .just/crates-io.py check

# Publishes every crate crates.io does not have at its version yet, dependencies first.
publish-crates-io:
    mise exec -- python3 .just/crates-io.py publish
