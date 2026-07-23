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

**Tech stack:** Rust 2021, no fixed MSRV, `serde`, `serde_json`, `encoding_rs`,
`ureq`, existing Core/Provider/Router contracts.

---

### Task 1: Widen the new Core option contracts test-first

**Files:**

- Modify: `crates/magic-market-core/src/options.rs`
- Modify: `crates/magic-market-core/tests/options.rs`
- Modify: `crates/magic-market-router/tests/intelligence_routing.rs`

- [x] Add RED tests for a source-known contract month with absent exact expiry
  and strike.
- [x] Add bid/ask quantities, strike, open/high/low/previous close, limits,
  amount and source name to option quotes.
- [x] Add source name, volume, high/low, trade code, strike, last and theoretical
  price to Greeks while retaining optional rho.
- [x] Keep checked serde and `SourcedRecord` evidence invariants.
- [x] Run focused Core and Router tests.

### Task 2: Implement Tencent market statistics test-first

**Files:**

- Modify: `crates/magic-tencent-rs/src/lib.rs`
- Modify: `crates/magic-tencent-rs/examples/live_probe.rs`
- Modify: `crates/magic-tencent-rs/examples/load_probe.rs`

- [x] Add extended fixture records and RED tests for fields
  38/39/44/45/46/47/48/49/52 and CNY market-cap unit conversion.
- [x] Keep the existing base quote parser valid for responses ending at field
  37; reject truncated enrichment only from the enrichment operation.
- [x] Accept explicit equity/index/fund identities for market statistics without
  weakening equity-only quote/order-book unit guarantees.
- [x] Implement `MarketStatisticsProvider` with exact cardinality, no prefix
  guessing, optional-field preservation and Tencent evidence.
- [x] Print every statistic for equity, index and ETF in `live_probe`; add a
  bounded market-statistics load operation.
- [x] Run deterministic, live and conservative load probes.

### Task 3: Implement Sina financial statements test-first

**Files:**

- Modify: `crates/magic-sina-rs/Cargo.toml`
- Create: `crates/magic-sina-rs/src/financials.rs`
- Modify: `crates/magic-sina-rs/src/lib.rs`
- Modify: `crates/magic-sina-rs/examples/live_probe.rs`
- Modify: `crates/magic-sina-rs/examples/load_probe.rs`

- [x] Add fixture tests for the corrected
  `result.data.report_list.<period>.data[]` shape.
- [x] Map balance/income/cash-flow requests to `fzb/lrb/llb` without exchange
  guessing and bound period counts.
- [x] Parse every finite numeric line, retain the source label and use
  deterministic normalized keys without manufacturing units or dates.
- [x] Implement `FinancialStatements` with strict source response validation,
  evidence and truthful incomplete-quality issues.
- [x] Print all three statements in `live_probe`; add a bounded financial load
  operation.
- [x] Run deterministic and real endpoint probes.

### Task 4: Implement Sina ETF options test-first

**Files:**

- Create: `crates/magic-sina-rs/src/options.rs`
- Modify: `crates/magic-sina-rs/src/lib.rs`
- Modify: `crates/magic-sina-rs/examples/live_probe.rs`
- Modify: `crates/magic-sina-rs/examples/load_probe.rs`

- [x] Add fixtures for month discovery, call/put code lists, 43-field T-quotes
  and 16-field Greeks with exactly three structural blank slots.
- [x] Support the verified ETF underlyings through an explicit code/category
  table and reject unknown assets.
- [x] Preserve source contract month where exact expiry is absent and fetch
  quote/Greeks only for explicitly requested bounded code lists.
- [x] Add a per-request Referer transport method while retaining compatibility
  with injected fixture transports.
- [x] Implement `OptionData`, full T-quote/Greeks parsing and source evidence.
- [x] Print discovery, quote and Greeks values in `live_probe`; add conservative
  options load operations.
- [x] Run deterministic and real endpoint probes.

### Task 5: Slice B documentation and release gate

**Files:**

- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `.planning/2026-07-23-a-stock-data-parity/task_plan.md`
- Modify: `.planning/2026-07-23-a-stock-data-parity/progress.md`

- [x] Document exact supported fields, units, endpoint stability and remaining
  reference-project Provider gaps.
- [x] Run format, Rust stable locked all-target check and all workspace tests.
- [x] Run strict workspace Clippy, rustdoc, doctests, docs links and compliance.
- [x] Review the complete diff, verify the user's requirements file is unstaged,
  commit Slice B, push `main` and record the commit.
