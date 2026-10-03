# D14 qualified daily change discovery contract gaps and next implementation

This report is for the Mac `stock_analysis` consumer and the Windows SDK owner.
The existing SDK can return bounded historical-bar observations, but the
inspected contracts cannot certify `QualifiedDailyChangeDiscovery`. This is a
source and consumer-contract assessment, not a new capability, deployed RPC,
source admission or runtime approval. D17, D20 and R08 remain independent.

The inspected SDK baseline is commit
`9da925a8bc4cdd9d820af92f36a915ec35bb400f`. This slice changes documentation only.
The existing six SZSE responses are feasibility observations obtained earlier;
they are not new captures or qualified discovery fixtures. See the companion
[primary source assessment](2026-10-03-d14-qualified-discovery-source-evidence.md)
and the [earlier native controls](2026-10-02-native-coverage-contract-controls.md).

## Existing consumer and SDK interfaces

The SDK's existing public Interface is `HistoricalBars(QueryRequest)` in
`crates/magic-market-grpc-contracts/proto/magic/market/v1/market.proto`.
`QualifiedDailyChangeDiscovery` is not an existing SDK operation or Rust type.
The public shared handoff materials inspected this turn also contain no
versioned WG07 request and result specification. That absence is scoped to the
inspected materials, not proof that no downstream design exists elsewhere.

Mac's current `daily_bars_async` request uses `{codes:[code],days:days}`. The
earlier `days=90` request did not carry explicit `start` and `end`. It therefore
must not be silently reinterpreted as 90 calendar days, 90 expected exchange
sessions, or evidence that either complete interval was discovered. The
companion report identifies the exact inspected downstream file and binding.

Hithink's existing opt-in
[HistoricalBars version 2](../grpc-historical-bars-coverage-v2.md) is a usable
Interface for observing and auditing one native response. Its envelope retains
the exact request payload hash, decoded request, response identity fields,
response body receipt and quality. Its scope is explicitly
`HithinkNativeDateRangeResponseObservationOnly`; it is not a discovery
qualification contract. The `OutcomeDailyBars` product is likewise a bounded
outcome bundle, not a discovery interval or expected-session certificate.

## Evidence required and what the current interfaces prove

| Requirement | Current evidence | Qualification decision |
| --- | --- | --- |
| Provider-returned security identity | TDX `SecurityBar` has no market/code fields. The adapter writes `request.instrument()` into normalized bars. Hithink preserves the actual `thscode` response field. | TDX normalization is request binding, not returned identity proof. Hithink identity is one necessary input, not sufficient qualification. |
| Exact discovery interval and immutable request identity | TDX explicitly rejects normalized `start` or `end`. Hithink v2 binds the exact request bytes and decoded bounds, but does not invent native bounds echoes. Mac's `days=90` omits exact bounds. | No implicit range or unit conversion. A new formal request must define its interval independently and preserve its identity. |
| Complete expected trading-day vector | Hithink reports `authority_calendar_coverage=Unknown`. TDX exact row cardinality does not enumerate authority sessions. | An OHLC row count, weekday arithmetic, benchmark dates or another source's unbound calendar cannot fill this field. |
| Exhaustion and per-code terminal outcome | Hithink reports `source_exhaustion=Unknown` and `missing_date_reasons=Unknown`. The SZSE report counts concern bounded report rows; the over-range response contains an error despite zero totals. | No missing issuer/date becomes a successful empty, no-trade, suspended, delisted or exhausted terminal outcome. |
| Publication and historical availability | Hithink reports `historical_publication_time=NotProvided`. Row trading dates, latest-bar timestamps and local observation time are different facts. | None can certify that the values were knowable at the requested `as_of`. |
| Revision and snapshot consistency | Hithink reports `source_revision=NotProvided`. The SZSE samples do not establish one immutable revision across a 90-day request or multiple windows. | Body hashes bind observed bytes only; they do not create source revision, replacement or finality semantics. |
| Adjustment identity | TDX normalization selects `Adjustment::Unadjusted`; native `SecurityBar` has no adjustment echo. Hithink distinguishes native adjustment absent, null and value. | Never borrow adjustment from the outer request or another provider. A declared normalization convention is not a native adjustment certificate. |
| Lifecycle and absence coverage | None of the inspected bar contracts supplies the complete per-code lifecycle/absence evidence required by this discovery use. | Listing, suspension, resumption, delisting and source rejection need individually defined evidence; empty rows do not prove them. D17/D20 are not thereby closed. |

The TDX evidence above is inspectable in
`crates/magic-tdx-rs/src/protocol/types.rs:8`,
`crates/magic-tdx-rs/src/adapter.rs:66`, `:352`, `:418` and `:895`.
Hithink's explicit unknown states are in
`crates/magic-hithink-rs/src/historical_coverage.rs:29`;
the versioned service envelope is assembled in
`crates/magic-market-composition/src/grpc_production.rs:4638`.
`docs/integrations/admissions.tsv` admits Hithink historical observations, not a
new qualified discovery capability. These filenames are public package inputs;
the accompanying manifest binds their exact bytes.

## Why no new observation module was implemented

A parser for the six old SZSE responses would expose bounded observations but
would not supply the formal consumer's missing terminal, calendar, revision,
publication or lifecycle evidence. A new SDK method that always returns an
unavailable result would also invent an Interface without an agreed consumer
contract. Neither closes WG07. No provider dependency, endpoint allowlist,
admission flag, protobuf method, existing canonical version or business rule is
changed by this documentation slice.

The official TQ historical-data documentation is a research candidate, not an
admitted Adapter for this Interface. Its map keys, pagination metadata and
stock-list terminal flag do not certify historical range completion or PIT.
The existing local-terminal negative control for quotes warns against treating
a requested map key as verified issuer membership; it is not evidence that the
historical-bar method has the same defect. Source-specific bar evidence would
be required before selecting that Adapter.

## Smallest connected implementation proposal

1. Mac should export the authoritative WG07 request/result specification and
   qualification caller Seam as public source or fixtures in a new shared
   package. It must choose what 90 means and define exact inclusive bounds,
   venue time zone, `as_of`, instrument set, adjustment identity and the
   authority-bound expected-session vector. The old `days` Interface remains
   unchanged. This is the material contract decision still required.
2. Select one provider only after it can supply returned identity, exact
   request/range binding, source exhaustion and per-code terminal semantics.
   Record which source supplies each fact. Define any separately sourced
   calendar/lifecycle join and its temporal/identity binding explicitly; do not
   silently combine responses into an atomic source batch. Source admission
   remains independent under BR-009.
3. Approve a Gate A design for a versioned discovery request/result and closed
   typed qualification failures at the actual caller Seam. The Module should
   keep observation ingestion separate from qualification policy. Existing
   HistoricalBars v1/v2 remains backward compatible. Do not register or
   advertise a qualified production capability merely because a validator can
   reject incomplete inputs.
4. Implement at that agreed Seam using deterministic fixtures first. The
   consumer must receive either a fully qualified result or a typed failure
   retaining every issuer's terminal reason and the actual evidence. Existing
   TDX request-derived identity and Hithink observation-only envelopes are
   negative qualification fixtures, never positive source-admission fixtures.
5. Run the agreed regression and compatibility gates, then obtain separately
   authorized source evidence and same-version RPC acceptance. This proposal
   grants no listener, credential, Provider call, capture, deployment, stop or
   fixed36 RPC authority.

## Required regression vectors for that implementation

- Reject missing or mismatched native issuer/venue, duplicate instruments and
  a normalized identity obtained only from the request shell.
- Reject ambiguous `days=90`, differing bounds/request hash, wrong interval,
  absent or conflicting adjustment, and an unbound `as_of`.
- Reject missing or duplicate expected sessions, a short row prefix, an
  over-range native error with zero rows, and an empty out-of-range page with
  nonzero declared total.
- Reject any code without a proved terminal outcome; distinguish rejected,
  unavailable, suspended, not-yet-listed and delisted source states without
  inventing states from absence. No partial qualified batch may escape.
- Reject caller truncation, mixed snapshots/revisions, post-`as_of`
  publication, unknown revision/publication or missing correction links.
- Keep v1/v2 observation serialization unchanged. Verify exactly bound
  synthetic qualified input only after the contract is agreed; such a unit
  fixture is not actual provider evidence or admission.

## Delivery status

This slice delivers reviewable source facts and a concrete next implementation
sequence. It does not deliver `QualifiedDailyChangeDiscovery`, recover missing
provider evidence or close the original technology/financial-news historical
coverage work. Rust tests, Clippy, cargo documentation and new business RPCs are
not claimed for this documentation-only slice. Fresh documentation/compliance
checks and public-file/hash verification are recorded in the shared handoff.
