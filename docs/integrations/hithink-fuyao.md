# Official HITHINK Fuyao Financial API

This integration is an independent Rust Provider named `HithinkFinance`. It
uses the official HITHINK Financial API and does not change or impersonate the
existing `magic-ths-rs` public-web Provider.

## Contract source

- Public documentation: <https://fuyao.aicubes.cn/docs/>
- Official repository: <https://github.com/HiThink-Tech/Financial-API>
- Contract snapshot inspected: repository commit
  `44b7aa34dd504675f3ddaa15b3d478ea16f97884`; the auction contract was
  introduced at `9dbef74d2ce535857e610eec265bcb9302942d48`.
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
| Realtime quote snapshot | `/api/a-share/prices/snapshot` | `thscodes` |
| Index historical bars | `/api/a-share-index/prices/historical` | `thscode`, `interval`, `start`, `end` |
| ETF historical bars | `/api/fund/market/historical` | `thscode`, `interval`, `start`, `end` |
| Valuation snapshot | `/api/a-share/valuations/snapshot` | `thscodes` |
| Adjustment events | `/api/a-share/corporate-actions/adjustment-factors` | `thscode`, `from`, `to` |
| Income statements | `/api/a-share/financials/income-statements` | `thscode`, `period`, `limit` |
| Balance sheets | `/api/a-share/financials/balance-sheets` | `thscode`, `period`, `limit` |
| Cash-flow statements | `/api/a-share/financials/cash-flow-statements` | `thscode`, `period`, `limit` |
| Exact ticker search | `/api/meta/tickers/search` | `q`, `exchange`, `asset_type`, `limit` |
| Upper pool | `/api/a-share/special-data/limit-up-pool` | `date_ms`, `page`, `size`, `sort_field`, `sort_dir` |
| Lower pool | `/api/a-share/special-data/limit-down-pool` | `date_ms`, `page`, `size`, `sort_field`, `sort_dir` |
| Broken pool | `/api/a-share/special-data/limit-break-pool` | `date_ms`, `page`, `size`, `sort_field`, `sort_dir` |
| Popularity | `/api/a-share/special-data/hot-stock-list` | `period` |
| Current auction observations | `/api/a-share/auction/snapshot` | `thscodes`, explicit `stage=live|final` |

The policy accepts JSON only, bounds each response at four MiB and uses the
composition-supplied timeout. Clones share one 500 ms request-start gate and a
request tracker. Provider business failures are not retried inside one gRPC
request: `4001` and `5xxx` remain typed failures so the caller can apply its own
bounded retry policy without hidden duplicate traffic.

## Admitted mappings

### HistoricalBars

The request must select one Shanghai, Shenzhen or Beijing six-digit equity, one
standard `.SH`/`.SZ` index, or one Shanghai/Shenzhen ETF, plus `Day`, an explicit
inclusive range and a positive limit. Equities and indices are bounded to ten
years; ETFs are bounded to five. The equity path sends `adjust=none` and
`offset=0`; index and ETF paths send no adjustment parameter. It checks the live
response context fields `thscode`, `interval` and asset-specific `adjust` state
against the request.

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

### RealtimeQuotes

The exact request contains 1..=60 unique Shanghai, Shenzhen or Beijing
A-share equities. Response count, request order, `thscode` and `ticker` must
match exactly. The adapter returns last price, previous close, open, high, low,
percentage change and turnover. Source volume is shares and is divided by 100
into Core lots. `price_change` is validated but is not projected because the
frozen quote record already carries price and percentage change.

The endpoint does not publish a name or a provider-issued timestamp for each
row. Every record therefore keeps `name=null`, `source_at=null` and
`status=Unavailable`, while `observed_at` is the local receipt time. Live
responses may carry an optional positive batch `data.timestamp`; it is retained
only as batch provenance (`unix-ms:<value>`) and must never be copied into
record evidence. A complete batch means all requested identities and numeric
fields were acquired and validated; it does not make these records eligible for
BR-033 strict source-time freshness.

### LimitPools

`Upper`, `Lower` and `Broken` use an explicit Asia/Shanghai trading date and
fixed `page=1..N`, `size=200`. Timestamp, total, page count and page size must
remain stable. Every declared page must arrive, the combined count must equal
`total`, and identities must be unique before the caller limit is applied.
Non-trading dates may return a verified empty batch. `PreviousUpper` remains
unsupported; the Provider never approximates it from another pool.

Only source fields with exact Core meaning are projected. For example,
`max_seal_money`, source flags and broken-pool turnover amount are validated but
not placed into unrelated Core fields. A live `open_times=null` is preserved as
an absent optional `break_count`; it is never changed to zero.

### Popularity

The Provider fixes `period=day`, meaning the official 24-hour hot-stock list.
It validates positive unique ranks, unique A-share identities, names, heat,
rank change and response timestamp before applying a maximum caller limit of
100. The documentation describes `heat` as a number; live responses on
2026-08-22 returned a numeric string. The decoder therefore accepts only a JSON
number or a trimmed, bounded, finite numeric string and rejects all other
coercions.

### FinancialStatements

The exact request contains 1..=8 unique A-share equities and one statement kind.
For each instrument the Provider requests `period=quarterly&limit=20` from the
matching income, balance or cash-flow path. All documented numeric lines are
retained as stable source keys. Explicit `null` remains a line with no value;
amounts are not zero-filled.

Every `period_end_ms` must be a unique Asia/Shanghai report-period date and its
year must match `fiscal_year`. Every `report_date_ms` must not predate the report
period. The live API proves that `data.timestamp` identifies the latest
`period_end_ms`, not response assembly or publication time. Each record therefore
keeps its own raw `report_date_ms` as `source_at=unix-ms:<value>`; batch
`source_at` is the latest record publication time.

### CorporateActions

The exact request selects one A-share equity and may include both `from` and
`to`; a future `to` is rejected before network I/O. Fuyao publishes ex-date,
cash per share and bonus shares per share. A row
with at least one positive term maps to one implemented `Distribution` event;
negative terms, all-zero rows, duplicate ex-dates, identity conflicts and rows
outside coverage reject the whole response. The endpoint publishes no data
timestamp. Batch and record `source_at` therefore remain absent; only the local
observation clock is stored in `observed_at`.

### SecurityMetadata

The exact request contains 1..=32 unique A-share equities, standard exchange
indices or exchange-traded funds. Each identity is queried independently with
an exact `q=<thscode>`, exchange and leaf asset-type filter. Exactly one matching
`thscode`, exchange and asset type must return. Name and currency are validated.

Fuyao does not publish board, ST state, listing date or price-limit rules on this
endpoint. Those fields remain absent and the Core record is explicitly
`Unavailable`; the batch can still be complete because every requested identity
was resolved. Live `000300.SH` returns provider-native `ticker=1B0300`; exact
`thscode` remains the mapped identity and the auxiliary ticker is only validated.

### CurrentAuctionObservations

The admitted request uses schema
`magic.market.current_auction_observations.request` version 1:

```json
{
  "instruments": [
    {"exchange":"Shanghai","code":"600519","asset_class":"Equity"}
  ],
  "stage": "live"
}
```

The request contains 1..=100 unique A-share equities and forwards the explicit
`live` or `final` stage unchanged. The response must contain exactly one row per
requested instrument in request order with matching `thscode` and ticker. It
uses record schema `magic.market.current_auction_observation` version 1 and
preserves every documented snapshot value.

`auction_volume` is source lots and becomes `auction_volume_shares` after an
exact multiplication by 100. The source's signed `auction_unmatched` is retained
under that name as a provider-native number: the public contract gives neither
its unit nor a sign-to-bid/ask mapping, so the adapter does not relabel or split
it. A source price of zero is the valid no-trade representation and becomes
`null`; zero volume and amount remain numeric zero.

`data.timestamp` is response assembly time. It becomes only the record
`evidence.observed_at` and batch `observed_at`; both source times remain absent.
The operation has no `trading_date`. It is therefore complete for this narrow
observation contract but is not eligible for exact-date storage, BR-033 source
freshness or complete Level-2 auction decisions.

## Legacy complete-Auctions diagnostic

The Provider also projects the exact official `/api/a-share/auction/snapshot`
response into the old complete-auction shape for an opt-in diagnostic only. It
uses 1..=100 unique A-share equities with fixed `stage=final` and requires
the observed `auction_phase=closed` and `data_status=final`, exact response count,
request order and `thscode`/`ticker` identity. All documented numeric fields are
decoded and validated before any record is returned. `auction_volume` is source
lots and is multiplied by 100 for the existing diagnostic record's share unit;
`auction_amount`, price, previous close, change percentage and volume ratio keep
their documented meanings; volume ratio is a decimal multiplier, not a percent.
Live responses prove that the single
`auction_unmatched` value may be signed. It is validated as finite but is never
guessed into either directional queue because the source contract does not
define a bid/ask mapping for the sign.

This path is deliberately repository-unadmitted and registered only as an
opt-in diagnostic. `data.timestamp` is the provider's response assembly time, so
it becomes `observed_at` only. The response contains no exact trading date or
record source time; batch and record `source_at` remain absent. The request uses
provider-specific schema `magic.market.hithink_current_auctions.request` with
`{"instruments":[...]}` rather than the exact-date
`magic.market.auctions.request`. Its record schema is
`magic.market.hithink_current_auction_snapshot`, keeping this partial current
shape distinct from the Eastmoney exact-date diagnostic. This prevents a current
snapshot from being misrepresented as evidence for a caller-selected trading
date.

Outside the provider's completed snapshot window, an exact
`auction_phase=closed` plus `data_status=not_ready` response is returned as the
typed, retryable `provider_unavailable` terminal with zero records. It is not
misclassified as a malformed provider response and its partial item is never
promoted into auction evidence.

The official API also has `/api/a-share/auction/short-term-benchmark`, whose
response contains the resolved query `date`, and
`/api/a-share/calendar/trading-days`. They are separate contracts: neither
response identifies the auction snapshot batch or binds a date to any snapshot
record. The benchmark explicitly keeps a caller-selected non-trading date rather
than rolling it back. These paths therefore cannot be joined to manufacture the
snapshot's missing `trading_date` or `source_at`, and are not registered as
auction transport dependencies.

## Official API coverage boundary

The inspected official capability map contains 59 endpoints across A-share,
index/board, fund, auction, special-data and market-dump families. Endpoint
availability alone does not authorize a lossy mapping into an existing gRPC
record. The Provider currently exposes fifteen exact paths above. They back
nine evidence-preserving admitted Core/RPC families, including observation-time
realtime quotes and the narrow current-observation operation. The lossy
projection into the separate complete
`Auctions` shape remains an explicit diagnostic and does not pass admission.

Examples that remain outside production mapping include board directories
without the Core-required `member_count`, constituent rows without atomic board
name/category evidence, fund
articles without a generally provable caller cutoff traversal, and market dumps
that belong to a separate file/storage workflow rather than one unary market
query. No other Provider fills these gaps inside a `HithinkFinance` batch.

## Deliberately unadmitted families

- Auction snapshots expose one undirected unmatched quantity and a response
  assembly timestamp, not an exact trading date, the two directional queues or
  record source time required by the complete Core auction contract. The safe
  subset is admitted only through `CurrentAuctionObservations`; complete
  `Auctions` remains unadmitted.
- Board directory and constituent responses do not atomically provide every
  field required by the existing board contracts.
- The official capability map does not expose minute K, ticks or Level-2, and
  its fund/special-data-specific records are not relabeled as unrelated existing
  Core operations. Other Providers are not used to fill those gaps inside a
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
- ten-row 24-hour popularity response;
- four-row standard-index and ETF unadjusted daily bars;
- twenty quarterly records for each of income, balance and cash-flow statements;
- three cash/bonus adjustment events in an explicit range;
- exact equity, standard-index and ETF metadata identities.

Two additional bounded raw live auction calls returned one exact `600519.SH`
row with `auction_phase=closed`, `data_status=final` and a positive response
assembly timestamp. Those calls prove the diagnostic wire shape and identity,
not a trading date or source time, and therefore do not promote auction
admission for the complete `Auctions` contract.

A separate bounded check on Saturday 2026-08-22 made the missing linkage
observable: the snapshot still returned one `600519.SH` row as `closed/final`,
while the short-term benchmark resolved `date=2026-08-22` and returned an empty
item list. The date-bearing endpoint therefore cannot prove that the snapshot
belongs to that date or to the previous trading day.

A separate three-call serial probe covered every newly admitted endpoint and
completed 30 request starts, zero active requests at exit, maximum concurrency
one and a measured minimum request-start gap above 500 ms. Deterministic tests
cover key
redaction, distinct bar times, shares-to-lots conversion, response-context
conflicts, valuation null/negative semantics, whole-batch identity rejection,
pagination contradictions, Beijing identities, numeric-string heat and safe
typed business failures. They also cover per-report publication evidence,
financial identity conflicts, cash/bonus event terms and coverage, field-level
metadata absence, the provider-native index ticker and null broken-pool
`open_times`. Auction tests cover explicit live/final forwarding, exact
state/identity/cardinality checks, no-trade zero-price handling,
lots-to-shares conversion, signed unmatched-value preservation,
observation-only timestamp semantics, directional queue absence, preflight
rejection and whole-batch rejection of malformed, unknown or conflicting
fields.

On 2026-09-09, the current-observation path was reviewed against official
HITHINK commit `44b7aa34dd504675f3ddaa15b3d478ea16f97884`. Two bounded live endpoint
reads and three serial reads returned the exact requested identity with a
positive response assembly timestamp and no synthesized source time. This
admits the truthful current-observation shape only; it does not change the
complete `AUCTIONS_ADMITTED=false` decision.

On 2026-09-15, two bounded live quote reads followed by three serial reads of
`600396.SH,000001.SZ` returned exactly two rows in request order. All five reads
returned changing prices/volumes, stable exact field names and a positive
optional batch timestamp; no row contained a name or its own source timestamp.
The serial starts remained naturally separated by the live request duration,
and the production client additionally enforces its clone-shared 500 ms start
gate. Deterministic tests cover exact URL construction, cardinality/order,
shares-to-lots conversion, numeric projection, Provider identity, explicit
`DataStatus::Unavailable`, record `source_at=null`, and batch-only timestamp
provenance.
