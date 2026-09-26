# Holds every crate manifest to one shape, which taplo, reading values, cannot see, and fails when a
# crate does not inherit the workspace's lints.
check-cargo-manifest:
    mise exec -- python3 .just/cargo-manifest.py
    cargo workspace-lints

# Puts every dependency under its group, `# external` or `# internal`, the workspace's included.
fix-cargo-manifest:
    mise exec -- python3 .just/cargo-manifest.py --fix
