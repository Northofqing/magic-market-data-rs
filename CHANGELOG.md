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

Serde input now passes through the same constructors used by Rust callers, so
invalid numeric values, identifiers, evidence, dates, OHLC ranges, order-book
levels, price-limit rules, status completeness, and quality states fail
explicitly.
