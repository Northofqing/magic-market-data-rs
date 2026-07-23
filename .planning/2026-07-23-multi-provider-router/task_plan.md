# Multi-provider router task plan

## Goal

Build and release a provider-neutral, evidence-preserving failover router for
all normalized market-data families, then produce and verify the current release
artifact without modifying the external `stock_analysis` repository.

## Constraints

- Preserve `ProviderId`, provider batch IDs, source timestamps and every failed
  routing attempt.
- Never turn an invalid caller request into a successful fallback.
- Never merge records from different providers into one successful batch.
- Never add cache, daemon, database, HTTP or downstream application policy to
  the first router release.
- Keep Rust 1.83 compatibility and the existing strict release gates.
- Preserve the user's untracked
  `docs/integrations/stock-analysis-market-data-requirements.md`.

## Phases

### Phase 1: Current release artifact

Status: complete

- Build the four probes for commit `ce7f1c6`.
- Verify every packaged file against `SHA256SUMS`.

### Phase 2: Router design and implementation plan

Status: complete

- Write and self-review the approved provider-neutral design.
- Write an exact TDD implementation plan.
- Commit the design/plan before implementation.

### Phase 3: Core evidence contract and router crate

Status: complete

- Add a common sourced-record evidence trait to Core.
- Add generic source adapters, acceptance policy, failover chain, trace and
  aggregate error types.
- Cover terminal failure, fallback, quality rejection, empty data, duplicate
  registration and evidence mismatch.

### Phase 4: Real provider wiring and deployment

Status: complete

- Add a live TDX-to-Tencent source-time/quality fallback probe.
- Update workspace, compliance, packaging, deployment and capability docs.
- Generate and verify the new five-probe release package.

### Phase 5: Final gates and delivery

Status: complete

- Run formatting, Rust 1.83 workspace check/test/Clippy, rustdoc/doctest, docs,
  compliance and diff checks.
- Perform a local self-review because subagent dispatch is prohibited by the
  active developer instruction.
- Commit, push and verify `origin/main`.

Completion evidence:

- The strict real route preserved the TDX quality rejection and selected a
  complete Tencent Quote with source time.
- The final post-review Rust 1.83 preflight passed.
- The five-probe release package was generated from a clean tracked worktree
  and every SHA-256 entry passed.
- Only the user's untracked requirements document remains outside Git.

### Phase 6: Post-approval Choice verification

Status: in_progress

- Verify that the official activator refreshes the project-local `userInfo`.
- Run Quote, order-book, money-flow, daily-bar and minute-bar requests through
  the official SDK after activation.
- If the entitlement becomes active, record the live evidence, rerun release
  gates, regenerate the release package, commit and push.

## Decisions

- Use a new `magic-market-router` crate that depends only on
  `magic-market-core`.
- Use one generic `FailoverChain<Request, Record>` plus family aliases instead
  of a monolithic provider enum.
- Provider-specific error mapping remains explicit at the registration
  boundary.
- A successful result is the first non-empty batch satisfying the configured
  quality/source-time policy and record evidence checks.
- Invalid requests stop immediately; recoverable errors and rejected batches
  may try the next provider.
- Every result/error contains an ordered attempt trace.

## Errors encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| `quality.rs` was queried but Core keeps `QualityReport` in `batch.rs` | 1 | Read `batch.rs` completely and use its public API. |
| Task 1 red test could not import `SourcedRecord` | 1 | Expected TDD failure; added and exported the common evidence trait. |
| Git could not write `.git/index.lock` because the data volume had 116 MiB free | 1 | Confirmed `target/debug` was a 4.8 GiB reproducible cache; removed only that directory and preserved `target/dist` plus `target/emquant`. |
| Task 2 red test could not find package `magic-market-router` | 1 | Expected TDD failure; added the workspace member and minimal source/error contracts. |
| Task 3 red test could not import routing state-machine types | 1 | Expected TDD failure; added acceptance, trace, aggregate error and failover implementations. |
| Task 4 red test could not import the eight family adapter constructors | 1 | Expected TDD failure; added generic Core-trait adapters and family aliases. |
| Sandboxed router live probe exhausted both sources because outbound TDX and Tencent DNS were blocked | 1 | Classified both as retryable transport attempts, then reran the identical command with approved network access; the real route passed. |
| Choice review was approved but the official SDK still returned `10001003/EQERR_NO_ACCESS` with the 2026-07-22 `userInfo` | 1 | Reproduced all supported families, confirmed the activation file was stale, and opened the official activator to refresh the post-approval token. |
| First post-approval activator launch exited without changing `userInfo`; a second SDK probe still returned `10001003` | 2 | Started `loginactivator_mac` directly from its runtime directory and kept the GUI process alive for the SMS refresh. |
| The refreshed 2026-07-23 activation token still returned `10001003` for every supported family | 3 | Confirmed the token timestamp, closed the activator, verified the documented `start(nullptr, ...)` login path and reran a clean SDK process; the remaining state is server-side entitlement propagation. |
| A minimal direct bridge request returned `10002002` inside the network sandbox | 1 | Reran the identical official connection outside the sandbox; it reached Choice and returned the authoritative account error `10001003`. |
| The first post-documentation release rebuild exhausted the 442 MiB remaining data volume | 1 | Confirmed the partial package and temporary build tree, removed only reproducible `target/debug` and `target/release` caches plus the exact incomplete output, then regenerated and verified all five probes. |
