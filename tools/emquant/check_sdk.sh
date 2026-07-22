#!/usr/bin/env bash
set -euo pipefail

sdk_root=${1:?usage: check_sdk.sh /path/to/EMQuantAPI_CPP_Mac}
header="$sdk_root/x64/EmQuantAPI/EmQuantAPI.h"
sample="$sdk_root/x64/EmQuantAPITestExe/main.cpp"
library="$sdk_root/x64/bin/libEMQuantAPIx64.dylib"
[[ -f "$header" ]] || { echo "missing header: $header" >&2; exit 1; }
[[ -f "$sample" ]] || { echo "missing sample: $sample" >&2; exit 1; }
[[ -f "$library" ]] || { echo "missing library: $library" >&2; exit 1; }
command -v c++ >/dev/null 2>&1 || { echo "c++ compiler is required" >&2; exit 1; }
c++ -std=c++11 -I"$sdk_root/x64/EmQuantAPI" -fsyntax-only "$sample"
printf 'EMQuant SDK layout and sample syntax: ok\n'
