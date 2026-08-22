# Official HITHINK Fuyao Financial API

This integration is an independent Rust Provider named `HithinkFinance`. It
uses the official HITHINK Financial API and does not change or impersonate the
existing `magic-ths-rs` public-web Provider.

## Contract source

- Public documentation: <https://fuyao.aicubes.cn/docs/>
- Official repository: <https://github.com/HiThink-Tech/Financial-API>
- Contract snapshot inspected: repository commit
  `9dbef74d2ce535857e610eec265bcb9302942d48`
- Base URL: `https://fuyao.aicubes.cn`
- Authentication: `X-api-key` from `HITHINK_FINANCE_API_KEY`
- Success: HTTP 200 and envelope `code=0` with non-null `data`

The service key is runtime configuration only. It is never serialized into
Core evidence, included in client Debug output, printed by probes or committed.

## Gate A transport design

`magic-hithink-rs` uses `magic-market-transport`; it adds no provider-local
HTTP/TLS stack. The shared Rustls client uses an exact lowercase host, no proxy,
no redirects, identity encoding and no automatic retry. Requests accept only
the following private fixed builders:

| Capability | Exact path | Query keys |
| --- | --- | --- |
| Historical bars | `/api/a-share/prices/historical` | `thscode`, `interval`, `start`, `end`, `adjust`, `offset` |
| Valuation snapshot | `/api/a-share/valuations/snapshot` | `thscodes` |
| Upper pool | `/api/a-share/special-data/limit-up-pool` | `date_ms`, `page`, `size`, `sort_field`, `sort_dir` |
| Lower pool | `/api/a-share/special-data/limit-down-pool` | `date_ms`, `page`, `size`, `sort_field`, `sort_dir` |
| Broken pool | `/api/a-share/special-data/limit-break-pool` | `date_ms`, `page`, `size`, `sort_field`, `sort_dir` |
| Popularity | `/api/a-share/special-data/hot-stock-list` | `period` |

The policy accepts JSON only, bounds each response at four MiB and uses the
composition-supplied timeout. Clones share one 500 ms request-start gate and a
request tracker. Provider business failures are not retried inside one gRPC
request: `4001` and `5xxx` remain typed failures so the caller can apply its own
bounded retry policy without hidden duplicate traffic.

## Admitted mappings

### HistoricalBars

The request must select one Shanghai, Shenzhen or Beijing six-digit equity,
`Day`, an explicit inclusive range no longer than ten years, and a positive
limit. The Provider sends `interval=1d`, `adjust=none` and `offset=0`. It checks
the live response context fields `thscode`, `interval` and `adjust` against the
request even though the rendered endpoint page currently omits those three
response fields.

All bars are decoded before the most recent caller limit is applied. Every
`date_ms` must be Asia/Shanghai midnight, unique and inside the request. The
batch timestamp must identify the newest returned date. OHLC consistency is
enforced by Core; source volume in shares is divided by 100 to Core lots and
turnover is preserved as amount. Each bar keeps its own date as `source_at`;
the batch keeps the raw provider millisecond time as `unix-ms:<value>`.

### MarketStatistics

The exact request contains 1..=100 unique A-share identities. Response count,
order, `thscode` and `ticker` must exactly match. `pe_ttm`, `pe_mrq` and `pb_mrq`
map to the current Core fields. Negative values and `null` remain unchanged in
meaning; missing values are never zero-filled. `ps_ttm` and `pcf_ttm` are
validated but not projected because frozen Core v1 has no matching fields.

The response timestamp is only the newest upstream time among five metrics.
It is batch provenance and is deliberately not copied into record evidence.

### LimitPools

`Upper`, `Lower` and `Broken` use an explicit Asia/Shanghai trading date and
fixed `page=1..N`, `size=200`. Timestamp, total, page count and page size must
remain stable. Every declared page must arrive, the combined count must equal
`total`, and identities must be unique before the caller limit is applied.
Non-trading dates may return a verified empty batch. `PreviousUpper` remains
unsupported; the Provider never approximates it from another pool.

Only source fields with exact Core meaning are projected. For example,
`max_seal_money`, source flags and broken-pool turnover amount are validated but
not placed into unrelated Core fields.

### Popularity

The Provider fixes `period=day`, meaning the official 24-hour hot-stock list.
It validates positive unique ranks, unique A-share identities, names, heat,
rank change and response timestamp before applying a maximum caller limit of
100. The documentation describes `heat` as a number; live responses on
2026-08-22 returned a numeric string. The decoder therefore accepts only a JSON
number or a trimmed, bounded, finite numeric string and rejects all other
coercions.

## Deliberately unadmitted families

- Explicit-symbol realtime snapshots return `timestamp=null`; local observation
  time cannot become provider source time.
- Auction snapshots expose one undirected unmatched quantity and a response
  assembly timestamp, not the two directional queues and record source time
  required by the complete Core auction contract.
- Corporate-action rows do not expose enough typed event and factor fields for
  the current richer Core contract.
- The official documentation does not expose minute K, ticks, Level-2,
  overseas quotes, macro series or full news/research bodies through these
  endpoints. Other Providers are not used to fill those gaps inside a
  `HithinkFinance` batch.

## Failure classification

The gRPC composition maps safe structured outcomes as follows:

| Fuyao outcome | gRPC provider failure | Retryable |
| --- | --- | --- |
| `2001`, `2003` | `provider_authentication_rejected` | false |
| `4001` | `provider_rate_limited` | true |
| `1001..1004`, `3001`, `3004` | `external_query_rejected` | false |
| `3002`, `5001..5003`, network failure | `provider_unavailable` | true |
| decode, identity, pagination or evidence conflict | `provider_response_invalid` | false |

Only the numeric code, safe `request_id` and closed category cross the external
boundary. Provider free text and credentials do not.

## Admission evidence

On 2026-08-22, two bounded live runs succeeded for:

- `600519.SH` unadjusted daily bars for 2026-08-18 through 2026-08-21;
- one-row valuation snapshot;
- explicit 2026-08-21 upper, lower and broken pools;
- ten-row 24-hour popularity response.

A separate three-call serial valuation probe completed with three requests,
zero active requests at exit, maximum concurrency one and a measured minimum
request-start gap of approximately 500 ms. Deterministic tests cover key
redaction, distinct bar times, shares-to-lots conversion, response-context
conflicts, valuation null/negative semantics, whole-batch identity rejection,
pagination contradictions, Beijing identities, numeric-string heat and safe
typed business failures.
