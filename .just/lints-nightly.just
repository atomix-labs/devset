# Checks the lints only nightly has: their features come from -Zcrate-attr, so no source needs a
# #![feature], and every crate still builds on stable.
check-lints-nightly:
    RUSTFLAGS="${RUSTFLAGS:-} -Zcrate-attr=feature(non_exhaustive_omitted_patterns_lint) -Zcrate-attr=feature(strict_provenance_lints) -Dnon_exhaustive_omitted_patterns -Dimplicit_provenance_casts" cargo check --workspace --all-targets --all-features --locked
