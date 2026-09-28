#!/usr/bin/env bash
# Builds the workspace's binaries for this machine, and archives each as
# dist/<bin>-<version>-<target>.tar.xz, the binary at the archive's root, with a .sha256 beside it:
# the names mise's github backend, cargo-binstall and ubi find on their own. On Linux the target is
# musl, so the binary is static and runs on any distribution; elsewhere it is the host's.
# <version> is $RELEASE_VERSION, or the workspace's. With --completions, each archive also holds
# completions/, the binary's `completions <shell>` for every shell it answers, named as each shell
# looks for them.
set -euo pipefail

completions=false
[[ ${1:-} != --completions ]] || completions=true

host=$(rustc -vV | sed -n 's/^host: //p')
case $host in
    *-linux-gnu) target=${host%-gnu}-musl ;;
    *) target=$host ;;
esac
rustup target add "$target" > /dev/null

metadata=$(cargo metadata --no-deps --format-version 1)
field() { python3 -c "import json, sys; m = json.load(sys.stdin); print($1)" <<< "$metadata"; }
version=${RELEASE_VERSION:-$(field 'm["packages"][0]["version"]')}
bins=$(field '"\n".join(t["name"] for p in m["packages"] for t in p["targets"] if "bin" in t["kind"])')
out=$(field 'm["target_directory"]')/$target/release

cargo build --release --locked --bins --target "$target"
mkdir -p dist
for bin in $bins; do
    archive=dist/$bin-$version-$target.tar.xz
    if $completions; then
        # The binary at the root still, where mise, cargo-binstall and ubi find it.
        stage=$(mktemp -d)
        cp "$out/$bin" "$stage/"
        mkdir "$stage/completions"
        "$out/$bin" completions bash > "$stage/completions/$bin.bash"
        "$out/$bin" completions zsh > "$stage/completions/_$bin"
        "$out/$bin" completions fish > "$stage/completions/$bin.fish"
        "$out/$bin" completions elvish > "$stage/completions/$bin.elv"
        "$out/$bin" completions powershell > "$stage/completions/_$bin.ps1"
        tar -C "$stage" -cJf "$archive" "$bin" completions
        rm -rf "$stage"
    else
        tar -C "$out" -cJf "$archive" "$bin"
    fi
    if command -v sha256sum > /dev/null; then
        (cd dist && sha256sum "${archive#dist/}" > "${archive#dist/}.sha256")
    else
        (cd dist && shasum -a 256 "${archive#dist/}" > "${archive#dist/}.sha256")
    fi
    echo "packaged $archive"
done
