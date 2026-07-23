# Coverage release gate

`check_thresholds.py` validates the JSON emitted by `cargo llvm-cov`. It is a
release checker, not a report formatter: malformed or incomplete evidence
fails closed.

## Thresholds

- production workspace line coverage: at least **80%**;
- configured critical data-chain aggregate: at least **95%**;
- every configured critical glob must match at least one measured file with a
  positive line count.

Threshold comparisons use integer covered/count values and integer
cross-multiplication. Rounded percentages from the JSON are not trusted.

## Production boundary

Only existing repository files matching `crates/*/src/**/*.rs` contribute.
Paths under `tests`, `examples`, `benches`, `fuzz` or `target`, generated files,
missing files and files outside the repository are excluded. Relative POSIX,
absolute POSIX and Windows paths are normalized to the same repository-relative
identity. A duplicate normalized production filename invalidates the report.

The critical globs are defined in `CRITICAL_GLOBS` in the checker. They cover
Core validation/evidence, Router failover, TDX codec/protocol/adapter/service
entry points and every public-intelligence Provider introduced by Slice 0.
`magic-tdx-rs/src/protocol/{adjuster,fq_service}.rs` are the real adjustment
paths; there is no `adjustment/` directory. `service/mod.rs` is the common
service entry; there is no `service/common.rs`.

Large `#[cfg(test)]` modules must not be used to inflate production coverage.
Move them to `tests/` or load them from a file outside `src/` with `#[path]`
before accepting the corresponding source-file percentage as release evidence.

## Invalid evidence

The checker returns:

- `0` when both thresholds and every critical-presence gate pass;
- `1` for a valid report below a threshold;
- `2` for invalid evidence, including malformed/empty JSON, missing fields,
  wrong types, negative counts, covered lines above counted lines, duplicate
  production filenames or absent critical globs.

Do not lower thresholds, remove critical paths or exclude production code to
obtain a pass.

## Commands

Local release validation uses the already provisioned coverage tool and never
installs a Rust toolchain, rustup component or coverage tool.

```bash
cargo llvm-cov --version
cargo llvm-cov clean --workspace
mkdir -p target/coverage
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 cargo llvm-cov \
  --workspace --all-features --locked --offline \
  --json --output-path target/coverage/coverage.json \
  -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

GitHub-hosted runners do not include `cargo-llvm-cov`. The required PR/release
job installs the auditable crates.io package at the exact version `0.8.7` with
its lockfile before producing evidence. That CI-only tool bootstrap does not
select or constrain a Rust release; CI continues to validate current stable
Rust. Changing the coverage-tool version requires a reviewed workflow change.

Checker regression tests are fast and run independently:

```bash
python3 -m unittest discover -s tools/coverage -p 'test_*.py' -v
```
