#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
cd "$repo_root"
if ! git diff --quiet || ! git diff --cached --quiet; then
  printf 'release packaging requires a clean tracked worktree\n' >&2
  exit 1
fi
untracked_build_input=
while IFS= read -r candidate; do
  untracked_build_input=$candidate
  break
done < <(git ls-files --others --exclude-standard -- \
  .cargo Cargo.toml Cargo.lock rust-toolchain rust-toolchain.toml crates tools/release)
if [[ -n "$untracked_build_input" ]]; then
  printf 'release packaging rejects untracked build input: %s\n' "$untracked_build_input" >&2
  exit 1
fi
for local_cargo_config in .cargo/config .cargo/config.toml; do
  if [[ -e "$local_cargo_config" ]] \
    && ! git ls-files --error-unmatch "$local_cargo_config" >/dev/null 2>&1; then
    printf 'release packaging rejects local Cargo config: %s\n' "$local_cargo_config" >&2
    exit 1
  fi
done
revision=$(git rev-parse HEAD)
dist_dir=${1:-"$repo_root/target/dist/$revision"}
bin_dir="$dist_dir/bin"
host_triple=$(rustc -vV | sed -n 's/^host: //p')
if [[ -z "$host_triple" ]] \
  || [[ "$host_triple" == *[!A-Za-z0-9_.-]* ]]; then
  printf 'unable to determine a safe Rust host triple\n' >&2
  exit 1
fi
package_build_root="$repo_root/target/package-build"
mkdir -p "$package_build_root"
package_target_dir=$(mktemp -d "$package_build_root/$revision.XXXXXX")
cleanup_package_build() {
  rm -rf -- "$package_target_dir"
}
trap cleanup_package_build EXIT

if [[ -d "$dist_dir" ]] && [[ -n $(find "$dist_dir" -mindepth 1 -print -quit) ]]; then
  printf 'release output already exists and is not empty: %s\n' "$dist_dir" >&2
  exit 1
fi

mkdir -p "$bin_dir"

case "$host_triple" in
  *-windows-*) executable_suffix=.exe ;;
  *) executable_suffix= ;;
esac
example_dir="$package_target_dir/$host_triple/release/examples"

build_probe() {
  package_name=$1
  example_name=$2
  installed_name=$3
  CARGO_TARGET_DIR="$package_target_dir" cargo build -p "$package_name" \
    --example "$example_name" --release --locked --offline --target "$host_triple"
  install -m 0755 "$example_dir/$example_name$executable_suffix" \
    "$bin_dir/$installed_name$executable_suffix"
}

build_probe magic-tdx-rs live_probe magic-tdx-live-probe
build_probe magic-emquant-rs live_probe magic-emquant-live-probe
build_probe magic-tencent-rs live_probe magic-tencent-live-probe
build_probe magic-tencent-rs load_probe magic-tencent-load-probe
build_probe magic-sina-rs live_probe magic-sina-live-probe
build_probe magic-sina-rs load_probe magic-sina-load-probe
build_probe magic-market-router live_probe magic-router-live-probe
build_probe magic-eastmoney-rs live_probe magic-eastmoney-live-probe
build_probe magic-eastmoney-rs load_probe magic-eastmoney-load-probe
build_probe magic-cninfo-rs live_probe magic-cninfo-live-probe
build_probe magic-cninfo-rs load_probe magic-cninfo-load-probe
build_probe magic-ths-rs live_probe magic-ths-live-probe
build_probe magic-ths-rs load_probe magic-ths-load-probe
build_probe magic-cls-rs live_probe magic-cls-live-probe
build_probe magic-cls-rs load_probe magic-cls-load-probe
build_probe magic-jin10-rs live_probe magic-jin10-live-probe
build_probe magic-jin10-rs load_probe magic-jin10-load-probe
build_probe magic-thepaper-rs live_probe magic-thepaper-live-probe
build_probe magic-thepaper-rs load_probe magic-thepaper-load-probe
build_probe magic-baidu-rs live_probe magic-baidu-live-probe
build_probe magic-baidu-rs load_probe magic-baidu-load-probe
build_probe magic-iwencai-rs live_probe magic-iwencai-live-probe
build_probe magic-iwencai-rs load_probe magic-iwencai-load-probe
build_probe magic-exchange-rs live_probe magic-exchange-live-probe
build_probe magic-exchange-rs load_probe magic-exchange-load-probe
build_probe magic-gov-rs live_probe magic-gov-live-probe
build_probe magic-yonhap-rs live_probe magic-yonhap-live-probe
build_probe magic-yonhap-rs load_probe magic-yonhap-load-probe
build_probe magic-wallstreetcn-rs live_probe magic-wallstreetcn-live-probe
build_probe magic-wallstreetcn-rs load_probe magic-wallstreetcn-load-probe

while IFS= read -r -d '' tracked_doc; do
  target_parent="$dist_dir/$(dirname "$tracked_doc")"
  mkdir -p "$target_parent"
  install -m 0644 "$tracked_doc" "$target_parent/"
done < <(git ls-files -z docs)
mkdir -p "$dist_dir/licenses"
install -m 0644 LICENSE-MIT LICENSE-APACHE "$dist_dir/licenses/"
install -m 0644 LICENSES/tdxrs-MIT.txt "$dist_dir/licenses/"
install -m 0644 README.md Cargo.lock "$dist_dir/"
printf '%s\n' "$revision" > "$dist_dir/RELEASE_REVISION"
printf '%s\n' "$host_triple" > "$dist_dir/TARGET_TRIPLE"
rustc -vV > "$dist_dir/RUSTC_VERSION"
cargo -V > "$dist_dir/CARGO_VERSION"
if command -v shasum >/dev/null 2>&1; then
  (
    cd "$dist_dir"
    while IFS= read -r packaged_file; do
      shasum -a 256 "$packaged_file"
    done < <(find bin docs licenses -type f -print | LC_ALL=C sort)
    shasum -a 256 Cargo.lock CARGO_VERSION README.md RELEASE_REVISION \
      RUSTC_VERSION TARGET_TRIPLE
  ) > "$dist_dir/SHA256SUMS"
elif command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$dist_dir"
    while IFS= read -r packaged_file; do
      sha256sum "$packaged_file"
    done < <(find bin docs licenses -type f -print | LC_ALL=C sort)
    sha256sum Cargo.lock CARGO_VERSION README.md RELEASE_REVISION \
      RUSTC_VERSION TARGET_TRIPLE
  ) > "$dist_dir/SHA256SUMS"
else
  printf 'neither shasum nor sha256sum is installed\n' >&2
  exit 1
fi
printf 'release package: %s\n' "$dist_dir"
