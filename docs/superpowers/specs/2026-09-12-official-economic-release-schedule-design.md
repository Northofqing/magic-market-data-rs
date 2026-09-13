# Official Economic Release Schedule Design

## Goal

Expose provider-published economic release dates without pretending that a
date-only schedule contains an actual release timestamp, consensus value,
importance score, or complete cross-country economic calendar.

The existing `EconomicCalendar` operation remains unchanged and unadmitted.
`EconomicReleaseObservations / Jin10` remains the contract for structured rows
that have already been published in Jin10's bounded flash window.

## First admitted source

The first adapter is FRED's official `fred/releases/dates` endpoint:

```text
GET https://api.stlouisfed.org/fred/releases/dates
```

FRED documents that release dates originate from the underlying data sources
and may precede availability on FRED. Future dates are requested with
`include_release_dates_with_no_data=true`. The existing runtime
`FRED_API_KEY` is reused and remains redacted.

## Deep module and seam

Core owns one small `EconomicReleaseScheduleProvider` interface. A caller
provides an inclusive date range and maximum row count and receives checked
provider-qualified schedule entries. Provider-specific pagination, response
envelopes, authentication, and filtering stay inside the FRED adapter.

Tests and gRPC composition use this public seam. Transport injection remains
an internal adapter seam for deterministic FRED tests.

## Contract

Add the append-only `EconomicReleaseSchedule` operation and unary RPC.

- Request schema: `magic.market.economic_release_schedule.request`, version 1.
- Record schema: `magic.market.economic_release_schedule_entry`, version 1.
- Request fields: inclusive `start`, inclusive `end`, and `limit` in `1..=100`.
- Date range: start must not exceed end and may span at most 366 days.
- Order: `release_date` ascending, then `release_id` ascending.
- Identity: `(provider, release_id, release_date)` must be unique.
- Empty: a fully validated source response with no rows inside the requested
  range is a complete zero-record batch.

Each record contains the positive provider release ID, exact non-empty release
name, provider `release_date`, optional original `release_last_updated`, and
record evidence. `release_date` remains an `IsoDate`; it is never rewritten as
midnight or promoted to `source_at`. `release_last_updated`, when present, is
retained as provider text and is not treated as the scheduled release instant.
Record and batch evidence therefore have `source_at=null` and use only the
local receipt time as `observed_at`.

`complete=true` means the bounded FRED response and requested projection were
fully validated. It does not claim that FRED covers every institution, country,
indicator, intraday release time, or later schedule revision.

## FRED acquisition bounds

The adapter requests JSON, ascending release-date order, future dates included,
and pages of at most 1000 rows. It validates `count`, `offset`, `limit`, request
ordering, every page, duplicate identities, and a hard maximum of 10 pages.
Any response that would require an eleventh page fails atomically. Request
starts retain the existing one-second FRED pacing.

The request's date range is sent as the official `realtime_start` and
`realtime_end` values and every returned `date` is independently filtered and
validated against the requested inclusive range. A response claiming more rows
than were acquired, repeating or skipping an offset, or changing envelope
identity fails without partial records.

## Failure behavior

Authentication, HTTP, pagination, schema, date-range, duplicate, ordering and
evidence failures remain typed. No news timestamp, current date, observation
time, FRED series observation, or another Provider may fill a missing release
date or source timestamp.

## Admission

Repository admission requires deterministic fixtures plus two independent
credentialed live runs and a three-call serial load probe. Until that evidence
passes, the gRPC route is diagnostic and `allow_unadmitted=true` is required.
