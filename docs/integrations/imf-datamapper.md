# IMF DataMapper integration

## Capability state

Economic and regional-series admission is false pending live evidence.

## Official host and paths

Only audited resources below
`https://www.imf.org/external/datamapper/api/v2` are permitted.
Requests identify this library with the fixed, auditable
`User-Agent: magic-imf-rs/0.2`; it is not a browser impersonation string.

## Request and response ceilings

Requests are bounded to 20 indicator keys, 20 areas and 50 years; catalog and
data bodies are capped at 8 MiB and 16 MiB, with one-second pacing.

## Identity, unit, missing, and source-time semantics

DATASET/AREA identities, full indicator metadata and the single source null
sentinel are validated before filtering. Timezone-less `last-modified` text is
retained as a revision label, not fabricated into UTC `source_at`.

## Authentication or usage-rights boundary

The public API needs no credential. No private/member API or browser state is
used.

## Deterministic tests

Fixtures cover catalogs, envelope identity, region/year filtering, missing
sentinels, non-finite values, metadata drift and all-key preflight.

## Live and load admission evidence

No bounded live/load sequence has passed as of 2026-07-29; capability remains
false. The formal `EconomicSeriesProvider` method therefore returns typed
`Unsupported` before transport; only the explicitly named
`probe_economic_series` diagnostic performs bounded I/O. An HTTP 403 remains a
typed, explicit transport failure and is never converted into empty data or an
admission claim.

## Explicit unsupported operations

Unknown datasets/areas, inferred timestamps, partial atomic batches and
unbounded cell grids are unsupported.
