# devset's demo, both themes: a repository takes a profile from an old release, adds a line of its
# own to a file the profile owns part of, and moves to a newer release, the line still there.
Require devset
Require git

Hide
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

Type "devset update atxp --tag v0.8.0"
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
