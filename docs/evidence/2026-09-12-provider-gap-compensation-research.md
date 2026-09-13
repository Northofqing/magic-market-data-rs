# Provider-gap compensation research — 2026-09-12

Status: first-party contract review completed; no production admission or
transport policy is changed by this note.

## Question and decision rule

This review covers the repository-unadmitted routes listed in
[`unadmitted-provider-routes.md`](../integrations/unadmitted-provider-routes.md),
plus TDX local `source_record_count` and CFETS DR007. An alternative is called
**equivalent** only when it can satisfy the existing operation's complete field,
identity, time, cardinality and provenance contract. A source that covers only
part of the business need is **supplemental** and needs its own operation and
Provider identity. It must not fill fields in, or impersonate, the blocked
Provider.

## Executive verdict

| Blocked route or field | Current truthful compensation | Classification | Recommended action |
| --- | --- | --- | --- |
| `HithinkFinance / Auctions` | Admitted `CurrentAuctionObservations / HithinkFinance` for the current undirected snapshot; admitted narrow exact-date `Auctions / EastmoneyMiaoxiang` when its key is available | Existing partial alternatives, not complete Level-2 equivalence | Keep complete HITHINK `Auctions` blocked. Procure an exchange or licensed Level-2 feed if directional unmatched bid/ask and provider event time are required. |
| `Jin10 / EconomicCalendar` | `EconomicReleaseObservations / Jin10` for already-published rows; FRED and official statistical/central-bank release schedules for future dates | Supplemental only | Add a new provider-qualified official-release-calendar contract; do not infer a calendar from news publication times. |
| `Imf / EconomicSeries` | IMF DataMapper or IMF SDMX after renewed contract/transport audit | Same-source equivalent candidate | P0: diagnose the current Rust-versus-raw HTTP discrepancy and rerun Gate A–D evidence. FRED/World Bank remain different datasets. |
| `EastmoneyMiaoxiang / FundFlowSeries` and `/ MoneyFlows` | Admitted public Eastmoney fund-flow routes | Equivalent at operation level; different acquisition interface | Use the existing production route. Keep Miaoxiang only as diagnostic until its own cardinality and load stability pass. |
| `Baidu / HistoricalBars` | Admitted HITHINK, TDX, Tencent, Sina, and entitled EmQuant historical-bar routes; Baidu remains admitted as `TechnicalBars` | Equivalent alternatives for their declared scopes | Do not widen Baidu's semantics. Select an admitted Provider explicitly. |
| `EmQuant / RealtimeQuotes` and `/ OrderBooks` | Tencent, TDX, Sina and SZSE production routes | Equivalent at operation level; not EmQuant identity | Use existing routes, or obtain the missing Choice/EmQuant entitlement and rerun provider-specific probes. |
| `EmQuant / MoneyFlows` | Admitted public Eastmoney `MoneyFlows` | Equivalent at operation level; not EmQuant identity | Use Eastmoney now. Treat TDX money-flow fields only as a new provider candidate after entitlement, time and unit evidence. |
| `SecuritiesTimes / GlobalNews` | Cailianpress, Eastmoney, Jin10, ThePaper, WallstreetCn, XinhuaFinance, Yicai and Yonhap | Equivalent operation, different editorial coverage | Keep the STCN route blocked pending first-party attribution re-audit or a licensed feed. |
| TDX local `source_record_count` | Local stream cursor/gap telemetry; official transaction rows in a separately queried tick operation | Observability supplement only | Keep `source_record_count=None`. For source-level loss proof, evaluate a licensed exchange feed with provider sequence numbers. |
| CFETS DR007 | Direct CFETS information-product/interface entitlement, or a separately contracted CFETS-authorized information vendor | Authorization-only equivalent | No public-web substitution. Obtain written rights, schema and entitlement before Gate A. |

## Findings

### 1. HITHINK auction data cannot be upgraded by joining its other endpoints

The official HITHINK contract at source commit
[`402574a`](https://github.com/HiThink-Tech/Financial-API/tree/402574a6221d5e255dba47166df8e5abb7149938)
states that `/api/a-share/auction/snapshot` returns one `auction_unmatched`
number and that `data.timestamp` is response assembly time. The rows contain no
`trading_date` and do not split unmatched quantity into bid and ask. The
separate benchmark has `date/date_ms`, but that date is not bound to the
snapshot batch. See the official
[auction endpoint contract](https://github.com/HiThink-Tech/Financial-API/blob/402574a6221d5e255dba47166df8e5abb7149938/docs/api/endpoints-auction.md)
and the repository's
[HITHINK admission contract](../integrations/hithink-fuyao.md).

Consequently, the existing `CurrentAuctionObservations` operation is the
maximum truthful projection of that endpoint. A complete replacement needs a
licensed market-data contract carrying provider event time, exact session/date,
matched facts and directional unmatched quantities. Exchange Level-2 is a
candidate rather than a free REST fallback: the SSE market-data specification
defines virtual matched and unmatched auction fields in its
[official interface specification](https://big5.sse.com.cn/site/cht/www.sse.com.cn/services/tradingtech/data/c/10813252/files/454eeb0a88c843cba9410db2bfd6b0c0.pdf),
while SZSE publishes its interfaces through the
[official technical-service catalog](https://www.szse.cn/marketServices/technicalservice/interface/index_1.html).
Either route requires its own licensing, schema and live admission evidence.

### 2. A useful economic calendar is possible, but not as Jin10 identity

The existing Jin10 public window proves only already-published structured
releases; it cannot prove all events for a future day or country. This is the
documented boundary of
[`EconomicReleaseObservations`](../integrations/jin10-web.md). The official
[Jin10 MCP catalog](https://mcp.jin10.com/app/doc.html) does not document a
calendar tool, and the former free calendar/API embedding service states that
it ended on 2025-12-01 on Jin10's
[official embedding page](https://www.jin10.com/example/websiteiframe.html).

There are authoritative supplements:

- FRED publishes API methods for all release dates and per-release dates in its
  [official Releases API](https://fred.stlouisfed.org/docs/api/fred/).
- China's NBS publishes its
  [official release calendar](https://www.stats.gov.cn/sj/fbrc/bnxxfb/).
- BLS publishes an official
  [release schedule](https://www.bls.gov/schedule/2026/) and
  [ICS feed](https://www.bls.gov/schedule/news_release/bls.ics); BEA publishes
  an official [JSON/ICS calendar](https://www.bea.gov/news/schedule/icalendar).
- The Federal Reserve and ECB publish official
  [FOMC](https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm) and
  [Governing Council](https://www.ecb.europa.eu/press/calendars/mgcgc/html/index.en.html)
  schedules.

No one of these sources supplies the complete Jin10 field set of country,
importance, prior/consensus/actual values and precise release time for all
regions. The safe design is a versioned `OfficialReleaseCalendar` composition
whose records retain the issuing institution as Provider and whose coverage is
explicit. News publication time remains release observation evidence, never a
future schedule.

### 3. IMF should be re-audited before seeking a replacement provider

IMF's current official API page says its data are available through SDMX 2.1
and 3.0 and that Swagger exploration requires a beta portal account; it directs
API questions to `datahelp@imf.org`. See the
[IMF Data API page](https://data.imf.org/en/Resource-Pages/IMF-API).

This review also made a bounded current check. On 2026-09-12, ordinary `curl`
requests to the official DataMapper
[`v2/indicators`](https://www.imf.org/external/datamapper/api/v2/indicators)
and
[`v2/NGDP_RPCH/USA/CHN`](https://www.imf.org/external/datamapper/api/v2/NGDP_RPCH/USA/CHN)
resources returned HTTP 200 JSON. A follow-up single-variable check found the
specific discrepancy: the Akamai edge returned HTTP 200 for curl's native
`curl/8.15.0` User-Agent, but HTTP 403 for an absent User-Agent,
`reqwest/0.12`, `magic-imf-rs/0.2`, and a simplified browser User-Agent. The
repository's release-mode `magic-imf-rs` probe therefore still returns
`Transport(HttpStatus { status: 403 })`. Production must not claim recovery by
making reqwest impersonate curl or a browser. The current DataMapper route
remains diagnostic while an official SDMX contract or an honestly identified
client path is obtained and re-admitted. The repository contract is documented in
[`imf-datamapper.md`](../integrations/imf-datamapper.md).

FRED and World Bank are already admitted for narrowly audited series; their
official APIs are valid intentional alternatives, not IMF substitutions. FRED
documents series and observation access in its
[official API](https://fred.stlouisfed.org/docs/api/fred/), while the World Bank
states that its V2 Indicators API exposes nearly 16,000 time series without an
API key in its
[official documentation](https://datahelpdesk.worldbank.org/knowledgebase/articles/889392).

### 4. Fund flow, historical bars, quote, order book and general news already have routes

These are redundancy gaps rather than operation gaps:

- Public Eastmoney `FundFlowSeries` and `MoneyFlows` are production-admitted
  with record-level source dates/times and all five buckets. The authenticated
  Miaoxiang natural-language route cannot improve that evidence until its own
  output cardinality and serial stability are proved. See
  [`eastmoney-web.md`](../integrations/eastmoney-web.md) and
  [`eastmoney-miaoxiang.md`](../integrations/eastmoney-miaoxiang.md).
- Baidu's source-provided OHLCV/amount plus MA values correctly remain the
  separate `TechnicalBars` contract. The official HITHINK
  [`/api/a-share/prices/historical`](https://github.com/HiThink-Tech/Financial-API/blob/402574a6221d5e255dba47166df8e5abb7149938/docs/api/endpoints-prices.md)
  supports one A-share, explicit start/end, daily bars and
  `none/forward/backward` adjustment, so the existing HITHINK production route
  is a strong daily-bar alternative. TDX also documents its own
  [`get_market_data`](https://help.tdx.com.cn/quant/docs/markdown/mindoc-1ctuhthaq5qmg/mindoc-1h10g60jt68sc.html)
  history contract.
- EmQuant quote, five-level book and money-flow adapters are implemented, but
  the current account's entitlement or field completeness is the blocker, as
  recorded in [`eastmoney-emquant.md`](../integrations/eastmoney-emquant.md).
  Existing production Providers should be selected instead of silently
  relabeling their records as EmQuant. If Choice/EmQuant identity is required,
  entitlement is the only truthful fix.
- Securities Times has no documented public API in the reviewed official site.
  Its official [quick-news page](https://www.stcn.com/article/list.html?type=kx)
  is a publication page, not a machine-data contract. The current first-page
  adapter remains blocked because live source attribution was blank or unsafe;
  see [`securities-times.md`](../integrations/securities-times.md). Other admitted
  news Providers preserve availability but not Securities Times editorial
  coverage or identity.

TDX exposes additional licensed/version-dependent money-flow formula fields and
live quote/order-book data in its
[official function list](https://help.tdx.com.cn/gspt/docs/markdown/redword/functionlist.html)
and
[official quote subscription contract](https://help.tdx.com.cn/quant/docs/markdown/mindoc-1hjbgqpdhv114/mindoc-1hjbidnohpjn4.html).
Those interfaces are candidates for a new TDX route only after field units,
entitlement, Beijing coverage, source time and bounded load evidence are proved;
they are not Miaoxiang or EmQuant records.

### 5. TDX source count and CFETS DR007 have no free equivalent

The TDX local snapshot response contains neither a source record count nor a
source timestamp; `ItemNum`, response array length, poll count and local bridge
sequence are different facts. This is captured in
[`tdx-local-terminal.md`](../integrations/tdx-local-terminal.md). TDX's official
[`get_tick_data`](https://help.tdx.com.cn/quant/docs/markdown/mindoc-1hho7blr2j340/mindoc-1hi3n0rqkakt4/mindoc-1hhob7ou94li8.html)
returns bounded transaction `records`, but their count describes a separate
caller-selected tick page, not the provenance cardinality of a quote snapshot.
A separate adapter-observed stream cursor is useful telemetry, but it must not
populate `source_record_count`.

CFETS explicitly offers market and benchmark data through CMDS/information
product interfaces with application, testing and production-enablement steps;
see its official
[data-interface service](https://www.chinamoney.com.cn/chinese/dataInterfaceService/).
It also states that users must obtain data through CFETS or an authorized
information provider and that copying, storing, processing or redistribution
requires permission; see the official
[information-product terms](https://www.chinamoney.com.cn/chinese/xxcpjjjsq/).
The official
[authorized-provider page](https://www.chinamoney.com.cn/chinese/xxssjfw/)
lists vendors including HITHINK and TongdaXin for relevant market/benchmark
products, but that does not prove that their current public APIs expose DR007
or authorize this repository's use. A vendor feed would require its own
Provider identity or an explicit licensed lineage model. DR007 must not be
replaced by R007, FDR007, FR007 or Shibor.

## Recommended order

1. **P0 — IMF re-audit:** the raw official DataMapper resources now answer 200
   while the Rust probe receives 403. Diagnose that exact request boundary and
   rerun Gate A–D; this is the lowest-cost same-source recovery candidate.
2. **P0 — official release calendar design:** add source-specific NBS, FRED,
   BLS/BEA and central-bank schedule records under a new contract. Keep Jin10
   observations separate.
3. **P1 — route existing production alternatives:** make clients omit blocked
   `preferred_provider` values for fund flow, bars, quote, book and general news;
   retain the selected Provider in every record.
4. **P1 — entitlement review:** if EmQuant-specific quote/book or a second
   money-flow source is operationally necessary, procure the exact Choice/TDX
   products first, then run field-specific probes.
5. **P2 — licensed infrastructure:** evaluate exchange Level-2/sequence feeds
   for complete auctions and loss detection, and CFETS or an authorized vendor
   for DR007. These are procurement and contract tasks before implementation.

Every new host/path or licensed native interface requires an approved Gate A
design, an exact entry in
[`http-transports.tsv`](../integrations/http-transports.tsv) when HTTP is used,
deterministic fixtures, typed failures, bounded live and serial-load probes, and
an admission-registry update. Blocking Provider I/O must follow
[`async-blocking.md`](../integrations/async-blocking.md). Documentation or endpoint
reachability alone is not admission evidence.
