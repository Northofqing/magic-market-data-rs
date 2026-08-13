# FRED API integration

## Capability state

Economic-series admission is true for the exact credentialed series contract
described below.

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

On 2026-08-13, two independent credentialed live runs each returned the four
requested 2025 quarterly `GDP` observations, followed by a three-call serial
load probe. All metadata/observation, unit, frequency, period, completeness,
pacing and redaction checks passed. The API key was injected only from a
Git-ignored local environment file and was not recorded in output or evidence.
The formal `EconomicSeriesProvider` path is admitted under the same bounds.

## Explicit unsupported operations

Credential discovery, key logging, unbounded pagination and cross-provider key
substitution are unsupported.
