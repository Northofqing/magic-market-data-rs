# Eastmoney limit-pool completeness design

## Status

Gate A design for BR-028. This slice changes source admission only; it does
not change prices, streaks, sorting, or downstream selection thresholds.

## Problem

The Eastmoney limit-pool endpoint publishes `data.tc`, the total number of
source rows, alongside `data.pool`. The existing adapter validates `qdate` and
the rows but discards `tc`, so a caller-bounded page can be mislabeled as a
complete batch. It also cannot represent a source-proven empty trading-day
pool because the generic batch helper rejects all empty record vectors.

## Contract

The parser validates `tc` before mapping rows:

1. `tc` is required, integral, non-negative, and no smaller than `pool.len()`.
2. `qdate` is required and must equal the requested trading date, including
   for an empty result.
3. `tc=0` requires an empty array and yields a strict empty batch with full
   provenance.
4. `tc == validated_rows.len()` yields a strict complete batch.
5. `tc > validated_rows.len()` yields a best-effort batch with an explicit
   truncation issue containing both counts.
6. Duplicate `(exchange, code)` source identities reject the whole batch
   before completeness is decided.

Downstream whole-market consumers must require `quality.is_complete()`. A
bounded consumer may inspect incomplete rows only if its own registered rule
explicitly allows that behavior; it may never call them whole-market data.

## Failure modes

- Missing/null/non-integral/negative `tc`: protocol error.
- `tc < pool.len()`: protocol contradiction.
- Missing/null `pool`: protocol error, including when `tc=0`.
- Duplicate identity or row decode failure: atomic batch failure.
- `tc > rows`: explicit incomplete quality, never verified-empty.
- Observation clock/provenance construction failure: explicit error.

## Old modules

| Module | Decision | Reason |
| --- | --- | --- |
| `BatchContext::finish` | adopt | remains the non-empty strict helper for other endpoint families |
| `limit_pool::parse_limit_pool` | replace admission tail | only this endpoint owns `tc` semantics |
| downstream row-count inference | reject | cannot prove the source total |

## Validation

- Focused parser tests cover complete, truncated, verified-empty, missing and
  contradictory totals, and duplicate identities.
- `cargo fmt --all -- --check`
- `cargo clippy -p magic-eastmoney-rs --all-targets -- -D warnings`
- `cargo test -p magic-eastmoney-rs`
- repository compliance and documentation checks at release Gate C/D.

## Rollback

Revert the BR-028 commit. Downstream whole-market A10 must remain disabled if
the completeness contract is unavailable; it must not fall back to row-count
guessing.
