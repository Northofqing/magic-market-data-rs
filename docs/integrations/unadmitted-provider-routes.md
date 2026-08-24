# Unadmitted Provider routes and explicit alternatives

This matrix records the ten repository-unadmitted Provider-by-operation
registrations exposed by the production gRPC capability service. An operation
alternative is not an in-request fallback: callers must either leave
`preferred_provider` empty and accept the selected admitted Provider, or select
an admitted Provider explicitly. A request that explicitly selects an
unadmitted Provider never changes Provider identity after a failure.

| Unadmitted route | Existing same-Provider interface | Admission result | Explicit admitted operation route |
| --- | --- | --- | --- |
| `HithinkFinance / Auctions` | Fuyao `/api/a-share/auction/snapshot` exists. `/api/a-share/auction/short-term-benchmark` and `/api/a-share/calendar/trading-days` also expose dates, but neither date is bound to a snapshot record. | Keep diagnostic: the snapshot omits its trading date, source time and directional unmatched queues. | `EastmoneyMiaoxiang` narrow exact-date auction, when its runtime Key is available. |
| `Jin10 / EconomicCalendar` | The public flash interface still exposes bounded release-related flashes, but Jin10 ended its free calendar/API embedding service on 2025-12-01. | Keep diagnostic: a latest-flash window cannot prove a complete calendar. | No equivalent Jin10 calendar substitute. Callers may explicitly select other admitted economic datasets, but they are not calendars and retain their own Provider identity. |
| `Imf / EconomicSeries` | The legacy DataMapper path returns HTTP 403. The official replacement is SDMX 2.1/3.0 and its Swagger exploration requires a beta portal account. | Keep blocked until an authenticated, versioned IMF dataflow/key and observation contract is available. | No equivalent IMF-series substitute. `Fred`, `Nbs`, `Pbc` and `WorldBank` are admitted only when the caller intentionally requests those different datasets. |
| `EastmoneyMiaoxiang / FundFlowSeries` | The fixed Miaoxiang Skills query exists and returns the five buckets, but the natural-language table/cardinality contract and serial stability are not proved. | Keep diagnostic. | `Eastmoney` public fund-flow series. |
| `Baidu / HistoricalBars` | The Baidu endpoint exists and its exact source-provided OHLCV/amount plus MA contract is admitted as `TechnicalBars`. It does not prove a generic trading-calendar/corporate-action history contract. | Keep generic `HistoricalBars` diagnostic; keep `TechnicalBars` admitted. | `Tencent`, `Tdx`, `Sina`, `HithinkFinance`, or entitled `EmQuant`, according to the requested exact scope. |
| `EastmoneyMiaoxiang / MoneyFlows` | Same fixed Miaoxiang Skills source as the series route; source methodology and serial stability remain unproved. | Keep diagnostic. | `Eastmoney` public latest daily money flow. |
| `EmQuant / MoneyFlows` | Official `css` supports the configured money-flow indicators. The 2026-08-22 live call returned only missing fields and no complete source date. | Keep diagnostic; a missing-field record is not production data. | `Eastmoney` public latest daily money flow. |
| `EmQuant / OrderBooks` | Official `css` is already implemented for the five-level indicator set. The current account returned `10001012 / EQERR_ACCESS_INSUFFICIENCE`. | Keep diagnostic; entitlement failure returns no production records. | `Tencent`, `Tdx`, `Sina`, or `Szse`. |
| `EmQuant / RealtimeQuotes` | Official `css` is already implemented for the quote indicator set. The current account returned `10001012 / EQERR_ACCESS_INSUFFICIENCE`. | Keep diagnostic; entitlement failure returns no production records. | `Tencent`, `Tdx`, `Sina`, or `Szse`. |
| `SecuritiesTimes / GlobalNews` | The first-party front-page metadata path exists, but the live source currently includes blank or unsafe source attribution. | Keep diagnostic until a bounded re-audit proves every record's first-party identity and record evidence. | `Cailianpress`, `Eastmoney`, `Jin10`, `ThePaper`, `WallstreetCn`, `XinhuaFinance`, `Yicai`, or `Yonhap`, selected explicitly without relabeling records. |

## HITHINK date finding

The official auction snapshot documents `data.timestamp` as response assembly
time. The separate short-term benchmark documents `date` as its resolved query
date and explicitly does not roll a non-trading date back. A bounded live check
on Saturday 2026-08-22 returned a `closed/final` snapshot for `600519.SH`, while
the benchmark resolved `date=2026-08-22` and returned an empty item list. The
calendar can identify that 2026-08-21 was a trading day, but no official field
binds that date to the returned snapshot. Joining these responses would infer
provenance, so it is prohibited.

## Verified operation alternatives

Bounded production gRPC probes on 2026-08-22 returned admitted records from:

- `Eastmoney` for `FundFlowSeries` and `MoneyFlows`, with per-record source date;
- `EastmoneyMiaoxiang` for the exact-date narrow `Auctions` contract;
- `Sina` and `Tdx` for daily bars, quotes and five-level order books;
- `Nbs`, `Pbc` and `WorldBank` for their own economic-series datasets.

These observations prove that admitted operation routes are runnable. They do
not authorize relabeling one Provider or dataset as another.
