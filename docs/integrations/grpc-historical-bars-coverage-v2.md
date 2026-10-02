# Hithink HistoricalBars observation coverage version 2

This is an opt-in canonical JSON version, not a new protobuf method, Provider
admission, source calendar or point-in-time guarantee. It follows the
[scoped Gate A design](../superpowers/specs/2026-10-02-hithink-historical-coverage-design.md).

## Request and compatibility

Select operation `HistoricalBars`, `preferred_provider=HithinkFinance`,
`allow_unadmitted=false`, schema `magic.market.historical_bars.request`,
`schema_version=2`. The request JSON is unchanged:

```json
{"instrument":{"exchange":"Shanghai","code":"600519","asset_class":"Equity"},"interval":"Day","start":"2026-08-18","end":"2026-08-19","limit":1}
```

Version 1 still returns individual `magic.market.bar` version-1 records in
ascending date order with unchanged fields/units/evidence. Actual local deletion
now makes the batch and outer response incomplete. All source rows are checked
before limiting; malformed older rows cannot hide behind a small limit. No
deletion retains the existing strict quality semantics, not calendar coverage.
Other Providers remain version 1 only. Unknown versions/schema fail before I/O.
Consumers must explicitly pin Hithink; default routing is unchanged.

Version 2 returns exactly one `magic.market.historical_bars.coverage` version-2
record, even for an empty native row set. The envelope has `request_id`, SHA-256
of exact original request payload bytes in `request_payload_sha256`, decoded
`request`, `coverage_scope=HithinkNativeDateRangeResponseObservationOnly`, and
`result={batch,coverage}`. The nested batch retains original records, provenance
and QualityReport. Outer provider/batch/source/observation fields match that batch.
The existing configured canonical-payload byte bound still applies.

## Coverage fields

| Field | Meaning |
| --- | --- |
| `response_validated` | All received rows and response context passed the existing bounded contract. Always true on success, not an exhaustion claim. |
| `validated_source_rows`, `returned_rows` | Counts before/after local caller limiting. No declared source total is invented. |
| `caller_limit_truncated` | True only when actual validated rows were locally removed. |
| `source_exhaustion` | `Unknown`: no native terminal/total guarantee is supplied. |
| `authority_calendar_coverage`, `missing_date_reasons` | `Unknown`: no authority session vector or per-date reason is supplied. |
| `source_revision`, `historical_publication_time` | `NotProvided`: no revision/publication evidence is supplied. |
| `pit_guarantee` | Always false. Row dates and batch timestamps are not historical availability instants. |
| `native_response` | Actual response `request_id`, `thscode`, `interval`, `timestamp_ms` and `adjust`. `adjust` distinguishes `{state:Absent}`, `{state:Null}`, and `{state:Value,value:none}`. No exchange, asset class or request bounds are fabricated as native echoes. |
| `response_receipt` | SHA-256/length of actual transport response body before JSON parsing, plus validated `final_url`. Not reserialized JSON, headers, TLS or HTTP/2 wire bytes. |

Outer `complete` mirrors batch quality. Even true plus zero records certifies
only validation of the observed native row set, not a holiday, suspension,
absence of trades or complete history. A failed query is a typed failure, never
a successful empty envelope. Hashes bind bytes but do not provide those bytes,
source signatures, revision guarantees or PIT certification.

This source candidate is not a deployed RPC receipt. Production build identity
and version-2 acceptance require an independently authorized deployment and
same-instance Health/business verification. D14/D17/D20 authority gaps and R08
automatic Confirmed are unaffected.
