# Runs every test, then the doctests nextest leaves to cargo; under CI, with nextest's `ci` profile.
check-nextest:
    cargo nextest run --workspace --all-features --no-tests=pass --profile {{ if env("CI", "") == "" { "default" } else { "ci" } }}
    cargo test --workspace --all-features --doc
