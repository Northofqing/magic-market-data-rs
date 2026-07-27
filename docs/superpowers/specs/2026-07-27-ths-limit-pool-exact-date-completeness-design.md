# Tonghuashun exact-date limit-pool completeness

**Status:** Gate A design ready
**Rule:** BR-032

## Problem

`magic-ths-rs` sends a requested date to the public upper-limit endpoint, but
the adapter currently assigns that request date to provenance without checking
the source `data.date`. It also accepts `data.info` without checking
`data.page.total`, so a caller-bounded first page can be mislabeled as a
complete whole-market batch.

This blocks downstream historical review: a complete exact-date pool is needed
for membership and theme aggregation, while a truncated or wrong-date batch
must fail closed.

## Data flow

```text
LimitPoolRequest(kind=Upper, date, limit)
  -> Tonghuashun public HTTPS endpoint
  -> require status_code=0
  -> require source data.date == request date
  -> require page=1 and response limit == request limit
  -> require 0 <= total <= 200
  -> parse all unique rows
  -> require validated row count == total
  -> strict DataBatch with exact source date and immutable batch identity
```

The request limit is a transport bound, not a post-fetch display limit. A
consumer needing a whole-market pool must request the verified maximum and may
apply its own bounded selection only after admission.

## Failure modes

- Missing, non-integral or contradictory page metadata: `Schema`/`Incomplete`.
- Source date missing or different from the request: `Incomplete`.
- `total` above 200 or above the request limit: `Incomplete`.
- Validated rows fewer or greater than `total`: `Incomplete`.
- Duplicate code or invalid numeric/source fields: existing strict failure.
- `total=0`, empty rows and exact date: complete evidence-bearing empty batch.
- Network, anti-bot and HTTP failures retain their existing typed errors.

No fallback, inferred date, default total or cross-source field merge is added.

## Tests and live evidence

- Fixture tests cover exact metadata, date mismatch, missing/wrong totals,
  caller truncation and source-proven empty.
- The live probe requests the full 200-row transport bound.
- A real historical-date run must show exact `source_at`, complete quality and
  a record count equal to the source total.

## Rollback

Revert the scoped commit. Downstream consumers then keep the capability
unavailable rather than accepting an unverified THS batch.
