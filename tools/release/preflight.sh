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

rustc -vV
cargo -V
cargo fmt --all -- --check
bash -n tools/compliance/check.sh tools/release/preflight.sh tools/release/package.sh
python3 -m unittest discover -s tools/coverage -p 'test_*.py'
coverage_json=${MAGIC_COVERAGE_JSON:-}
require_coverage=${MAGIC_REQUIRE_COVERAGE_EVIDENCE:-0}
if [[ -n "$coverage_json" ]]; then
  if [[ ! -f "$coverage_json" ]]; then
    printf 'coverage evidence does not exist: %s\n' "$coverage_json" >&2
    exit 1
  fi
  python3 tools/coverage/check_thresholds.py "$coverage_json"
elif [[ "$require_coverage" == 1 ]]; then
  printf 'release preflight requires MAGIC_COVERAGE_JSON evidence\n' >&2
  exit 1
else
  printf 'coverage evidence: deferred to required PR/release coverage job\n'
fi
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo check --workspace --all-targets --all-features --locked --offline
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo test --workspace --all-targets --all-features --locked --offline \
  -- --test-threads=1
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo clippy --workspace --all-targets --all-features --locked --offline \
  -- -D warnings
CARGO_TARGET_DIR="$preflight_target_dir" RUSTDOCFLAGS='-D warnings' \
  cargo doc --workspace --all-features --no-deps --locked --offline
CARGO_TARGET_DIR="$preflight_target_dir" \
  cargo test --workspace --all-features --doc --locked --offline \
  -- --test-threads=1
bash tools/docs/check_links.sh
bash tools/compliance/check.sh
git diff --check
printf 'release preflight: passed\n'
