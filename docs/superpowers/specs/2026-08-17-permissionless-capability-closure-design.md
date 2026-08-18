# Permissionless Capability Closure Design

## Goal

Close every data capability that can be proved with the repository's existing
public, fixed-endpoint providers and the installed TDX terminal.  Capabilities
that require Level-2 entitlements, written CFETS authorization, or an IMF beta
API contract remain explicit `UNADMITTED`/`Unsupported`; this slice never
fabricates their missing fields or substitutes a local observation time for a
provider source time.

## Gate A boundary

- Reuse only already registered endpoints and HTTP stacks.  This slice does not
  add a host, path family, redirect, proxy, MIME, timeout, body-size policy, or
  HTTP dependency.  Any later network expansion requires its own Gate A design
  and a matching `http-transports.tsv` update.
- Keep the current gRPC operation numbers and envelope stable.  Provider-specific
  JSON request/record schemas may be added only under the existing versioned
  operation payload boundary; prior diagnostic schemas remain selectable by an
  explicit diagnostic provider.
- Preserve BR-033 source-time rules, BR-034 full-market evidence, BR-035 auction
  completeness, BR-044 anomaly identity, BR-045 diagnostic isolation, BR-047
  watchlist ownership, and BR-048 TDX field admission.
- Admission changes are per family and occur only after deterministic tests,
  two live probes, and three serial load requests pass.  A successful diagnostic
  request does not change repository admission.
- CFFEX production HTTPS reuses the already registered
  `magic-market-transport` reqwest/rustls stack with an exact bounded notice-path
  allowlist.  The existing provider-local ureq client remains only behind the
  separately named plaintext diagnostic.  There is no automatic HTTPS-to-HTTP
  downgrade and failure of the shared HTTPS stack remains explicit.

## Implementable work

1. **Complete-market rankings.** Reuse Eastmoney's fixed A-share `clist`
   endpoint and the existing bounded full-pagination parser.  Production output
   requires Shanghai, Shenzhen and Beijing coverage, exact declared/received
   cardinality, unique instruments, descending values, continuous ranks, names,
   units, source date/session and bounded source-time skew.  The caller limit is
   applied only after the complete universe is validated.
2. **Market breadth.** Reuse the existing `MarketBreadthAnalysis` contract.  A
   production composition must retain a proved complete universe, complete
   dynamic quote snapshot, complete upper/lower limit pools, coverage, source
   session/date and maximum dynamic-source skew.  MX aggregate counts remain a
   separately named diagnostic and cannot satisfy this contract.
3. **Public Eastmoney fund flow.** Exercise the fixed public fund-flow contract
   independently of Miaoxiang.  Exact instrument, interval, date, units,
   cardinality and source evidence must survive two live and three serial
   requests before either `FundFlowSeries` or `MoneyFlows` is admitted.
4. **Technical bars.** Preserve the meaning "source-supplied MA values".  The
   exact Baidu contract may be admitted as source-supplied unadjusted daily
   OHLCV/amount plus optional MA5/MA10/MA20 after its own two-live/three-load gate.
   It makes no adjusted-continuity, calendar-completeness or corporate-action
   claim; generic Baidu `HistoricalBars` remains false.  TDX bars are not
   relabelled as Baidu evidence or silently converted into source indicators.
5. **TDX local observations.** The fixed loopback may expose `LastClose` or OHLC
   only when the official response supplies those exact fields and live fixtures
   prove their units and identity.  `source_record_count` remains absent because
   poll sequence/sample count is not a provider record count.  Existing Now,
   cumulative Volume and cumulative Amount admissions remain unchanged.
6. **Local anomaly events.** Complete calendar/session reset wiring and retain
   provider message/observation time.  Production thresholds, windows,
   hysteresis and cooldown have no repository defaults; they must be supplied by
   a versioned rule configuration and shadow evidence before the three anomaly
   admission constants can change.

## Permission-blocked work

- **Auctions:** public sources may return partial volume/amount.  Matched price,
  previous close, unmatched queues, volume ratio and exact provider time remain
  null without an authorized Level-2/broker contract.
- **T0 evidence:** available quote/book/minute/trade families stay diagnostic
  while TDX quote/book source timestamps are absent.
- **CFETS DR007:** no automated public-page collection is added.  Production
  requires written CMDS/information-product authorization and its documented
  endpoint/schema/entitlement.
- **IMF:** no DataMapper scraping or authenticated-browser emulation is added.
  Production requires the official beta SDMX/API contract and credentials.
- **CFFEX delivery:** the plaintext fixed-path diagnostic stays unadmitted and
  missing delivery method remains `NotProvided`.  Production admission may be
  reconsidered only if the shared HTTPS path passes the full parser and serial
  live-evidence gates; browser/search reachability alone is not evidence.

## Failure and performance model

All acquisition is bounded by existing response, page, retry and pacing limits.
Full-market work validates pagination before applying caller limits and fails on
universe drift, duplicates, missing evidence, mixed dates/sessions or excessive
skew.  Provider calls remain blocking and are isolated from async gRPC according
to `docs/integrations/async-blocking.md`.  No failure becomes a successful empty
batch, zero-filled field, or admission promotion.
