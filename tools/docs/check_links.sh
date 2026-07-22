#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$root"
status=0
while IFS= read -r file; do
  while IFS= read -r link; do
    target=${link#*](}; target=${target%)}
    case "$target" in http://*|https://*|mailto:*) continue;; esac
    target=${target%%#*}
    test -z "$target" && continue
    test -e "$(dirname "$file")/$target" || test -e "$target" || { echo "$file: missing link target $target" >&2; status=1; }
  done < <(rg -o '\[[^]]+\]\([^)]*\)' "$file" || true)
done < <(rg --files docs crates -g '*.md')
exit "$status"
