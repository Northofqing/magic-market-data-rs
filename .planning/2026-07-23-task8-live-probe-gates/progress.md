# Progress

## 2026-07-23

- Created `/private/tmp/magic_market_slice0_probe_gates` on
  `feat/slice0-live-probe-gates` from exact commit `45c75be`.
- Read repository rules, Slice 0 Task 8, approved provider designs, and
  BR-009..BR-011.
- Completed pre-flight; Gate A supplement is required before P0 code changes.
- Added the Gate A admission-state/shared-verifier design and registered
  BR-012 before implementation.
- Added external Core RED tests, then implemented the shared batch/verified
  empty verifier and stable machine states; 5 focused tests pass.
- Moved THS inline test bodies to a path-based private test module and changed
  no-consensus behavior from an incomplete pseudo-record to typed
  `VerifiedEmpty`.
- THS live probe now applies the common verifier to consensus, strong reasons,
  upper-limit pool, and popularity and emits stable admission states.
- Core tests plus THS all-target tests pass after adding the atomic
  mixed-consensus failure test. Strict Clippy remains blocked by an unchanged
  Rust 1.95 lint in `magic-market-core/src/provider.rs:1147`.
- The same scoped Clippy run passes with only that pre-existing
  `manual_is_multiple_of` lint allowed; the new THS typed-empty error is boxed
  to keep provider results small.
- Added a regression proving malformed fractional UNIX timestamps do not pass
  admission by silently discarding a non-numeric suffix.
- Eastmoney live probe now verifies every advertised family and emits stable
  admission states; unadvertised fund-flow/news remain diagnostics.
- Eastmoney Dragon-Tiger rejects negative gross buy/sell values, arithmetic
  disagreement between buy/sell/net, duplicate entry identities, and duplicate
  same-side seat identities. The touched private tests moved to a path module.
- Common timestamp admission now validates Eastmoney's existing `unix-ms:`
  observation format rather than treating it as an opaque string.
