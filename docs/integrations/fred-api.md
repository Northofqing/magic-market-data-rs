# FRED API integration

## Capability state

Economic-series and date-only economic-release-schedule admission are true for
the exact credentialed contracts described below.

## Official host and paths

Only `https://api.stlouisfed.org/fred/series`,
`/fred/series/observations`, and `/fred/releases/dates` are permitted.

## Request and response ceilings

At most 20 series are composed atomically, request starts are one second apart,
the timeout is 30 seconds, and the single bounded observation page must be
complete with no remaining pagination.

Release-schedule requests cover an inclusive range of at most 366 days and
return at most 100 entries. The adapter acquires every declared 1000-row page
before applying the caller limit and rejects responses requiring more than ten
pages.

## Identity, unit, missing, and source-time semantics

FRED series IDs remain provider-qualified. `.` is missing; numeric zero is
present. Frequency, date range, metadata, revision and batch evidence are
validated before normalization.

Release-schedule records preserve FRED `release_id`, `release_name`, exact
`release_date`, and optional original `release_last_updated`. A date is not an
instant: neither it nor local observation time is promoted to `source_at`.
FRED guarantees release-date ordering but does not define same-date ID order;
the adapter validates nondecreasing dates and then deterministically orders ties
by release ID.

## Authentication or usage-rights boundary

`FRED_API_KEY` is read only at runtime, never stored in records, Debug output,
errors or documentation evidence.

## Deterministic tests

Fixtures cover metadata/observation agreement, incomplete pagination,
missing/zero, duplicate keys, non-finite values and all-key preflight.

## Live and load admission evidence

On 2026-08-13, two independent credentialed live runs each returned the four
requested 2025 quarterly `GDP` observations, followed by a three-call serial
load probe. All metadata/observation, unit, frequency, period, completeness,
pacing and redaction checks passed. The API key was injected only from a
Git-ignored local environment file and was not recorded in output or evidence.
The formal `EconomicSeriesProvider` path is admitted under the same bounds.

On 2026-09-13, two independent credentialed release-schedule live probes each
returned 20 complete records for the next 30 UTC calendar days. A three-call
serial load probe then returned the same complete bounded shape on all calls.
Batch and record `source_at` remained absent. The formal
`EconomicReleaseScheduleProvider` path is admitted under those exact bounds.

## Explicit unsupported operations

Credential discovery, key logging, unbounded pagination, fabricated release
times, calendar-wide completeness claims and cross-provider key substitution
are unsupported.
