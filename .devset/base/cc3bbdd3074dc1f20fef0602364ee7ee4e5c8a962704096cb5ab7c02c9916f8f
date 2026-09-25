# Every cargo command names the host as its target, so RUSTFLAGS reach no host build script: a build
# script built under `target-cpu=native` can die of SIGILL (openssl-sys does).
export CARGO_BUILD_TARGET := `rustc -vV | sed -n 's/^host: //p'`
