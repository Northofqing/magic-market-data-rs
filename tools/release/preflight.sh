#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
cd "$repo_root"

preflight_root="$repo_root/target/preflight"
mkdir -p "$preflight_root"
preflight_target_dir=$(mktemp -d "$preflight_root/run.XXXXXX")
cleanup_preflight() {
  rm -rf -- "$preflight_target_dir"
}
trap cleanup_preflight EXIT

cargo fmt --all -- --check
bash -n tools/release/preflight.sh tools/release/package.sh
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo check --workspace --all-targets --locked --offline
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo build --workspace --all-targets --release --locked --offline
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo test --workspace --all-targets --locked --offline
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo clippy --workspace --all-targets --locked --offline -- -D warnings
CARGO_TARGET_DIR="$preflight_target_dir" RUSTDOCFLAGS='-D warnings' \
  cargo doc --workspace --no-deps --locked --offline
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo test --workspace --doc --locked --offline
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
printf 'release preflight: passed\n'
