# gRPC Provider Parity Design

## Goal

Expose every existing evidence-backed read-only Provider through the versioned
gRPC boundary without changing Provider HTTP policies, endpoint allowlists,
admission state, provenance, or fail-closed behavior.

Reserve append-only interfaces for the five composed data products
`IndexQuotes`, `IntradayShape`, `T0Evidence`, `OutcomeDailyBars`, and
`UpperLimitPoolReview`. Their exact v1 JSON contracts are defined in
`docs/integrations/grpc-derived-products.md`; this design does not admit or
fabricate their production implementations.

## Gate A boundary

- Reuse existing Provider clients and registered HTTP transports. The only
  allowlist correction is the literal `quotes.sina.cn` host already embedded in
  Sina's K-line and financial clients; it is recorded in
  `http-transports.tsv`. No suffix/wildcard matching, caller-supplied endpoint,
  path, MIME, timeout, redirect, proxy, body-size policy or dependency is
  widened.
- Existing gRPC operations receive additional exact Provider registrations.
- Add one append-only operation, `InstrumentNews`, backed only by the admitted
  Sina contract in BR-025. Its payload is the existing
  `InstrumentDateRangeRequest` JSON and its records use
  `magic.market.news_item` schema version 1.
- Add append-only operation values 56 through 60 for the five composed products.
  `IndexQuotes` is admitted only after its strict-freshness Tencent composition,
  deterministic tests, two live probes and three serial requests pass.
  `IntradayShape`, `OutcomeDailyBars`, and `UpperLimitPoolReview` are admitted
  only after the same evidence gate passes for their exact compositions.
  `T0Evidence` remains repository-unadmitted because public TDX Quote/book
  source time is unproved; explicit diagnostic access cannot promote it.
- News metadata remains metadata-only where required by BR-012, BR-020,
  BR-031, and BR-042. Registration never enables article-body fetching.
- NBS, PBC, and World Bank reuse the existing `EconomicSeries` request and
  output contracts. Each Provider continues to enforce its exact admitted
  namespace, period, region, unit, and cardinality scope before I/O.
- TDX public, Sina, SZSE, SSE, and EMQuant are selectable Provider identities;
  no request gains URL, executable-path, transport, credential, or method
  injection.
- EMQuant is always diagnostic-only. A managed bridge must be discoverable
  before a diagnostic handler is exposed; repository admission remains false
  because entitlement and runtime presence are deployment facts not covered by
  `admissions.tsv`.
- IMF, CFFEX delivery, TDX auction, LocalAnalysis anomaly families, and all
  other blocked rows remain blocked. A later same-day Gate C observation
  admitted only Yonhap's exact Economy feed after 2 live and 3 serial requests;
  Rolling and the other explicit channels remain diagnostic.

## Registration matrix

| Operation | Added Providers | State |
| --- | --- | --- |
| `GlobalNews` | CLS, ThePaper, XinhuaFinance, Yicai, SecuritiesTimes | existing Provider admission |
| `InstrumentNews` | Sina | admitted by BR-025 |
| `IndexQuotes` | Tencent | admitted bounded index quote composition with strict source freshness |
| `IntradayShape` | LocalAnalysis | admitted deterministic regular-session minute derivation; deterministic test plus 2 live/3 serial requests passed |
| `T0Evidence` | TDX | opt-in diagnostic returns the exact four available families; remains incomplete/unadmitted because Quote/book source time is absent |
| `OutcomeDailyBars` | TDX | admitted TDX-only exact-through daily-bar preimage; deterministic contract plus 2 live/3 serial requests passed |
| `UpperLimitPoolReview` | Eastmoney | admitted atomic four-pool composition; deterministic test plus 2 live/3 serial requests passed |
| `EconomicSeries` | Nbs, Pbc, WorldBank | exact provider-scoped admission |
| `RealtimeQuotes` | Tdx, Sina, Szse | existing Provider capability |
| `HistoricalBars` | Tdx, Sina | existing Provider capability |
| `MinuteData` | Tdx, Sina | existing Provider capability |
| `OrderBooks` | Tdx, Sina, Szse | existing Provider capability |
| `Trades` | Tdx | existing Provider capability |
| `SecurityMetadata` | Tdx, Sina | existing Provider capability |
| `Announcements` | Sse, Szse | existing official Provider capability |
| `DragonTiger` | Sse, Szse | existing official Provider capability |
| quote/daily-or-intraday-bar/order-book/money-flow | EmQuant | diagnostic-only when bridge exists |

## Compatibility

The Protobuf enum receives `OPERATION_INSTRUMENT_NEWS = 55` and append-only
values 56 through 60 for the five composed products; all prior numeric values
and message fields remain unchanged. The RPCs are additive. Existing clients
continue to decode prior messages and ignore the new service methods.

## Failure model

Provider typed failures map to the existing gRPC status taxonomy. Missing
runtime secrets or bridge executables are advertised as runtime-unavailable or
diagnostic-unavailable capabilities and fail before Provider I/O. No Provider
failure becomes an empty successful response.
