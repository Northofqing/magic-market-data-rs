# FRED API integration

## Capability state

Economic-series admission is false pending configured live evidence.

## Official host and paths

Only `https://api.stlouisfed.org/fred/series` and
`/fred/series/observations` are permitted.

## Request and response ceilings

At most 20 series are composed atomically, request starts are one second apart,
the timeout is 30 seconds, and the single bounded observation page must be
complete with no remaining pagination.

## Identity, unit, missing, and source-time semantics

FRED series IDs remain provider-qualified. `.` is missing; numeric zero is
present. Frequency, date range, metadata, revision and batch evidence are
validated before normalization.

## Authentication or usage-rights boundary

`FRED_API_KEY` is read only at runtime, never stored in records, Debug output,
errors or documentation evidence.

## Deterministic tests

Fixtures cover metadata/observation agreement, incomplete pagination,
missing/zero, duplicate keys, non-finite values and all-key preflight.

## Live and load admission evidence

No secret-bearing live run has been recorded as of 2026-07-29; capability
remains false. The formal `EconomicSeriesProvider` method therefore returns
typed `Unsupported` before transport; only the explicitly named
`probe_economic_series` diagnostic performs bounded I/O.

## Explicit unsupported operations

Credential discovery, key logging, unbounded pagination and cross-provider key
substitution are unsupported.
