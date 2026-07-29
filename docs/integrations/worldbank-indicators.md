# World Bank Indicators integration

## Capability state

Economic-series admission is false because the approved contract requires a
non-empty structured unit and audited official responses do not reliably
provide one.

## Official host and paths

Only audited indicator and country resources below
`https://api.worldbank.org/v2` are permitted.

## Request and response ceilings

All pages are composed atomically under declared page/row/body ceilings, a
30-second timeout and one-second request-start pacing.

## Identity, unit, missing, and source-time semantics

Indicator code, source ID, ISO-3 code and country name must remain stable over
all pages. The indicator endpoint's real metadata envelope contains only
pagination fields; the source ID is validated from the indicator row. Data-page
metadata remains independently strict for `sourceid` and `lastupdated`, without
inventing those fields on the indicator envelope. Null is missing and zero is
present. Units are never inferred from prose.

## Authentication or usage-rights boundary

The public API requires no credential; no private data catalog is queried.

## Deterministic tests

Fixtures cover the audited indicator-envelope shape, indicator-row source and
identity drift, complete pagination, data-page source/revision drift, null/zero,
structured-unit blocking and provider-key preflight.

## Live and load admission evidence

The 2026-07-29 official indicator response reached the structured row and
reported an empty `unit`. That blocker is explicit, so production capability
remains false even when transport succeeds.

## Explicit unsupported operations

Inferred units, slash-bearing path keys, partial pages and production economic
series are unsupported.
