# Audits every workflow, action and Dependabot config the repository owns, offline, so needing no
# token; a file .gitattributes marks `linguist-vendored` or `linguist-generated` is someone else's.
check-zizmor:
    #!/usr/bin/env bash
    set -euo pipefail
    inputs=()
    while IFS= read -r -d '' file; do [[ -f $file ]] && inputs+=("$file"); done \
        < <(git ls-files -z --cached --others --exclude-standard -- \
            '.github/workflows/*.yml' '.github/workflows/*.yaml' '.github/dependabot.yml' \
            '.github/dependabot.yaml' ':(glob)**/action.yml' ':(glob)**/action.yaml' \
            ':(exclude,attr:linguist-vendored)' ':(exclude,attr:linguist-vendored=true)' \
            ':(exclude,attr:linguist-generated)' ':(exclude,attr:linguist-generated=true)')
    (( ${#inputs[@]} )) || exit 0
    env -u GH_TOKEN -u GITHUB_TOKEN zizmor --offline --config .github/zizmor.yml "${inputs[@]}"
