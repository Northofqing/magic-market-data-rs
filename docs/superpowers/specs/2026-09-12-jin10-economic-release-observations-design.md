# Jin10 Economic Release Observations Design

## Goal

Publish the structured macroeconomic releases that are present in Jin10's
current public financial-flash window without claiming that the window is a
complete economic calendar.

The existing `EconomicCalendar` operation remains repository-unadmitted. Jin10
retired its free calendar/API embedding service, and a latest-flash window
cannot prove all scheduled events for a date or range.

## Verified source boundary

Reuse the existing read-only request:

```text
GET https://flash-api.jin10.com/get_flash_list?channel=-8200&vip=1
```

No endpoint, host, redirect, timeout, response-size, authentication, or pacing
policy changes. Public unlocked rows with source `type=1` contain structured
economic-release fields. Type-0/type-2 news records and locked records are not
part of this operation.

The source is a rolling mixed-content window, normally 20 rows and bounded by
the adapter at 21 rows. A successful empty result proves only that the fetched
window contained no eligible public release observation. It does not prove an
empty day, date range, country calendar, or future schedule.

## Contract

Add the append-only `EconomicReleaseObservations` operation and unary RPC.

- Request schema: `magic.market.economic_release_observations.request`, version
  1.
- Record schema: `magic.market.economic_release_observation`, version 1.
- Request fields: `limit` in `1..=20` and optional exact source `country`.
- Response order: source release time newest first, then limited to the caller's
  maximum.
- Empty eligible window: admitted, complete, zero records, and no batch
  `source_at`.
- Non-empty batch `source_at`: the newest record's original source time.

Each record preserves source event and indicator identity, country, name,
period, scheduled time, observed release time, previous/consensus/actual/revised
values, unit, importance, impact, and record evidence. Numeric zero remains the
text value `"0"`.

The normalized `scheduled_at` and `released_at` fields are RFC3339 instants. The
record evidence `source_at` retains the original Jin10 row `time` string and
must represent the same instant as `released_at`. `observed_at` is local receipt
time and cannot replace either source timestamp. Every record provider and
batch ID must match the response provider and batch.

`complete=true` means the bounded upstream response was fully checked and every
eligible row was either returned or excluded by the explicit country/limit
request. It never means calendar completeness.

## Failure behavior

The whole request fails without partial records for malformed envelopes,
oversized source windows, duplicate IDs, malformed or missing required fields,
invalid timestamps, invalid importance, evidence/provider/batch conflicts, or a
source time after observation time. HTTP and upstream availability failures
remain typed provider failures. The adapter does not parse ordinary news text
to invent events, values, countries, or timestamps.

## Architecture

The provider crate owns the public-window interpretation behind a dedicated
`EconomicReleaseObservationsProvider` seam. It shares transport and parsing
primitives with the diagnostic calendar adapter but has its own request type,
batch identity, empty-result policy, admission constant, gRPC operation, and
external schemas. This keeps the rolling-window semantics cohesive and avoids
weakening the deeper `EconomicCalendar` contract.

## Verification

- Core request bounds, optional country, and unknown-field rejection.
- Provider fixtures for field preservation, original evidence time, newest-first
  sorting, exact country filtering, legitimate zero values, and verified-empty
  source windows.
- Provider fixtures rejecting duplicates, malformed timestamps, missing fields,
  invalid importance, and oversized windows.
- gRPC registry tests proving 62 exact operations, 61 admitted operations, and
  `EconomicCalendar` as the sole blocked operation.
- Two read-only live probes and a three-request serial load probe. A fully
  validated empty public window counts as verified-empty evidence for this
  narrow operation.
- Formatting, workspace tests, Clippy with warnings denied, compliance,
  documentation links, release preflight, and client-bundle checksum checks.
