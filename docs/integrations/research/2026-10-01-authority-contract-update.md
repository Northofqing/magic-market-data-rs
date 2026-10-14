# D14 / D17 / D20 authority-contract update, 2026-10-01

This is a primary-source research update, not a Provider admission or a new RPC.
The earlier D14 and D17/D20 feasibility notes remain applicable. This run adds
an actually retrieved SZSE exact-date report, dated SZSE monthly tables, and
daily-filter controls for the SSE suspension service. It does not repair the
original TDX response's missing identity echo. No Provider, proto, production
listener, or cross-host transfer was changed.

Observations below were made on this Windows VM on **2026-10-01 23:14–23:18
Asia/Shanghai** (15:14–15:18 UTC). Source dates are kept separate from these
local receipt times. Seven original response bodies were retained under
`target/authority-research-20261001-v1/`; they are local, ignored research
artifacts, not a deployed or admitted data bundle. The SHA-256 values below
are hashes of response-body bytes, not publisher-issued signatures or
publication timestamps. URI hashes use the UTF-8 bytes of the exact displayed
URI; they do not cover headers and are not upstream request IDs.

## D14: new independent SZSE exact-date positive observation

The [SZSE security chart page](https://www.szse.cn/market/trend/index.html?code=300005)
declares the historical stock catalog `1815_stock_snapshot`. Its first-party
[report script](https://res.szse.cn/modules/report/js/report_new.min.js)
uses `/api/report/ShowReport/data`; the returned catalog metadata declares
`txtDMorJC`, `txtBeginDate`, `txtEndDate`, `TABKEY`, and `PAGENO` controls.
`TABKEY=tab1` is labelled stocks. The date control publishes the client
validator `dateRange('all','${txtEndDate}','5d')`; that is evidence of the UI's
bounded query policy, not permission to remove it or evidence of a stable
external API version.

The actual read-only query was:

```text
https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1815_stock_snapshot&TABKEY=tab1&txtDMorJC=300005&txtBeginDate=2026-07-16&txtEndDate=2026-07-17&PAGENO=1
```

With the chart-page Referer, ordinary `User-Agent: Mozilla/5.0`, and
`Accept: application/json`, the VM received HTTP 200 `application/json`,
3,744 bytes, twice with the same body SHA-256
`c00db4286efd1ecd6ea2eb9862a41e00e648cb489f57c3ee995e793c6931e164`.
The URI SHA-256 is
`8440a2e004c516f11cc41ce58421eab063f6fa9ea6968515a5621e07d14ec06d`.
The retained file is `szse-300005-20260716-17.json`. The final capture was
observed at `2026-10-01T15:17:18.9406309Z`.

The response contains one stock tab. Its own metadata reports
`catalogid=1815_stock_snapshot`, `tabkey=tab1`, `pagesize=10`, `pageno=1`,
`pagecount=1`, `recordcount=2`. Its conditions' `defaultValue` fields echo
`300005`, `2026-07-16`, `2026-07-17`, and `tab1`. Both data rows independently
carry `zqdm=300005`, `zqjc=探路者`, and `jyrq`:

| Source date | `qss` previous close | `ks` open | `zg` high | `zd` low | `ss` close | `cjgs` (10,000 shares) | `cjje` (10,000 CNY) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 2026-07-17 | 18.59 | 18.45 | 19.25 | 14.87 | 14.87 | 9,676.71 | 155,297.95 |
| 2026-07-16 | 18.87 | 18.88 | 19.80 | 18.13 | 18.59 | 3,593.85 | 66,878.34 |

These values and units are published by the same response's `data` and
`metadata.cols`. Source order is descending date. The response footer states
that volume and amount aggregate all trading methods. They must not silently
replace the chart's differently scoped volume/amount. This is an independent
official-domain observation of the two dated price rows, not a conclusion
about limit validity or corporate-action absence.

For this exact request, all **source-reported report rows** fit in its one
declared page. That is a materially stronger bounded row-set observation than
latest-N bars: code, exact date filters, row dates, total and page count are
available together. It still does not publish a per-security exchange-session
coverage certificate, price-adjustment basis, immutable snapshot/revision,
publication timestamp, historical correction contract, or source assertion
that a missing report row has a specified trading-status meaning. The response
has no explicit exchange/asset-class field in each row; stock category comes
from its tab metadata and the venue from the owning SZSE endpoint. Those
bindings must be made explicit in any later reviewed contract.

### Exact-date empty control

The otherwise identical [2026-07-18 query](https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1815_stock_snapshot&TABKEY=tab1&txtDMorJC=300005&txtBeginDate=2026-07-18&txtEndDate=2026-07-18&PAGENO=1)
returned HTTP 200, 3,363 bytes, `recordcount=0`, `pagecount=0`, `pageno=1`,
and `data=[]`. The response still echoes the code and both date filters.
Body SHA-256:
`9e0bde5adaf70386aac652466b365782909ea8135fa5414246e76953231736d7`;
URI SHA-256:
`4d3be47f365e99d4f0336c2ae470abe55247cadb5dc9e60652e98204a8190cdf`.
File: `szse-300005-20260718-empty.json`, observed
`2026-10-01T15:17:19.1967820Z`.

This is a real zero-row **stock-report** result for the displayed query.
It is not a source-certified `no_suspension`, `no_action`, `unlisted`, or
`normal_trading` result. A calendar/status feed and documented report-empty
semantics would be separate prerequisites for any such classification.

### Chart control: identity is present, request-window termination is absent

The [official chart script](https://res.szse.cn/modules/marketdata/trend/js/trend.min.js)
uses `/api/market/ssjjhq/getHistoryData` and sends only `cycleType=32`,
`marketId=1`, and code for daily charts. The [actual 300005 request](https://www.szse.cn/api/market/ssjjhq/getHistoryData?cycleType=32&marketId=1&code=300005)
returned HTTP 200, 22,498 bytes, body SHA-256
`cb5c0a9da81d4b1c5d56e5d2118e116cd657c22007740fbafa1bce7543113ef3`.
File: `szse-300005-day-chart.json`, observed
`2026-10-01T15:18:14.5067188Z`. Its `data.code=300005` and name are present;
`picupdata` has 201 rows from 2025-12-04 through 2026-09-30, including the
same 07-16 and 07-17 closing prices above. The envelope has only `code`,
`data`, and `message`; data has code, name, and two chart arrays. No requested
from/to, declared historical total, exhausted cursor, or range-finality field
was present. Its 201-row coverage must not be relabelled an exact-window
terminal assertion. This source is also not TDX.

Initial requests with an unrelated SSE Referer received HTTP 403; the bounded
retry with the actual SZSE page Referer received the JSON described above.
This is reachability evidence for the page's own public requests, not an
authentication bypass, an endpoint-admission change, or a proven 403 root
cause. The earlier research statement that the chart endpoint had not been
retrieved is now superseded by this capture.

## D17: daily SSE filters and a new dated SZSE monthly table

The earlier positive 688277 suspension-window body was re-acquired from the
[SSE UI query](https://query.sse.com.cn/commonSoaQuery.do?jsonCallBack=cb&isPagination=true&sqlId=GW_PL_JYTS_TFPXX&pageHelp.pageSize=25&pageHelp.pageNo=1&keyWords=&startStopDate=20260723&endStopDate=20260723&productCode=688277)
with the [official suspension page](https://www.sse.com.cn/disclosure/dealinstruc/suspension/)
as Referer. A 07-16-only filter and a 07-23-only filter both return the same
919-byte body as the earlier 07-16…30 filter: `total=1`, `pageCount=1`,
`pageNo=1`, product code 688277, start label 20260716, end label 20260729,
type LXTP. The 07-23 raw body is retained as `sse-688277-20260723.jsonp`,
observed `2026-10-01T15:17:19.3423138Z`, SHA-256
`86e9188942eeb9ae4fe95ffb5ed235fa5f60fbab8efa41885d86828cd964490c`.
Its URI hash is
`129d1ed2b4327af58943d4da541b05f14510b7cbd307fd3d116a4673e8710824`.
This supports an overlapping, positive suspension-span observation; it does
not add every daily listed/board/risk/limit field or event finality.

The [07-30-only query](https://query.sse.com.cn/commonSoaQuery.do?jsonCallBack=cb&isPagination=true&sqlId=GW_PL_JYTS_TFPXX&pageHelp.pageSize=25&pageHelp.pageNo=1&keyWords=&startStopDate=20260730&endStopDate=20260730&productCode=688277)
returns HTTP 200, `total=0`, `pageCount=0`, `result=[]`, 485 bytes. It has
exactly the same body SHA-256 as the independently repeated 688561 07-16…30
empty query:
`553e577856c7f1806f3fc5468868347cb1bdfba132555090cf66f57a08ae1949`.
File: `sse-688277-20260730-empty.jsonp`, observed
`2026-10-01T15:17:19.4375400Z`; URI hash:
`f32a995f4700306a6404d77eb7d026f563b77f27ecdebbca897d367eb95081b5`.

**Clarification of the earlier note:** this SSE body echoes `sqlId`, not
the product-code and date filters. `securityCode` and `queryDate` are empty;
`pageHelp.startDate`, `endDate`, and `searchDate` are null. Positive row code
and dates are present, but empty-response issuer/date identity exists only
in the retained request. Thus a response hash alone cannot bind that empty
result to 688277 or 07-30. Neither the page nor the sampled response supplies
an exhaustive daily-universe/correction/finality statement. The empty result
cannot become a certified no-event day or an inferred 07-30 resumption event.

Newly retrieved [SZSE July 2026 monthly suspension table](https://docs.static.szse.cn/www/market/periodical/month/W020260806351615093691.html)
is linked by the [official July month page](https://www.szse.cn/market/periodical/month/t20260806_622028.html).
The VM received HTTP 200 `text/html; charset=GBK`, 7,493 bytes, body SHA-256
`5e1657ecd3c5c67d62e986b9bb3c8be9bd3a8b347fead27b6fc9c5b877a69275`.
File: `szse-suspension-202607.html`, observed
`2026-10-01T15:17:20.3122313Z`. Its source heading identifies July 2026;
columns identify security code/name, reason, suspension time, and resume
time. A positive example is 300567 / 精测电子, suspension label
2026/07/09 09:30 and resume label 2026/07/16 09:30. Some rows instead print
`9999/12/31 00:00` as resume time; that value must not become an actual
resumption instant. The local HTML contains 22 data rows plus a header;
this is a locally counted table, not a publisher-issued count/final manifest.

This dated public table improves positive-event auditability. It is not a
per-trading-day complete listed-security status file. It provides no
demonstrated all-day/intraday universe, revision history, per-day negative
attestation, or complete board/risk/limit state. No actual historical
`cpxx`, SZSE `securities`/`cashauctionparams`, or BSE `bj_securityinfo`/
`bj_tradingparams` payload was acquired in this run. A bounded official-site
search for July-file names again found specifications, not a downloadable
dated manifest. Search absence is not evidence that an archive does not exist.

## D20: new official 300005 dividend positive, no all-action empty contract

The [SZSE May 2026 month page](https://www.szse.cn/market/periodical/month/t20260605_620906.html)
links an actual [May dividend/bonus/rights-issue table](https://docs.static.szse.cn/www/market/periodical/month/W020260605534753848014.html).
HTTP 200 `text/html; charset=GBK`, 216,047 bytes was retained as
`szse-dividend-202605.html`, observed `2026-10-01T15:17:19.9927169Z`.
Body SHA-256:
`de031d6f782d1511ae0f7d8d9e0488d00f09f92e42d449155725415b33284f9d`.
Its source row for 300005 / 探路者 publishes the following values:

| Source field | Published value |
| --- | --- |
| Bonus shares / bonus-per-share | 0 / 0.000 |
| Cash dividend / dividend-per-share | 8,516,643 / 0.010 |
| Ex-date / registration date | 2026/05/29 / 2026/05/28 |
| Ex-price / previous closing price | 15.200 / 15.210 |
| Rights-issue fields | Blank |

Keep these as the table's published precision and blank cells. Do not
reconstruct a more precise tax/share-count basis or interpret blanks as
zero terms. This is an identity- and effective-date-bearing positive
distribution observation inside the original D14 latest-page span. It does
not assert that the observed July price move was caused by this May event.

The [July table of the same named classes](https://docs.static.szse.cn/www/market/periodical/month/W020260806351614974469.html)
also returned HTTP 200, 122,630 bytes, SHA-256
`c853ebbee874fe14b41957dabfdfd59a6c8668930bee25104e26ac81f9d2b3be`.
No 300005 row appeared among its locally counted 327 data rows. This is a
bounded observation of that monthly table, not a verified July `no_action`.
The table's named classes are dividends, bonus shares and rights issues;
it supplies neither an all-required-action-class manifest nor a per-issuer
completed-empty/correction contract. Splits, other capital changes,
implementation/proposal/cancellation states and amendments must not be
filled from this absence.

The [SZSE's own data-services description](https://investor.szse.cn/English/services/dataServices/index.html)
identifies SSIC as its authorized information distributor and advertises
corporate-action information in standard or customized formats. This is a
concrete first-party channel for requesting the missing class/coverage
contract; it does not establish entitlement, historical files, or negative
coverage on this VM. The [SSE information company's historical product](https://www.sseinfo.com/services/assortment/historical/)
advertises daily bars and security-reference files, and its
[intelligent-data service](https://www.sseinfo.com/services/assortment/znsj/)
advertises historical/static/announcement data plus a business-application
route. Those Shanghai products are not evidence for Shenzhen 300005 and no
licensed delivery was obtained here.

## Exact remaining prerequisites

| Target | Minimum new source-owned evidence before the requested claim can be delivered |
| --- | --- |
| D14 original TDX acquisition | Response-local market/code/asset/interval identity and a source-defined exact-window/terminal contract for the original TDX data, or an explicitly separate admitted acquisition. The new SZSE rows cannot retrofit an identity echo into the TDX application body. |
| Independent SZSE exact-window bars | Documented adjustment/aggregation/units, requested-window and historical-empty semantics, all declared pages with stable query/snapshot identity, correction/finality and publication/version evidence, plus the precise trading-session universe or explicit per-date gaps. Demonstrate the complete original 2026-05-22…09-28 span through permitted bounded queries before claiming its full coverage. No public-UI reachability claim substitutes for Provider admission/HTTP registry evidence. |
| D17 arbitrary security/day | Actual authorized dated reference/status deliveries for every requested venue/session, complete security universe and field definitions for listing/board/risk/limit/suspension, all applicable revisions/intraday updates, counts/checksums and completed-day finality. A source-certified no-event day requires exhaustive event coverage or complete status snapshots, not the web query's empty row set. |
| D20 all-action history / empty day | A source-owned enumeration of every in-scope action class and lifecycle state, issuer/effective-date identity, exact terms, cancellations/corrections, exact covered interval/day universe and explicit completed-empty semantics; then actual positive and certified-empty raw samples with version/count/hash evidence. A dividend-only or monthly table cannot satisfy this contract. |

The [current SZSE interface index](https://www.szse.cn/marketServices/technicalservice/interface/index.html)
now exposes the first-party [1.43 PDF](https://www.szse.cn/marketServices/technicalservice/interface/P020260918541117849849.pdf)
(2026-09-18). This supplies a specific current specification location, not
the July 2026 deliveries or their applicable historical schema. That schema
crosswalk and historical access remain prerequisites from the earlier note.

Until these prerequisites are met, preserve explicit unavailable/unknown
outcomes for unsupported negative or daily-completeness claims. The new
positive SZSE exact-date report and dated tables can be retained for research
and a later narrowly scoped design review; this note does not enable them as
an unverified runtime fallback.
