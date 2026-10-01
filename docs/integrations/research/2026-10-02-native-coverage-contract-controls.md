# Native exchange coverage controls, 2026-10-02

This bounded follow-up tests native issuer/date/empty/page behaviour rather
than repeating the earlier source survey. Its repository basis is production
source revision `67c832e43f36f188e4d769f409691c0b1d9a2ea2` in the isolated
`grpc-coverage-20261002` worktree. There is no claim that these HTTP controls
ran through that revision's gRPC service or that a new Provider was admitted.
No credential, production deployment, build, commit, dependency, or cross-host
transfer was used. The required `autonomous-long-task` skill was absent from
the supplied skill catalog and this checkout; the background task continued
under the available `research` workflow and the explicit bounded assignment.

Nine original public response bodies plus `manifest.json` are retained under
`target/authority-controls-20261002/`. Captures occurred at **2026-10-02
01:12:44–01:13:17 Asia/Shanghai** (2026-10-01 17:12:44–17:13:17 UTC).
Observation time is a local receipt time, not source publication/finality.
The manifest records exact URIs, public headers, body lengths, HTTP statuses,
observation times, SHA-256 hashes, and distinct capture IDs. Capture IDs are
local labels; neither source supplies an upstream request ID in these bodies.
URI hashes cover UTF-8 URI bytes only, not headers or issuer authenticity.

## SZSE: native controls distinguish rows, failed queries and past-end pages

The [official 300005 chart/report page](https://www.szse.cn/market/trend/index.html?code=300005)
exposes stock-report catalog `1815_stock_snapshot`; its first-party
[report script](https://res.szse.cn/modules/report/js/report_new.min.js)
uses `ShowReport/data`. Each control below uses that first-party report
endpoint, `SHOWTYPE=JSON`, `CATALOGID=1815_stock_snapshot`, `TABKEY=tab1`,
the exact `txtDMorJC`/`txtBeginDate`/`txtEndDate` filters and `PAGENO`.
Referer is the official chart page for the queried code, with ordinary
`User-Agent: Mozilla/5.0` and `Accept: application/json`. Redirects were
disabled. All six responses were HTTP 200 `application/json`.

| Capture (file stem) | Code and inclusive filter | Requested page | Native `recordcount` / `pagecount` / `pageno` | Returned rows | Native `error` |
| --- | --- | ---: | --- | ---: | --- |
| `szse-300005-two-days` | 300005; 07-16…07-17 | 1 | 2 / 1 / 1 | 2 | null |
| `szse-300004-two-days` | 300004; 07-16…07-17 | 1 | 2 / 1 / 1 | 2 | null |
| `szse-300005-date-empty` | 300005; 07-18 only | 1 | 0 / 0 / 1 | 0 | null |
| `szse-999999-issuer-empty` | 999999; 07-16…07-17 | 1 | 0 / 0 / 1 | 0 | null |
| `szse-300005-two-days-page2` | 300005; 07-16…07-17 | 2 | 2 / 1 / 2 | 0 | null |
| `szse-300005-over5d-page1` | 300005; 07-16…07-30 | 1 | 0 / 0 / 1 | 0 | non-empty range rejection |

All date labels in this table are 2026. The exact source URLs and originals
are in the manifest; directly inspect the
[300005 two-day source](https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1815_stock_snapshot&TABKEY=tab1&txtDMorJC=300005&txtBeginDate=2026-07-16&txtEndDate=2026-07-17&PAGENO=1),
[300004 control](https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1815_stock_snapshot&TABKEY=tab1&txtDMorJC=300004&txtBeginDate=2026-07-16&txtEndDate=2026-07-17&PAGENO=1),
[date-empty control](https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1815_stock_snapshot&TABKEY=tab1&txtDMorJC=300005&txtBeginDate=2026-07-18&txtEndDate=2026-07-18&PAGENO=1),
[issuer-filter empty control](https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1815_stock_snapshot&TABKEY=tab1&txtDMorJC=999999&txtBeginDate=2026-07-16&txtEndDate=2026-07-17&PAGENO=1),
[past-end page](https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1815_stock_snapshot&TABKEY=tab1&txtDMorJC=300005&txtBeginDate=2026-07-16&txtEndDate=2026-07-17&PAGENO=2),
and [over-five-day query](https://www.szse.cn/api/report/ShowReport/data?SHOWTYPE=JSON&CATALOGID=1815_stock_snapshot&TABKEY=tab1&txtDMorJC=300005&txtBeginDate=2026-07-16&txtEndDate=2026-07-30&PAGENO=1).

### Issuer/date binding: meaningful positive control, weaker empty identity

Both positive responses echo their requested code and date filters through
`metadata.conditions[].defaultValue`. More strongly, each returned data row
has its own `zqdm`, `zqjc`, `jyrq`, previous close and OHLC. For 300005 the
source name is 探路者 and closes are 18.59 (07-16), 14.87 (07-17); for 300004
the source name is 南风股份 and closes are 8.37, 7.80. Both responses have
exactly the two requested dated rows, in descending date order, and different
body hashes. The source's tab metadata labels `tab1` stocks. Row identity
does not explicitly include exchange or asset class; the venue is the owning
SZSE endpoint and stock category is the tab metadata. These are independent
SZSE observations, never TDX identity echoes.

The native date-empty and 999999 filter-empty bodies echo their own filters
and return `error=null`, zero native counts and no rows. They prove a zero-row
report response for those queries. The source does not distinguish the empty
causes with a trading-status or instrument-existence field. In particular,
echoing `999999` into a condition is not a security-directory attestation.
Thus condition echo and zero counts cannot alone prove the queried security
was listed, suspended, unlisted, normally trading, or free of corporate actions.

### Over-five-day negative: HTTP 200 and zero totals can conceal rejection

The 07-16…07-30 response echoes the longer dates but has native `error`:

> 最多只能查询五天的数据，请缩小查询范围再查询

Its native total and page count are both zero, and `data=[]`. Each response's
date condition also declares `dateRange('all','${txtEndDate}','5d')` as its
UI validation. This probe demonstrates an actual backend refusal, not merely
a front-end warning. The text does not specify calendar-day versus trading-day
boundary arithmetic; this test brackets only the valid two-day and rejected
15-calendar-day filters. It cannot be used to justify a larger window.

Any later adapter must inspect source errors before calling zero counts a
verified empty report. A longer-window acquisition would need separately
permitted bounded subqueries with preserved request/range identities; a
consumer must not publish this range-rejected response as `complete=true`.

### Page-end negative: an empty page is not an empty query

For the valid two-day query, page 1 declares two total rows and one page and
returns both rows. Page 2 returns no rows and `error=null`, while **retaining
`recordcount=2`, `pagecount=1`, `pageno=2`, `pagesize=10`**. The source does not
clamp the page number or emit a typed out-of-range error. A caller that starts
at page 2 and trusts `data=[]` would miss both rows. Terminal consumption must
therefore start at the first declared page, validate all pages/counts, and not
reinterpret an out-of-range empty page as a query-wide empty result.

Page 1 accounts for the entire declared row set for this two-day filter.
No explicit `complete`, `terminal`, immutable snapshot token, publication
timestamp or correction/revision identifier is present. The observed total
and page count delimit the source-reported report rows; they do not define
every expected trading session or provide a daily authoritative state universe.
This bounded run did not acquire a legitimate multi-page positive query: the
longer single-security query was explicitly rejected. It makes no claim that
independent pages or subwindows would share an atomic source revision.

## SSE: positive row identity survives, empty request identity does not

These controls use the public [official suspension page](https://www.sse.com.cn/disclosure/dealinstruc/suspension/)
and its first-party [query script](https://www.sse.com.cn/xhtml/home/2021public/querySearch/search_tradeTip_2021.js).
Each GET uses `GW_PL_JYTS_TFPXX`, `pageHelp.pageSize=25`, page 1 and the exact
source-date filters, with the official suspension-page Referer and the same
ordinary User-Agent/Accept headers. All three returned HTTP 200 JSONP.

| Capture | Product filter | Date filter | Native total / page count | Result |
| --- | --- | --- | --- | --- |
| `sse-688277-july-window` | 688277 | 20260716…20260730 | 1 / 1 | One positive suspension span: row productCode 688277, start 20260716, end 20260729 |
| `sse-688561-july-window` | 688561 | 20260716…20260730 | 0 / 0 | Empty result |
| `sse-688277-date-empty` | 688277 | 20260730…20260730 | 0 / 0 | Empty result |

The [688277 window response](https://query.sse.com.cn/commonSoaQuery.do?jsonCallBack=cb&isPagination=true&sqlId=GW_PL_JYTS_TFPXX&pageHelp.pageSize=25&pageHelp.pageNo=1&keyWords=&startStopDate=20260716&endStopDate=20260730&productCode=688277)
has an identity-bearing positive row and `type=LXTP`. The result's dates are
the suspension span, not an echo of the query's ending 07-30 date. Its source
body exposes `sqlId`, but `securityCode` and `queryDate` are empty;
`pageHelp.startDate`, `endDate`, `searchDate`, and `endPage` are null. The same
missing fields occur in the [688561 control](https://query.sse.com.cn/commonSoaQuery.do?jsonCallBack=cb&isPagination=true&sqlId=GW_PL_JYTS_TFPXX&pageHelp.pageSize=25&pageHelp.pageNo=1&keyWords=&startStopDate=20260716&endStopDate=20260730&productCode=688561)
and [688277 date-empty response](https://query.sse.com.cn/commonSoaQuery.do?jsonCallBack=cb&isPagination=true&sqlId=GW_PL_JYTS_TFPXX&pageHelp.pageSize=25&pageHelp.pageNo=1&keyWords=&startStopDate=20260730&endStopDate=20260730&productCode=688277).
`actionErrors=[]` and `fieldErrors={}` in all three.

The two differently filtered empty bodies are byte-identical: 485 bytes and
SHA-256 `553e577856c7f1806f3fc5468868347cb1bdfba132555090cf66f57a08ae1949`.
Their URI hashes differ. A retained response hash or `total=0` alone cannot
bind the empty result to a particular issuer/date window. Request provenance
must be retained separately, and does not itself supply a source-owned
complete historical coverage/correction contract. These controls do not
prove 688561 normal trading, no 688277 07-30 event, or an actual resumption
event on that date. The positive-span observation remains narrower than a
complete daily listing/board/risk/limit/suspension state contract.

## Original-response hashes and remaining limits

The originals contain no credentials. SHA-256 and lengths from the captures:

| File | Bytes | Body SHA-256 |
| --- | ---: | --- |
| `szse-300005-two-days.json` | 3744 | `c00db4286efd1ecd6ea2eb9862a41e00e648cb489f57c3ee995e793c6931e164` |
| `szse-300004-two-days.json` | 3738 | `5e04043c45e1a62716bd26483951bc68270dcad9430c9ab3d5ef4b2d4b747932` |
| `szse-300005-date-empty.json` | 3363 | `9e0bde5adaf70386aac652466b365782909ea8135fa5414246e76953231736d7` |
| `szse-999999-issuer-empty.json` | 3363 | `c78cb023f94566d204547419750e2ef3ac64a29d1433a8edd346dfee5a116359` |
| `szse-300005-two-days-page2.json` | 3363 | `0618a79e255849f07e45746c6ef094ed13e7e0775c8aa0c936ef168db3f549e8` |
| `szse-300005-over5d-page1.json` | 3405 | `5d5f75b43404497402bed95cd154c92fb62bc830ff145ebeb98cde21e034230b` |
| `sse-688277-july-window.jsonp` | 919 | `86e9188942eeb9ae4fe95ffb5ed235fa5f60fbab8efa41885d86828cd964490c` |
| `sse-688561-july-window.jsonp` | 485 | `553e577856c7f1806f3fc5468868347cb1bdfba132555090cf66f57a08ae1949` |
| `sse-688277-date-empty.jsonp` | 485 | `553e577856c7f1806f3fc5468868347cb1bdfba132555090cf66f57a08ae1949` |

The actionable acceptance boundaries established here are: inspect native
errors before empty classification; distinguish page-empty from query-empty;
validate positive row identity independently of echoed input; retain exact
request provenance when the response lacks its filters. Still required for
authoritative D14/D17/D20 promotion are source-defined adjustment and data
scope, complete trading-day/issuer universe, applicable historical schema,
source revision/publication/finality and all corrections, and an exhaustive
all-action/explicit-empty contract. This task found no new source-owned
terminal flag, certified no-event response, or authoritative daily file set.
It also provides no evidence that TDX's absent identity echo has changed.
