#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
revision="$(git -C "${repo_root}" rev-parse HEAD)"
if [[ ! "${revision}" =~ ^[0-9a-f]{40}$ ]]; then
  echo "benchmark requires a full lowercase Git SHA-1 revision" >&2
  exit 2
fi
initial_status="$(git -C "${repo_root}" status --porcelain=v1 --untracked-files=all)"
if [[ -n "${initial_status}" ]]; then
  echo "benchmark requires a clean worktree and index" >&2
  echo "${initial_status}" >&2
  exit 2
fi

verify_exact_source() {
  current_revision="$(git -C "${repo_root}" rev-parse HEAD)"
  if [[ "${current_revision}" != "${revision}" ]]; then
    echo "benchmark revision changed during execution" >&2
    return 2
  fi
  if ! git -C "${repo_root}" diff --quiet; then
    echo "tracked worktree files changed during benchmark execution" >&2
    return 2
  fi
  if ! git -C "${repo_root}" diff --cached --quiet; then
    echo "index changed during benchmark execution" >&2
    return 2
  fi
}

artifact_root="${MAGIC_RELEASE_BENCH_DIR:-}"
if [[ -z "${artifact_root}" ]]; then
  mkdir -p "${repo_root}/target"
  artifact_root="$(mktemp -d "${repo_root}/target/release-profile.XXXXXX")"
elif [[ -e "${artifact_root}" ]]; then
  echo "benchmark artifact directory already exists: ${artifact_root}" >&2
  exit 2
else
  mkdir -p "${artifact_root}"
fi

default_target="${artifact_root}/default-target"
candidate_target="${artifact_root}/candidate-target"
default_runs="${artifact_root}/default-runs"
candidate_runs="${artifact_root}/candidate-runs"
mkdir -p "${default_runs}" "${candidate_runs}"

echo "benchmark_artifact_root=${artifact_root}"
echo "building_profile=default"
CARGO_TARGET_DIR="${default_target}" \
  CARGO_PROFILE_RELEASE_LTO=false \
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
  cargo build --manifest-path "${repo_root}/Cargo.toml" \
  -p magic-tdx-rs --example parse_bench --release --locked --offline

echo "building_profile=thin-lto-codegen1"
CARGO_TARGET_DIR="${candidate_target}" \
  CARGO_PROFILE_RELEASE_LTO=thin \
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  cargo build --manifest-path "${repo_root}/Cargo.toml" \
  -p magic-tdx-rs --example parse_bench --release --locked --offline

default_binary="${default_target}/release/examples/parse_bench"
candidate_binary="${candidate_target}/release/examples/parse_bench"

echo "warming_profile=default"
"${default_binary}" >/dev/null
echo "warming_profile=thin-lto-codegen1"
"${candidate_binary}" >/dev/null

run_profile() {
  profile="$1"
  run="$2"
  if [[ "${profile}" == "default" ]]; then
    binary="${default_binary}"
    output="${default_runs}/run-${run}.json"
  else
    binary="${candidate_binary}"
    output="${candidate_runs}/run-${run}.json"
  fi
  echo "measuring_profile=${profile} run=${run}"
  "${binary}" >"${output}"
}

for run in 1 2 3 4 5; do
  if ((run % 2 == 1)); then
    run_profile default "${run}"
    run_profile thin-lto-codegen1 "${run}"
  else
    run_profile thin-lto-codegen1 "${run}"
    run_profile default "${run}"
  fi
done

verify_exact_source
rustc_version="$(rustc --version)"
cargo_version="$(cargo --version)"
platform_version="$(uname -a)"

set +e
python3 "${repo_root}/tools/bench/compare_release_profiles.py" collect \
  --default-dir "${default_runs}" \
  --candidate-dir "${candidate_runs}" \
  --default-binary "${default_binary}" \
  --candidate-binary "${candidate_binary}" \
  --evidence "${artifact_root}/evidence.json" \
  --report "${artifact_root}/report.json" \
  --revision "${revision}" \
  --rustc "${rustc_version}" \
  --cargo "${cargo_version}" \
  --platform "${platform_version}"
status=$?
set -e

verify_exact_source
echo "benchmark_evidence=${artifact_root}/evidence.json"
echo "benchmark_report=${artifact_root}/report.json"
exit "${status}"
