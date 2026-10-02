# HithinkFinance HistoricalBars records and coverage limits

The source successor corrects actual local caller-limit truncation to
`complete=false` and adds an opt-in
[version-2 observation envelope](grpc-historical-bars-coverage-v2.md).
The production observations below remain unchanged historical receipts, not
evidence that this successor is deployed.

This is the public record contract for Mac consumers of the existing
`HistoricalBars` RPC, not a new endpoint or a coverage admission. Production
source `67c832e43f36f188e4d769f409691c0b1d9a2ea2` returns exact-range daily
observations through `HithinkFinance`. The record shape is already version 1.
Its `complete=true` does not certify every trading day, suspension status,
immutable history or point-in-time availability; the limit control below
demonstrates that a truncated result can also carry that flag in this build.

## Request and record schema

Use `preferred_provider=HithinkFinance`, `allow_unadmitted=false`, operation 1,
request schema `magic.market.historical_bars.request`, `schema_version=1`,
and `content_type=application/json; charset=utf-8`:

```json
{"instrument":{"exchange":"Shanghai","code":"688277","asset_class":"Equity"},"interval":"Day","start":"2026-07-16","end":"2026-07-30","limit":15}
```

Start and end are inclusive Asia/Shanghai dates. They must both be explicit.
The calendar vector here has 15 dates, so limit 15 avoids caller-side
truncation for one daily bar per date; it does not prescribe 15 trading days.
The upstream equity request sends `thscode=688277.SH`, `interval=1d`,
`adjust=none`, `offset=0` and Shanghai midnight through end-of-day millisecond
bounds. Returned records use schema `magic.market.bar`, version 1 and the
same JSON content type. Numeric wrappers serialize as JSON numbers.

| Record field | Meaning and source mapping |
| --- | --- |
| `instrument` | Core `{exchange,code,asset_class}`. The adapter checks upstream `data.thscode` against the requested canonical code before constructing this identity. The original upstream `thscode` and separate exchange/asset echo are not carried in the record. Do not label the request shell as native identity evidence. |
| `interval` | `Day`, checked against upstream `data.interval=1d`. |
| `bar_start`, `bar_end` | The same `YYYY-MM-DD` date, from an upstream `date_ms` required to be Shanghai midnight. Not an intraday trading interval or publication instant. |
| `open`, `high`, `low`, `close` | Unadjusted stock price, CNY per share, from upstream `*_price`. Core checks consistent OHLC bounds. |
| `volume` | Core lots of 100 shares. Upstream shares are divided by 100; fractional lots are preserved. Do not multiply amount by this factor. |
| `amount` | Upstream `turnover`, CNY, numeric or the Core optional field. This admitted adapter returns `Some` for every valid source row and rejects negative/nonfinite values. |
| `adjustment` | `Unadjusted`; upstream equity `data.adjust` must equal `none`. No adjusted-price/factor history is implied. |
| `source_at` | The row's original date as `YYYY-MM-DD`. It is not a publication or correction timestamp. |
| `observed_at` | Local response observation, Unix seconds plus fractional nanoseconds as a string. Not source publication time. |
| `provider` | `Tonghuashun`, the underlying record provider. The outer QueryResponse selects `HithinkFinance`. These are deliberately different. |
| `batch_id` | Upstream success envelope `request_id`; identical to the outer batch ID and all records from that acquisition. Distinct acquisitions can have different IDs for unchanged prices. |

Per-record evidence is flattened into the last four fields, not nested under
`evidence`. The outer `source_at=unix-ms:<value>` preserves upstream batch
`data.timestamp`, required to identify the latest returned source date. It
must not be compared for string equality with row `source_at=YYYY-MM-DD`.
All rows must be unique and within the explicit request, are sorted ascending
by source date, and only then are the latest caller limit retained.

## Same production process observations

On 2026-10-02 around 01:16 Asia/Shanghai, a read-only mTLS acquisition from
`10.211.55.3:50051` returned the following. Before and after every RPC,
Health was live/ready, its identity error was empty, process start was
`1790862122793`, and all four build identity fields were unchanged.

| Query | Exact request payload SHA-256 | Result |
| --- | --- | --- |
| 688277, 07-16 through 07-30, limit 15 | `52e719ce1dbd59e2181a6a5769a915a48f039cb3f9a8f0ac6a583702d36a162b` | One bar, 07-30; `complete=true`. |
| 688561, the same dates, limit 15 | `d598ccf1da2fe560498eaa1dfe2521280450ac16cb049e7383612cfe9503a3a5` | Eleven bars: 07-16, 17, 20, 21, 22, 23, 24, 27, 28, 29, 30; `complete=true`. |
| 688561, the same dates, limit 1 | `372a97e00fd5c72d7ff221e0bddf9690ac5bc76ffd1b176c52f993b63493b08e` | Only 07-30; still `complete=true`. Ten observed source dates are omitted by this caller limit. |

For 688277 the single 07-30 bar has OHLC 22.5, 22.5, 17.2, 17.4;
volume 302553.5 lots; amount 603447516.21 CNY. The other dates' absence is
an observation, not proof of their suspension reason or a certified reopening
event. The separately retained exchange suspension span is narrower evidence,
and must not be injected into this provider record.

Service identity: version `0.2.0`; source
`67c832e43f36f188e4d769f409691c0b1d9a2ea2`; descriptor
`abf28a3e0028488a7579da4d961e1a7c1408482bdc0500122c1956d225e480cf`;
binary `9302a3036c7ed272fdf24dd84b4a66881ab682649e9af76b68ef62b2ad083854`.
The three original receipts, complete payloads, individual record hashes,
Health and Capabilities are in the public evidence handoff, run
`36763c29f80a46b9bef088c717368d66`. No provider key or client credential is
part of that handoff. It does not change the running listener or Mac's approved
candidate.

## Missing dates and empty results

This version exposes no expected trading-date vector, source total/terminal
marker, per-date gap reason, caller truncation flag, covered-date manifest,
revision ID, native query-bound echo or historical publication state. A consumer
can compare actual source dates to a separately authorized session calendar,
but must label absent dates as unknown unless independent status evidence exists.
Neither weekends nor absent bars should be zero-filled as trading observations.

Native `item=[]` can become a zero-record batch if its timestamp and context
are otherwise valid. That is only an empty provider row set. It does not
certify no trades, a holiday, a suspended/unlisted instrument or complete
historical coverage. No zero-row HistoricalBars positive receipt was acquired
in the three controls above. Provider unavailable/authentication/decoding errors
remain typed failures with zero records, never an admitted empty substitute.

## Receipt byte fidelity

`rpc.response.records[].data_utf8` preserves every CanonicalRecord data byte
after UTF-8 decoding; `data_sha256` hashes its exact bytes, not the parsed JSON
or a client reserialization. `decoded` is a convenience view only. The full
known QueryResponse, Health and Capabilities can also be decoded from the
`*_protobuf_reencoded.bytes_hex` fields. Those are explicitly prost-reencoded
known protobuf fields, not original HTTP/2 wire bytes or upstream HTTP bodies.
Original typed error trailers, when a call fails, are retained separately as
`error_detail_bytes_hex` and `error_detail_sha256`. The production log files
were zero bytes during this run, so no matching server log/trace receipt was
available; request IDs and the original error trailer remain the correlation
evidence. No trace ID is invented.
