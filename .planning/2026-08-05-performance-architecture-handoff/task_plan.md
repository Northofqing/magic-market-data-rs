# Task Plan: Performance and Architecture Handoff

## Goal
Hand off a measured performance and architecture assessment of the workspace,
with per-item data-acquisition risk analysis, so a following session can open
Gate A designs for the approved slices without repeating the measurement work.

## Current Phase
Phase 2 — analysis complete, no implementation started

## Scope Boundary
This session performed **assessment only**. No workspace source file was
modified. All benchmarks ran from throwaway crates under `/tmp` with path
dependencies on the real workspace crates. Nothing here is released, and no
claim in `findings.md` may be published under Gate D until it is reproduced
under the repository's own provenance harness.

## Phases

### Phase 1: Assessment
- [x] Establish workspace baseline (crate count, LOC, test and Clippy status)
- [x] Audit layering, contracts, transport split, and error surface
- [x] Measure parser, allocation, JSON, pacing, and compression costs
- [x] Reconcile measurements against `docs/PERFORMANCE_RESULTS.md`
- [x] Correct three overstated claims made earlier in the session
- **Status:** complete

### Phase 2: Handoff
- [x] Record measured evidence and reproduction commands
- [x] Map each candidate change to its data-acquisition risk
- [x] Map each candidate change to Gates, business rules, and registries
- [ ] Obtain user review of this handoff
- **Status:** in progress

### Phase 3: Gate A Design (not started)
- [ ] Open a design for P0 pacing migration
- [ ] Open a design for P1 bounded decompression
- [ ] Decide whether P3/P4 belong in this major or the next
- **Status:** not started

## Verified Baseline

Both commands were run to completion on 2026-08-05 at `06b4d0f`:

```
cargo test --workspace --all-targets      → 209 suites, 1574 passed, 0 failed, 2 ignored
cargo clippy --workspace --all-targets    → 0 warnings
```

A first attempt failed with `No space left on device`. That was a host disk
condition, not a code defect. `cargo clean` freed the volume and both commands
then passed. Anyone re-running this needs roughly 3 GB free for `target/`.

## Candidate Changes

Ordered by end-to-end value, not by speedup multiple. See `findings.md` for the
measurements behind every number.

| ID | Change | Measured gain | Data-acquisition risk |
| --- | --- | --- | --- |
| P0 | Migrate 10 crates to shared `RequestGate` | 1.20x throughput; p95 11.12s → 604ms | Medium — raises upstream request rate |
| P1 | Bounded gzip decompression | 3.8x on plaintext JSON payloads | Medium — body-limit semantics, WAF behaviour |
| P2 | Fix benchmark variance, then re-evaluate LTO | −11.5% on the low-noise workload | None |
| P3 | `InstrumentId` allocation work | 18.6x construct, 56.8x clone | **High** — can reject valid instruments |
| P4 | Typed JSON with `Cow<'a, str>` | 4.68x | **High** — naive `&str` breaks escaped JSON |
| P5 | `LazyLock` for the F10 regex | Order of magnitude in that function | None |

## Decisions Made

- **End-to-end gain is 3.8–4.1x, and roughly 90% of it comes from P1 alone.**
  The library is I/O bound. P3 and P4 have the largest multiples but together
  are worth about 6% of wall time on a 20 Mbps link. Priority follows wall-time
  contribution, not speedup multiple.
- **P0 is reclassified from a performance fix to an availability fix.** Its
  throughput gain is only 1.20x. Its real value is removing a 10-second
  head-of-line stall that blocks every concurrent caller of the same client.
- **P0 must ship rate-neutral.** Raise each crate's interval from `1s` to
  `1.2s` when migrating so the observed upstream request rate is unchanged.
  Lower it later only with live-probe evidence.
- **P3 is downgraded to its allocation half for this major.** The `Copy`
  conversion is deferred; see `findings.md` for the 8-byte defect that makes
  the obvious implementation unsafe.
- **P2 is a prerequisite, not an optimization.** With 22–40% run-to-run spread,
  the existing 5% gate cannot resolve any of the candidate changes. Fixing
  sampling unblocks verification for everything else.
- **Stale todos left untouched.** The session todo table carried five unrelated
  in-progress entries from another context. They were not modified.

## Non-Goals

- No source change, no dependency change, no registry change in this session.
- No claim here is Gate D releasable. `docs/PERFORMANCE_RESULTS.md` must not be
  updated from these numbers; they were produced on a shared development host
  outside the provenance harness.
