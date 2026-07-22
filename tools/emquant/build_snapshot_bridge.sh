#!/usr/bin/env bash
set -euo pipefail

sdk_root=${1:?usage: build_snapshot_bridge.sh /path/to/EMQuantAPI_CPP_Mac [output]}
output=${2:-target/emquant/emquant-snapshot}
mkdir -p "$(dirname "$output")"
c++ -std=c++11 -Wall -Wextra -Werror \
  -isystem "$sdk_root/x64/EmQuantAPI" \
  tools/emquant/snapshot_bridge.cpp -ldl -o "$output"
printf 'built %s\n' "$output"
