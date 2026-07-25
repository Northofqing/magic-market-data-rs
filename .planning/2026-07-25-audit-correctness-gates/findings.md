# Findings & Decisions

## Requirements

- Fix the issues that were verified against the current repository.
- Preserve explicit failures and source provenance.
- Do not add downstream path dependencies.
- Run formatting, tests, Clippy, compliance, and documentation checks before
  release.
- Make technical choices autonomously after the approved design.

## Research Findings

- `crates/magic-eastmoney-rs/src/lib.rs` currently maps every six-digit code
  beginning with `9` to Beijing. This deterministically misclassifies Shanghai
  B-share code `900901`.
- Other verified providers already use the narrower rule: `4`, `8`, and `920`
  map to Beijing; other `9` prefixes are rejected.
- `normalize_quotes` in `crates/magic-tdx-rs/src/adapter.rs` passes the mandatory
  current price directly to `Price::new`. Zero therefore produces a generic
  positivity error with no instrument/field context, while optional OHLC fields
  already preserve zero as absence.
- `TdxHqClient::probe_servers` performs a second connection, handshake, request,
  response read, and parse through `unwrap`, even though the first probe is
  fallible. A transient failure in the second probe can panic the process.
- TDX production code contains numerous `Mutex::lock().unwrap()` calls. Test
  unwraps and examples are not production defects and are not part of this
  remediation.
- `crates/magic-tdx-rs/src/lib.rs` has `#![allow(clippy::all)]`; the current green
  Clippy result does not inspect that crate under the workspace `all = deny`
  policy.
- `tools/coverage/check_thresholds.py` implements only the overall 80% test and
  has one boundary test. The committed release plan also requires production
  path filtering, a 95% aggregate for critical paths, missing-path failure,
  Windows path handling, and malformed-report failure.
- The exact scheduled-CI coverage command currently reports 26,370 of 36,682
  lines, or 71.89%.
- The historical critical set currently reports 1,780 of 2,721 lines, or
  65.42%. The largest gaps are TDX `adapter.rs` and `protocol` modules.
- `tools/release/preflight.sh` checks debug targets, tests, Clippy, rustdoc,
  links, and compliance, but does not compile all release targets.

## Technical Decisions

| Decision | Rationale |
|----------|-----------|
| Reject zero TDX current price atomically with instrument and raw-field context | Quote batches are cardinality-checked and mandatory current price cannot truthfully be represented as `None`. |
| Omit a server from `probe_servers` if either probe phase fails | This preserves the existing return contract while eliminating process panic. |
| Use `f64::total_cmp` for latency ordering | Probe latency is finite by construction, and total ordering removes the remaining comparison unwrap. |
| Propagate lock poison through `TdxError` in fallible paths | This preserves explicit failure semantics. |
| Recover with a warning only for compatibility-bound infallible paths | This avoids a public API break while ensuring poison is not silently swallowed or cascaded into another panic. |
| Add tests rather than exclusions to satisfy coverage | Production omissions must stay visible. |

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| The real report already fails the existing 80% checker | Treat the red gate as existing debt and add focused behavior tests until it is genuinely green. |
| The main checkout has unrelated dirty/untracked files | Created `.worktrees/audit-correctness-gates` on `fix/audit-correctness-gates`. |

## Resources

- `docs/superpowers/plans/2026-07-21-magic-market-data-rs-phase-5-release.md`
- `crates/magic-eastmoney-rs/src/lib.rs`
- `crates/magic-tdx-rs/src/adapter.rs`
- `crates/magic-tdx-rs/src/net/client.rs`
- `tools/coverage/check_thresholds.py`
- `tools/release/preflight.sh`

## Visual/Browser Findings

- No browser or visual research was required.

