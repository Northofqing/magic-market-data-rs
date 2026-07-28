# Official Macro, Global Data, SEC, and Financial News Design

## Objective

Extend `magic-market-data-rs` with production-shaped, read-only Providers for:

- National Bureau of Statistics of China (NBS) economic time series;
- People's Bank of China (PBC) monetary and aggregate-financing series;
- CFETS/China Money benchmark rates and RMB official FX fixings;
- FRED, IMF, and World Bank public global economic series;
- SEC EDGAR filing metadata;
- bounded public financial-news metadata from Xinhua Finance, Yicai, and
  Securities Times.

The result must expose provider-neutral records with exact upstream identity,
source evidence, bounded requests, atomic pagination, and typed failure
semantics. A Provider capability remains false until both deterministic
contract tests and a bounded live production-client probe prove the current
source.

## Scope and Non-Goals

This first batch includes public and official read-only data that can be
accessed without a paid market-data entitlement. It does not include:

- Tushare paid endpoints, Wind, Choice, iFinD, Bloomberg, or other licensed
  terminals;
- bypassing login, CAPTCHA, paywall, cookie, device, or anti-bot controls;
- browser-profile extraction or reuse of a user's logged-in session;
- broker cash, position, order, execution, or Level-2 account data;
- bulk mirroring, full-text news redistribution, article-page crawling, or
  search indexing;
- automatic download of every SEC filing attachment;
- claiming that similarly named economic series from different institutions
  are interchangeable;
- synthesizing values, units, publication times, revisions, or missing
  observations.

If a news publisher does not expose a stable, first-party, public metadata
endpoint that passes the source audit, its crate may provide an explicitly
named diagnostic probe, but its production capability remains false and its
public Provider operation returns typed `Unsupported`.

## Considered Approaches

### Source-aligned Provider crates — selected

Each upstream receives its own crate and `ProviderId`. This makes protocol
changes, request limits, source rights, live admission, and evidence
independently auditable. Provider-neutral records live in Core, and Router
continues to depend on Core rather than concrete Providers.

### One combined macro Provider

A combined crate would reduce workspace members, but it would mix unrelated
rate limits, response formats, licensing terms, identifiers, and outage
domains. It also makes provenance easier to mislabel. This approach is
rejected.

### Model macro data as economic-calendar events

The existing calendar contract represents scheduled events and forecasts.
Historical observations have series identity, period, unit, scale, status,
and revision behavior that calendar events cannot express safely. Extending
calendar records would blur those meanings, so a separate contract is
required.

### Reuse current FX snapshots for official fixings

The existing `FxQuote` is a current-market snapshot over a small closed pair
set. An official daily central parity has a fixing date, official quotation
unit, and revision semantics. A separate official-fixing contract is
required.

## Architecture

### Workspace layers

The dependency direction is:

```text
magic-market-core
        ^
        |
magic-market-transport
        ^
        |
source Provider crates

magic-market-router -> magic-market-core
```

`magic-market-transport` is a provider-neutral internal support crate. It owns
bounded HTTPS primitives only: exact endpoint policy, no-redirect transport,
closed media-type validation, response size limits, timeout limits,
clone-shared request-start pacing, and redacted diagnostics. It owns no source
URLs, payload models, or market semantics.

The new request gate reserves the next permitted request-start instant while
holding a mutex, releases the mutex, then waits if required. It never holds a
mutex during sleep, DNS, TLS, body reads, decompression, or parsing. This
avoids serializing complete HTTP round trips and remains safe across client
clones.

Source-specific crates own endpoint paths, headers, identifiers, payload
validation, pagination, mapping, capability flags, fixtures, live probes, and
integration documentation. They depend only on Core, the transport support
crate, and registry dependencies already approved by the workspace. No
Provider crate depends on another Provider or on a downstream project.

### Provider identities and crates

The following stable identities and workspace members are added:

| Provider identity | Crate | Source family |
| --- | --- | --- |
| `ProviderId::Nbs` | `magic-nbs-rs` | NBS official data |
| `ProviderId::Pbc` | `magic-pbc-rs` | PBC official data |
| `ProviderId::Cfets` | `magic-cfets-rs` | CFETS/China Money |
| `ProviderId::Fred` | `magic-fred-rs` | Federal Reserve Economic Data |
| `ProviderId::Imf` | `magic-imf-rs` | IMF public data APIs |
| `ProviderId::WorldBank` | `magic-worldbank-rs` | World Bank Indicators |
| `ProviderId::SecEdgar` | `magic-sec-rs` | SEC EDGAR |
| `ProviderId::XinhuaFinance` | `magic-xinhua-rs` | Xinhua financial news metadata |
| `ProviderId::Yicai` | `magic-yicai-rs` | Yicai financial news metadata |
| `ProviderId::SecuritiesTimes` | `magic-stcn-rs` | Securities Times news metadata |

The enum names are the stable serialized identities. Renaming or merging them
is a breaking contract change.

## Core Economic-Series Contract

Core adds `macro_data.rs` and exports the following checked types:

- `EconomicSeriesKey`: provider-scoped namespace plus non-empty upstream
  series code;
- `EconomicFrequency`: `Daily`, `Weekly`, `Monthly`, `Quarterly`, `Annual`,
  or `Irregular`;
- `EconomicPeriod`: a checked daily date, ISO week, calendar month, quarter,
  year, or source-defined irregular period consistent with its frequency;
- `EconomicObservationStatus`: `Present`, `Missing`, `NotApplicable`,
  `Confidential`, or `SourceDefined`;
- `EconomicRevision`: optional source revision label plus
  `Preliminary`, `Revised`, `Final`, or `SourceDefined` kind;
- `EconomicSeriesRequest`;
- `EconomicObservation`;
- `EconomicDataCapabilities`;
- `EconomicSeriesProvider`.

`EconomicSeriesRequest` accepts 1 through 100 unique provider-native series
keys, an inclusive checked period range, and a positive row ceiling no greater
than 10,000. A Provider may advertise a lower bound. Duplicate or
cross-provider keys fail before I/O.

Each `EconomicObservation` contains:

- exact `EconomicSeriesKey`;
- source indicator name;
- optional source region code and name;
- frequency and checked period;
- `Option<FiniteNumber>` value;
- source unit and optional scale;
- optional seasonal-adjustment label;
- explicit observation status;
- optional source release timestamp;
- optional source revision;
- `SourceEvidence`.

`Present` requires a finite value. Every non-present status requires `None`.
Neither deserialization nor a Provider can represent a missing source value as
zero. Unit and scale are source facts; a Provider cannot silently convert
percent to fraction, thousands to units, current prices to constant prices, or
seasonally adjusted to unadjusted data.

The release timestamp is optional because many historical sources publish only
a period or release date. `SourceEvidence::source_at` is populated only when
the source proves an unambiguous release timestamp. Fetch completion remains
`observed_at` and cannot substitute for release time.

Within one batch, business identity is provider, series key, region identity,
frequency, and period. Exact duplicates may collapse only when every source
fact is equal; conflicting duplicates fail. Rows sort by requested series
order, region identity, and increasing period. Pagination and multi-series
composition are atomic.

## Core Benchmark-Rate and Official-Fixing Contracts

Core adds `reference_data.rs` rather than overloading current market snapshots.

`ReferenceRateKind` supports:

- Shibor with a checked source tenor;
- Loan Prime Rate with a checked source tenor;
- DR007;
- a source-defined official benchmark identifier.

`ReferenceRateRequest` accepts 1 through 50 unique rate identities, an
inclusive date range, and a positive row bound. `ReferenceRateObservation`
contains kind, fixing date, finite rate, explicit `RatioUnit`, optional source
publication timestamp, optional revision, and evidence.

`CurrencyCode` validates exactly three ASCII letters without claiming that an
unknown code is an ISO-assigned currency. `OfficialFxFixingRequest` accepts 1
through 50 unique base/quote identities and an inclusive date range.
`OfficialFxFixing` contains base currency, quote currency, fixing date, finite
positive value, the source quotation base such as one or one hundred foreign
currency units, optional publication timestamp, optional revision, and
evidence.

Quotation base is mandatory and cannot be inferred from currency name. A
CFETS fixing is not relabeled as a live `FxQuote`.

`ReferenceDataCapabilities` reports benchmark rates and official FX fixings
separately. A Provider may admit one without admitting the other.

## Core SEC Filing Contract

Core adds `filings.rs` with:

- `SecCompanyIdentity`, containing a checked ten-digit normalized CIK and an
  optional source ticker;
- `CompanyFilingRequest`, with 1 through 100 company identities, an optional
  checked set of form types, inclusive filing-date range, and a maximum of
  1,000 records;
- `CompanyFiling`;
- `FilingCapabilities`;
- `CompanyFilingsProvider`.

Each `CompanyFiling` contains CIK, optional ticker, company name, form type,
filing date, optional report period, accession number, primary document name,
canonical filing index URL, canonical primary-document URL, optional source
acceptance timestamp, and evidence.

CIK plus accession number is the business identity. Accession syntax,
document path components, form types, dates, source ordering, and URL hosts
are checked. A conflicting duplicate fails the complete request. Phase one
returns metadata and canonical links only; it does not fetch or store filing
attachments.

The SEC client requires an operator-supplied descriptive `User-Agent` that
contains an application identity and contact address. It uses only official
SEC hosts, never sends cookies, defaults below the published request ceiling,
shares pacing across clones, treats `429` as an explicit failure, and does not
hide throttling behind unbounded retries.

## Provider Responsibilities

### NBS

`magic-nbs-rs` owns the official NBS data endpoint family, provider-native
database/indicator/region identifiers, response-code validation, metadata,
period mapping, missing markers, and bounded paging. It must reject a payload
whose returned query identity differs from the request.

The first admitted slice targets published national monthly, quarterly, and
annual series. Regional series remain false until their separate identity and
pagination probes pass. The integration document records the exact admitted
host, paths, request fields, response limits, and probe evidence.

### PBC

`magic-pbc-rs` reads first-party PBC statistical releases for money supply,
social financing, credit, and related official tables. HTML, spreadsheet, or
structured endpoint families are separate parsers behind one Provider and
cannot be used as silent fallbacks for one another.

A table is admitted only when its title, period, unit, header shape, row
identity, and source publication evidence are validated. Layout changes,
merged cells without a deterministic mapping, footnote ambiguity, or a
document replacement during one request fail explicitly. Publication pages
may supply release metadata, but local file timestamps never become source
timestamps.

### CFETS/China Money

`magic-cfets-rs` exposes only official public families proven by the live
audit: Shibor, LPR, DR007, and RMB central-parity fixings. Each family has its
own capability bit and exact host/path allowlist.

The Provider preserves the published percent or quotation unit, fixing date,
tenor, and source label. It does not convert a fixing into a realtime quote or
scrape data that the source marks as licensed-only. Any authorization or usage
restriction discovered during endpoint audit keeps the affected capability
false.

### FRED

`magic-fred-rs` maps explicitly requested FRED series IDs and observation
metadata. If the official endpoint requires an API key, the key is supplied
through runtime configuration, redacted from errors and probe output, never
committed, and absence returns typed `Authentication`.

Source missing markers map to explicit missing status. Realtime period,
vintage date, units, frequency, and seasonal-adjustment metadata are
preserved when requested and cannot be silently collapsed.

### IMF

`magic-imf-rs` uses a documented public IMF data API selected during source
audit and records the dataset namespace in every series key. Dataset,
indicator, country/area, frequency, and period identities must match the
request. A similarly named indicator from another IMF dataset is distinct.

The Provider bounds the Cartesian product of indicators, areas, and periods
before I/O and rejects partial paging or unit changes within a series.

### World Bank

`magic-worldbank-rs` maps World Bank indicator codes, economies, dates,
values, units, and source metadata. It validates both metadata and data pages,
requires stable total-page counts across one atomic request, and preserves
aggregate regions as source region identities rather than pretending they are
countries.

Source `null` observations become explicit missing records only when the
response supplies their requested identity and period. They never become
zero.

### SEC EDGAR

`magic-sec-rs` uses official SEC submissions and archive metadata endpoints.
The client validates returned CIK, accession, form, primary-document path,
filing date, and ordering before mapping. A submissions response and any
referenced older-submission files form one atomic request.

Phase one does not parse XBRL facts and does not download filing bodies or
attachments. Those require a separate design because taxonomy, amendments,
units, contexts, and document rights have different contracts.

### Xinhua Finance, Yicai, and Securities Times

Each news crate starts with `global_news=false`. Admission requires a
source-specific audit proving a stable first-party public RSS, JSON, or
server-rendered listing endpoint with a bounded response and reliable source
publication time.

An admitted row maps only:

- provider-native ID or a canonical-URL-derived ID;
- title;
- publisher;
- canonical article URL;
- source publication time;
- source-supplied topics;
- language;
- evidence.

`summary` and `content` are always `None`. Article pages, descriptions,
images, videos, login state, hidden application APIs, copied syndicated
bodies, and inferred instrument identities are excluded. A source that has no
admissible public endpoint remains explicitly unsupported; another publisher's
copy cannot be relabeled as that source.

## Transport and Resource Policy

Every new Provider has a closed HTTPS allowlist covering scheme, host, port,
path family, query keys, and final URL. Credentials in URLs, redirects,
cross-host final URLs, plain HTTP, unrecognized media types, compressed or
uncompressed size overflow, and unbounded decompression fail.

Defaults and maximums are provider-specific, but every client enforces:

- connect/read timeout between 1 and 60 seconds;
- a response ceiling no greater than 16 MiB;
- a decoded record ceiling fixed before allocation;
- clone-shared request-start pacing;
- no mutex held during waiting or network I/O;
- no automatic retry unless the exact operation is idempotent and the retry
  count and backoff are both bounded;
- redaction of API keys, contact values, cookies, and query secrets;
- typed request, authentication, transport, HTTP, redirect, media-type,
  resource-limit, protocol, provenance, unsupported, and Core-contract errors.

Provider payload parsers never receive an unbounded `Vec`. Response bodies are
read through a bounded reader. JSON/XML nesting, spreadsheet dimensions, HTML
table dimensions, pages, series, regions, observations, and news rows all have
hard ceilings tested at their boundary.

## Capability Admission and Data Flow

For each capability:

1. Validate and bound the caller request before I/O.
2. Build only an allowlisted first-party HTTPS request.
3. Reserve a paced request start without holding a lock through sleep or I/O.
4. Perform a bounded no-redirect fetch.
5. Validate status, final URL, media type, body bounds, source identity, and
   payload-level success fields.
6. Parse the entire claimed response or complete page set.
7. Validate metadata, units, identities, ordering, duplicates, source times,
   and cross-field invariants.
8. Apply caller output limits only after complete-source validation.
9. Construct record evidence and atomic batch provenance with the same
   provider and batch identity.
10. Run common admission verification before advertising the capability.

An ordinary empty payload is not a successful batch. A successful empty result
requires source evidence proving the requested range has no rows. Fixture
fallbacks, stale caches, alternate publishers, and local timestamps cannot
turn a live failure into success.

## Routing

Router adds provider-neutral sources and chains for:

- economic series;
- reference rates;
- official FX fixings;
- company filing metadata.

Existing `global_news_source` and `GlobalNewsRouter` are reused for the three
news Providers.

Economic fallback is legal only when the request carries an exact
provider-neutral mapping registered for the same published series definition,
frequency, unit, seasonal adjustment, region coverage, and revision/vintage
semantics. In phase one, provider-native series keys do not meet that
equivalence requirement, so the default route targets one Provider and does
not silently substitute another institution's series.

Reference-rate and fixing routes require the same official benchmark identity
and date semantics. Filing routing has only SEC EDGAR in phase one. All Router
adapters reject record/batch Provider mismatches and preserve the winning
source rather than rewriting evidence.

## Testing and Probes

Each new Provider includes:

- synthetic success fixtures with no copied article bodies or licensed data;
- malformed, truncated, oversized, wrong-identity, wrong-unit, duplicate,
  ordering, pagination, and deserialization-bypass tests;
- request validation before transport;
- exact URL, header, final-URL, MIME, timeout, body, row, and page bounds;
- request-gate concurrency tests proving no lock is held during wait or I/O;
- record/batch evidence and common probe-admission tests;
- a bounded `live_probe` that prints identities, values or titles, units,
  source times, observation times, and batch IDs without secrets;
- a serial `load_probe` limited to a small documented request count;
- truthful capability tests for admitted, unauthenticated, unverified, and
  unsupported states.

Live admission requires at least two consecutive bounded production-client
fetches for the same contract, followed by the serial load probe. A transient
success does not admit a capability. A later release probe failure blocks the
release rather than causing fixture substitution.

Macro tests explicitly cover:

- zero versus missing values;
- negative values where economically valid;
- scale and unit changes;
- preliminary, revised, and final observations;
- periods at calendar, leap-year, quarter, and ISO-week boundaries;
- stable metadata across pages and series;
- regional aggregates versus countries or administrative regions.

SEC tests explicitly cover CIK normalization, accession paths, amendments,
older-submission pagination, form filtering, exact SEC hosts, user-agent
validation, and metadata-only behavior.

News tests explicitly prove that summaries, bodies, descriptions, images,
cookies, and inferred instruments never enter normalized records or probe
output.

## Documentation and Release Registration

The implementation updates:

- root workspace membership and Core public exports;
- root README capability matrix, data contracts, configuration, and probe
  commands;
- one README and integration document per Provider;
- deployment hosts, paths, credentials, pacing, health checks, and failure
  behavior;
- upstream provenance and source-rights boundaries;
- registered business rules for macro observations, official rates/fixings,
  SEC metadata, and metadata-only public news;
- compliance dependency boundaries and secret scanning;
- package manifests and expected probe artifacts.

No capability table may say supported until the corresponding live admission
evidence exists. Auth-required FRED operation is documented as configured
rather than public-anonymous when an API key is required.

Before release, run:

- `cargo fmt --all -- --check`;
- `cargo check --workspace --all-targets --locked --offline`;
- `cargo test --workspace --all-targets --locked --offline`;
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`;
- warning-free Rustdoc and workspace doctests;
- documentation-link checks;
- compliance and dependency-boundary checks;
- strict coverage thresholds;
- package verification;
- the complete release preflight.

## Delivery Order

Implementation proceeds in independently reviewable slices:

1. Core identities, economic-series, reference-rate/fixing, filing contracts,
   and serde invariant tests.
2. Shared bounded transport support and concurrency tests.
3. NBS, PBC, and CFETS Providers.
4. FRED, IMF, and World Bank Providers.
5. SEC EDGAR metadata Provider.
6. Xinhua Finance, Yicai, and Securities Times source audits and
   metadata-only Providers where admitted.
7. Router adapters, documentation, packaging, live/load admission, coverage,
   and complete release gates.

Each slice preserves explicit unsupported capability states until its own
evidence is complete. A blocked source does not prevent completed independent
Providers from being merged, but documentation must state the exact remaining
unsupported source.

## Acceptance Criteria

The first batch is complete when:

- every admitted record family has checked Core constructors and
  deserialization cannot bypass invariants;
- every Provider has an exact identity, bounded request contract, source
  evidence, atomic pagination, typed errors, deterministic fixtures, and live
  probe;
- NBS, PBC, CFETS, FRED, IMF, World Bank, and SEC capabilities truthfully
  reflect their production-client evidence;
- Xinhua Finance, Yicai, and Securities Times are either admitted through
  first-party public metadata endpoints or remain explicitly unsupported with
  documented audit evidence;
- no article body, login state, paid entitlement, secret, fabricated
  timestamp, zero-filled missing value, or mislabeled fallback is introduced;
- Router preserves exact Provider evidence and does not substitute
  non-equivalent economic series;
- README and integration documentation match actual capability flags;
- all required engineering and release gates pass from the committed tree.
