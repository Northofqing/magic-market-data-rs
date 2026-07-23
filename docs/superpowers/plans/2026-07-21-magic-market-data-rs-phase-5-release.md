# Phase 5: Compatibility, Performance, Documentation, and Release Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the complete library implementation into auditable Gate B/C/D evidence: exhaustive compatibility, reproducible performance, coverage, MSRV/platform CI, SemVer/security/license checks, read-only live validation, and complete mechanically checked documentation.

**Architecture:** Repository scripts are the evidence interface. Deterministic gates run on every pull request; expensive differential/coverage/security jobs run in CI; fixed-host A/B and live diagnostics are explicit manual workflows whose raw JSON is preserved and whose absence blocks release readiness rather than becoming a skip-success.

**Tech Stack:** Rust stable, GitHub Actions, actions/checkout v6.0.2 pinned at `de0fac2e4500dabe0009e67214ff5f5447ce83dd`, cargo-llvm-cov, cargo-semver-checks, cargo-deny, cargo-audit, Criterion JSON, Python 3 standard library evidence checkers.

---

## Exit gate

Phase 5 and the repository delivery are complete only when every design acceptance item has an evidence file linked to exact commits and commands, controlled live A/B has valid samples, and a human auditor signs the reviewed evidence. A missing runner, unavailable public endpoint, insufficient sample, unsigned review, or threshold failure is `Blocked`; it is never converted into a green skipped job.

### Task 1: Complete the documentation system and executable examples

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `crates/magic-market-core/README.md`
- Modify: `crates/magic-tdx-rs/README.md`
- Create: `docs/ARCHITECTURE.md`
- Modify: `docs/API_GUIDE.md`
- Modify: `docs/DATA_MODEL.md`
- Modify: `docs/PROTOCOL.md`
- Modify: `docs/ERROR_HANDLING.md`
- Modify: `docs/CLIENTS_AND_CONCURRENCY.md`
- Modify: `docs/RATE_LIMITING.md`
- Create: `docs/COMPATIBILITY.md`
- Create: `docs/MIGRATION_FROM_TDXRS.md`
- Modify: `docs/UPSTREAM.md`
- Create: `docs/PERFORMANCE.md`
- Create: `docs/TESTING.md`
- Create: `docs/OPERATIONS.md`
- Create: `docs/SECURITY.md`
- Create: `docs/CONTRIBUTING.md`
- Create: `docs/CHANGELOG.md`
- Create: `docs/SUPPORT.md`
- Create: `tools/docs/check_links.sh`
- Create: `tools/docs/check_docs.py`

- [ ] **Step 1: Write the documentation checker before filling gaps**

`check_docs.py` loads the capability JSON and public-API snapshot, then fails on: a missing required document; missing heading/anchor for any public client, capability, error kind, business rule, or compatibility row; an unknown protocol field presented as known; a performance claim without artifact path; a network example without explicit endpoint input; or README use of stale absolute benchmark numbers. `check_links.sh` validates local Markdown links/anchors and uses Python `urllib` with bounded timeout/retries for external links.

- [ ] **Step 2: Run documentation checks and capture the expected gap list**

Run: `bash tools/docs/check_links.sh && python3 tools/docs/check_docs.py`

Expected: non-zero exit listing every not-yet-created required document; save that output in `progress.md`, not as a release artifact.

- [ ] **Step 3: Write the architecture/API/data/protocol/error client guides**

Document the virtual workspace boundary, module ownership, source-to-normalized flow, four execution strategies, strict/best-effort naming, typed error/retryability matrix, source/fetched time, cache and quality metadata, every protocol evidence level, adjustment atomicity, and business-rule identifiers. Link symbols to rustdoc and examples that compile.

- [ ] **Step 4: Generate and explain compatibility from the executable inventory**

Generate the operation table in `COMPATIBILITY.md` from `capabilities.json` with a checked-in command that is idempotent. Every row states Adopt/Replaced/Intentional Difference, clients, facade, fixture, and test. `MIGRATION_FROM_TDXRS.md` maps upstream constructors/methods/types/errors to the new facade, calls out removal of magic integers/Python helpers, and explains strict differences without promising source compatibility.

- [ ] **Step 5: Write performance/testing/operations/security/maintenance docs**

`PERFORMANCE.md` defines A/B environment, warm-up, sample, limiter, thresholds, invalid-evidence conditions, and artifact schema without claiming unrun results. `TESTING.md` maps unit/golden/differential/property/fuzz/loopback/live layers. `OPERATIONS.md` covers endpoint/timeouts/snapshots/tracing/read-only diagnostics. `SECURITY.md` covers decompression/allocation bounds, untrusted packets/files, dependency/licensing, and no account/order surface. Complete contributing, changelog, support, upstream, and crate/root READMEs.

- [ ] **Step 6: Run docs, examples, and links**

```bash
cargo test --workspace --doc --all-features
cargo check --workspace --examples --all-features
cargo doc --workspace --all-features --no-deps
bash tools/docs/check_links.sh
python3 tools/docs/check_docs.py
```

Expected: every command exits `0`; no unlinked compatibility or public API item remains.

- [ ] **Step 7: Commit the complete documentation set**

```bash
git add README.md README.en.md crates/*/README.md docs tools/docs
git diff --cached --check
git commit -m "docs: complete market data documentation system"
```

### Task 2: Harden repository compliance and dependency policy

**Files:**
- Modify: `tools/compliance/check.sh`
- Create: `deny.toml`
- Create: `tools/compliance/check_capabilities.py`
- Create: `tools/compliance/check_provenance.py`
- Create: `tools/compliance/check_production_paths.py`
- Create: `artifacts/compatibility/.gitkeep`
- Create: `artifacts/benchmarks/.gitkeep`
- Create: `artifacts/release/.gitkeep`

- [ ] **Step 1: Add self-tests for compliance checkers**

Each Python checker accepts a repository root override. Build temporary miniature fixtures in unit tests and assert detection of: production mock/fixture/test code, `todo!`/`unimplemented!`/panic-style failure, zero/default substitution pattern, sibling/path dependency, missing source license row, changed upstream digest, unknown compatibility disposition, duplicate/missing upstream symbol, and unregistered policy constants.

- [ ] **Step 2: Run checker tests before implementations**

Run: `python3 -m unittest discover tools/compliance -p 'test_*.py'`

Expected: failures for unimplemented checker modules.

- [ ] **Step 3: Implement scoped compliance checks**

`check_production_paths.py` scans only production manifests/source and allows explicit audited panic-free indexing only when annotated in a machine-readable allowlist with reason. `check_provenance.py` verifies pinned commit/license/digests/target existence and patch scope. `check_capabilities.py` compares pinned symbol inventory, JSON matrix, fixtures, and tests. Extend `check.sh` to run them plus business-rule, docs, license, fake implementation, and external-downstream isolation checks; it must never read the adjacent `stock_analysis` repository or external databases.

- [ ] **Step 4: Configure dependency source/license/advisory policy**

`deny.toml` allows only MIT, Apache-2.0, BSD-2/3-Clause, ISC, Unicode-3.0, and explicitly reviewed compatible licenses; denies unknown registries/Git sources, duplicate major versions unless annotated, yanked crates, vulnerabilities, and unmaintained warnings selected by policy. The workspace uses crates.io releases only for production dependencies and keeps `Cargo.lock` committed.

- [ ] **Step 5: Run and commit compliance**

```bash
python3 -m unittest discover tools/compliance -p 'test_*.py'
bash tools/compliance/check.sh
cargo deny check
```

Expected: all pass with no ignored failure exit.

```bash
git add tools/compliance deny.toml artifacts
git diff --cached --check
git commit -m "build: enforce library compliance evidence"
```

### Task 3: Enforce coverage thresholds

**Files:**
- Create: `tools/coverage/check_thresholds.py`
- Create: `tools/coverage/test_check_thresholds.py`
- Create: `tools/coverage/README.md`

- [ ] **Step 1: Test exact threshold boundaries**

Create synthetic llvm-cov JSON cases for overall 79.99/80.00 and critical-file aggregate 94.99/95.00, Windows path separators, generated/fuzz/example exclusions, absent critical files, and malformed JSON. Threshold equality passes; below/missing/malformed fails with actual and required percentages.

- [ ] **Step 2: Run tests and verify the checker is absent**

Run: `python3 -m unittest tools.coverage.test_check_thresholds`

Expected: import failure.

- [ ] **Step 3: Implement deterministic coverage calculation**

Read llvm-cov regions/lines, exclude `tests`, `benches`, `examples`, `fuzz`, and generated target output, and calculate line coverage from production `crates/*/src`. Require overall >=80%. Require aggregate >=95% for `codec/`, `protocol/`, `adjustment/`, `service/common.rs`, and `adapter.rs`; fail if any configured critical path has no measured file.

- [ ] **Step 4: Produce and validate the real report**

```bash
cargo llvm-cov --workspace --all-features --json --output-path target/coverage/coverage.json -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
```

Expected: checker exits `0`. If coverage is below threshold, add focused behavior tests and rerun; do not exclude production files or lower thresholds.

- [ ] **Step 5: Commit coverage policy and tests**

```bash
git add tools/coverage
git diff --cached --check
git commit -m "test: enforce coverage thresholds"
```

### Task 4: Build reproducible differential compatibility evidence

**Files:**
- Create: `tools/upstream/differential.sh`
- Create: `tools/upstream/compat_emit.rs`
- Create: `tools/upstream/compare.py`
- Create: `tools/upstream/test_compare.py`
- Create: `crates/magic-tdx-rs/examples/compatibility_emit.rs`
- Modify: `crates/magic-tdx-rs/Cargo.toml`

- [ ] **Step 1: Test the comparison schema and intentional differences**

Synthetic JSON tests cover exact strings/integers/options, finite float tolerance declared per field, reordered/missing/extra records, fixture digest mismatch, upstream commit mismatch, adopted mismatch, and expected strict-error difference. The comparator rejects NaN/infinity and any difference absent from `capabilities.json`.

- [ ] **Step 2: Implement two emitters over the same manifest**

The target example reads fixture entries and emits canonical JSON with operation, fixture digest, source record fields, and typed error. `tools/upstream/compat_emit.rs` is copied by `differential.sh` into the verified patched upstream checkout as an example, reads the same files, calls only pinned upstream public parser/reader functions, and emits canonical upstream JSON. Copying the harness happens after patch verification and does not alter upstream library logic.

- [ ] **Step 3: Implement the reproducible runner**

`differential.sh` takes the artifact directory as its only positional argument, creates a checkout under `target/upstream/tdxrs`, calls fetch/verify, records patch/commit/toolchain/fixture digests, runs upstream and target emitters, invokes `compare.py`, and writes `upstream.json`, `target.json`, `comparison.json`, and `environment.json`. It exits non-zero on any adopted mismatch, unexpected difference, missing fixture, or incomplete operation inventory.

- [ ] **Step 4: Run the full differential suite**

Run: `bash tools/upstream/differential.sh artifacts/compatibility/local-phase5`

Expected: exit `0`; `comparison.json` reports every operation row with zero unexplained mismatches and explicitly named intentional differences.

- [ ] **Step 5: Commit the harness and reviewed compatibility artifacts**

```bash
git add tools/upstream crates/magic-tdx-rs/examples/compatibility_emit.rs crates/magic-tdx-rs/Cargo.toml artifacts/compatibility/local-phase5
git diff --cached --check
git commit -m "test: prove pinned upstream compatibility"
```

### Task 5: Enforce deterministic A/B performance gates

**Files:**
- Create: `tools/bench/run_ab.sh`
- Create: `tools/bench/upstream_bench.rs`
- Create: `tools/bench/compare.py`
- Create: `tools/bench/test_compare.py`
- Modify: `crates/magic-tdx-rs/benches/codec.rs`
- Modify: `crates/magic-tdx-rs/benches/reader.rs`
- Modify: `crates/magic-tdx-rs/benches/loopback_clients.rs`

- [ ] **Step 1: Test statistics and validity gates**

Synthetic cases cover alternating A/B order, insufficient warm-up, fewer than 30 measured samples, different fixtures/server/limiter/profile/toolchain, 4.99/5.00/5.01% deterministic regression, lower success rate, unbounded queue depth, and missing environment fields. Equality at 5% passes; invalid metadata fails before comparing.

- [ ] **Step 2: Emit machine-readable raw samples from both implementations**

Benchmark codec/readers and loopback Blocking/Async/Direct/Smart at concurrency 1/5/60, quotes at 1/5/60 plus chunked 121, and representative bars/minute/trades/finance/XDXR/readers. Alternate target/upstream per sample under release profile, same fixture/server, same limiter and disabled-limiter variants. Record per-sample nanoseconds, success/error, bytes/records, queue depth, connections, RSS, OS/CPU/arch/Rust/commit/lock digest, warm-up, and sample index.

- [ ] **Step 3: Implement deterministic comparison**

`compare.py` validates metadata, computes robust central throughput/latency summaries and confidence output, then fails when any deterministic codec/reader/client throughput regression exceeds 5%, success rate drops, queue exceeds configured capacity, or memory growth indicates unbounded behavior. Compare each strategy only to its pinned-upstream counterpart at the same concurrency.

- [ ] **Step 4: Run on the fixed benchmark host**

Run: `bash tools/bench/run_ab.sh artifacts/benchmarks/local-phase5`

Expected: exit `0` only on a quiet fixed host meeting environment/sample requirements. A noisy/changed host yields an invalid-evidence failure and must be rerun, not waived.

- [ ] **Step 5: Commit harness and valid raw deterministic evidence**

```bash
git add tools/bench crates/magic-tdx-rs/benches artifacts/benchmarks/local-phase5 docs/PERFORMANCE.md
git diff --cached --check
git commit -m "perf: verify deterministic upstream parity"
```

### Task 6: Implement controlled read-only live A/B diagnostics

**Files:**
- Create: `crates/magic-tdx-rs/src/bin/tdx-diagnostic.rs`
- Modify: `crates/magic-tdx-rs/Cargo.toml`
- Create: `tools/bench/run_live_ab.sh`
- Create: `tools/bench/compare_live.py`
- Create: `tools/bench/test_compare_live.py`
- Create: `artifacts/live/.gitkeep`

- [ ] **Step 1: Test live evidence validation without network**

Synthetic comparison cases cover different endpoint/day/market phase/request/limiter, fewer than 30 alternating samples, 9.99/10.00/10.01% median/p95 regression, lower target success rate, classified no-data, transport outage, and redaction. Equality at 10% passes; unavailable network/market and insufficient samples are `Blocked`, never pass.

- [ ] **Step 2: Implement a feature-gated read-only diagnostic binary**

The `live-diagnostic` feature enables a binary accepting explicit endpoint, operation, market/code, sample count, timeout, limiter, and JSON output path. Permit only quotes/bars/minute/list/finance/XDXR/F10 read operations; reject any unrecognized operation. Output endpoint hash/redacted address, wall/market phase, source/fetched time, count, typed error, attempt, latency, commit, and config. Exit non-zero on failures and never contain account/order APIs.

- [ ] **Step 3: Implement alternating upstream/target live runner**

`run_live_ab.sh` takes the reviewed endpoint and artifact directory as its two positional arguments, verifies the pinned upstream, uses identical read-only requests/config, alternates implementation order, writes raw samples/environment, and calls `compare_live.py`. Require stable endpoint/phase and >=30 successful attempted pairs. Enforce target median and p95 regression <=10% and success rate not below upstream.

- [ ] **Step 4: Run the controlled live gate**

Run during an approved stable market window:

```bash
test -n "$TDX_DIAGNOSTIC_ENDPOINT"
bash tools/bench/run_live_ab.sh "$TDX_DIAGNOSTIC_ENDPOINT" artifacts/live/release-candidate
```

Expected: valid `comparison.json` with median/p95 <=10% regression and no success-rate decrease. The executor sets `TDX_DIAGNOSTIC_ENDPOINT` to the reviewed public market-data endpoint before running; if no endpoint/window/network is available, record `Blocked` and do not complete Phase 5.

- [ ] **Step 5: Review/redact and commit live evidence**

Verify artifacts contain no account, token, private host, or unrestricted raw payload, then:

```bash
git add crates/magic-tdx-rs/src/bin/tdx-diagnostic.rs crates/magic-tdx-rs/Cargo.toml tools/bench artifacts/live/release-candidate docs/PERFORMANCE.md docs/OPERATIONS.md
git diff --cached --check
git commit -m "test: record controlled live validation"
```

### Task 7: Add cross-platform, MSRV, documentation, security, and manual benchmark CI

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/msrv.yml`
- Create: `.github/workflows/security.yml`
- Create: `.github/workflows/bench.yml`

- [ ] **Step 1: Add least-privilege deterministic CI**

Every workflow sets `permissions: contents: read`, pins checkout to `de0fac2e4500dabe0009e67214ff5f5447ce83dd`, uses `fetch-depth: 0` where history is required, and runs locked Cargo commands. Do not expose secrets to pull requests and do not run live public-network diagnostics on ordinary PR/push events.

- [ ] **Step 2: Implement native OS/architecture test matrix**

`ci.yml` runs fmt/strict Clippy/tests/doctests/docs/examples/compliance/docs checks on `ubuntu-24.04` x64, `ubuntu-24.04-arm` Arm64, `macos-15-intel` x64, `macos-15` Arm64, and `windows-2025` x64. Add `cargo check --target aarch64-pc-windows-msvc` on Windows and fail the job if the target cannot build. Arm64 hosted-preview unavailability blocks the required check; it is not `continue-on-error`.

- [ ] **Step 3: Implement rolling stable toolchain validation**

CI installs the current stable toolchain, prints `rustc -Vv`, and runs
`cargo check --workspace --all-targets --locked` plus core/TDX deterministic
tests. No fixed MSRV job is maintained; dependency updates must keep stable
green or be rejected.

- [ ] **Step 4: Implement coverage, SemVer, dependency, and provenance jobs**

`security.yml` runs coverage thresholds, `cargo deny check`, `cargo audit`, provenance/compatibility verification, and `cargo semver-checks` against the recorded Phase 4 facade-freeze commit. Preserve JSON reports as workflow artifacts even on threshold failure.

- [ ] **Step 5: Implement manual fixed-host benchmark/live workflow**

`bench.yml` uses `workflow_dispatch` and `[self-hosted, benchmark, x64]`, accepts reviewed endpoint only for the live job, runs deterministic A/B then optional live A/B, uploads raw JSON, and fails on invalid/blocked evidence. It does not claim portability evidence; it supplies comparable fixed-host performance evidence.

- [ ] **Step 6: Validate workflow syntax and commit**

Run `go run github.com/rhysd/actionlint/cmd/actionlint@v1.7.7 .github/workflows/*.yml` and inspect every external `uses:` value for a full commit SHA.

Expected: actionlint exits `0`; no moving tag and no write permission remains.

```bash
git add .github/workflows
git diff --cached --check
git commit -m "ci: enforce portable release gates"
```

### Task 8: Freeze SemVer evidence and release metadata

**Files:**
- Create: `artifacts/release/public-api-magic-market-core.txt`
- Create: `artifacts/release/public-api-magic-tdx-rs.txt`
- Create: `artifacts/release/evidence.schema.json`
- Create: `artifacts/release/evidence.json`
- Create: `artifacts/release/audit-signoff.schema.json`
- Modify: `docs/CHANGELOG.md`

- [ ] **Step 1: Generate public API snapshots from the facade-freeze commit and HEAD**

Use `cargo public-api --simplified` for both crates, save HEAD snapshots, and retain the Phase 4 commit SHA as SemVer baseline. Review that internal codec/transport/pool/task items are absent and every intended facade item is documented.

- [ ] **Step 2: Run SemVer checks against Phase 4**

```bash
cargo semver-checks check-release -p magic-market-core --baseline-rev "$PHASE4_SHA"
cargo semver-checks check-release -p magic-tdx-rs --baseline-rev "$PHASE4_SHA"
```

Expected: both exit `0`. Set `PHASE4_SHA` to the exact reviewed facade-freeze commit recorded in `progress.md`; do not use a branch name or moving tag.

- [ ] **Step 3: Define and fill release evidence**

The JSON schema requires repository commit, upstream commit/patch digest, Cargo.lock digest, toolchain/OS/arch, every Gate B/C/D command/status/artifact digest, coverage totals, compatibility row/mismatch counts, deterministic/live threshold results, CI run URLs, documentation check, known blockers, rollback commits, and audit-signoff path. Validate `evidence.json` against the schema and verify every referenced artifact digest.

- [ ] **Step 4: Update changelog truthfully**

List delivered capabilities and intentional differences. Do not call the release ready while `known_blockers` is non-empty or audit signoff/live/cross-platform evidence is absent. Keep version `0.1.0` until all pre-1.0 stabilization criteria are reviewed.

- [ ] **Step 5: Commit SemVer and evidence metadata**

```bash
git add artifacts/release docs/CHANGELOG.md
git diff --cached --check
git commit -m "docs: record release candidate evidence"
```

### Task 9: Run Gate B/C/D, obtain audit signoff, and close the project plan

**Files:**
- Create after human review: `artifacts/release/audit-signoff.json`
- Modify: `artifacts/release/evidence.json`
- Modify: `.planning/2026-07-21-magic-tdx-rs/task_plan.md`
- Modify: `.planning/2026-07-21-magic-tdx-rs/progress.md`

- [ ] **Step 1: Run the complete deterministic Gate B/C command set**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo check --workspace --examples --all-features
cargo doc --workspace --all-features --no-deps
bash tools/compliance/check.sh
bash tools/docs/check_links.sh
python3 tools/docs/check_docs.py
cargo check --workspace --all-targets --locked
```

Expected: every command exits `0` at the exact release-candidate commit.

- [ ] **Step 2: Run the complete Gate D command set**

```bash
cargo llvm-cov --workspace --all-features --json --output-path target/coverage/coverage.json -- --test-threads=1
python3 tools/coverage/check_thresholds.py target/coverage/coverage.json
cargo deny check
cargo audit
bash tools/upstream/differential.sh artifacts/compatibility/release-candidate
bash tools/bench/run_ab.sh artifacts/benchmarks/release-candidate
```

Also require green CI for the full OS/architecture matrix and valid `artifacts/live/release-candidate/comparison.json`. Expected: all commands/jobs pass and evidence meets 80%/95%, 5%, and 10% thresholds.

- [ ] **Step 3: Obtain independent human audit signoff**

The human reviewer writes `audit-signoff.json` containing reviewer identity, timestamp, exact repository/upstream commits, reviewed evidence digest, checklist results, blocking findings (empty), and signature/attestation method. The implementation agent must not fabricate, self-sign, or mark this step complete on the reviewer's behalf.

- [ ] **Step 4: Validate the final evidence bundle**

Run: `python3 tools/compliance/check_release_evidence.py artifacts/release/evidence.json`

Expected: exit `0`, every artifact digest resolves, CI URLs correspond to the same commit, blockers are empty, and audit signoff validates.

- [ ] **Step 5: Close planning records only after all evidence is real**

Mark Phases 5–8 complete in `task_plan.md`, append exact commands/results/CI URLs/artifact digests/commit SHAs in `progress.md`, and retain the separate downstream-adoption statement. If any item is unavailable or failed, keep Phase 8 `in_progress` and report `Blocked` with the exact missing evidence.

- [ ] **Step 6: Commit the final audited closeout**

```bash
git add artifacts/release/audit-signoff.json artifacts/release/evidence.json .planning/2026-07-21-magic-tdx-rs/task_plan.md .planning/2026-07-21-magic-tdx-rs/progress.md
git diff --cached --check
git commit -m "docs: close audited library release gates"
```

The external `stock_analysis` adoption remains a separate future design/plan and is not marked complete here.
