# HITHINK current auction observations design

## Gate A boundary

This design is approved by the caller's 2026-09-09 instruction to implement a
separate interface after reviewing the official HITHINK Financial API contract.
It adds one read-only gRPC operation and does not widen any HTTP host, path,
timeout, body-size, redirect, proxy or authentication policy. The existing
`/api/a-share/auction/snapshot` allowlisted transport is reused.

## Problem

The complete Core `Auctions` contract requires an exact trading date, provider
source time and directional unmatched bid/ask quantities. HITHINK's current
snapshot publishes none of those. Mapping the response into `AuctionSnapshot`
therefore discards the useful signed `auction_unmatched` value and necessarily
marks every record unavailable.

The upstream contract instead provides a useful current observation: an
explicit `live` or `final` request stage, response phase and status, nullable
auction values, a single directionless unmatched number, and a response
assembly timestamp.

## Decision

Add `CurrentAuctionObservations` as a distinct append-only gRPC operation.

- The request contains 1..=100 unique A-share instruments and a required
  `stage` of `live` or `final`.
- The provider forwards the stage unchanged to the existing official endpoint.
- Records preserve every documented numeric field. Matched volume is explicitly
  named and converted from source lots to shares. `auction_unmatched` stays a
  signed provider-native value because the source publishes neither its unit
  nor a bid/ask sign mapping.
- A zero auction/open/last price means that no price is currently available and
  is returned as `null`; it is not a protocol failure. Zero quantities and
  amounts remain valid zero observations.
- The response assembly `timestamp` becomes only `evidence.observed_at` and
  batch `observed_at`. Record and batch `source_at` remain absent.
- No `trading_date`, directional unmatched quantities, source timestamp or
  alternate-Provider data is synthesized.
- Exact response cardinality, order and identity are atomic. Unknown fields,
  malformed numbers, contradictory response states and `not_ready` reject the
  whole request with a typed failure and no partial records.

The existing complete `Auctions` contract and `AUCTIONS_ADMITTED=false` remain
unchanged. A current observation cannot satisfy an exact-date or BR-033
source-freshness requirement.

## Public contract

- Operation/RPC: `CurrentAuctionObservations`
- Request schema: `magic.market.current_auction_observations.request`, version 1
- Record schema: `magic.market.current_auction_observation`, version 1
- Provider: `HithinkFinance` only; explicit provider requests never fall through

## Verification

Deterministic tests cover live/final forwarding, zero-price/no-trade handling,
single unmatched-value preservation, evidence timestamps, identity/cardinality
fail-closed behavior, unknown fields and the continued rejection of formal
HITHINK `Auctions`. Admission evidence uses two bounded live reads and three
serial reads without logging the API key.
