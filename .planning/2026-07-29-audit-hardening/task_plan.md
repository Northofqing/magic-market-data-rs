# Task Plan: Audit Hardening

## Goal
Eliminate silent TDX packet corruption, remove the Exchange request-gate
contention, and add machine-checked admission, timestamp, numeric-tolerance,
and release-performance contracts without weakening provenance or dependency
direction.

## Current Phase
Implementation

## Phases

### Phase 1: Requirements and Design
- [x] Audit each reported issue against production code and tests.
- [x] Confirm the repair scope and success criteria with the user.
- [x] Agree on targeted boundary hardening rather than a sweeping rewrite.
- [x] Write and self-review the design spec.
- [x] Commit and receive user approval for the written design spec.
- **Status:** complete

### Phase 2: Detailed Implementation Plan
- [x] Map exact files and test-first steps.
- [x] Self-review the implementation plan.
- **Status:** complete

### Phase 3: Implementation
- [x] Make TDX binary parsing fallible and atomic.
- [x] Migrate Exchange pacing/policy to shared transport primitives.
- [x] Add BR-009 admission-registry compliance checks.
- [x] Centralize fixed-offset timestamps and typed numeric tolerances.
- [ ] Benchmark candidate release profiles and apply only proven settings.
- **Status:** in_progress

### Phase 4: Testing and Verification
- [ ] Run focused red/green tests for every changed subsystem.
- [ ] Run formatting, all-target/all-feature tests, strict Clippy, Rustdoc,
      documentation links, compliance, coverage, packaging, and release
      preflight.
- **Status:** pending

### Phase 5: Delivery
- [ ] Complete code review and resolve every Critical/Important finding.
- [ ] Integrate the approved branch without overwriting unrelated work.
- [ ] Deliver exact evidence and remaining external blockers.
- **Status:** pending

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Use strict fallible parsing and atomic batch semantics | Truncated packets must never become zero-valued or partial market records. |
| Preserve provider-neutral Router production dependencies | Concrete capability registration belongs in compliance/release tooling. |
| Preserve source-specific numeric tolerances | Cents, percentage points, relative sums, and source precision are different contracts. |
| Benchmark release profiles before changing them | The claimed performance improvement is not currently evidenced. |
| Keep core financial values as checked `f64` | NaN/Inf are already rejected; a fixed-point migration is out of scope. |

## Errors Encountered
| Error | Resolution |
|-------|------------|
| Initial audit used a nonexistent `value_serde` test target | Read the actual `values` and `serde_contracts` targets and relied on their checked constructors/tests. |
| A concurrent workspace test held Cargo's artifact lock | Did not interrupt the unrelated run; created an isolated worktree with its own target directory. |
| Focused tests exhausted the disk while writing Rust incremental artifacts | Confirmed the isolated `target` held 6.0 GiB of reproducible output, ran `cargo clean` only in the worktree, and reran the same tests. |
