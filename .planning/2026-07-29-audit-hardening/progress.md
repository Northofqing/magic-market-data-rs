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
- Added an offline three-workload release benchmark, five-run comparison
  policy, and reproducible profile runner. The clean-revision candidate passed,
  so thin LTO and one codegen unit were enabled and documented.
- Completed an independent code review. No Critical findings were reported;
  all nine Important findings were repaired: remaining TDX partial/overflow
  paths, full Exchange endpoint policies and client-level contention proof,
  tracked BR-009 evidence, and complete exact-revision benchmark evidence.
- Expanded the release benchmark to four workloads by adding zlib
  compression/decompression roundtrips. A new clean-revision formal A/B run is
  required before the final profile claim is retained.

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
| Release-profile comparison on `e0bc91a` | Candidate must meet all predeclared thresholds | 6.86% combined improvement, no regression, binary -4.79% | pass |
| TDX review-hardening suite | Remaining declared records and cumulative arithmetic fail atomically | 368 unit tests and every integration/example target passed | pass |
| Exchange shared-policy suite | Exact query/MIME contracts and non-serialized slow I/O | Transport and Exchange tests passed; strict Clippy passed | pass |
| BR-009 tracked-evidence suite | Reject untracked source/evidence and symlink escape | 6 checker tests and 17-row compliance registry passed | pass |
| Four-workload evidence validation | Reject missing metadata/workloads/throughput/revision facts | 8 comparison-policy tests, example check and strict Clippy passed | pass |

### Errors
| Error | Resolution |
|-------|------------|
| Incorrect Core test target name during audit | Inspected the actual `values` and `serde_contracts` tests. |
| Existing workspace test occupied root target lock | Kept it untouched and isolated subsequent work. |
| Rust build cache filled the remaining disk during focused tests | Removed only the isolated worktree's 6.0 GiB `target` cache and reran successfully. |
