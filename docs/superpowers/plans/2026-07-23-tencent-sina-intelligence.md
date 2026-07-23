# Tencent and Sina Intelligence Implementation Plan

> **Execution:** Use the `executing-plans` workflow task-by-task. Keep the
> user's untracked requirements document unstaged.

**Goal:** Complete Tencent market-statistics enrichment and Sina financial
statement/ETF-option families with strict normalized contracts, deterministic
fixtures, bounded live probes and honest source evidence.

**Architecture:** Preserve the existing source-aligned Provider crates and
injected bounded transports. Tencent reuses one quote response but parses
enrichment independently so short base quotes remain valid. Sina adds isolated
financial and option modules; Core option records are widened only where the
source exposes real fields or lacks an exact expiry.

**Tech stack:** Rust 2021, MSRV 1.83, `serde`, `serde_json`, `encoding_rs`,
`ureq`, existing Core/Provider/Router contracts.

---

### Task 1: Widen the new Core option contracts test-first

**Files:**

- Modify: `crates/magic-market-core/src/options.rs`
- Modify: `crates/magic-market-core/tests/options.rs`
- Modify: `crates/magic-market-router/tests/intelligence_routing.rs`

- [ ] Add RED tests for a source-known contract month with absent exact expiry
  and strike.
- [ ] Add bid/ask quantities, strike, open/high/low/previous close, limits,
  amount and source name to option quotes.
- [ ] Add source name, volume, high/low, trade code, strike, last and theoretical
  price to Greeks while retaining optional rho.
- [ ] Keep checked serde and `SourcedRecord` evidence invariants.
- [ ] Run focused Core and Router tests.

### Task 2: Implement Tencent market statistics test-first

**Files:**

- Modify: `crates/magic-tencent-rs/src/lib.rs`
- Modify: `crates/magic-tencent-rs/examples/live_probe.rs`
- Modify: `crates/magic-tencent-rs/examples/load_probe.rs`

- [ ] Add extended fixture records and RED tests for fields
  38/39/44/45/46/47/48/49/52 and CNY market-cap unit conversion.
- [ ] Keep the existing base quote parser valid for responses ending at field
  37; reject truncated enrichment only from the enrichment operation.
- [ ] Accept explicit equity/index/fund identities for market statistics without
  weakening equity-only quote/order-book unit guarantees.
- [ ] Implement `MarketStatisticsProvider` with exact cardinality, no prefix
  guessing, optional-field preservation and Tencent evidence.
- [ ] Print every statistic for equity, index and ETF in `live_probe`; add a
  bounded market-statistics load operation.
- [ ] Run deterministic, live and conservative load probes.

### Task 3: Implement Sina financial statements test-first

**Files:**

- Modify: `crates/magic-sina-rs/Cargo.toml`
- Create: `crates/magic-sina-rs/src/financials.rs`
- Modify: `crates/magic-sina-rs/src/lib.rs`
- Modify: `crates/magic-sina-rs/examples/live_probe.rs`
- Modify: `crates/magic-sina-rs/examples/load_probe.rs`

- [ ] Add fixture tests for the corrected
  `result.data.report_list.<period>.data[]` shape.
- [ ] Map balance/income/cash-flow requests to `fzb/lrb/llb` without exchange
  guessing and bound period counts.
- [ ] Parse every finite numeric line, retain the source label and use
  deterministic normalized keys without manufacturing units or dates.
- [ ] Implement `FinancialStatements` with strict source response validation,
  evidence and truthful incomplete-quality issues.
- [ ] Print all three statements in `live_probe`; add a bounded financial load
  operation.
- [ ] Run deterministic and real endpoint probes.

### Task 4: Implement Sina ETF options test-first

**Files:**

- Create: `crates/magic-sina-rs/src/options.rs`
- Modify: `crates/magic-sina-rs/src/lib.rs`
- Modify: `crates/magic-sina-rs/examples/live_probe.rs`
- Modify: `crates/magic-sina-rs/examples/load_probe.rs`

- [ ] Add fixtures for month discovery, call/put code lists, 43-field T-quotes
  and 16-field Greeks with exactly three structural blank slots.
- [ ] Support the verified ETF underlyings through an explicit code/category
  table and reject unknown assets.
- [ ] Preserve source contract month where exact expiry is absent and fetch
  quote/Greeks only for explicitly requested bounded code lists.
- [ ] Add a per-request Referer transport method while retaining compatibility
  with injected fixture transports.
- [ ] Implement `OptionData`, full T-quote/Greeks parsing and source evidence.
- [ ] Print discovery, quote and Greeks values in `live_probe`; add conservative
  options load operations.
- [ ] Run deterministic and real endpoint probes.

### Task 5: Slice B documentation and release gate

**Files:**

- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `.planning/2026-07-23-a-stock-data-parity/task_plan.md`
- Modify: `.planning/2026-07-23-a-stock-data-parity/progress.md`

- [ ] Document exact supported fields, units, endpoint stability and remaining
  reference-project Provider gaps.
- [ ] Run format, Rust 1.83 locked all-target check and all workspace tests.
- [ ] Run strict workspace Clippy, rustdoc, doctests, docs links and compliance.
- [ ] Review the complete diff, verify the user's requirements file is unstaged,
  commit Slice B, push `main` and record the commit.
