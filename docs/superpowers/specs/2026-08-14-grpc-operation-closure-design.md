# gRPC Production Operation Closure Design

**Status:** Gate A approved in conversation on 2026-08-14

## Objective

Reduce the explicitly blocked gRPC operation set without weakening the
provider contracts, fabricating unavailable fields, or relabelling a narrower
data family as a wider one.

## Admitted implementation slices

### Security metadata

`SecurityMetadata` uses the existing Tencent provider implementation and the
existing BR-007 field-level availability contract. The exact production scope
is bounded instrument identity, source name, source-backed ST flag and an
explicitly derived board. Listing date and price-limit rule/version remain
unavailable when the source does not supply them. The gRPC response preserves
`complete=false`, record status and unavailable fields; it must not advertise
those fields as complete.

This is not a contract relaxation: BR-007 explicitly requires unproved fields
to remain unavailable. The operation registration describes this narrower
scope instead of claiming complete exchange master data.

### TDX company profile

`SecurityProfiles` uses the existing TDX public TCP protocol and exact fixed
production endpoint already used by the TDX composition. It supports one to
eight unique Shanghai or Shenzhen equities per request. For every instrument:

- the TDX security-list/finance metadata path supplies the exact instrument
  identity, source name and optional source-backed listing date;
- the TDX F10 category response must contain exactly one non-empty `公司概况`
  section;
- every non-empty line in the complete decoded section is retained as an
  ordered source-labelled profile fact, up to the explicit 256-fact bound; it
  is not parsed into guessed industry, share counts or other fields;
- missing identity, name, section, empty content, duplicate request or any
  failed instrument rejects the complete request;
- F10 has no provider timestamp, so local observation time is retained only as
  observation evidence and never promoted to `source_at`.

The wire protocol's `u16` content length is the per-section bound. The explicit
eight-instrument request limit plus the existing gRPC response-size limit bound
the full response. The Provider returns a strict atomic batch.

## Operations that remain fail-closed

- `Auctions` requires an authorized Level-2/broker source satisfying BR-035.
- public TDX cumulative turnover is not `MoneyFlows`; EMQuant production
  composition requires a supported target-platform SDK runtime and independent
  admission evidence.
- `FuturesDelivery`, `TechnicalBars` and `FundFlowSeries` retain their existing
  live-evidence blockers.
- `PostCloseFlows` and complete `MarketRankings` retain BR-019/BR-034 atomic
  snapshot blockers; Provider Top-N must not impersonate either contract.
- `MarketBreadth` remains blocked until an admitted complete-market snapshot is
  available.

These blockers stay visible through `GetCapabilities` and fail before Provider
I/O. A later slice may close them only with their own Gate A evidence; this
design does not authorize changing their admission flags.

## Network and dependency impact

No HTTP/TLS host, endpoint allowlist or dependency is added or widened. The
implementation reuses Tencent HTTPS and the existing TDX public TCP/F10
protocol. Core and Router remain transport-neutral, and no downstream path
dependency is introduced.

## Verification

- deterministic normalization and failure tests for the TDX profile boundary;
- composition tests proving exact schemas, provider selection and incomplete
  metadata preservation;
- two bounded TDX security-profile live probes and three serial load probes
  before the operation is described as production-available;
- workspace format, tests, Clippy, compliance, docs and gRPC live verification.
