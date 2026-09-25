# Moves every Cargo requirement and git revision past the cooldown, one at a time, and settles the locks.
bump-cargo-bump:
    mise exec -- python3 .just/cargo-bump.py "${BUMP_REPORT_DIR:+$BUMP_REPORT_DIR/cargo.md}"
