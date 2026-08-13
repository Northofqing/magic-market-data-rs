# National Bureau of Statistics integration

## Capability state

National and regional economic-series admission is true only for the exact
July 2026 national and Beijing headline CPI year-on-year indices. All other
indicator, period, geography or cardinality requests fail before I/O.

## Official host and paths

The legacy bounded landing diagnostic remains on `https://www.stats.gov.cn/`.
The admitted machine path uses the Bureau's anonymous first-party
`https://data.stats.gov.cn` JSON service and only these prefixes:

- GET `/dg/website/publicrelease/web/external/new/queryIndexTreeAsync`;
- GET `/dg/website/publicrelease/web/external/new/queryIndicatorsByCid`;
- GET `/dg/website/publicrelease/web/external/getDaCatalogTreeByIndicatorCid`;
- GET `/dg/website/publicrelease/web/external/getDasByDaCatalogId`;
- POST `/dg/website/publicrelease/web/external/stream/esData`.

The Rust client dynamically and exactly follows the catalog identities for
monthly or provincial-monthly data, price indices, year-on-year CPI, the
current 2026 national or provincial table, headline CPI indicator, all-regions
catalog and Beijing area identity. It never persists the observed 32-character
catalog or indicator IDs as production constants.

## Request and response ceilings

JSON responses are capped at 4 MiB, catalog levels at 128 nodes, indicators at
64, area catalogs at 32, area rows at 64, the normalized result at one period
and one row, and request starts are paced by one second. Shared transport keeps
HTTPS, no redirects, no proxy, bounded wall-clock timeout and exact final-URL
binding.

## Identity, unit, missing, and source-time semantics

The admitted keys are `national-cpi-yoy/headline` and
`beijing-cpi-yoy/headline`, both for period `2026-07` and `max_rows=1`.
Catalog names, parent/level/leaf identity, indicator ID, `%` unit, national area
`000000000000` or Beijing `110000000000`/`北京市`, period code/name, and response
identity are all cross-checked. `100.5` and `100.2` are retained as source index
values, not converted to 0.5 or 0.2 percent inflation rates. The API did not
expose an independent publication timestamp in this contract, so observation
time is not promoted as source time.

## Authentication or usage-rights boundary

The admitted requests were reproduced without login, Cookie, CAPTCHA,
browser-only headers or session extraction. Browser automation was used only to
observe the public request contract, then a plain Rust shared-transport client
and an independent anonymous command-line client reproduced it.

## Live and load admission evidence

On 2026-08-13 two independent formal live runs each returned the July 2026
national headline CPI year-on-year index `100.5%` and Beijing index `100.2%`.
Three-call serial formal load probes then passed for each scope. Deterministic
tests cover the prior diagnostic parser, exact admitted capabilities and
fail-before-I/O behavior outside scope.

## Explicit unsupported operations

Other regions, arbitrary indicators, other months, historical ranges,
exports/downloads, browser emulation and unbounded queries are unsupported.
Each requires its own catalog and area identity, units and two-live/three-load
evidence before admission can expand.
