# Magic TDX Bar-Jump Policy Boundary

**Date:** 2026-07-28
**Status:** Approved for implementation
**Business rule:** BR-022

## 1. Decision

Magic TDX admits an atomic historical-bar batch based on source structure,
identity, units, time ordering and provenance. It does not reject a batch only
because adjacent closes differ by more than a fixed percentage.

The removed adapter threshold was not a reliable provider-integrity check:
IPO, relisting, resumption, corporate actions and genuine price discovery can
all produce moves greater than ±20%. Rejecting the source row at normalization
prevents a downstream consumer from applying richer lifecycle evidence and its
own mandatory confirmation policy.

This decision does not declare a large move safe. It preserves the real source
fact so the consumer can decide. `stock_analysis` continues to own its BR-171
alert plus manual-confirmation gate before the series enters selection,
valuation or outcome computation.

## 2. Data flow

```text
real TDX packet
  -> complete packet decode
  -> exact request cardinality
  -> calendar/time identity and strict increasing order
  -> finite positive OHLC and consistent high/low envelope
  -> non-negative volume/amount and positive-volume amount consistency
  -> exact ProviderId::Tdx, source_at and batch provenance
  -> normalized Core Bar batch (large moves preserved unchanged)
  -> downstream consumer-specific lifecycle/jump confirmation
```

Provider admission never fabricates, clamps, smooths, sorts, deduplicates or
confirms a price. Missing or structurally invalid facts remain explicit
failures.

## 3. Failure modes

| Condition | Magic TDX behavior |
| --- | --- |
| Empty, partial or wrong-cardinality packet | reject atomically |
| Duplicate/non-increasing or future source time | reject atomically |
| Non-finite/non-positive OHLC or inconsistent envelope | reject atomically |
| Negative volume/amount or positive volume with zero amount | reject atomically |
| Missing/mixed identity or provenance | reject atomically |
| Structurally valid adjacent move greater than ±20% | admit unchanged |
| Downstream confirmation unavailable | downstream rejects; upstream does not relabel it |

## 4. Old-module relation

| Module/rule | Decision | Reason |
| --- | --- | --- |
| `normalize_bars` structural validation | adopt | exact provider admission boundary |
| `validate_bar_jump` fixed threshold | delete | economic policy incorrectly embedded in provider normalization |
| Core generic price-series quality helper | unchanged | separate generic API; not called by this adapter slice |
| `stock_analysis` BR-171 | retain downstream | mandatory alert/manual confirmation remains consumer-owned |
| lifecycle/corporate-action evidence | consume downstream | evidence may explain a jump but is not fabricated by TDX normalization |

## 5. Verification and rollback

Tests must prove a structurally valid move above +20% and below -20% is
normalized unchanged with exact TDX provenance. Existing bad-value, OHLC,
amount, cardinality, time-order and source-identity failures must remain red.

Run:

```bash
cargo fmt --check
cargo test -p magic-tdx-rs normalized_bar_batches_
cargo check -p magic-tdx-rs
cargo clippy -p magic-tdx-rs -- -D warnings
bash tools/compliance/check.sh
```

Rollback only this policy-boundary slice with `git revert <commit>`. Rollback
must not remove the downstream BR-171 gate or weaken structural/provenance
validation.
