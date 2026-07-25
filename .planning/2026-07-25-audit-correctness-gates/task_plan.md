# Task Plan: Audit Correctness and Release Gates

## Goal

Fix the verified Eastmoney and TDX correctness defects, restore meaningful
Clippy and coverage enforcement, and make the release preflight compile release
artifacts without weakening public data contracts or quality thresholds.

## Current Phase

Awaiting written-spec review

## Phases

### Phase 1: Approved Design and Baseline

- [x] Verify each reported defect against current source.
- [x] Measure targeted test, Clippy, and real llvm-cov baselines.
- [x] Present alternatives and obtain user approval for strict remediation.
- [x] Write and self-review the approved design.
- [x] Commit the approved design.
- **Status:** complete

### Phase 2: Detailed Implementation Plan

- [ ] Map every modified and test file.
- [ ] Write a TDD-oriented implementation plan with exact commands.
- [ ] Self-review the plan against the approved design.
- **Status:** pending

### Phase 3: Correctness and Panic-Safety Fixes

- [ ] Tighten Eastmoney 9-prefix exchange validation.
- [ ] Make TDX zero current-price failure explicit and source-contextual.
- [ ] Remove proven TDX network and synchronization panic paths.
- [ ] Remove the crate-wide Clippy suppression and resolve resulting findings.
- **Status:** pending

### Phase 4: Coverage and Release Gates

- [ ] Implement the documented overall and critical-path coverage contract.
- [ ] Add behavior tests until the real report meets both thresholds.
- [ ] Add release compilation to preflight.
- **Status:** pending

### Phase 5: Full Verification and Delivery

- [ ] Run formatting, workspace tests, Clippy, docs, compliance, coverage, and release checks.
- [ ] Review the final diff for provenance and contract regressions.
- [ ] Integrate the isolated branch without overwriting user-owned files.
- **Status:** pending

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
| Exclude architecture enhancements | Async routing, dynamic provider registration, and shared transports are not required to correct these defects. |

## Errors Encountered

| Error | Attempt | Resolution |
|-------|---------|------------|
| Parallel baseline commands yielded before all Cargo processes completed | 1 | Waited for the processes and reran targeted suites sequentially with quiet output. |
| Existing coverage checker exited 1 on the real report | 1 | Recorded this as the verified 71.89% baseline; do not lower the threshold. |

## Notes

- The primary worktree contains user-owned uncommitted planning and integration
  files. All implementation work stays in this linked worktree.
- Re-read this plan before scope or contract decisions.
