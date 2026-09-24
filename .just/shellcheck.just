# Lints every shell script the repository owns, following what each sources; a file .gitattributes
# marks `linguist-vendored` or `linguist-generated` is someone else's.
check-shellcheck:
    #!/usr/bin/env bash
    set -euo pipefail
    files=()
    while IFS= read -r -d '' file; do [[ -f $file ]] && files+=("$file"); done \
        < <(git ls-files -z --cached --others --exclude-standard -- '*.sh' '*.bash' \
            ':(exclude,attr:linguist-vendored)' ':(exclude,attr:linguist-vendored=true)' \
            ':(exclude,attr:linguist-generated)' ':(exclude,attr:linguist-generated=true)')
    (( ${#files[@]} )) || exit 0
    shellcheck -x "${files[@]}"
