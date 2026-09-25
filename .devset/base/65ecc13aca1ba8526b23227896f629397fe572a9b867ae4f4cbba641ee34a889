# Tests the policies, then holds every workflow and action to them.
check-conftest:
    #!/usr/bin/env bash
    set -euo pipefail
    [[ -d policy ]] || exit 0
    conftest verify --policy policy
    inputs=()
    while IFS= read -r -d '' file; do [[ -f $file ]] && inputs+=("$file"); done \
        < <(git ls-files -z --cached --others --exclude-standard -- '.github/*.yml' '.github/*.yaml')
    (( ${#inputs[@]} )) || exit 0
    conftest test --policy policy --all-namespaces --combine "${inputs[@]}"
