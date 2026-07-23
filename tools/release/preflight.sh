#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
cd "$repo_root"

cargo fmt --all -- --check
bash -n tools/release/preflight.sh tools/release/package.sh
RUSTUP_TOOLCHAIN=1.83.0 cargo check --workspace --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo test --workspace --all-targets --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo clippy --workspace --all-targets --locked --offline -- -D warnings
RUSTDOCFLAGS='-D warnings' RUSTUP_TOOLCHAIN=1.83.0 \
  cargo doc --workspace --no-deps --locked --offline
RUSTUP_TOOLCHAIN=1.83.0 cargo test --workspace --doc --locked --offline
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
printf 'release preflight: passed\n'
