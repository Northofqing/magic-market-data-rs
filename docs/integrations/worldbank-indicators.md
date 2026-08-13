# World Bank Indicators integration

## Capability state

Economic-series admission is true only for the exact audited contract:
WDI source `2`, `country:USA`, indicator `NY.GDP.MKTP.CD`, annual period 2024,
and `max_rows=1`. The capability constant denotes this series-scoped contract,
not every World Bank indicator. Every other indicator, country, period,
cardinality or source fails typed `Unsupported` before I/O.

## Official host and paths

Only audited indicator and country resources below
`https://api.worldbank.org/v2` are permitted. Gate A on 2026-08-13 added only
the same-host WDI prefix `/v2/sources/2/series` for exact per-series metadata;
host, JSON MIME, 8 MiB response ceiling, 30-second timeout, redirect policy and
one-second request-start pacing were not widened. Other source IDs fail typed
`Unsupported` before I/O.

## Request and response ceilings

All pages are composed atomically under declared page/row/body ceilings, a
30-second timeout and one-second request-start pacing.

## Identity, unit, missing, and source-time semantics

Indicator code, source ID, ISO-3 code and country name must remain stable over
all pages. The indicator endpoint's real metadata envelope contains only
pagination fields; the source ID is validated from the indicator row. Data-page
metadata remains independently strict for `sourceid` and `lastupdated`, without
inventing those fields on the indicator envelope. The indicator row's empty
`unit` is not interpreted. Instead, the exact
`/v2/sources/2/series/{indicator}/metadata?format=json` response must contain one
matching `Series` variable, a unique non-empty `Unitofmeasure`, exact
`IndicatorName`, and `Periodicity=Annual`; declared metatype cardinality is
checked before use. Null is missing and zero is present. Units are never
inferred from the indicator name or prose.

## Authentication or usage-rights boundary

The public API requires no credential; no private data catalog is queried.

## Deterministic tests

Fixtures cover the audited indicator-envelope and per-series-metadata shapes,
metadata source/variable/name/unit/frequency/cardinality drift, duplicate
metatypes, complete pagination, data-page source/revision drift, null/zero,
the legacy structured-unit blocker and all-key preflight.

## Live and load admission evidence

On 2026-08-13 two consecutive bounded production-client diagnostics both
returned exactly one `NY.GDP.MKTP.CD`, `USA`, annual-2024 record:
`29298013000000` `current US$`, with official page `lastupdated=2026-07-13`.
The official per-series Metadata API supplied `Unitofmeasure=current US$` and
`Periodicity=Annual`; the empty indicator-row `unit` was not used. A serial load
probe then completed three calls and three records without retry or partial
composition. Those diagnostic runs established the contract without admission.
After the exact production allowlist was added, the same formal
`EconomicSeriesProvider::economic_series` path passed two consecutive live runs
and the three-call serial load run. The probes emit standard passed markers only
after checking that one exact record is returned.

`ECONOMIC_SERIES_ADMITTED=true` therefore means only the exact scope above.
The formal Provider rejects all other World Bank series before transport; no
catalog-wide or regional-series claim is made.

## Explicit unsupported operations

Inferred units, slash-bearing path keys, non-WDI sources, indicators without
their own metadata/live evidence, any non-USA/non-2024 production request and
partial pages are unsupported.
