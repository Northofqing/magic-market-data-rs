# Production Data Closure Design

Date: 2026-07-27

Status: Gate A approved by the user's standing instruction to implement every
identified project capability, confirm every pending item, use parallel work where
useful, and not let uncertainty block honest feature delivery.

## Goal

Turn the remaining public-market-data gaps into production contracts with explicit
identity, completeness, freshness, provenance, and live-admission evidence. Where
the required source is licensed or account-authenticated, land a usable integration
boundary and keep the public-provider capability explicitly unavailable.

This design covers:

1. complete security metadata and normalized corporate actions;
2. full-market rankings, market breadth, concept projection, and research consensus;
3. CFFEX delivery-calendar production admission;
4. strict five-second realtime quote admission;
5. opening-auction and broker-account boundaries.

It does not add a downstream path dependency or turn locally derived observations
into source facts.

## Current Evidence

### Security lifecycle

- `SecurityMetadata` exists, but it is not a `SourcedRecord`, so the declared
  Router cannot use the normal `FailoverChain::route` path.
- TDX metadata omits listing date and supplies derived board/rule fields.
- TDX finance already contains raw `ipo_date`, but response market/code identity
  is not validated.
- TDX XDXR already has DTO, parser, sync/direct/async clients, but declared-count
  truncation can return a partial success and no provider-neutral corporate-action
  contract exists.

### Rankings and consensus

- `MarketRankings` and `ConceptHits` have Core and Router seams but no production
  provider.
- Eastmoney board flows and strict 15:35 post-close rankings are reusable.
- Tencent exposes volume ratio for caller-supplied batches, not a complete
  full-market ranking.
- No typed market-breadth contract exists.
- THS consensus parsing is implemented; application composition and current live
  admission are missing.

### CFFEX, realtime, and auctions

- The CFFEX official-notice parser is strict, but the production capability is
  fixed false and the formal trait always returns `Unsupported`.
- The 2026-07-27 CFFEX live probe failed during TLS initialization from the current
  deployment path; both Rustls and curl/LibreSSL failed after ClientHello.
- Router admission checks whether `source_at` exists, not whether it is fresh.
- The complete auction record requires unmatched bid/ask quantities and a source
  timestamp. No admitted public provider supplies those fields.

## Alternatives

### A. One cross-workspace implementation

Add every type, provider, router, and live probe in one change. This creates a
large review surface and makes it difficult to tell whether a green test proves a
source contract or only a composition helper.

### B. Source-pure capability tracks

Land each capability through Core contract, provider normalization, Router
admission, deterministic tests, live probe, and documentation. Reuse existing
source-specific contracts when their semantics already match.

This is the selected approach.

### C. Crawler-first aggregate

Join whatever public pages are available into a single broad response. This cannot
prove atomicity, full-market coverage, Level-2 queue fields, source timestamps, or
broker account identity and is rejected as the production contract.

## Architecture

### Track 1: Security master and lifecycle

#### Core

- Make `SecurityMetadata` implement `SourcedRecord`.
- Add checked `CorporateActionStatus`, `CorporateActionTerms`,
  `CorporateAction`, `CorporateActionRequest`, and `CorporateActions`.
- Every corporate-action record contains `InstrumentId`, action category,
  effective date, optional record/ex/dividend dates, typed terms, status, and
  `SourceEvidence`.
- Empty results are successful only when the provider proves the requested
  instrument/date range was completely searched.

#### TDX

- Validate finance response market/code against the request instead of copying the
  request identity into an unverified response.
- Validate IPO date as zero-or-valid `YYYYMMDD`; reject malformed or future dates.
- Consume exactly the declared XDXR record count. A truncated record is an atomic
  protocol error.
- Normalize only proven XDXR categories:
  distribution, split, and reverse split. Unknown categories remain explicit
  schema/admission failures until separately specified.
- Sort and deduplicate normalized actions, enforce request range, and give every
  record the same provider/batch evidence as its `DataBatch`.

#### Security metadata composition

- TDX supplies exact listing date after finance identity validation.
- Existing code-derived board identity remains marked incomplete.
- Official exchange or separately admitted public security-master responses may
  enrich board/ST/listing identity, but derived code-prefix rules never become
  source-backed facts.
- Price-limit policy receives an explicit rule version and effective date. Policy
  derivation is labelled `LocalAnalysis`; it is not attributed to a provider.

### Track 2: Rankings, breadth, concepts, and consensus

#### Typed contracts

- Extend ranking semantics with an explicit metric and unit:
  `VolumeRatio`, `MainNetInflow`, and board-flow metrics.
- Ranking records always include rank, code and name for instruments, scope,
  source session/date, metric value/unit, and evidence.
- Add `MarketBreadthSnapshot` and `MarketBreadth` as a separate contract with
  universe, total/valid/up/down/flat, limit-up/down, coverage, maximum source skew,
  and evidence. Breadth is not encoded as a fake ranked instrument.

#### Providers

- Reuse Eastmoney `BoardFlows`; add exact-cardinality and provider-order evidence
  checks.
- Reuse Eastmoney `PostCloseFlows` for the strict current-day 15:35 contract and
  add a dedicated live operation.
- Probe Eastmoney's full-market list for source-ranked volume ratio and main net
  inflow. Admit each metric independently only after exact pagination, unique
  instrument identity, code+name, common source time, ordering, and three-market
  coverage pass.
- If the direct ranking source is unavailable, implement a labelled composite
  snapshot using a discovered universe plus Tencent batches. It reports coverage
  and maximum skew and never claims atomic source ranking.
- Project TDX concept board memberships into `ConceptHit` only for the proven
  Shanghai/Shenzhen concept scope; Beijing remains explicit `Unsupported`.

#### Consensus

- Keep Router provider-neutral.
- Add an integration example that depends on both `magic-ths-rs` and
  `magic-market-router`, registers `consensus_source`, and demonstrates
  verified-empty and failover behavior.
- Run a current THS live admission and retain the artifact.
- Complete Eastmoney target-price aggregation as a distinct research contract;
  contributor count, observation period, target range/mean, source date, and
  evidence must be present before admission.

### Track 3: CFFEX and realtime freshness

#### CFFEX

- Keep official CFFEX HTTPS hosts and paths allowlisted, redirects disabled,
  bounded responses, MIME validation, timeout, and pacing.
- Make transport selection explicit in configuration. A second TLS backend may be
  added only as an operator-selected backend, never as a silent fallback.
- Make the diagnostic and formal trait share one internal implementation.
- The formal capability remains false until a bounded live probe for a current or
  recent month returns the exact IF/IH/IC/IM set and passes human review.
- Do not infer the third-Friday date and do not substitute a third-party calendar
  for official admission.

#### Five-second quote policy

- Extend Router admission with optional `max_source_age`.
- Parse provider time using the existing Core admission timestamp parser.
- For a strict quote route, reject absent, malformed, future, mixed, or stale
  record/batch timestamps. The oldest record controls batch age.
- `age == 5s` is accepted and `age > 5s` is rejected.
- Apply the five-second policy during continuous trading only. Pre-open, lunch
  break, post-close, replay, and historical consumers select an explicit different
  policy.
- TDX remains available only to routes that do not require a source timestamp;
  `observed_at` never substitutes for `source_at`.

### Track 4: Licensed and authenticated boundaries

#### Opening auction

- Preserve the existing complete `Auctions` contract.
- Add a documented provider integration seam and a conformance test kit for an
  authorized Level-2/broker source.
- Public pages with only indicative price/volume may be exposed through a narrower
  diagnostic type, with capability false for complete auctions.
- Never infer unmatched queues or use fetch time as provider time.

#### Broker account data

- Cash, positions, orders, and executions stay outside this public market-data
  workspace.
- Document the required authenticated broker gateway boundary and identifiers.
- Do not read browser cookies or scrape a logged-in account to create a production
  account API.

## Error and Atomicity Rules

- Invalid requests stop routing.
- Transport, rate-limit, and admitted recoverable provider failures may try the
  next source.
- Identity mismatch, malformed dates, declared-count truncation, duplicates,
  inconsistent source time, incomplete coverage, and unknown normalized categories
  are explicit failures.
- A successful empty batch must carry evidence that proves the exact request was
  searched.
- Cross-provider joins retain field-level evidence and are never rewritten as a
  single-source record.

## Evidence and Release Gates

Every capability must have:

1. deterministic parser/normalizer tests;
2. request identity, range, ordering, duplicate, empty, and provenance tests;
3. Router rejection/failover tests;
4. a bounded live operation with exact assertions, not merely non-empty output;
5. an admission artifact recording command, timestamp, provider/source time,
   record count, and result;
6. formatting, workspace check/test, Clippy, compliance, docs, coverage, and
   release packaging gates.

A live endpoint that remains inaccessible does not block delivery of the parser,
transport choice, formal contract, probe, and explicit capability state. It does
block a claim that the capability has passed production admission.

## Acceptance

The program is complete only when:

- every public-data track above has an implemented and tested contract;
- every advertised provider capability has current admission evidence;
- unsupported licensed/account capabilities have usable integration seams and
  cannot be mistaken for public availability;
- README and integration documentation show code and name together for ranked
  securities and state the exact coverage/freshness boundary;
- no downstream path dependency or provenance substitution is introduced.
