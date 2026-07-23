# Changelog

## 0.2.0 - Unreleased

This release resets the pre-1.0 normalized-data boundary around checked,
immutable records.

Breaking migrations:

- `Provenance::new`, `with_source_at`, and `with_batch_id` now return
  `Result`; empty evidence is rejected.
- `DataBatch::best_effort` now returns `Result`; blank quality issues are
  rejected and completeness is derived from the issue list.
- Normalized records, quality reports, provenance, and requests expose
  read-only accessors instead of public invariant-bearing fields.
- `Quote::new` and `Bar::with_source_at` now return `Result`; provider adapters
  propagate `CoreError` as a typed source.
- TDX historical-bar adapters reject date ranges explicitly because the TDX
  request used here cannot honor normalized `start`/`end` semantics.
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
- Pinned the HTTPS URL/IDNA/zeroize dependency chain in `Cargo.lock` so Cargo
  1.83 can parse and compile it without the transitive edition-2024 failure.
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
  CNInfo, CLS, SSE, SZSE, HKEX and local-analysis provider identities.
- Added provider-neutral contracts, conservative capabilities and traits for
  market statistics/technical bars, research/consensus/semantic search,
  signals/boards/dragon-tiger/popularity, historical fund flow/capital data,
  news/announcements/interaction, company statements, four limit pools and ETF
  options.
- Expanded `magic-market-router` with thin provider-neutral adapters for every
  new normalized family; all records continue through the existing
  provider/batch evidence rejection and ordered attempt trace.
- Added `magic-market-analysis`, a network-free crate for checked SMA, forward
  PE, PEG, configurable PE-digestion scenarios, limit-pool sentiment and
  cross-source freshness/value diagnostics with retained input evidence.

Serde input now passes through the same constructors used by Rust callers, so
invalid numeric values, identifiers, evidence, dates, OHLC ranges, order-book
levels, price-limit rules, status completeness, and quality states fail
explicitly.
