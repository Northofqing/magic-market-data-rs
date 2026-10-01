# Official publication service integration

## Gate A: authority and scope

The user's 2026-10-01 instruction “接入吧” authorizes connecting the eight
admitted original-publication sources to the existing unified query service and
providing a runnable periodic collector. This increment does not deploy or
restart an existing service. BR-009, BR-010, BR-021 and BR-067 continue to apply.

## Contract

- Append `OfficialPublications` (64) and `OfficialPublication` (65) to the
  existing v1 Operation enum and MarketDataService. Existing values and news v2
  schemas remain stable.
- Requests use schema version 1: `magic.market.official_publications.request`
  with `{ "limit": 1..20 }`, and `magic.market.official_publication.request`
  with `{ "url": "exact HTTPS original URL" }`. Unknown fields fail.
- `preferred_provider` selects the exact native publisher: Nbs, Pbc, Ndrc, Mof,
  Miit, Mofcom, Nea or Csrc. Nbs is the explicit default; an article URL must
  belong to the selected publisher. No fallback, cross-source merge or
  caller-defined listing endpoint is introduced. Gacc stays explicitly blocked,
  including `allow_unadmitted=true`.
- A successful query returns exactly one v1 native listing or original payload:
  `magic.market.official_publication_listing` or
  `magic.market.official_publication`. Retain the entire evidence envelope,
  original labels, precision, label origin, response URL/hash, observed_at and
  bounded source_rows. `source_at` stays absent because no time zone is proved.
  `complete` means this bounded response passed validation, not complete history.
- Batch identity binds operation, publisher and serialized native envelope,
  including URL, response hash, selection and observation. Independent reads
  remain independent batches.
- Both operations share one source client and its limiter within a registry.
  Provider timeout (at least one second) may shorten the existing 15-second maximum but cannot widen
  it. Transport, body, TLS, redirect, URL and minimum pacing policies are retained.
  Existing gRPC semaphore/spawn_blocking handling owns asynchronous isolation.
  Typed source failures survive the service boundary without response-body leaks.
  Composition directly imports the shared transport's error enum for this mapping;
  register that dependency as shared transport in `http-transports.tsv`. It adds
  no provider-local HTTP/TLS stack or new request path.

## Periodic collection

Provide an opt-in `official-news-collector` executable at the composition
boundary. It uses the same registered operations, polls all eight sources
serially, writes append-only NDJSON evidence for every listing/original success
and failure, and flushes each event. Require an explicit output file, default to
five latest entries and 300 seconds of delay after each completed round. Bound
limit to 1..20 and interval to 60..86400 seconds. Optional positive `--rounds`
terminates verification runs; zero means continuous operation. No overlapping
rounds, unbounded task queue, silent empty success, immediate HTTP retry, inferred
news time, history claim or in-memory growing deduplication set is introduced.
Repeated observations are retained so source revisions remain observable. File
errors stop collection explicitly. This is a collection journal, not a complete
history database; it is started explicitly and is not installed as an OS service.

## Implementation and verification plan

1. Add timeout cap and native-to-service adapters with exact capabilities.
2. Append protobuf/service/server mappings and refresh public client bundle
   contract metadata, leaving deployment identity untouched.
3. Add the bounded collector and its invocation documentation.
4. Test request rejection before I/O, evidence retention, shared pacing, blocked
   Gacc, typed failures, gRPC method/enum parity and collection failure isolation.
5. Exercise both service operations against all eight real sources in bounded
   rounds; retain exact failure and scope evidence. Run workspace formatting,
   tests, Clippy, compliance, docs, links and client bundle compatibility checks.
