# A-Stock Data Core and Analysis Implementation Plan

> **Execution:** Use the `executing-plans` workflow task-by-task. Keep the
> user's untracked requirements document unstaged.

**Goal:** Add the provider-neutral contracts, evidence, capabilities, router
adapters and pure analysis needed by every remaining reference-project
capability.

**Architecture:** Reuse the existing `DataBatch`, `SourcedRecord`, typed market
values and generic failover router. Add validated reusable primitives first,
then domain records/traits in isolated Core modules, then a network-free
analysis crate. Provider crates consume these APIs in later plans.

**Tech stack:** Rust 2021, MSRV 1.83, `serde`, `thiserror`, existing Core and
Router crates; no network dependency in this slice.

---

### Task 1: Add RED identity and evidence contract tests

**Files:**

- Modify: `crates/magic-market-core/tests/provider_identity.rs`
- Modify: `crates/magic-market-core/tests/values.rs`
- Create: `crates/magic-market-core/tests/source_evidence.rs`

- [x] Require explicit Provider IDs for Baidu, Tonghuashun, Iwencai, CNInfo,
  CLS, SSE, SZSE, HKEX and local analysis.
- [x] Require `AssetClass::Option`.
- [x] Require checked `NonEmptyText`, `HttpsUrl`, `IsoDate`, `FiniteNumber`,
  `PositiveU32` and `SourceEvidence` round trips.
- [x] Assert empty/control/oversized text, non-HTTPS URLs, impossible dates,
  non-finite numbers, zero ranks and empty batch evidence are rejected through
  direct construction and serde.
- [x] Run the focused tests and record the expected missing-symbol failures.

### Task 2: Implement shared validated primitives and evidence

**Files:**

- Create: `crates/magic-market-core/src/validated.rs`
- Create: `crates/magic-market-core/src/evidence.rs`
- Modify: `crates/magic-market-core/src/instrument.rs`
- Modify: `crates/magic-market-core/src/provider.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

- [x] Implement trimmed, bounded, control-free text and HTTPS URL wrappers with
  checked deserialization.
- [x] Implement real Gregorian `YYYY-MM-DD` validation, finite signed number
  and positive-rank wrappers.
- [x] Implement `SourceEvidence` with provider, optional source time, observed
  time and batch ID; all text is checked and evidence is serde-safe.
- [x] Add Provider/asset variants without changing existing enum serialization.
- [x] Run the focused Core tests and require the new contracts to pass.

### Task 3: Add market enrichment and research contracts

**Files:**

- Create: `crates/magic-market-core/src/enrichment.rs`
- Create: `crates/magic-market-core/src/research.rs`
- Create: `crates/magic-market-core/tests/enrichment.rs`
- Create: `crates/magic-market-core/tests/research.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

- [x] Add `MarketStatistics` and `TechnicalBar` with optional fields and strict
  instrument/bar/evidence identity.
- [x] Add report scope, report, earnings estimate, consensus snapshot,
  semantic channel/document and bounded query request types.
- [x] Add `MarketStatisticsProvider`, `TechnicalBarsProvider`,
  `ResearchReports`, `ConsensusData` and `SemanticSearch` traits.
- [x] Implement `SourcedRecord` for every routable record.
- [x] Assert checked serde, missing-value preservation and no fabricated
  source time.

### Task 4: Add signals, fund-flow and capital contracts

**Files:**

- Create: `crates/magic-market-core/src/signals.rs`
- Create: `crates/magic-market-core/src/capital.rs`
- Create: `crates/magic-market-core/tests/signals.rs`
- Create: `crates/magic-market-core/tests/capital.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

- [x] Add board membership/category, strong-stock reason, dragon-tiger
  entry/seat, ranking, popularity and concept-hit records.
- [x] Add interval-aware `FundFlowPoint`, `BoardFlow`, margin, block trade,
  holder count, lockup and dividend records.
- [x] Represent every source-missing metric as `Option`; assert zero is retained
  only when the source explicitly supplied zero.
- [x] Add typed requests and traits for instrument/date/market-scoped batches.
- [x] Add domain capability structs with conservative all-false defaults.

### Task 5: Add content, company, limit-pool and option contracts

**Files:**

- Create: `crates/magic-market-core/src/content.rs`
- Create: `crates/magic-market-core/src/company.rs`
- Create: `crates/magic-market-core/src/limit_pool.rs`
- Create: `crates/magic-market-core/src/options.rs`
- Create: `crates/magic-market-core/tests/content.rs`
- Create: `crates/magic-market-core/tests/company.rs`
- Create: `crates/magic-market-core/tests/limit_pool.rs`
- Create: `crates/magic-market-core/tests/options.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

- [x] Add news, announcement and investor-question records with HTTPS canonical
  references and optional unanswered state.
- [x] Add security profile and three-statement financial records with stable
  keys plus retained source labels/units.
- [x] Add four raw limit-pool kinds and entries.
- [x] Add option contract identity, call/put, quote and exact Greeks records.
- [x] Add provider traits and conservative capabilities for all four domains.

### Task 6: Add Router adapters for every normalized family

**Files:**

- Modify: `crates/magic-market-router/src/adapters.rs`
- Modify: `crates/magic-market-router/src/lib.rs`
- Modify: `crates/magic-market-router/tests/adapters.rs`
- Create: `crates/magic-market-router/tests/intelligence_routing.rs`

- [x] Add router aliases for each routable record/request pair.
- [x] Add thin source functions bound only to Core traits.
- [x] Prove an intelligence record routes through the generic acceptance engine
  with provider/batch mismatch rejection and complete attempt traces.
- [x] Keep Router free of all concrete Provider crate dependencies.

### Task 7: Implement the pure analysis crate test-first

**Files:**

- Modify: `Cargo.toml`
- Create: `crates/magic-market-analysis/Cargo.toml`
- Create: `crates/magic-market-analysis/src/lib.rs`
- Create: `crates/magic-market-analysis/src/moving_average.rs`
- Create: `crates/magic-market-analysis/src/valuation.rs`
- Create: `crates/magic-market-analysis/src/limit_sentiment.rs`
- Create: `crates/magic-market-analysis/src/diagnostics.rs`
- Create: `crates/magic-market-analysis/tests/analysis.rs`

- [x] Write failing tests for SMA warm-up/ordering, forward PE, PEG, configured
  valuation scenarios, limit sentiment zero denominators and freshness/spread
  diagnostics.
- [x] Implement network-free deterministic functions returning typed errors
  for invalid/non-finite/zero-denominator input.
- [x] Attribute derived outputs to `ProviderId::LocalAnalysis` and retain input
  evidence for multi-source results.
- [x] Run the crate test suite and Clippy with warnings denied.

### Task 8: Slice A compatibility and quality gate

**Files:**

- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `.planning/2026-07-23-a-stock-data-parity/task_plan.md`
- Modify: `.planning/2026-07-23-a-stock-data-parity/progress.md`

- [x] Document the provider-neutral domains as implemented but not yet
  live-connected.
- [x] Run `cargo fmt --all --check`.
- [x] Run `RUSTUP_TOOLCHAIN=1.83.0 cargo check --workspace --all-targets
  --locked`.
- [x] Run `cargo test --workspace --all-targets --locked --offline`.
- [x] Run strict workspace Clippy and rustdoc.
- [ ] Review `git diff --check`, verify the user's requirements file is
  unstaged, commit Slice A and push `main`.
