devset shares your repositories' configuration, and keeps it in sync without
losing local edits: a repository takes profiles, and every update merges with
what it changed.

<!-- changes -->

## Install

```sh
curl --proto '=https' --tlsv1.2 -fsSL https://atomix-labs.github.io/devset/install.sh | sh -s -- -v {version}
```

Or `mise use -g github:atomix-labs/devset@{version}`, `cargo binstall
devset-cli@{version}`, or `cargo install --locked devset-cli@{version}`. Each
archive below holds `completions/` for bash, zsh, fish, elvish and PowerShell;
[the manual](https://atomix-labs.github.io/devset/install.html) says where each
shell looks.

## Verify

Every archive has its `.sha256` beside it, and its build attestation:

```sh
sha256sum -c devset-{version}-x86_64-unknown-linux-musl.tar.xz.sha256
gh attestation verify devset-{version}-x86_64-unknown-linux-musl.tar.xz --repo atomix-labs/devset
```
