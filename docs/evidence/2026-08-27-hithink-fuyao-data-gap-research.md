# HITHINK Fuyao data-gap research — 2026-08-27

Status: first-party contract review completed; no production admission is
changed by this note.

## Scope and source pin

This review answers whether the official HITHINK Fuyao Financial API already
provides a date/security-addressable industrial-chain, industry-chain,
concept-relation, or `chain_daily` interface, and which official but currently
unwired endpoints could close known data gaps.

The upstream source is pinned to official `HiThink-Tech/Financial-API` commit
[`9dbef74d2ce535857e610eec265bcb9302942d48`](https://github.com/HiThink-Tech/Financial-API/tree/9dbef74d2ce535857e610eec265bcb9302942d48)
(2026-08-17). This is the same snapshot named by the repository's
[HITHINK integration contract](../integrations/hithink-fuyao.md). The official
repository declares `docs/api/` to be its sole REST contract source and the
capability map to be exhaustive at 59 endpoints; the public documentation is a
generated presentation of that contract. See the upstream
[REST contract and common protocol](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/README.md)
and
[59-endpoint capability map](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/capability-map.md).

All public data endpoints use `GET https://fuyao.aicubes.cn/...`, authenticate
with `X-api-key`, and return the envelope
`{code, message, request_id, data}`. Success requires HTTP 200, `code=0`, and
non-null `data`; a business error with `data=null` is not verified-empty. The
upstream `request_id` is suitable as a provider batch/correlation identity, but
the API key is runtime-only and must never enter evidence or logs. Endpoint
`data.timestamp` fields are milliseconds, but their documented meaning must be
checked per endpoint before they can become `source_at`; local observation time
cannot replace an absent provider time. These rules come from the official
[common protocol and error table](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/README.md#L5-L65).

## Verdict on industrial-chain and `chain_daily`

There is **no direct official endpoint** for an industrial/supply-chain graph,
upstream/downstream relationships, an industry-chain hierarchy, historical
board membership, reverse stock-to-all-board membership, or a dataset named
`chain_daily`. The exhaustive capability map and a source-tree search at the
pinned commit contain no such contract.

The nearest official interfaces are four index/board endpoints:

| Endpoint | Request and paging | Provider time and identity | What it can prove | Why it is not `chain_daily` |
| --- | --- | --- | --- | --- |
| `/api/a-share-index/catalog/ths-index-list` | One optional `tag`: `cn_concept`, `region`, `tszs`, or `industry`; one category returns in full with no paging. | Envelope `request_id`; `data.timestamp`; each item has only board `thscode` and `name`. | The current provider-native directory for one board category. | It is a classification directory, not a supply-chain graph. It has no hierarchy, relationship direction, effective date, or member count. |
| `/api/a-share-index/constituents/ths-stock-list` | Exactly one board/index `thscode`; no paging. | Envelope `request_id`; `data.timestamp`; each row has constituent `thscode`, `ticker`, and stock `name`. | Current constituents of one requested board. | It explicitly provides current membership only and no historical additions/removals. The response does not echo board name/category, so it cannot alone prove the complete local `BoardMembership` identity. |
| `/api/a-share-index/prices/historical` | Exactly one `.TI`, `.SH`, or `.SZ` index/board, `interval=1d`, explicit millisecond `start`/`end`, at most ten years; no paging. | Envelope `request_id`; response context and latest-bar `timestamp`; each bar has `date_ms` and OHLCV/turnover fields. | Daily market series for a known board identity. | Price history is not membership or industrial-chain history. It cannot show which stocks belonged to a board on a past date. |
| `/api/a-share-index/prices/snapshot` | Explicit comma-separated board/index `thscodes`; signature-compatible `limit`/`offset` have no effect. | Envelope `request_id`; item identity follows the price-snapshot shape. For explicit-symbol snapshots that shared shape documents `data.timestamp=null`. | Current board price fields when present. | It has neither membership relations nor a usable provider source time for strict record evidence. |

The exact board contracts and their field lists are first-party documented in
[endpoints-index.md](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/endpoints-index.md).
The detail page fixes index history to `interval=1d`; the shorter capability
map's wording “day/week/month” is therefore inconsistent with the authoritative
endpoint detail and must not be used to enable weekly or monthly requests.

### Repository admission impact

The current Rust Provider's closed allowlist contains fourteen paths and does
not include the board catalog, constituents, snapshot, trading calendar, or
market-dump endpoints; see the
[current exact-path allowlist](../../crates/magic-hithink-rs/src/lib.rs#L26-L57).
The current gRPC operation set has `BoardDirectory`, `BoardConstituents`, and
`BoardMemberships`, but no `TradingCalendar`, `IndustrialChain`, or
`ChainDaily` operation; see the
[gRPC operation mapping](../../crates/magic-market-grpc-server/src/app.rs#L367-L430).

The existing board contracts also prevent a silent mapping:

- `BoardDefinition` requires a positive, provider-proved `member_count`, while
  the Fuyao directory returns only board code and name. Calling every
  constituent endpoint merely to manufacture counts would be an N+1
  multi-response composition, not one atomic directory snapshot. See the
  [Core board definition](../../crates/magic-market-core/src/discovery.rs#L9-L55).
- `BoardMembership` requires board code, board name, category, instrument, and
  one evidence object. The constituents response supplies the instrument rows
  but not the board name/category in its response. Joining a separately timed
  catalog response is admissible only under a new explicit multi-source
  composition contract that proves identical snapshot timestamps and rejects
  any conflict; the current unary contract does not do that. See the
  [Core membership shape](../../crates/magic-market-core/src/signals.rs#L17-L24).
- Reverse membership has no upstream reverse endpoint. Enumerating every board
  and every constituent list would be rate-heavy, non-atomic, and unable to
  prove that all calls describe one snapshot. It must not be exposed as a
  complete `BoardMemberships` result.
- `.TI` is a provider-native board identity not represented by the current
  exchange-backed `InstrumentId`. Board daily bars therefore need either a
  provider-scoped board identifier or a new board-bar contract; relabeling a
  `.TI` board as a Shanghai/Shenzhen security would violate identity.

Consequently, the correct near-term product is a versioned **current board
snapshot** contract (directory plus constituents, preserving `.TI`, category,
`request_id`, and raw `timestamp`) and a separate **board daily bars** contract.
Neither should be named or stored as industrial-chain/`chain_daily` data. A
real industrial-chain feature still needs a first-party source that publishes
node type, directed edges, effective dates, and record-level evidence.

## Official endpoints that can close current gaps

The following paths exist upstream but are absent from the current fourteen
path allowlist. “Candidate” means the source fields are promising enough for a
new Gate A–D implementation and live evidence; it is not production admission.

| Priority | Official endpoint | Exact contract | Gap it can address | Evidence/admission assessment |
| --- | --- | --- | --- | --- |
| P0 candidate | `/api/dump/market-dumps/daily-k/download-url` | No query parameters. Returns a roughly ten-year, whole-market unadjusted daily-K Parquet download URL. The URL expires in about five minutes. The file rows contain `thscode`, `currency`, `interval=1d`, `adjusted=none`, `date_ms`, OHLC, share volume, and turnover. | Rebuild an incomplete or corrupted `stock_daily` store without thousands of per-symbol REST calls. | This is a two-hop file workflow, not a unary `HistoricalBars` call. Admission needs a separately registered S3/download transport, bounded file size, schema and row validation, a captured content digest/ETag, envelope `request_id`, local `observed_at`, rejection of an unsettled current-day bar, and transactional load. The signed URL or its expiry is not `source_at`. |
| P0 candidate | `/api/dump/market-dumps/daily-k-10d/download-url` | Same Parquet schema, most recent ten trading days, no query parameters. Rows can overlap the local database. | Daily incremental `stock_daily` refresh and repair of a short gap. | UPSERT/deduplicate by `(thscode, date_ms)` exactly as the upstream contract requires. If the local gap exceeds the covered trading days, fail closed and use the full dump. The same two-hop evidence requirements apply. |
| P1 candidate | `/api/a-share/calendar/trading-days` | No parameters or paging; fixed `[Shanghai today - 1 year, today]` window. `data.timestamp`; ascending rows with `date_ms` at Shanghai midnight and `date` in `yyyyMMdd`. | Authoritative recent A-share trading-day input for schedulers, cutoff checks, and settlement gates. | Envelope `request_id`, `data.timestamp`, and each `date_ms/date` pair provide a strong batch. The repository needs a new `TradingCalendar` Core/gRPC contract; it must not be mapped to the economic-release `EconomicCalendar` operation. It cannot answer dates older than one year. |
| P1 candidate | `/api/a-share-index/catalog/ths-index-list` + `/api/a-share-index/constituents/ths-stock-list` | Current category directory, then exactly one current constituent list per `.TI`/standard index; neither endpoint pages. | Stabilize current concept/industry board discovery and constituent reads. | Useful only under a versioned current-snapshot contract with provider-scoped identity. It cannot provide historical membership, reverse-complete membership, or Core `member_count` atomically. |
| P1 candidate | `/api/a-share-index/prices/historical` | One board/index; exact `start`/`end`; `1d`; ten-year maximum; no paging. | Board/industry/concept daily market series after a board has been resolved. | Has dated bars and provider response identity suitable for record evidence, but needs a `.TI`-aware board-bar schema. It is not stock `chain_daily` and cannot backfill membership history. |
| P1 candidate | `/api/a-share/special-data/dragon-tiger-list` | `board_type=all|org|hot_money`; optional explicit `date=YYYY-MM-DD` within one year; no paging. Response has `timestamp`, echoed `board_type`, explicit `trade_date`, counts, stock identities, buy/sell/net values, reasons, concepts, and optional hot-money rows. | Add an independent provider route for `MarketDragonTiger`/selected `DragonTiger` facts when its field subset matches Core. | This is the strongest unwired same-shape candidate because the response carries an explicit trading date and identities. Deterministic mapping must validate counts, `buy-sell=net`, units, duplicates, and `trade_date`; it must not invent seat detail absent from the endpoint. |
| P2 candidate | `/api/meta/tickers/list` | Filters `exchange` and `asset_type`; `limit<=10000`, `offset>=0`; stop on empty/short page. `data.timestamp`; each row has `thscode`, ticker, name, exchange, and asset type. | Full A-share/board universe synchronization and identity validation, including Beijing names/codes. | Multi-page admission must require stable `timestamp`, no duplicate/contradictory identity, bounded pages, and complete short-page termination. It complements the existing exact-search metadata route; it does not provide price freshness. |
| P2 diagnostic | `/api/a-share/special-data/anomaly-analysis-list` and `.../anomaly-analysis-stock` | Current-day only. List can filter closed `tag_codes`; stock form accepts 1–50 explicit A-share codes. Response has `timestamp`, identity, analysis text, keywords, and tag name. No pagination is documented. | Explain current candidate anomalies/limit moves. | The response has no explicit trading date and no historical query, while local `StrongStockReason` requires a trading date. It cannot be production-mapped until the upstream response binds a date or a new current-only schema keeps the absence explicit. Free text remains bounded untrusted source data. |
| P2 diagnostic | `/api/a-share/special-data/limit-up-ladder` | No parameters; fixed recent 30-trading-day matrix, with date, tier buckets and at most four stocks per tier. | A compact recent consecutive-limit overview. | It is intentionally truncated and cannot prove a complete daily limit pool or complete review universe. Use only in a new diagnostic/summary schema; never infer missing stocks. |
| Separate operation only | `/api/a-share/auction/short-term-benchmark` | Optional `date=YYYY-MM-DD`; response echoes `date`, `date_ms`, identities, `auction_pct`, and tags; non-trading dates do not roll back. | Date-bound auction-strength ranking. | It can support a new benchmark operation, but it does not bind its date to `/auction/snapshot` and has none of the complete price/volume/directional unmatched fields. It therefore cannot promote the existing HITHINK `Auctions` route. |

First-party field and paging sources:

- market dumps:
  [endpoints-market-dumps.md](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/endpoints-market-dumps.md);
- trading calendar:
  [endpoints-calendar.md](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/endpoints-calendar.md);
- symbol list:
  [endpoints-meta.md](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/endpoints-meta.md);
- dragon-tiger, anomaly analysis, and limit-up ladder:
  [endpoints-special-data.md](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/endpoints-special-data.md);
- auction benchmark:
  [endpoints-auction.md](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/endpoints-auction.md).

## Gaps this API cannot truthfully fill

The pinned 59-endpoint map has no equivalent for the following current gaps:

| Gap | First-party finding | Required action |
| --- | --- | --- |
| Stock or board `MoneyFlow` | No A-share/board money-flow or order-flow endpoint exists. Dragon-tiger buy/sell/net is a special-list statistic, not continuous money flow. | Keep the provider unavailable for this operation; use a separately admitted provider without relabeling. |
| `T0Evidence` | The official capability boundary explicitly excludes minute K, tick, and Level-2. | Continue with TDX/another exact intraday source. Daily Fuyao bars cannot substitute. |
| Equity `InstrumentNews` | The only news endpoint is `/api/fund/news/article-list`, requiring `fund_type` and a fund `thscode`; it returns fund article metadata with opaque cursor pagination. It is not A-share instrument news. | Do not map fund articles to equity news. Keep the existing equity-news provider path or add another first-party equity source. See the official [fund-news contract](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/endpoints-fund.md#L481-L500). |
| Analyst `Consensus` | Financial statements and `/financials/indicators` are reported fundamentals, not analyst estimates, target prices, contributor counts, or forecast periods. | Do not use actual financials to fill consensus. A dedicated estimates source is required. |
| Exact-source realtime Quote | Explicit-symbol A-share snapshots document `data.timestamp=null`; a batch observation clock or paged-mode latest timestamp cannot be copied into each record. | Keep strict quote admission blocked unless the upstream publishes per-record source time or the client uses another admitted provider. See the official [price snapshot contract](https://github.com/HiThink-Tech/Financial-API/blob/9dbef74d2ce535857e610eec265bcb9302942d48/docs/api/endpoints-prices.md#L5-L70). |
| Account/FIFO/T+1 ledger | The API is market/reference data only and has no account, position, order, or execution ledger endpoint. | Fix in the account/strategy data owner; no HITHINK market-data compensation is truthful. |
| Historical industrial-chain graph | No direct graph, edge, hierarchy, effective-date, or historical board-membership contract exists. `concept_list` inside dragon-tiger rows is only a label subset for those returned records. | Obtain a first-party industrial-chain dataset or define a clearly named current board-classification snapshot; never infer historical chains from current membership. |

## Recommended implementation order

1. Integrate the full and ten-day market dumps as a separately admitted,
   transactional ingestion workflow to repair and maintain `stock_daily`.
2. Add a versioned recent A-share trading-calendar operation backed by
   `/api/a-share/calendar/trading-days`.
3. Add provider-scoped current board snapshot and board-daily-bar contracts;
   keep `.TI` identity and the absence of historical membership explicit.
4. Evaluate `dragon-tiger-list` as an additional dated provider route with
   focused deterministic and live evidence.
5. Keep anomaly analysis, limit-up ladder, and auction benchmark in distinct
   current/summary schemas until their semantics match an operation exactly.

No path should be added to the transport allowlist or admissions registry on
the strength of documentation alone. Each selected candidate still requires an
approved transport design, deterministic fixtures, whole-response identity and
time validation, bounded live/load probes, and an explicit registry update.
