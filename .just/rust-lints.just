# Checks the lints only nightly has: their features come from -Zcrate-attr, so no source needs a
# #![feature], and every crate still builds on stable; a crate that enables them itself is no fault.
check-rust-lints:
    RUSTFLAGS="${RUSTFLAGS:-} -Zcrate-attr=feature(non_exhaustive_omitted_patterns_lint) -Zcrate-attr=feature(strict_provenance_lints) -Aduplicate_features -Dnon_exhaustive_omitted_patterns -Dimplicit_provenance_casts" cargo check --workspace --all-targets --all-features --locked
