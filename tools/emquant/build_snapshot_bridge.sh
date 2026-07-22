#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
sdk_root=${1:?usage: build_snapshot_bridge.sh /path/to/EMQuantAPI_CPP_Mac [output]}
output=${2:-"$repo_root/target/emquant/emquant-snapshot"}
sdk_bin="$sdk_root/x64/bin"
runtime_dir="$(dirname "$output")/runtime"
mkdir -p "$(dirname "$output")"
c++ -std=c++11 -Wall -Wextra -Werror \
  -isystem "$sdk_root/x64/EmQuantAPI" \
  "$script_dir/snapshot_bridge.cpp" -ldl -o "$output"
mkdir -p "$runtime_dir"
runtime_library="$runtime_dir/libEMQuantAPIx64.dylib"
runtime_activator="$runtime_dir/loginactivator_mac"
if [[ -L "$runtime_library" ]]; then
  unlink "$runtime_library"
fi
install -m 0755 "$sdk_bin/libEMQuantAPIx64.dylib" "$runtime_library"
if [[ $(uname -s) == Darwin ]]; then
  # The SDK download is currently unsigned and quarantined. Sign only the
  # ignored project-local copy so dlopen works without altering vendor files.
  xattr -c "$runtime_library"
  codesign --force --sign - --timestamp=none "$runtime_library"
fi
install -m 0755 "$sdk_bin/loginactivator_mac" "$runtime_activator"
if [[ $(uname -s) == Darwin ]]; then
  xattr -c "$runtime_activator"
  codesign --force --sign - --timestamp=none "$runtime_activator"
fi
mkdir -p "$runtime_dir/image"
cp -R "$sdk_bin/image/." "$runtime_dir/image/"
install -m 0600 "$sdk_bin/ServerList.json.e" "$runtime_dir/ServerList.json.e"
if [[ -f "$sdk_bin/userInfo" ]]; then
  install -m 0600 "$sdk_bin/userInfo" "$runtime_dir/userInfo"
elif [[ -f "$runtime_dir/userInfo" ]]; then
  chmod 0600 "$runtime_dir/userInfo"
  printf 'preserved existing EMQuant activation file %s\n' "$runtime_dir/userInfo"
else
  printf 'warning: EMQuant activation is required; run %s\n' "$runtime_activator" >&2
fi
printf 'built %s\n' "$output"
printf 'installed SDK runtime %s\n' "$runtime_dir"
