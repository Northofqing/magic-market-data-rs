# Changelog

## 0.2.0 - Unreleased

This release resets the pre-1.0 normalized-data boundary around checked,
immutable records.

Breaking migrations:

- TDX low-level byte/number readers and `helpers::get_price` are now fallible.
  TDX parsers reject truncated declared batches atomically instead of
  panicking, filling missing fields with zero, or returning a shorter vector.
- Removed the repository Rust version selector and MSRV declaration. Local and
  release gates use the active default toolchain, CI uses current stable, and
  packaged artifacts record the actual `rustc` and Cargo versions.
- `Provenance::new`, `with_source_at`, and `with_batch_id` now return
  `Result`; empty evidence is rejected.
- `DataBatch::best_effort` now returns `Result`; blank quality issues are
  rejected and completeness is derived from the issue list.
- Normalized records, quality reports, provenance, and requests expose
  read-only accessors instead of public invariant-bearing fields.
- `DragonTigerEntry` and `DragonTigerSeat` now require checked constructors
  and checked Serde. Seat JSON adds required `instrument` and `trading_date`
  fields; legacy seat payloads without them must be migrated before decoding.
- `Quote::new` and `Bar::with_source_at` now return `Result`; provider adapters
  propagate `CoreError` as a typed source.
- TDX historical-bar adapters reject date ranges explicitly because the TDX
  request used here cannot honor normalized `start`/`end` semantics.
- TDX normalized historical bars now honor the complete positive `u16`
  request-limit domain through atomic exact pages at the 800-row wire boundary;
  any failed, empty, short or malformed page rejects the whole batch.
- TDX normalized quotes and order books no longer promote the unverified raw
  quote-time field into `source_at`; the raw protocol field remains available
  for future audited decoding.
- Provider live probes now accumulate all supported-family failures, enforce
  response cardinality, classify current-day minute availability separately
  from latest-session history, and exit nonzero on any unexpected result.
- China-local trading-phase detection now handles UTC+8 day rollover and the
  11:30-13:00 lunch break correctly.
- TDX half-present pre-open book levels are normalized atomically as unavailable
  with quality evidence; negative and non-finite source levels remain errors.
- Added `magic-tencent-rs`, a supplemental HTTPS/GBK provider with strict
  cardinality, verified source timestamps, CNY amounts, source-lot quantities,
  deterministic fixtures, live probes, and a bounded concurrent load probe.
- Added provider-neutral checked `MinutePoint`/`MinuteDataRequest` contracts and
  implemented normalized current/dated minute data for TDX and Tencent.
- Live-verified TDX Beijing market `2` for Quote, daily bars, five-level books,
  minute data, and current trades; Beijing security metadata now fails early
  with the exact unsupported security-list boundary.
- Expanded Tencent to Beijing Quote/books, 1/5/15/30/60-minute and
  day/week/month bars, current/dated minute data, current-session paged trades,
  and partial security metadata. Unsupported year bars, historical trades,
  Beijing intraday bars/trades, money flow, and auction remain explicit.
- Expanded the Tencent bounded load probe with per-family and mixed modes; the
  100-request/8-worker mixed live run completed without failures.
- Added Tencent `MarketStatisticsProvider` for turnover rate, PE/PB, market
  capitalization and source price limits across equities, indices and funds. A
  12-request/3-worker live statistics run completed without failures.
- Added `magic-sina-rs`, a supplemental HTTPS/GB18030/JSON Provider with strict
  沪深京 Quote/five-level books, selected intraday/daily bars, current
  latest-session minute accumulation and partial security metadata.
- Normalized every Sina source-share quantity to lots at the adapter boundary,
  retained missing daily amounts and real limit-up book gaps, and kept trades,
  money flow, auction and unverified periods explicitly unsupported.
- Added Sina live and bounded load probes. The 2026-07-23 real live run passed
  all declared families; the 20-request/4-worker mixed run completed without
  failures and the final run reported 11.69 requests/s with 207.786 ms p50
  latency.
- Added Sina balance-sheet, income-statement and cash-flow providers with
  nullable source fields, duplicate-field diagnostics and checked report
  evidence. A 6-request/2-worker real load run returned 48 periods without
  failures.
- Added Sina ETF-option month/contract discovery, top-of-book T-quotes and
  source Greeks/IV for 510050, 510300, 588000 and 510500. The real live probe
  has verified 510050; the other three implemented underlyings still require
  separate live verification. A 6-request/2-worker 510050 option load run
  completed without failures.
- Expanded Core option contracts with checked expiry months, optional expiry
  dates/strikes, full quote fields and source Greek values while preserving
  explicit absence instead of calculating unverified values.
- Added `magic-market-router`, a provider-neutral first-acceptable-batch
  failover chain with explicit failure classification, quality/source-time
  gates, record/batch evidence checks and ordered attempt traces for every Core
  data family.
- Added a real TDX-to-Tencent routing probe. With complete quality and source
  time required, it preserved the TDX quality rejection and selected a
  source-timestamped Tencent Quote without merging or rewriting Provider
  evidence.
- Pinned the HTTPS URL/IDNA/zeroize dependency chain in `Cargo.lock` after
  reproducing a transitive edition-2024 manifest incompatibility.
- Added release preflight/package scripts and an operator deployment runbook
  covering platform artifacts, network access, secrets, EMQuant activation,
  health evidence, observability, rollback, and release verification.
- Expanded release packaging and deployment health checks from five to seven
  probes by adding the Sina live/load binaries and integration contract.
- Re-verified EMQuant after Choice entitlement propagation: official SDK login,
  live daily bars and daily money flow now pass. Quote, order book and minute
  history remain truthfully gated by
  `10001012/EQERR_ACCESS_INSUFFICIENCE`.
- Normalized the SDK's non-zero-padded `YYYY/M/D` daily dates to strict ISO
  dates and added readable query-time SDK entitlement diagnostics.
- Expanded the root README into a Chinese-first developer and operator entry
  manual covering workspace boundaries, normalized evidence, exact Provider
  capabilities, deterministic setup, real/load probes, failover routing,
  release packaging, deployment constraints and security navigation.
- Added checked primitives and record-level `SourceEvidence` for the
  intelligence expansion, plus first-class Baidu, Tonghuashun, iwencai,
  CNInfo, CLS, Jin10, The Paper, SSE, SZSE, HKEX and local-analysis provider
  identities.
- Added provider-neutral contracts, conservative capabilities and traits for
  market statistics/technical bars, research/consensus/semantic search,
  signals/boards/dragon-tiger/popularity, historical fund flow/capital data,
  news/announcements/interaction, company statements, four limit pools and ETF
  options.
- Expanded `magic-market-router` with thin provider-neutral adapters for every
  new normalized family; all records continue through the existing
  provider/batch evidence rejection and ordered attempt trace.
- Added the previously missing bounded `PostCloseFlow` Top10 contract and
  Router adapter. It retains source-backed board/price-limit metadata and is
  not advertised by any Provider until exact 15:35 ranking semantics are
  verified. Checked records couple source date to trading date, and routing
  rejects date mismatches, over-limit batches and duplicate ranks/instruments.
- Added `magic-market-analysis`, a network-free crate for checked SMA, forward
  PE, PEG, configurable PE-digestion scenarios, limit-pool sentiment and
  cross-source freshness/value diagnostics with retained input evidence.
- Added `magic-eastmoney-rs` as a public-web identity separate from
  Choice/EMQuant, covering reports, minute/daily fund-flow parsing, three board
  flow categories, dragon-tiger entries/seats, margin, block trades, holder
  counts, lockups, dividends, four limit pools and popularity.
  Seat requests reserve one atomic ten-record buy-five/sell-five group and
  reject limits below ten or incomplete source groups.
  The real probe passes all advertised families. Both fund-flow hosts close the
  current development network connection before an HTTP response, so that
  family remains unadvertised and is reported as an expected-failure diagnostic
  without failing the advertised-capability probe.
- Kept Eastmoney keyword news unadvertised because its response has no
  structured source instrument identity; the method returns `Unsupported`
  instead of attaching the requested keyword instrument to a strict record.
- Added `magic-cninfo-rs` for cached exact stock/org mapping, paged
  announcements, canonical detail/PDF URLs and investor Q&A. Real live probes
  returned three announcements and three Q&A records; a serial 3/3 load run
  maintained a minimum 1002 ms request-start interval.
- Added `magic-ths-rs` for consensus, strong-stock reasons, source-backed upper
  limit-pool reasons/status and popularity. Real live probes passed all
  declared families; a serial 3/3 load run maintained a minimum 1000 ms start
  interval.
- Added `magic-cls-rs` for signed, newest-first global telegraph/news records
  with source timestamps, publishers, related instruments and topics. The
  bounded 2/2 live load sample returned 20 records without failure.
- Added `magic-jin10-rs` for unlocked public type-0 flashes and type-2 articles
  from the official 7x24 stream. Locked VIP rows are omitted without requesting
  protected details; the live probe returned five normalized records. Caller
  output remains capped at 20 while the independently verified source window
  permits a transient 21st row and rejects 22 or more.
- Added `magic-thepaper-rs` for native articles on The Paper finance channel
  `25951`. External forwards are omitted rather than relabeled; the live probe
  returned five normalized records with source times, sections and tags.
- Added `magic-baidu-rs` for unadjusted daily technical bars with source
  MA5/MA10/MA20. The live probe returned five 华电辽能 bars and the bounded 2/2
  load sample returned 40 records without failure.
- Added `magic-iwencai-rs` for explicit API-Key/X-Claw authenticated semantic
  search, result de-duplication and secret-redacted errors. Without a licensed
  key, the real endpoint's HTTP 401 is reported as authentication failure and
  never replaced by fixture or Cookie-derived data.
- Added `magic-exchange-rs` with first-class SSE/SZSE/HKEX identities:
  SSE/SZSE announcements and dragon-tiger data, SZSE Quote/five-level book,
  and lossless HKEX northbound DailyStat/Top10. Production traits enforce
  full pagination, cross-page de-duplication, complete buy-five/sell-five
  groups, source identity/time, exact units and clone-shared serial gates. The
  merged tree retains deterministic fixtures and explicit live/load probe
  commands; post-merge production admission must be regenerated rather than
  inferred from either parent commit.
- Switched the workspace, CI and release preflight to the rolling stable Rust
  toolchain and removed the fixed MSRV declaration.
- All public-web providers enforce HTTPS host allowlists, zero redirects,
  timeouts, response-size/request bounds, strict non-empty output, source
  evidence, deterministic injected transports, live probes and conservative
  bounded load probes.
- Closed final strict-identity review findings: public adapters no longer map
  Shanghai `900xxx` B shares to Beijing, CLS related instruments distinguish
  verified ETF/index/equity code families, and malformed present THS metadata
  fails schema validation instead of disappearing as an absent field.
- Expanded release packaging from seven to twenty-five uniquely named probe
  binaries and added secret-gated iWencai plus public-web live/load workflow
  coverage.
- Added checked `CorporateActionRequest`/`CorporateActionResponse`,
  category-specific lifecycle terms, explicit `admission_as_of`, and normalized
  TDX XDXR categories 1 through 14. The specialized Router owns one immutable
  admission date, accepts only its sealed validating source adapter, and
  retains that boundary in the selected outcome.
- Added production TDX board directory/constituent and concept-hit projections
  from versioned block snapshots, with instrument code and name retained
  together and explicit unsupported Beijing boundaries.
- Added checked full-market volume-ratio and main-net-inflow ranking contracts,
  independent capability flags, exact code/name identity, complete pagination,
  whole-operation Eastmoney host fallback, all-three-exchange coverage and a
  common zero-skew source timestamp requirement. Live admission remains false
  until a stable full-universe snapshot passes.
- Kept the strict 15:35 Eastmoney post-close adapter as a bounded diagnostic
  after the current live source returned missing metrics and mixed per-security
  timestamps. `CapitalCapabilities.post_close_flow` remains false and the
  formal `PostCloseFlows` implementation returns typed `Unsupported`.
- Added checked market-breadth analysis over an evidenced security universe,
  atomic quotes and complete limit pools, including bounded coverage/source
  skew and overflow-safe partition accounting.
- Added Eastmoney target-price observations and aggregates with exact lower and
  upper fields, contributor/sample evidence and typed first-page
  `VerifiedEmpty`; THS consensus routing retains matching instrument code and
  name.
- Added nanosecond-precise Router freshness admission with an inclusive
  five-second source-age boundary and the oldest-record batch-time rule. The
  live TDX-to-Tencent route rejects unverified TDX source time and admits only a
  complete Tencent batch.
- Added a fail-closed authorized Level-2 opening-auction conformance policy that
  binds Provider, provenance source, trading date, explicit China offset,
  exact `09:15:00..=09:25:00` source window, complete fields, cardinality and
  atomic evidence. No public Provider is advertised without a licensed feed.
- Added CFFEX delivery-calendar diagnostics with exact canonical `/cn/jystz`
  HTTPS paths, explicit Rustls or optional native-TLS selection, strict
  allowlists and typed transport failures. Production capability remains false
  because neither TLS backend completed an official HTTPS response in the
  recorded environment.
- Added an explicit gRPC `allow_unadmitted` diagnostic path for six registered
  read operations. Responses remain `UNADMITTED` and incomplete, retain the
  exact blocker, and leave unavailable source fields as `null`; the default
  path and the auction/market-breadth operations still fail before Provider
  I/O.
- Added bounded diagnostic records for the first Eastmoney ranking page and an
  explicitly dated post-close page. Both report source coverage instead of
  claiming a complete atomic market, and use schemas distinct from the strict
  production records.
- Added an authenticated Eastmoney Miaoxiang diagnostic client with a redacted
  environment Key, fixed query templates and bounded responses. When configured,
  gRPC can return daily tiered fund flow, partial opening-auction volume/amount,
  and partial all-A breadth counts; unproved fields remain null and all three
  admission families remain false. These four fixed-template operations are
  default-readable without request-level Provider or unadmitted flags while the
  response remains explicitly incomplete and `UNADMITTED`.
- Split the per-Provider timeout from the whole blocking-worker deadline so
  multi-request tasks keep a bounded total budget without widening any one
  Provider HTTP request.
- Added authenticated `MarketEventService.SetWatchlist` full-replacement control.
- Hardened TDX TCP decoding with exact bounded decompression, async read/write
  and response deadlines, real connection-pool concurrency, reconnect, block
  code preflight and chronological async pagination. Removed the unauthenticated
  port-80 financial archive fallback.
- Sealed option invariants behind checked constructors, completed normalized
  record evidence/status routing, rejected ambiguous numeric evidence dates and
  null batch IDs, and moved canonical event hashing to `sha2` without changing
  golden digests.
- Added TDX Agent idle heartbeats and server-side liveness expiry, bounded gRPC
  TLS identity file reads, fixed Tencent/Sina endpoint identities, and one
  shared production pacing lane for Eastmoney public and MX traffic.
- Admitted TDX TQ-Local current price, cumulative lot volume and cumulative CNY
  amount as production observation families while retaining empty source time,
  empty source-record count and UNADMITTED LocalAnalysis events.
  The active Agent advertises its bound, applies only canonical explicit EQUITY
  identities, restarts the fixed sibling monitor with a new generation, exposes
  desired/applied revisions, and never lets subscriber filters or the control
  request change endpoints, thresholds, admission, or account boundaries.

Serde input now passes through the same constructors used by Rust callers, so
invalid numeric values, identifiers, evidence, dates, OHLC ranges, order-book
levels, price-limit rules, status completeness, and quality states fail
explicitly.
