# devset's demo, both themes: a repository takes a profile from an old release of atxp, v0.6.0 on
# purpose, adds a line of its own to a file the profile owns part of, and moves to the release devset
# pins, the line still there. It records the devset this checkout builds, so the demo of a release
# shows that release; `just fix-docs` keeps the tag it moves to at devset's pin.
Require cargo
Require git

Hide
Type "cargo build --quiet --locked --bin devset && export PATH=$PWD/target/debug:$PATH"
Enter
Wait@900s
Type "cd $(mktemp -d) && mkdir hello && cd hello && git init -q && clear"
Enter
Sleep 500ms
Show

Type "devset add atxp/spelling --git https://github.com/atomix-labs/atxp --tag v0.6.0"
Sleep 600ms
Enter
Wait@30s
Sleep 2s

# The repository's own edit: a key the profile does not own.
Type `echo 'extend-ignore-re = ["TKT-[0-9]+"]' >> typos.toml`
Sleep 600ms
Enter
Sleep 1.5s

Type "devset update --dry-run"
Sleep 400ms
Enter
Wait@30s
Sleep 3s

Type "devset update atxp --tag v0.12.1"
Sleep 400ms
Enter
Wait@30s
Sleep 2s

Hide
Type "clear"
Enter
Show
Type "cat typos.toml"
Sleep 400ms
Enter
Sleep 5s
