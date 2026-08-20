# gRPC news evidence v2 and bundle consistency design

## Gate A boundary

This change is approved by the 2026-08-19 production-blocking defect report.
It is deliberately limited to existing read-only Provider clients and the
existing gRPC operations.

- `GlobalNews` keeps its current request shape, but both the request payload and
  every returned `magic.market.news_item` payload use schema version 2. Version
  1 remains frozen and is not silently reinterpreted.
- Version 2 serializes the complete `NewsItem`, including its record-local
  `SourceEvidence`, and validates the entire batch before serializing any
  record. Missing, mixed, conflicting, future, unordered, or batch-substituted
  evidence fails atomically as `invalid_evidence`, with `retryable=false`.
- Provider source strings remain byte-for-byte source evidence. Published news
  time may be normalized to RFC3339, but it must identify the same instant.
  Batch `source_at` is the newest returned record's raw source string and is
  never copied into an older record.
- `InstrumentNews` version 2 adds the mandatory unambiguous RFC3339
  `captured_through` instant. Sina still receives the exact inclusive source
  date range; after complete page validation, the gRPC composition includes
  only records at or before the caller cutoff and rebuilds batch provenance
  from the newest retained record. If the complete, fully validated upstream
  batch has no record at or before the cutoff, the response is a truthful
  admitted empty batch with `records=[]`, no `source_at`, and the real upstream
  `batch_id`/`observed_at`. No server-current date supplies the cutoff, and
  filtering never hides invalid upstream record evidence.
- `ErrorDetail` receives append-only evidence fields. They expose only a stable
  Provider name, evidence code, field, and optional record index; credentials,
  upstream payloads, and unbounded Provider text are excluded.
- Eastmoney continues to fetch only
  `https://roll.eastmoney.com/finance.html`. The exact first-party
  `futures.eastmoney.com/a/<numeric-id>.html`,
  `bond.eastmoney.com/a/<numeric-id>.html`, and
  `hk.eastmoney.com/a/<numeric-id>.html` hosts are added only to the retained
  metadata-link allowlist and are never fetched by `GlobalNews`.
- Jin10 continues to exclude locked rows before reading protected content.
  Source `data.lock=true` is the complete exclusion signal; `vip_level` is not
  required because it is neither stable nor consumed.
- ThePaper requires both forward flags to be valid and equal. A non-empty
  external `link` is a stronger exclusion signal even when both flags claim a
  native row, so external publishers are never relabeled as ThePaper. Its
  normalized RFC3339 `published_at` preserves the exact millisecond instant of
  the raw `unix-ms:` evidence string.
- CLS nonzero `errno` responses use a typed external-query rejection. Clients
  receive only the stable Provider/reason/retry classification; bounded,
  control-free Provider reasons are retained in server stderr for correlation.
- CFFEX production `FuturesDelivery` remains the checked-in BR-051 fixed
  schedule with no runtime I/O. The stale client bundle and documentation are
  refreshed; the plaintext notice reader remains diagnostic only.
- The client bundle is assembled from the same source proto and documentation,
  includes the derived-products document, and carries a public-file SHA-256
  manifest. Private keys and bearer tokens are not included in the manifest.

No Provider request endpoint, path, query, timeout, response body limit,
redirect policy, proxy policy, TLS policy, or downstream dependency changes.

## Version 2 contracts

`GlobalNews` request data remains:

```json
{"limit":20}
```

The outer canonical request and every record use `schema_version=2`. Each
record contains the full `NewsItem` JSON and a non-null `evidence` object.

`InstrumentNews` version 2 request data is:

```json
{
  "instrument":{"exchange":"Shanghai","code":"600396","asset_class":"Equity"},
  "start":"2026-08-19",
  "end":"2026-08-19",
  "limit":20,
  "captured_through":"2026-08-19T16:15:37+08:00"
}
```

`start` and `end` remain an all-or-nothing inclusive source-date range.
`captured_through` is mandatory, must be an unambiguous instant, and its China
calendar date must equal `end` when an explicit range is present. This prevents
a caller cutoff from being widened by a server clock or an unrelated end date.

## Atomic evidence checks

For every returned news record, the gRPC boundary requires:

1. the expected typed Provider identity;
2. the response batch ID and observation time;
3. a non-empty original `source_at` whose instant equals `published_at` and is
   not later than `observed_at`;
4. strict newest-to-oldest record order; and
5. batch `source_at` exactly equal to the newest record's original source
   string.

Provider source formats admitted by the v2 validator include Eastmoney minute
text (`YYYY-MM-DD HH:MM`, interpreted as Asia/Shanghai only for comparison),
Jin10 second text (`YYYY-MM-DD HH:MM:SS`, likewise), CLS epoch seconds,
ThePaper `unix-ms:<milliseconds>`, fractional epoch observation timestamps, and
explicit-offset RFC3339. The comparison conversion is not serialized.

## Failure contract

Evidence failures use gRPC `FAILED_PRECONDITION` plus `ErrorDetail`:

- `reason_code=invalid_evidence`
- `retryable=false`
- `admission=UNADMITTED`
- exact `provider`, `evidence_code`, `evidence_field`, and optional
  `record_index`

Provider schema conflicts in Consensus use the same outer classification with
stable field-level codes. No partially validated batch is returned.
