# IMF DataMapper integration

## Capability state

Economic and regional-series admission remains false because the approved
credential-free DataMapper endpoint is consistently denied before JSON data is
returned, while the replacement IMF Data API documentation is login-gated.

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

On 2026-09-12 the bounded Rust diagnostic and an exact curl replay using the
library's fixed `User-Agent: magic-imf-rs/0.2` both returned HTTP 403 from
Akamai before JSON data was returned. Single-variable replays isolated the
denial to the user-agent policy: curl's own `curl/8.15.0` user agent received a
200 JSON response, while the library user agent, an empty user agent,
`reqwest/0.12`, and a generic browser-like user agent each received 403. This
does not establish a stable, documented machine-client contract, so the
adapter must not impersonate curl or a browser to gain admission. No browser
headers, cookies, proxy, retry, or bypass were introduced.

The current official IMF API page states that data is available through SDMX
2.1/3.0 but sends API exploration to `portal.api.imf.org`, whose Swagger page
redirects to Microsoft Azure API Management sign-in and explicitly requires a
beta portal account. The public `sdmxcentral.imf.org` guide describes a
structure/schema registry and data-processing services; it does not prove a
credential-free WEO observation contract, so it was not substituted for
DataMapper. No new IMF host/path was registered.

The formal `EconomicSeriesProvider` method therefore still returns typed
`Unsupported` before transport; only `probe_economic_series` performs bounded
diagnostic I/O. To unblock implementation, the user must obtain a beta portal
account (or written credential-free endpoint documentation) and provide the
official WEO SDMX base URL, authentication/subscription-header requirements,
dataflow/key shape, unit/scale/frequency/revision fields, usage limits and
redistribution terms. Alternatively, contact `datahelp@imf.org` for those
facts. They can then support a new Gate A host/path/MIME registry entry and
deterministic fixtures; secrets must remain outside Git.

## Explicit unsupported operations

Unknown datasets/areas, inferred timestamps, partial atomic batches and
unbounded cell grids are unsupported.
