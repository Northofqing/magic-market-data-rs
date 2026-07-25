# Task Plan: Audit Correctness and Release Gates

## Goal

Fix the verified Eastmoney and TDX correctness defects, restore meaningful
Clippy and coverage enforcement, and make the release preflight compile release
artifacts without weakening public data contracts or quality thresholds.

## Current Phase

Phase 5

## Phases

### Phase 1: Approved Design and Baseline

- [x] Verify each reported defect against current source.
- [x] Measure targeted test, Clippy, and real llvm-cov baselines.
- [x] Present alternatives and obtain user approval for strict remediation.
- [x] Write and self-review the approved design.
- [x] Commit the approved design.
- **Status:** complete

### Phase 2: Detailed Implementation Plan

- [x] Map every modified and test file.
- [x] Write a TDD-oriented implementation plan with exact commands.
- [x] Self-review the plan against the approved design.
- **Status:** complete

### Phase 3: Correctness and Panic-Safety Fixes

- [x] Tighten Eastmoney 9-prefix exchange validation.
- [x] Make TDX zero current-price failure explicit and source-contextual.
- [x] Remove proven TDX network and synchronization panic paths.
- [x] Remove the crate-wide Clippy suppression and resolve resulting findings.
- **Status:** complete

### Phase 4: Coverage and Release Gates

- [x] Implement the documented overall and critical-path coverage contract.
- [x] Add behavior tests until the real critical set reaches 95%.
- [x] Add behavior tests until the real workspace report reaches 80% overall.
- [x] Add release compilation to preflight.
- **Status:** complete

### Phase 5: Full Verification and Delivery

- [x] Run formatting, workspace tests, Clippy, docs, compliance, coverage, and release checks.
- [x] Complete an independent review and resolve its three Important findings.
- [x] Re-run the complete preflight after review fixes.
- [x] Resolve the post-fix review's CFFEX provenance and dormant-source findings.
- [ ] Re-run coverage/preflight and obtain final independent review.
- [ ] Integrate the isolated branch without overwriting user-owned files.
- **Status:** in_progress

## Key Questions

1. How can poisoned TDX synchronization state be handled without breaking public
   method signatures? Use typed errors where methods are fallible and a
   warning-emitting recovery helper only where compatibility requires an
   infallible signature.
2. Can the documented coverage thresholds be restored immediately? The current
   report is below both thresholds, so focused tests must be added before the
   stricter checker can be considered complete.

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| Keep `Price` strictly positive and reject a zero TDX current price | A zero packet value does not prove a tradable price; substituting previous close would fabricate data. |
| Recognize Beijing 9-prefix equities only when the code starts with `920` | This matches verified project providers and prevents Shanghai B-share code `900901` from being mislabeled. |
| Preserve public TDX signatures | Correctness fixes should not impose an unrelated semver-breaking API migration. |
| Keep 80% overall and 95% critical coverage thresholds | Lowering or ratcheting the thresholds would contradict the committed release contract. |
| Count LLVM production segments and exclude exact `#[cfg(test)]` item spans | Whole-file summaries allow inline tests to inflate production coverage and do not prove workspace completeness. |
| Remove the uncalled placeholder codec module | A public copy-through “decompressor” was misleading; real zlib handling already exists in production network clients. |
| Preserve unproved CFFEX delivery method as `NotProvided` | The event notice proves contracts, date, and settlement-price wording but does not independently prove cash settlement. |
| Delete dormant `protocol/packet.rs` | It contained executable-looking code but was never registered by `protocol/mod.rs`; source completeness is clearer without dead production-shaped files. |
| Exclude architecture enhancements | Async routing, dynamic provider registration, and shared transports are not required to correct these defects. |

## Errors Encountered

| Error | Attempt | Resolution |
|-------|---------|------------|
| Parallel baseline commands yielded before all Cargo processes completed | 1 | Waited for the processes and reran targeted suites sequentially with quiet output. |
| Existing coverage checker exited 1 on the real report | 1 | Recorded this as the verified 71.89% baseline; do not lower the threshold. |
| Initial sync-module patch used the wrong `lib.rs` context order | 1 | Inspected the current module/export order and reapplied against the exact context. |
| Initial probe-test patch assumed `get_xdxr_info` ended `client.rs` | 1 | Inspected the file tail and appended the test module after block-info methods. |
| Cargo test name filter treated `net::client::tests|sync::tests` literally | 1 | Use separate substring filters (`probe_one` and `sync::tests`) instead of regex syntax. |
| `cargo clippy --fix` could not create its internal locking listener in the sandbox | 1 | Re-ran the same scoped command with the approved sandbox escalation. |
| `cargo fmt --all -- --check` found formatting changes after Clippy fixes | 1 | Ran `cargo fmt --all`, then re-ran strict Clippy and the full TDX test suite. |
| An unquoted shell regex was parsed as pipeline commands | 1 | Re-ran the inspection with the complete expression quoted. |
| The first llvm-cov report write failed because `target/coverage` did not exist | 1 | Created the directory and added that prerequisite to the documented command. |
| Strict Clippy found inconsistent grouping in invalid-date test literals | 1 | Re-grouped the literals by decimal place and re-ran strict Clippy successfully. |
| The first full preflight found three Clippy findings in newly added tests | 1 | Replaced the bool comparison and cloned singleton slices, then reran workspace Clippy and the complete preflight successfully. |
| `cargo deny check` was unavailable in the local toolchain | 1 | Verified that both push/PR CI and the scheduled security workflow execute the pinned `cargo-deny-action`; did not misclassify a missing local binary as a repository defect. |
| Independent review found a placeholder codec and two coverage-integrity defects | 1 | Removed the uncalled module, canonicalized and completed workspace source discovery, switched to LLVM segments, excluded exact inline test items, and added checker contract tests. |
| First post-review preflight found four cloned singleton slices in TDX tests | 1 | Replaced them with `std::slice::from_ref`, passed targeted strict Clippy, then passed the complete clean preflight. |
| Post-fix review found CFFEX cash method was inferred from settlement-price wording | 1 | Added `NotProvided`, stopped emitting `Cash`, updated BR-018 and all current docs, and retained event/date validation. |
| CFFEX live rerun failed TLS inside and outside the sandbox | 2 | Preserved the typed transport failure and changed current acceptance docs from live passed to live blocked. |

## Notes

- The primary worktree contains user-owned uncommitted planning and integration
  files. All implementation work stays in this linked worktree.
- Re-read this plan before scope or contract decisions.
- The post-review strict report is 80.06% overall (22355/27922) and 95.97%
  for the configured critical aggregate (1881/1960). The smaller denominator
  reflects removal of `#[cfg(test)]` attributed item spans, not relaxed source
  completeness or thresholds.
- The final report after removing dormant `protocol/packet.rs` remains 80.06%
  overall (22355/27922) and 95.97% critical (1881/1960).
- `cargo build --workspace --all-targets --release` emits existing example
  filename-collision warnings for repeated `live_probe`/`load_probe` names.
  Cargo still succeeds, but those examples should be renamed in a separate
  compatibility-focused change before Cargo turns the warning into an error.
