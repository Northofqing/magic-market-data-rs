# R-08 FuturesDelivery gRPC handoff design

## Gate A scope

The external `FuturesDelivery` interface remains `QueryRequest` protocol v1 with
`magic.market.futures_delivery.request` version 2 carrying required positive
`year` and `month`. The only admitted production adapter is CFFEX's checked-in
2026 IF/IH/IC/IM monthly schedule. This handoff documents and tests the existing
interface; it does not add an operation or extend source coverage.

## Evidence and failure semantics

- A valid 2026 month returns exactly four complete records with one CFFEX batch
  identity and source-preserving evidence. Records explicitly say `Planned`,
  carry product-rule and holiday-calendar URLs, and do not claim a monthly
  delivery notice or completed settlement. The fixed schedule is not a runtime
  network observation; absent source timestamps remain absent.
- Version 1 and malformed or missing request fields fail as invalid arguments. A valid request
  shape for a year other than 2026 fails explicitly and is never a verified
  empty month. There is no zero-record success in the formal 2026 schedule.
- The public client-bundle contract must show the envelope, base64 JSON payload,
  record schema, a redacted real response shape and typed failure examples.
- Compatibility tests exercise all twelve months and the failure boundary
  through the composition/gRPC seam rather than asking consumers to recreate
  calendar rules.

## Release path

Keep the dirty main checkout untouched. Build and verify only this isolated
committed revision, deploy that revision's gRPC binary, then perform real mTLS
+ Bearer calls and record the running executable hash and build identity.
