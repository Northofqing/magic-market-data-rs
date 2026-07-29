# CFETS integration

## Capability state

Shibor, LPR and official FX fixing admission flags are true after bounded live
and serial-load admission. DR007 remains false and fails before I/O.

## Official host and paths

Only the audited HTTPS CFETS/China Money public rate and central-parity routes
declared in the crate endpoint policy are permitted.

## Request and response ceilings

JSON responses are capped at 2 MiB, requests start at one-second intervals,
FX history is limited to 20 pages and 1,000 rows.

## Identity, unit, missing, and source-time semantics

Shibor has exactly eight tenors; LPR has exactly 1Y and 5Y. DR007 is not R007
or Shibor. FX parsing requires the complete closed 25-heading catalog and
preserves base/quote orientation and quotation base, including 100JPY/CNY.

## Authentication or usage-rights boundary

No member login or private trading endpoint is used. Operators remain
responsible for source terms and redistribution rights.

## Deterministic tests

Fixtures cover exact tenor order, complete FX heading order, pagination,
duplicate dates, pair orientation, empty-response rejection and fail-before-I/O.

## Live and load admission evidence

On 2026-07-29, Shibor, LPR and official FX each passed two bounded live probes
for `2026-07-20` through `2026-07-29`, followed by its three-call serial load
probe with at least one second between actual request starts. DR007 has no
equivalent proven public history contract and remains false.

## Explicit unsupported operations

DR007 history, realtime quotes, inferred quotation bases and partial/empty
strict batches are unsupported.
