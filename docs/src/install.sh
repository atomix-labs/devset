#!/bin/sh
# Installs devset from its GitHub releases: the static binary for this machine, checked against the
# release's checksum and, where the GitHub CLI is logged in, its build attestation.
#
#   curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh
#   curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh -s -- -v 0.2.0
#
# Options: -v <version>, -b <dir>, --uninstall, -h. The environment's DEVSET_VERSION and
# DEVSET_INSTALL_DIR say -v and -b; DEVSET_NO_ATTEST=1 skips the attestation check.
#
# Everything is in functions, and `main` is called on the last line: a download cut short runs
# nothing.

set -eu

REPO="atomix-labs/devset"

say() {
    printf 'devset-install: %s\n' "$*" >&2
}

die() {
    say "error: $*"
    exit 1
}

has() {
    command -v "$1" >/dev/null 2>&1
}

usage() {
    cat >&2 <<EOF
Installs devset from https://github.com/$REPO/releases.

Usage: install.sh [-v <version>] [-b <dir>] [--uninstall]

  -v <version>   the release to install, as 0.2.0; the latest unless given
  -b <dir>       where to put devset; \$XDG_BIN_HOME or ~/.local/bin unless given
  --uninstall    remove devset from that directory
  -h, --help     print this

The environment's DEVSET_VERSION and DEVSET_INSTALL_DIR say -v and -b, and
DEVSET_NO_ATTEST=1 skips the check of the build's attestation.
EOF
}

# The Rust target of this machine's binary.
target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os" in
        Linux) ;;
        Darwin)
            # A shell under Rosetta says x86_64 on Apple silicon.
            if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
                arch=arm64
            fi
            ;;
        *) die "no release for $os; build it: cargo install --locked devset-cli" ;;
    esac
    case "$os/$arch" in
        Linux/x86_64 | Linux/amd64) echo x86_64-unknown-linux-musl ;;
        Linux/aarch64 | Linux/arm64) echo aarch64-unknown-linux-musl ;;
        Darwin/arm64) echo aarch64-apple-darwin ;;
        *) die "no release for $os on $arch; build it: cargo install --locked devset-cli" ;;
    esac
}

# Downloads $1 to $2.
fetch() {
    if has curl; then
        curl --proto '=https' --tlsv1.2 -fsSL "$1" -o "$2"
    elif has wget; then
        wget --https-only -q "$1" -O "$2"
    else
        die "neither curl nor wget is installed"
    fi
}

# The latest release's version, from where GitHub's `releases/latest` redirects: no API, no limit.
latest() {
    url="https://github.com/$REPO/releases/latest"
    if has curl; then
        tag=$(curl --proto '=https' --tlsv1.2 -fsSLI -o /dev/null -w '%{url_effective}' "$url")
    elif has wget; then
        tag=$(wget --https-only -q -S --spider "$url" 2>&1 | awk 'tolower($1) == "location:" { url = $2 } END { print url }')
    else
        die "neither curl nor wget is installed"
    fi
    tag=${tag##*/}
    case "$tag" in
        v[0-9]*) echo "${tag#v}" ;;
        *) die "could not find the latest release; name one with -v" ;;
    esac
}

# Checks the archive in $1 against its checksum, in the same directory.
verify_checksum() {
    if has sha256sum; then
        sum="sha256sum"
    elif has shasum; then
        sum="shasum -a 256"
    else
        die "neither sha256sum nor shasum is installed, so the download cannot be checked"
    fi
    (cd "$(dirname "$1")" && $sum -c "$(basename "$1").sha256" >/dev/null 2>&1) ||
        die "$(basename "$1") does not match its checksum"
}

# Checks the build attestation of the archive in $1, for version $2, when the GitHub CLI can.
verify_attestation() {
    if [ "${DEVSET_NO_ATTEST:-}" = 1 ]; then
        say "skipping the attestation check: DEVSET_NO_ATTEST=1"
        return
    fi
    case "$2" in
        0.0.* | 0.1.*)
            say "releases before 0.2.0 carry no attestation"
            return
            ;;
    esac
    if ! has gh; then
        say "the GitHub CLI is not installed; to check the build's attestation too, install it"
        return
    fi
    if ! gh auth status >/dev/null 2>&1; then
        say "the GitHub CLI is not logged in; to check the build's attestation too, run gh auth login"
        return
    fi
    gh attestation verify "$1" --repo "$REPO" >/dev/null ||
        die "$(basename "$1") has no valid attestation from $REPO; see https://github.com/$REPO/attestations"
    say "checked the build's attestation"
}

# Says how to put $1 on PATH, if it is not.
advise() {
    case ":$PATH:" in
        *":$1:"*) ;;
        *) say "$1 is not on your PATH; add it: export PATH=\"$1:\$PATH\"" ;;
    esac
}

main() {
    version=${DEVSET_VERSION:-}
    dir=${DEVSET_INSTALL_DIR:-${XDG_BIN_HOME:-${HOME:?}/.local/bin}}
    uninstall=
    while [ $# -gt 0 ]; do
        case "$1" in
            -v) [ $# -ge 2 ] || die "-v needs a version"; version=$2; shift 2 ;;
            -b) [ $# -ge 2 ] || die "-b needs a directory"; dir=$2; shift 2 ;;
            --uninstall) uninstall=1; shift ;;
            -h | --help) usage; exit 0 ;;
            *) usage; die "unknown option: $1" ;;
        esac
    done

    if [ -n "$uninstall" ]; then
        if [ -e "$dir/devset" ]; then
            rm -f "$dir/devset"
            say "removed $dir/devset"
        else
            say "$dir/devset is not there"
        fi
        return
    fi

    target=$(target)
    version=${version#v}
    [ -n "$version" ] || version=$(latest)
    archive="devset-$version-$target.tar.xz"
    base="https://github.com/$REPO/releases/download/v$version"

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT INT TERM
    say "downloading devset $version for $target"
    fetch "$base/$archive" "$tmp/$archive" || die "devset $version has no $archive"
    fetch "$base/$archive.sha256" "$tmp/$archive.sha256" || die "devset $version has no checksum for $archive"
    verify_checksum "$tmp/$archive"
    verify_attestation "$tmp/$archive" "$version"

    tar -xJf "$tmp/$archive" -C "$tmp" devset || die "could not unpack $archive: tar needs xz"
    mkdir -p "$dir"
    # Into place in one rename, so a running devset is never half written.
    cp "$tmp/devset" "$dir/.devset.$$"
    chmod 755 "$dir/.devset.$$"
    mv -f "$dir/.devset.$$" "$dir/devset"
    say "installed devset $version to $dir/devset"
    advise "$dir"
}

main "$@"
