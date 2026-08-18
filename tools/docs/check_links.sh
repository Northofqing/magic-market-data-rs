#!/usr/bin/env bash
set -euo pipefail
cd -- "${BASH_SOURCE[0]%/*}/../.."
status=0
while IFS=: read -r file _line link; do
  file=${file//\\//}
  file_dir=${file%/*}
  [[ "$file_dir" == "$file" ]] && file_dir=.
  target=${link#*](}; target=${target%)}
  case "$target" in http://*|https://*|mailto:*) continue;; esac
  target=${target%%#*}
  test -z "$target" && continue
  test -e "$file_dir/$target" || test -e "$target" || { echo "$file: missing link target $target" >&2; status=1; }
done < <(rg -n --with-filename -o -g '*.md' '\[[^]]+\]\([^)]*\)' docs crates || true) || true
exit "$status"
