# Concurrent Realtime Quote Routing Design

## Goal

Keep realtime prices available when one admitted Provider is slow or unavailable.
An unpinned `RealtimeQuotes` request races every runtime-available, repository-
admitted quote Provider and returns the first data-bearing normalized batch.
The selected batch remains atomic and keeps its original Provider identity.

The route also admits the official HITHINK Fuyao explicit-symbol snapshot as an
observation-time quote source. Missing record source time remains explicit; it
is never replaced by local time or a batch timestamp.

## Deep module and seams

`OperationRegistry::execute` remains the external routing interface. Concurrent
execution, bounded per-Provider work and failure aggregation stay inside that
module. Callers continue to send the existing `RealtimeQuotes` command and read
the existing `QueryResult.selected_provider` plus serialized record evidence.

`HithinkClient` implements the existing Core `RealtimeQuotes` interface. Its
HTTP envelope, request bounds, identity validation, unit conversion and missing-
time semantics remain inside the HITHINK adapter.

Tests exercise only these two public seams. They do not depend on worker thread
names, registration storage or private normalization helpers.

## Routing contract

- An explicit `preferred_provider` remains pinned and never races or falls
  through.
- An unpinned request concurrently starts every admitted and runtime-available
  `RealtimeQuotes` registration.
- The first successful response containing normalized records wins, whether or
  not optional record evidence makes the batch quality partial.
- Empty responses and typed failures do not win. If every candidate fails or is
  empty, the RPC returns one bounded typed attempt per Provider in deterministic
  registration order.
- Records from different Providers are never merged. `selected_provider`, each
  record's Provider, batch ID, observation time and source time remain those of
  the winning batch.
- One in-flight race call per Provider bounds abandoned work. A Provider already
  serving an earlier race is reported as temporarily busy for the newer race.
  Provider-native timeouts still bound a losing call after a winner returns.

## HITHINK quote contract

The adapter adds only this Gate A approved endpoint to the existing fixed host:

```text
GET https://fuyao.aicubes.cn/api/a-share/prices/snapshot?thscodes=<codes>
```

The request accepts 1 through 60 unique A-share equities and sends the exact
comma-separated `.SH`, `.SZ` or `.BJ` identities. The response must contain the
same cardinality, order, `thscode` and ticker identities. All price, percentage,
volume and turnover values are finite; positive price fields remain prices,
source volume in shares converts to Core lots, and turnover remains CNY.

The source does not return a security name or a per-record source timestamp.
Each normalized record therefore has:

- `provider=Tonghuashun`;
- `name=null`;
- `source_at=null`;
- local receipt time only in `observed_at`;
- `status=Unavailable`, explicitly marking that the complete numeric snapshot
  is not eligible for strict field/evidence completeness.

The batch can still be complete when every requested identity and numeric field
was validated. The upstream `data.timestamp` is optional. When present and
positive, its original millisecond value is retained only as batch provenance
`source_at=unix-ms:<value>`; it is never copied into record evidence. When absent,
batch `source_at` is also absent.

This observation-time source is not eligible for BR-033 strict five-second
freshness. Consumers requiring strict source time must inspect record status and
`source_at` rather than treating transport success as freshness proof.

## Failure behavior

Missing or duplicate request identities, wrong asset classes, unknown response
fields, cardinality/order/identity conflicts, invalid numbers, negative volume
or turnover, malformed timestamps and Provider failures reject the entire
HITHINK batch. No partial rows, zero-filled fields or synthesized times are
returned.

## Admission and verification

Repository admission requires deterministic fixtures, two bounded credentialed
live probes and a three-call serial load probe. The release also runs formatting,
focused and workspace tests, Clippy, compliance, documentation links and bundle
preflight checks.
