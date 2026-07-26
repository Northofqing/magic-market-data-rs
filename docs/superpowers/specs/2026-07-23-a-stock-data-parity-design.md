# A-Stock Data Capability Expansion Design

**Date:** 2026-07-23  
**Status:** Approved for implementation by the user's standing instruction to
proceed without confirmation pauses  
**Reference:** `simonlin1212/a-stock-data` version 3.5.0 at audit time

## 1. Objective

Expand `magic-market-data-rs` from a normalized low-level market-data workspace
into a typed, read-only A-share data and intelligence workspace. The useful
capabilities demonstrated by the reference project will be reimplemented in
Rust while preserving the repository's existing dependency direction:

```text
consumer
   |
   +-- magic-market-router --------+
   |                               |
   +-- domain traits from Core     |
                                   v
                         normalized Core records
                                   ^
                                   |
            source-aligned Provider implementations
```

The target covers market enrichment, research, signals, capital/chip data,
news, basic/company data, announcements, limit pools, ETF options and
sentiment/interaction. It does not include trading, private account state,
credential interception, browser automation or client traffic capture.

Parity means a typed capability with deterministic fixture coverage and a
bounded real-data probe. It does not mean copying Python source, reproducing
unsafe fallback behavior or claiming that unstable public endpoints provide a
commercial SLA.

## 2. Non-negotiable rules

- The repository does not declare an MSRV or select an exact Rust release.
  Developers use their default toolchain and CI uses current stable.
- Workspace libraries keep `#![forbid(unsafe_code)]`.
- Core never depends on a concrete Provider.
- Every accepted record carries provider, source timestamp when supplied,
  observation timestamp and batch/request evidence.
- Missing data remains absent; parsers never convert missing numeric values to
  zero.
- HTTPS certificate and hostname verification cannot be disabled.
- Plaintext-only routes are unsupported until an HTTPS equivalent is proved.
- Responses, pages, retries, redirects, concurrency and download sizes are
  bounded.
- An advertised capability must have a fixture test and a live probe.
- Live unavailability, entitlement denial and schema drift are reported as
  errors or unsupported capabilities, never replaced by fabricated records.
- API keys and bearer tokens are environment-injected, redacted and excluded
  from debug output, persisted artifacts and probe output.
- Public-web data is read-only and is not assumed to grant redistribution
  rights. Deployment operators remain responsible for source terms and
  licensing.

## 3. Architecture choice

### 3.1 Selected approach

Use source-aligned Provider crates and domain-aligned Core modules.

Provider crates own endpoint paths, wire formats, charset decoding, signing,
rate limits, pagination and source-specific errors. Core owns normalized
business meaning and validation. Router owns failover and acceptance traces.
Analytics owns deterministic calculations derived from normalized records.

This avoids two failure modes:

- A single giant provider would couple unrelated rate limits, authentication
  and schemas.
- A Python subprocess would turn the Rust workspace into a shell around an
  untyped runtime and make deployment, cancellation and provenance harder.

### 3.2 Workspace shape

Existing crates remain:

- `magic-market-core`
- `magic-market-router`
- `magic-tdx-rs`
- `magic-emquant-rs`
- `magic-tencent-rs`
- `magic-sina-rs`

New crates are:

- `magic-eastmoney-rs`
- `magic-cninfo-rs`
- `magic-ths-rs`
- `magic-cls-rs`
- `magic-baidu-rs`
- `magic-iwencai-rs`
- `magic-exchange-rs`
- `magic-market-analysis`

`magic-tencent-rs` gains market statistics. `magic-sina-rs` gains financial
statements and ETF options. Existing TDX finance, F10, corporate action and
market-data features remain the preferred implementations where they already
exceed the reference.

## 4. Core domain model

The existing market records stay source-compatible. New records are split into
modules and re-exported from `magic_market_core`.

### 4.1 Shared primitives

`ProviderId` gains explicit identities:

- `Baidu`
- `Tonghuashun`
- `Iwencai`
- `Cninfo`
- `Cailianpress`
- `Sse`
- `Szse`
- `Hkex`
- `LocalAnalysis`

`AssetClass` gains `Option`. Option contracts also use a dedicated
`OptionContract` because an option identity cannot be represented safely by an
equity code alone.

All new records contain `SourceEvidence`. Multi-source aggregates contain one
evidence item per component instead of pretending a join was atomic.

Domain-specific capabilities coexist with the existing flat market-data
`Capabilities`:

- `ResearchCapabilities`
- `SignalCapabilities`
- `CapitalCapabilities`
- `ContentCapabilities`
- `LimitPoolCapabilities`
- `OptionCapabilities`
- `InteractionCapabilities`

### 4.2 Market enrichment

`MarketStatistics` contains optional:

- turnover rate
- trailing PE
- static PE
- PB
- total market capitalization
- floating market capitalization
- upper/lower price limit
- volume ratio

`TechnicalBar` wraps a normalized bar with optional source MA5/MA10/MA20.
Locally calculated moving averages are emitted by `magic-market-analysis` with
`ProviderId::LocalAnalysis`; source-provided and derived values are never
conflated.

### 4.3 Research

- `ResearchReport`: instrument or industry scope, title, organization, author,
  rating, publication time, canonical URL and optional PDF reference.
- `EarningsEstimate`: fiscal year, EPS, revenue/profit when supplied.
- `ConsensusSnapshot`: instrument, observation time, estimates and contributor
  count.
- `SemanticSearchDocument`: channel, title, excerpt, canonical URL, publication
  time and source-specific document identifier.

Arbitrary iwencai tabular query results remain a Provider-owned typed table
API. They are not forced into unrelated Core records or routed as market data.

### 4.4 Signals and board membership

- `BoardMembership`: instrument, board code/name and explicit
  `Industry | Concept | Region | Unknown` category.
- `StrongStockReason`: instrument, trading date, reason, related subjects and
  optional limit state.
- `DragonTigerEntry`: instrument/date/reason, buy/sell/net amount and optional
  turnover.
- `DragonTigerSeat`: entry identity, side, rank, seat name and amount.
- `MarketRankingEntry`: ranking kind, scope, rank, instrument and metrics.
- `PopularityRank`: provider rank and optional joined quote evidence.
- `ConceptHit`: instrument, concept and matched evidence.

Unknown board types remain `Unknown`; names are not used to fabricate a type.

### 4.5 Fund flow and capital/chip data

- `FundFlowPoint`: instrument or board scope, explicit interval/date/time and
  optional main/super-large/large/medium/small net amounts and ratios.
- `BoardFlow`: board identity/category, interval, rank, return and flow fields.
- `MarginBalance`: date, financing and securities-lending balances and changes.
- `BlockTrade`: date/time, instrument, price, volume, amount, buyer and seller.
- `HolderCount`: report date, holder count and optional change/range fields.
- `LockupEvent`: listing date, share type, unlocked shares and market value.
- `DividendPlan`: report date, implementation state, cash/bonus/transfer/
  allotment/reduction fields.

The existing realtime `MoneyFlow` is not overloaded with historical series.

### 4.6 Content, announcements and interaction

- `NewsItem`: title, summary/content, publisher, canonical URL, publication
  time, instruments/topics and language.
- `Announcement`: instrument, category, title, publication time, announcement
  identifier and canonical/PDF URL.
- `InvestorQuestion`: instrument/company, question, question time, optional
  answer and answer time.

Canonical URLs are validated as HTTPS. Text is size-bounded and normalized
without removing meaningful Unicode.

### 4.7 Company profile and financial statements

- `SecurityProfile`: names, exchange, industry, listing date, capital/share
  fields and optional source-specific F10 facts.
- `FinancialStatement`: instrument, statement kind, report period,
  announcement time, currency and typed line items.
- `FinancialLine`: stable English key, source label, optional numeric value and
  optional unit.

Statement kinds are balance sheet, income statement and cash-flow statement.
The raw source label is retained so a newly added field is not silently mapped
to the wrong semantic key.

### 4.8 Limit pools

- `LimitPoolKind`: upper-limit, broken-limit, lower-limit or previous-upper.
- `LimitPoolEntry`: instrument, date/time, prices, change, turnover, sealed
  amount, first/last seal time, break count, streak, industry and reason.
- `LimitSentiment`: counts by pool, seal rate and derived breadth indicators.

Raw pools are Provider data. Sentiment is a deterministic local analysis with
its own evidence and divide-by-zero behavior.

### 4.9 ETF options

- `OptionContract`: provider contract code, underlying, expiry, call/put and
  strike when available.
- `OptionQuote`: bid/ask/last, volume, open interest, change and quote time.
- `OptionGreeks`: delta, gamma, theta, vega, rho and implied volatility.

Sina's positional payloads are parsed against an exact expected shape. The
three deliberate empty slots in the Greeks response are fixture-tested to
prevent index shifts.

## 5. Provider design

### 5.1 Common Provider pattern

Each new Provider crate has:

- a configuration type with bounded defaults
- an injectable `Transport` trait
- a production HTTPS transport
- wire DTOs private to the crate
- checked mapping into Core records
- fixture/unit/contract tests
- `examples/live_probe.rs`
- `examples/load_probe.rs` for sources used in repeated or paginated calls
- a capability declaration that advertises only implemented, verified families

Transport responses expose status, headers, final URL and body. Parsers have a
maximum input size and must reject HTML/login/error pages presented with a
successful HTTP code.

### 5.2 Eastmoney

`magic-eastmoney-rs` is one crate because multiple Eastmoney hosts share a
single operational risk surface. Every request, including POST popularity
calls, passes through one clone-shared gate.

Default policy:

- concurrency: 1
- minimum start interval: 1.1 seconds
- bounded exponential retry for connect, 429 and 5xx
- no retry for 401/403 or schema failures
- maximum three attempts
- explicit request and response deadlines
- no cross-origin redirect

Modules:

- `research`: stock/industry reports and validated PDF fetch
- `flow`: minute/daily instrument flow and board flow
- `signals`: memberships, rankings, dragon-tiger and seats
- `capital`: margin, block trade, holder count, lockup and dividend
- `news`: instrument/global news
- `limit_pool`: four push2ex pools
- `popularity`: rank and concept hits
- `profile`: company/security facts

Embedded protocol values such as `ut` are versioned configuration, not secrets.
PDF fetch validates HTTPS origin, content type or `%PDF-` magic and a 64 MiB
maximum; the library returns bytes/metadata and never chooses a filesystem path
for the caller.

### 5.3 Tencent

The existing client gains a `market_statistics` trait method and parser for
verified fields 38, 39, 44, 45, 46, 47, 48, 49 and 52. Existing quote behavior
and output remain unchanged. Absent or non-finite fields remain `None`.

### 5.4 Sina

The existing HTTPS/GBK transport is reused. Equity statements and ETF options
are isolated modules with their own capabilities. Option contract validation
does not weaken existing equity symbol validation.

### 5.5 CNInfo

`magic-cninfo-rs` implements:

- HTTPS stock-to-org mapping with cache expiry and no guessed IDs
- paginated announcements
- IRM keyword lookup and question/answer retrieval

POST forms are encoded by the HTTP client, not string concatenation. The
Provider fails explicitly when HTTPS mapping is unavailable or an organization
cannot be resolved.

### 5.6 Tonghuashun

`magic-ths-rs` implements HTTPS routes only:

- consensus EPS tables
- popularity ranking
- limit/strong-stock reason
- optional financial statement fallback

GBK HTML parsing requires named header matching. “First table” and positional
fallbacks are forbidden. A family remains unadvertised when only a plaintext
route can be found.

### 5.7 CLS

`magic-cls-rs` implements signed news retrieval. Signing is deterministic and
unit-tested from canonical query strings. Wall-clock and nonce sources are
injectable. It does not persist cookies or account identity.

### 5.8 Baidu

`magic-baidu-rs` implements daily K lines with source MA5/10/20. It is an
optional cross-check/indicator source rather than a replacement for TDX bars.

### 5.9 iwencai

`magic-iwencai-rs` is optional at runtime and enabled only with an injected
Bearer token. It implements semantic search channels and a bounded structured
table API. Token values are secret wrappers with redacted `Debug`.

Authentication failure is not treated as provider degradation. No token is
read from the local browser, desktop client or keychain automatically.

### 5.10 Exchanges/HKEX

`magic-exchange-rs` contains explicit SSE, SZSE and HKEX clients for verified
HTTPS public/authorized endpoints. It is the only location for official-source
fallbacks and northbound data. An exchange capability is unavailable until the
endpoint has:

- a documented HTTPS route
- schema fixtures
- pagination and throttling tests
- a real successful probe
- a clear source timestamp

There is no insecure-TLS or guessed-schema compatibility mode.

The release live gate executes the official CFFEX delivery-calendar probe as
an independent job. A transport or authentication failure at SSE, SZSE or HKEX
must remain visible, but it must not prevent CFFEX from producing its own
source evidence. Public-web probes use browser-equivalent static request
headers only; they do not persist cookies, credentials or account identity.

## 6. Router and aggregation

`magic-market-router` keeps its generic `FailoverChain<Request, Record>`
implementation. Thin adapters and request aliases are added for new Core
families.

Acceptance rules remain domain-specific:

- instrument-scoped requests require exact instrument identity
- date-scoped requests require exact requested date/range or explicit partial
  status
- market/board queries require nonempty batches and valid rank identity
- multi-page providers expose partial-page failure instead of returning a
  silently truncated “complete” batch

Router attempts retain provider, elapsed time, error class and rejection
reason. Authentication/entitlement failures are not retried through the same
provider. Source selection never changes a record's original evidence.

Cross-source compositions live above the router. For example, forward PE may
combine a Tencent price and a Tonghuashun estimate, but the result retains both
evidence records and reports a partial result when one component is absent.

## 7. Analysis crate

`magic-market-analysis` is pure, deterministic and network-free. Initial
functions:

- SMA5/SMA10/SMA20 over normalized daily closes
- forward PE from price and EPS
- PEG from PE and growth
- configurable PE-digestion scenarios
- limit-pool sentiment
- cross-source freshness/spread diagnostics

All calculations reject non-finite inputs and define zero-denominator behavior.
Subjective valuation anchors are configuration, never Core truth.

## 8. Error model

Provider errors use a common taxonomy while retaining source detail:

- `InvalidRequest`
- `Unsupported`
- `Authentication`
- `Entitlement`
- `RateLimited`
- `Transport`
- `HttpStatus`
- `SchemaDrift`
- `Decode`
- `Incomplete`
- `DataQuality`

Secrets and full response bodies are excluded from `Display`/`Debug`. A bounded
redacted response excerpt may be attached to decode errors. Retrying is derived
from category, not string matching.

## 9. Testing and evidence

Each family passes four gates.

### 9.1 Deterministic

- valid response fixture
- missing optional fields
- missing required identity/time
- malformed/HTML/login response
- pagination boundaries
- serde round-trip and invalid deserialization
- exact source scaling and units

### 9.2 Transport

- HTTPS-only origin validation
- timeout/body/page limits
- limiter shared across cloned clients
- retry category and attempt cap
- redirect and content-type rejection
- secret redaction

### 9.3 Live

Provider probes print concise real records for 华电辽能 (`600396`, Shanghai
market) where instrument scope applies. Market/industry/option queries print a
small bounded sample. Output includes provider, source time, observed time,
count and representative fields.

A live failure is printed with its category. The capability remains
unadvertised until at least one real probe has succeeded in the current schema.
Credentialed/entitlement-only probes report their prerequisite separately.

### 9.4 Load

For repeatable public sources:

- bounded request count and concurrency
- p50/p95/p99 latency
- success/error/status counts
- effective requests per second
- source-specific limiter wait

Load probes respect production throttles. They are resilience checks, not
attempts to bypass rate limits.

## 10. Deployment and packaging

The release package contains:

- Rust libraries and examples
- per-provider live/load probes
- one aggregate `full_stack_probe`
- endpoint/capability matrix
- environment variable reference
- source terms/SLA warning
- deployment guide
- last verified live/load result

No API key, cookie, activation state, downloaded PDF, source fixture containing
personal data or desktop-client artifact is packaged.

Preflight expands to cover every workspace member on the active default
toolchain and records the actual compiler/Cargo versions:

- format
- locked build/check
- unit/integration/doc tests
- clippy with warnings denied
- docs with warnings denied
- compliance and link checks
- package smoke test

## 11. Delivery slices and completion criteria

### Slice A: Core, router and analysis foundation

All normalized types, validation, capability declarations, generic router
adapters and pure calculations compile and pass deterministic tests.

### Slice B: Existing-source expansion

Tencent market statistics and Sina financial/options are fixture-tested and
live-probed. TDX mappings are reused where they already cover the domain.

### Slice C: Eastmoney

All admitted report, flow, board, capital, news, profile, popularity and
limit-pool families use the shared limiter and pass fixture/live/load gates.

### Slice D: CNInfo and Tonghuashun

Announcements/IRM and consensus/hot/reason families pass HTTPS and parser
gates. Plaintext-only families remain explicitly unsupported rather than
silently skipped.

### Slice E: CLS, Baidu and iwencai

Signed news, MA-bearing bars and authenticated semantic search pass their
respective gates. iwencai remains optional without a key.

### Slice F: Exchanges, aggregation and release

Verified official fallbacks are added, aggregate workflows preserve
multi-source evidence, documentation/package/preflight are complete and the
workspace is committed and pushed.

The work is complete only when the README capability matrix distinguishes:

- deterministic implementation
- live verified
- credential/entitlement required
- degraded/unsupported

No unchecked row may be described as fully connected.

## 12. Clean-room and attribution policy

The reference repository is used to discover desired behavior, source names and
public response shapes. Production Rust code is independently designed around
observed protocols and this repository's contracts. Python implementation text
is not copied. The documentation may acknowledge the reference link, but Cargo
packages will not include fake upstream metadata or claim to be a fork.
