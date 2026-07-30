# Progress Log

## Session: 2026-07-29

### Current Status
- **Phase:** 3 - Implementation
- **Started:** 2026-07-29

### Actions Taken
- Audited TDX readers/parsers/tests, Core numeric serde, China timestamps,
  release manifests, Exchange transport/gating, official admission evidence,
  Router structure, transport integration tests, and numeric tolerance sites.
- User approved the targeted repair scope and the proposed architecture.
- Created isolated worktree `.worktrees/audit-hardening` on branch
  `fix/audit-hardening` from `main` at `3556c1c`.
- Wrote and self-reviewed
  `docs/superpowers/specs/2026-07-29-audit-hardening-design.md`; placeholder,
  contradiction, scope, and ambiguity scans passed.
- Committed the design as `3e166dd`; the user approved the written spec and
  instructed inline execution.
- The clean isolated baseline passed workspace build and all-target tests.
- Wrote and self-reviewed the seven-task, test-first implementation plan at
  `docs/superpowers/plans/2026-07-29-audit-hardening.md`. Coverage,
  placeholder, command, and diff-whitespace checks passed.
- Implemented the checked TDX packet cursor, fallible public low-level readers,
  exact declared-record parsing, explicit K-line tail rules, atomic
  security-list/quote/trade failures, and stronger truncation/fuzz properties.
- Replaced the Exchange full-I/O mutex gate with shared short-lock request
  reservations, retained explicit Rustls/native-tls wire behavior, and reused
  compatible shared endpoint/request validation.
- Added the 17-row machine-readable BR-009 registry, its source-drift checker
  and deterministic checker tests. Re-ran WallstreetCN's bounded production
  protocol to meet the uniform threshold without fabricating or grandfathering
  evidence.
- Added Core fixed-offset RFC3339 and strict wall-clock values; removed the
  duplicate Eastmoney, THS, CNInfo, CLS, and ThePaper Gregorian converters and
  migrated BR-019 to typed clock comparison.
- Added a checked absolute/relative numeric-tolerance value and migrated the
  money, order-book, Tencent, Sina, and SZSE policies without changing their
  business units or source acceptance boundaries.
- Added an offline release benchmark, five-run comparison policy, and
  reproducible profile runner. The initial three-workload result provisionally
  enabled thin LTO and one codegen unit pending final review.
- Completed an independent code review. No Critical findings were reported;
  all nine Important findings were repaired: remaining TDX partial/overflow
  paths, full Exchange endpoint policies and client-level contention proof,
  tracked BR-009 evidence, and complete exact-revision benchmark evidence.
- Expanded the release benchmark to four workloads by adding zlib
  compression/decompression roundtrips. The clean-revision formal A/B run
  improved the combined median by only 1.29%, below the required 5%; the
  release-profile override was therefore removed.
- Follow-up review reported three remaining Important gaps. TDX raw parsers
  now reject negative prices and non-`u32` volumes, benchmark schema/tool
  metadata are exact, and the runner detects runtime untracked files and
  inherited build configuration. Four fake-build runner integration tests
  exercise the clean success and each failure boundary.
- Repeated the formal four-workload A/B benchmark with the then-current runner
  on clean revision `d9555c6`. It passed the historical numerical policy at
  7.25%, but did not isolate automatic Cargo config and therefore does not
  satisfy the current provenance gate. The earlier session also used a
  different revision whose parser hot path and default binary differed. Both
  raw datasets are archived as non-qualifying historical evidence and Cargo's
  default release profile remains in force.
- Final review found that an exact worktree SHA did not by itself prove the
  source consumed during each build. The runner now builds the captured commit
  from an isolated archive snapshot, isolates Cargo home/cache from user
  configuration, and rejects every automatic Cargo config on the build path.
- A second review showed that the extracted snapshot itself was still
  writable. The runner now removes all write modes, hashes the complete source
  tree, runs Cargo from `/`, and revalidates the tree, config boundaries, and
  Git state around every build, warm-up, measurement, and comparison.
- Follow-up review found that the isolated Cargo-home root could still accept
  a config between profiles. Its root is now read-only after precreating
  Cargo's lock file, and every boundary rejects either Cargo-home config path
  and any restored write mode.
- Moved critical-module test bodies out of production source attribution and
  added explicit Core/TDX failure-boundary tests. A fresh all-feature,
  offline, single-job llvm-cov run passed every test and the unchanged release
  thresholds.

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| TDX focused protocol/fuzz tests | Existing baseline passes | 14 passed, 0 failed | pass |
| `cargo build --workspace --locked --offline` | Clean build | Passed | pass |
| `cargo test --workspace --all-targets --locked --offline` | Clean baseline | Passed, only explicitly ignored live HTTPS tests | pass |
| `cargo test -p magic-tdx-rs --all-targets --locked --offline` | Strict decoder remains compatible with complete packets | 363 unit tests plus all integration/example targets passed | pass |
| `cargo clippy -p magic-tdx-rs --all-targets --locked --offline -- -D warnings` | No new lint debt | Passed | pass |
| `cargo test -p magic-market-transport --all-targets --locked --offline` | Shared pacing contract remains correct | 24 tests passed | pass |
| `cargo test -p magic-exchange-rs --all-targets --locked --offline` | Exchange starts stay spaced without serializing I/O | All tests passed; 2 live HTTPS tests ignored | pass |
| `cargo clippy -p magic-exchange-rs --all-targets --locked --offline -- -D warnings` | No new lint debt | Passed | pass |
| WallstreetCN bounded live/load admission | Preserve existing production capability under the uniform threshold | 2 live probes × 20 rows and 3 serial loads × 10 rows passed | pass |
| `python3 -m unittest tools/compliance/test_check_admissions.py` | Checker rejects every modeled drift | 4 tests passed | pass |
| `bash tools/compliance/check.sh` | All source/docs/business-rule contracts pass | Passed; 17 admission constants registered | pass |
| Fixed-time provider test group | Shared converter preserves every provider contract | Core plus Eastmoney/THS/CNInfo/CLS/ThePaper passed | pass |
| Numeric-policy all-target tests | Existing source boundaries remain accepted/rejected | Core/Tencent/Sina/Exchange passed; Exchange live HTTPS tests remained ignored | pass |
| Numeric-policy strict Clippy | No new lint debt | Passed with `-D warnings` | pass |
| Four-workload comparison on `8c8e9b5` | Candidate must meet all predeclared thresholds | 1.29% combined improvement, zlib +3.03%, binary -4.85%; candidate rejected | pass |
| TDX review-hardening suite | Remaining declared records and cumulative arithmetic fail atomically | 368 unit tests and every integration/example target passed | pass |
| Exchange shared-policy suite | Exact query/MIME contracts and non-serialized slow I/O | Transport and Exchange tests passed; strict Clippy passed | pass |
| BR-009 tracked-evidence suite | Reject untracked source/evidence and symlink escape | 6 checker tests and 17-row compliance registry passed | pass |
| Four-workload evidence validation | Reject missing metadata/workloads/throughput/revision facts | 8 comparison-policy tests, example check and strict Clippy passed | pass |
| Follow-up TDX domain suite | Raw public decoders must not return negative prices/quantities | 368 unit tests plus all integration/example targets passed; strict Clippy passed | pass |
| Benchmark failure-closed suite | Reject weak schemas, forged tools, runtime untracked files and build env | 15 benchmark tests passed, including 4 runner integration tests | pass |
| Historical four-workload comparison on `d9555c6` | Preserve raw results without promoting evidence that fails the current provenance gate | Session B passed its old numerical policy at 7.25%, but automatic Cargo config was not isolated; evidence is explicitly non-qualifying | pass |
| Snapshot/config runner isolation | Source and Cargo-home root are read-only and digest/config/Git state is checked at every execution boundary | 10 runner tests pass, including snapshot tampering, transient worktree mutation, controlled CWD, Cargo-home injection, and ancestry config boundaries | pass |
| Benchmark and runner suite | All comparison/schema/runner contracts pass | 21 Python tests passed | pass |
| Full Gate D coverage evidence | Overall >=80% and critical paths >=95% without excluding production code | Overall 45,493/51,765 = 87.88%; critical 26,656/28,056 = 95.01% | pass |

### Errors
| Error | Resolution |
|-------|------------|
| Incorrect Core test target name during audit | Inspected the actual `values` and `serde_contracts` tests. |
| Existing workspace test occupied root target lock | Kept it untouched and isolated subsequent work. |
| Rust build cache filled the remaining disk during focused tests | Removed only the isolated worktree's 6.0 GiB `target` cache and reran successfully. |
| Parallel TDX logging tests raced on one global atomic level | Serialized the three state-mutating tests with a test-only mutex; the normal parallel suite now passes. |
