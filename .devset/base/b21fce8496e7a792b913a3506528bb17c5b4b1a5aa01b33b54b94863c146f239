# Every cargo command names the host as its target, so RUSTFLAGS reach no host build script: a build
# script built under `target-cpu=native` can die of SIGILL (openssl-sys does).
export CARGO_BUILD_TARGET := `rustc -vV | sed -n 's/^host: //p'`

# Installs rustup where it is missing, with no toolchain of its own, then the toolchain
# rust-toolchain.toml names.
setup-rust-toolchain:
    bash .just/rust-toolchain.sh
