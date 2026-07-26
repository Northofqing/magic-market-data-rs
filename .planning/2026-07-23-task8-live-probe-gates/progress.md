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
- iWencai live probe now reports `skipped_missing_secret` with a typed
  authentication error when no key exists, `failed` on request error, and
  never reports `admitted` while its capability remains false.
- The first iWencai verification attempt stopped at an isolated rustfmt
  line-folding difference before tests ran; the source was corrected and the
  full batch will be restarted from fmt.
- Committed the iWencai live status batch as `b2bc5cd`; missing credentials
  terminate with `skipped_missing_secret`, provider rejection terminates with
  `failed`, and an authenticated diagnostic cannot admit the currently false
  semantic-search capability.
- Brought the parent's isolated stable-Clippy Core fix into this branch as
  `cd23be8` (same patch as `82ef7b1`) rather than allowing the lint.
- iWencai `cargo fmt --check`, all-target offline tests (10 total including
  examples), strict Clippy with `-D warnings`, and rustdoc with `-D warnings`
  all pass.
- Baidu is now conservatively unadvertised and a successful real response
  remains `diagnostic_complete_unadmitted` until latest-session,
  trading-calendar, adjacent-change and corporate-action continuity are all
  independently proved.
- CLS now requests one latest record and applies the common provider,
  freshness, provenance and identity admission gate. CNInfo independently
  gates one announcement and one investor question and admits only if both
  advertised families pass.
- The first combined provider verification stopped at a leading blank line in
  the mechanically moved Baidu private test module before tests ran; it was
  removed and the complete batch will restart from fmt.
- Baidu/CLS/CNInfo complete-batch verification passes: fmt, 32 all-target
  offline tests, strict Clippy with `-D warnings`, and rustdoc with
  `-D warnings`.
- Added a shared Core request tracker and serial-load verifier that reject
  zero starts, active requests, concurrency other than one, and under-paced
  actual request starts.
- Baidu, CLS, CNInfo, THS, iWencai and Eastmoney now record clone-shared
  request starts immediately before production transport, after provider
  pacing, and expose immutable load snapshots.
- Provider load examples now report actual request starts, minimum transport
  start gap and maximum concurrency. CNInfo covers both advertised families,
  THS covers all four, and Eastmoney's default bounded suite covers all 17
  advertised high-level families. Baidu/iWencai remain explicitly unadmitted
  where capability admission is still false.
- Moved the remaining touched inline test bodies to private path modules.
- Final scoped verification passes: `cargo fmt --all`, all-target offline
  tests for Core plus all six providers, strict all-target Clippy with
  `-D warnings`, and rustdoc with `-D warnings`.
