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

Serde input now passes through the same constructors used by Rust callers, so
invalid numeric values, identifiers, evidence, dates, OHLC ranges, order-book
levels, price-limit rules, status completeness, and quality states fail
explicitly.
