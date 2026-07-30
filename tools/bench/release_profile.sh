#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -P "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
revision="$(git -C "${repo_root}" rev-parse HEAD)"
if [[ ! "${revision}" =~ ^[0-9a-f]{40}$ ]]; then
  echo "benchmark requires a full lowercase Git SHA-1 revision" >&2
  exit 2
fi

reject_build_environment() {
  rejected=0
  while IFS='=' read -r name _; do
    case "${name}" in
      CARGO_* | RUST* | SCCACHE_*)
        echo "benchmark rejects inherited build environment variable: ${name}" >&2
        rejected=1
        ;;
    esac
  done < <(env)
  if [[ "${rejected}" != 0 ]]; then
    return 2
  fi
}

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
  current_status="$(git -C "${repo_root}" status --porcelain=v1 --untracked-files=all)"
  if [[ -n "${current_status}" ]]; then
    echo "benchmark requires a clean worktree and index" >&2
    echo "${current_status}" >&2
    return 2
  fi
}

reject_automatic_cargo_configs() {
  search_root="$1"
  while true; do
    for candidate in \
      "${search_root}/.cargo/config" \
      "${search_root}/.cargo/config.toml"; do
      if [[ -e "${candidate}" || -L "${candidate}" ]]; then
        echo "benchmark rejects automatic Cargo config: ${candidate}" >&2
        return 2
      fi
    done
    if [[ "${search_root}" == "/" ]]; then
      break
    fi
    search_root="$(dirname "${search_root}")"
  done
}

create_isolated_cargo_home() {
  destination="$1"
  if [[ -z "${HOME:-}" ]]; then
    echo "benchmark requires HOME to locate the offline Cargo cache" >&2
    return 2
  fi
  source_home="${HOME}/.cargo"
  mkdir -p "${destination}"
  for cache_directory in registry git; do
    source_path="${source_home}/${cache_directory}"
    if [[ -d "${source_path}" ]]; then
      source_path="$(cd -P "${source_path}" && pwd -P)"
      ln -s "${source_path}" "${destination}/${cache_directory}"
    fi
  done
  # Cargo takes an advisory lock on this file. Precreate it before making the
  # home root read-only so builds cannot add configuration between profiles.
  : >"${destination}/.package-cache"
  chmod a-w "${destination}"
}

verify_isolated_cargo_home() {
  for candidate in \
    "${isolated_cargo_home}/config" \
    "${isolated_cargo_home}/config.toml"; do
    if [[ -e "${candidate}" || -L "${candidate}" ]]; then
      echo "benchmark rejects isolated Cargo home config: ${candidate}" >&2
      return 2
    fi
  done
  python3 - "${isolated_cargo_home}" <<'PY'
import os
import stat
import sys

root = sys.argv[1]
mode = stat.S_IMODE(os.lstat(root).st_mode)
if mode & 0o222:
    print(f"benchmark isolated Cargo home is writable: {root}", file=sys.stderr)
    raise SystemExit(2)
PY
}

snapshot_digest() {
  python3 - "$1" <<'PY'
import hashlib
import os
import stat
import sys

root = os.path.realpath(sys.argv[1])
digest = hashlib.sha256()


def add_bytes(value):
    digest.update(len(value).to_bytes(8, "big"))
    digest.update(value)


def visit(path, relative):
    metadata = os.lstat(path)
    mode = stat.S_IMODE(metadata.st_mode)
    encoded_relative = os.fsencode(relative)
    if stat.S_ISLNK(metadata.st_mode):
        add_bytes(b"symlink")
        add_bytes(encoded_relative)
        add_bytes(os.fsencode(os.readlink(path)))
        return
    if mode & 0o222:
        print(f"benchmark source snapshot is writable: {path}", file=sys.stderr)
        raise SystemExit(2)
    if stat.S_ISDIR(metadata.st_mode):
        add_bytes(b"directory")
        add_bytes(encoded_relative)
        add_bytes(f"{mode:o}".encode())
        with os.scandir(path) as entries:
            ordered = sorted(entries, key=lambda entry: os.fsencode(entry.name))
        for entry in ordered:
            child_relative = entry.name if relative == "." else f"{relative}/{entry.name}"
            visit(entry.path, child_relative)
        return
    if stat.S_ISREG(metadata.st_mode):
        add_bytes(b"file")
        add_bytes(encoded_relative)
        add_bytes(f"{mode:o}".encode())
        with open(path, "rb") as handle:
            while chunk := handle.read(1024 * 1024):
                digest.update(chunk)
        return
    print(f"benchmark source snapshot contains unsupported file type: {path}", file=sys.stderr)
    raise SystemExit(2)


visit(root, ".")
print(digest.hexdigest())
PY
}

reject_build_environment
verify_exact_source

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
artifact_root="$(cd -P "${artifact_root}" && pwd -P)"
if [[ "${artifact_root}" == "${repo_root}" || "${artifact_root}" == "${repo_root}/"* ]]; then
  if [[ "${artifact_root}" != "${repo_root}/target/"* ]] \
    || ! git -C "${repo_root}" check-ignore -q -- "${artifact_root}"; then
    echo "in-repository benchmark artifacts must use a Git-ignored path" >&2
    exit 2
  fi
fi
verify_exact_source

default_target="${artifact_root}/default-target"
candidate_target="${artifact_root}/candidate-target"
default_runs="${artifact_root}/default-runs"
candidate_runs="${artifact_root}/candidate-runs"
source_root="${artifact_root}/source"
isolated_cargo_home="${artifact_root}/cargo-home"
mkdir -p "${default_runs}" "${candidate_runs}" "${source_root}"

git -C "${repo_root}" archive --format=tar "${revision}" \
  | tar -xf - -C "${source_root}"
source_root="$(cd -P "${source_root}" && pwd -P)"
reject_automatic_cargo_configs "${source_root}"
create_isolated_cargo_home "${isolated_cargo_home}"
chmod -R a-w "${source_root}"
expected_source_digest="$(snapshot_digest "${source_root}")"

verify_benchmark_inputs() {
  verify_exact_source
  reject_automatic_cargo_configs "${source_root}"
  # Cargo configuration discovery begins at the process working directory.
  # Builds run from /, whose ancestry cannot contain another directory.
  reject_automatic_cargo_configs "/"
  verify_isolated_cargo_home
  current_source_digest="$(snapshot_digest "${source_root}")"
  if [[ "${current_source_digest}" != "${expected_source_digest}" ]]; then
    echo "benchmark source snapshot changed during execution" >&2
    return 2
  fi
}

verify_benchmark_inputs
verify_exact_source

echo "benchmark_artifact_root=${artifact_root}"
echo "benchmark_source_revision=${revision}"
echo "building_profile=default"
verify_benchmark_inputs
(
  cd /
  CARGO_HOME="${isolated_cargo_home}" \
    CARGO_TARGET_DIR="${default_target}" \
    CARGO_PROFILE_RELEASE_LTO=false \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
    cargo build --manifest-path "${source_root}/Cargo.toml" \
    -p magic-tdx-rs --example parse_bench --release --locked --offline
)
verify_benchmark_inputs

echo "building_profile=thin-lto-codegen1"
verify_benchmark_inputs
(
  cd /
  CARGO_HOME="${isolated_cargo_home}" \
    CARGO_TARGET_DIR="${candidate_target}" \
    CARGO_PROFILE_RELEASE_LTO=thin \
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    cargo build --manifest-path "${source_root}/Cargo.toml" \
    -p magic-tdx-rs --example parse_bench --release --locked --offline
)
verify_benchmark_inputs

default_binary="${default_target}/release/examples/parse_bench"
candidate_binary="${candidate_target}/release/examples/parse_bench"

echo "warming_profile=default"
verify_benchmark_inputs
"${default_binary}" >/dev/null
verify_benchmark_inputs
echo "warming_profile=thin-lto-codegen1"
"${candidate_binary}" >/dev/null
verify_benchmark_inputs

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
  verify_benchmark_inputs
  "${binary}" >"${output}"
  verify_benchmark_inputs
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

verify_benchmark_inputs
rustc_version="$(rustc --version)"
cargo_version="$(
  cd /
  CARGO_HOME="${isolated_cargo_home}" cargo --version
)"
platform_version="$(uname -a)"

verify_benchmark_inputs
set +e
(
  cd /
  python3 "${source_root}/tools/bench/compare_release_profiles.py" collect \
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
)
status=$?
set -e

verify_benchmark_inputs
echo "benchmark_evidence=${artifact_root}/evidence.json"
echo "benchmark_report=${artifact_root}/report.json"
exit "${status}"
