# Phase 1: Workspace and Core Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the standalone governed virtual workspace and deliver the complete provider-neutral `magic-market-core` contract with checked values, requests, provenance, quality, models, and capability traits.

**Architecture:** The repository root is a two-member virtual Cargo workspace with no root package. `magic-market-core` contains no TDX names, endpoints, networking, Tokio, or downstream application types; it exposes small checked value objects and capability-specific synchronous traits whose results always carry provenance.

**Tech Stack:** Rust stable, Cargo resolver 2, Serde, thiserror, uuid, proptest, shell compliance checks.

---

## Exit gate

Phase 1 is complete only when governance/compliance files exist, both library crates compile as workspace members, every core contract test passes, public rustdoc is warning-free, and the root lockfile is committed. No protocol or network implementation belongs in this phase.

### Task 1: Establish repository governance and the virtual workspace

**Files:**
- Create: `AGENTS.md`
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `LICENSE-APACHE`
- Create: `LICENSE-MIT`
- Create: `LICENSES/tdxrs-MIT.txt`
- Create: `provenance/upstream-files.toml`
- Create: `docs/ENGINEERING_RULES.md`
- Create: `docs/business_rules.md`
- Create: `tools/compliance/check.sh`
- Create: `crates/magic-market-core/Cargo.toml`
- Create: `crates/magic-market-core/src/lib.rs`
- Create: `crates/magic-tdx-rs/Cargo.toml`
- Create: `crates/magic-tdx-rs/src/lib.rs`
- Create: `crates/magic-tdx-rs/README.md`
- Create: `crates/magic-market-core/README.md`

- [ ] **Step 1: Write the compliance test before the workspace files**

Create `tools/compliance/check.sh` as an executable script with these exact checks:

```bash
#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$repo_root"

required=(
  AGENTS.md Cargo.toml Cargo.lock LICENSE-APACHE LICENSE-MIT
  LICENSES/tdxrs-MIT.txt docs/ENGINEERING_RULES.md docs/business_rules.md
  provenance/upstream-files.toml
)
for path in "${required[@]}"; do
  test -s "$path" || { echo "missing required file: $path" >&2; exit 1; }
done

rg -q '^members = \["crates/magic-market-core", "crates/magic-tdx-rs"\]$' Cargo.toml
if rg -n 'stock_analysis|path\s*=\s*"\.\./' crates/*/Cargo.toml; then
  echo "production manifests must not depend on sibling checkouts" >&2
  exit 1
fi
if rg -n '(mock|fixture|TEST_CODE)' crates/*/src --glob '*.rs'; then
  echo "test-only names found in production source" >&2
  exit 1
fi
if rg -n '(todo!\(|unimplemented!\(|panic!\(|unwrap\(|expect\()' crates/*/src --glob '*.rs'; then
  echo "panic or unfinished implementation found in production source" >&2
  exit 1
fi
rg -q '^## BR-001 ' docs/business_rules.md
rg -q '^## Gate A ' docs/ENGINEERING_RULES.md
rg -q '^## Gate D ' docs/ENGINEERING_RULES.md
```

- [ ] **Step 2: Run the compliance test and verify the expected failure**

Run: `bash tools/compliance/check.sh`

Expected: non-zero exit with `missing required file: AGENTS.md`.

- [ ] **Step 3: Add governance documents and register behavior-changing rules**

Write `AGENTS.md` with mandatory pre-flight reporting, Gate A→B→C→D ordering, explicit-failure/data-redline rules 2.1–2.10, business-rule registration before behavior changes, exact validation commands, no external-downstream reads, and truthful `In Progress / Blocked` reporting. Write `docs/ENGINEERING_RULES.md` with the same four gates and repository-specific evidence requirements.

Start `docs/business_rules.md` with these fixed headings and decisions:

```markdown
# Business rules

## BR-001 Quote request cardinality
Strict quote requests accept 1 through 60 instruments. `quotes_chunked` is the only API that may split a larger request; it preserves input order and duplicates and reports every source chunk.

## BR-002 Strict pagination
Strict paginated operations are atomic. Any failed, missing, duplicated, or out-of-range page fails the whole operation; explicitly named best-effort APIs return page outcomes and missing ranges.

## BR-003 Pool and queue policy
Blocking defaults to five connections; Async defaults to four connection tasks with bounded per-connection channels and deterministic round-robin selection. Pool and queue waits are covered by the operation deadline.

## BR-004 Smart server policy
Smart selection orders eligible servers by observed health and latency, applies observable cooldown after failure, and shares one retry budget across the whole operation. Exhaustion returns an aggregate error.

## BR-005 Adaptive rate limits
The compatible schedule is 15/30/60 requests per second with an absolute 200 requests-per-second configuration ceiling, Asia/Shanghai market phases, an injectable clock, conservative unknown-phase behavior, and explicit PerClient or PerConnection scope.

## BR-006 Cache policy
Caching is disabled by default. Enabled caches expose hit/miss and age in result metadata and never relabel stale values as fresh source data.
```

- [ ] **Step 4: Create the virtual workspace and crate manifests**

Use this root manifest:

```toml
[workspace]
members = ["crates/magic-market-core", "crates/magic-tdx-rs"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/Northofqing/magic-market-data-rs"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
uuid = { version = "1", features = ["serde", "v4"] }

[workspace.lints.rust]
missing_docs = "deny"
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
```

Set `rust-toolchain.toml` to channel `stable` with `rustfmt` and `clippy`. Both crate manifests inherit workspace package fields and lints; `magic-tdx-rs` depends on `magic-market-core = { path = "../magic-market-core", version = "=0.1.0" }`. Give each crate a documented `#![forbid(unsafe_code)]` library root. Do not add a root `[package]` section.

- [ ] **Step 5: Preserve licenses and source provenance entry points**

Copy the upstream MIT text byte-for-byte from pinned commit `18b05ffc9d8a257b5ba5add8a2d1ab038261747d` into `LICENSES/tdxrs-MIT.txt`. Add project MIT and Apache-2.0 license texts. Create `provenance/upstream-files.toml` with this schema and no source rows yet:

```toml
upstream_repository = "https://github.com/jiangtaovan/tdxrs"
upstream_commit = "18b05ffc9d8a257b5ba5add8a2d1ab038261747d"
upstream_version = "0.6.7"
upstream_license = "MIT"

files = []
```

- [ ] **Step 6: Generate the lockfile and run the governance gate**

Run:

```bash
cargo generate-lockfile
bash tools/compliance/check.sh
cargo metadata --no-deps --format-version 1
```

Expected: all commands exit `0`; metadata lists exactly `magic-market-core` and `magic-tdx-rs` workspace members and no root package.

- [ ] **Step 7: Commit the governance scaffold**

```bash
git add AGENTS.md Cargo.toml Cargo.lock rust-toolchain.toml .gitignore LICENSE-APACHE LICENSE-MIT LICENSES/tdxrs-MIT.txt docs/ENGINEERING_RULES.md docs/business_rules.md tools/compliance/check.sh provenance/upstream-files.toml crates/magic-market-core crates/magic-tdx-rs
git diff --cached --check
git commit -m "chore: establish standalone market data workspace"
```

### Task 2: Implement instruments and checked numeric values

**Files:**
- Create: `crates/magic-market-core/src/error.rs`
- Create: `crates/magic-market-core/src/instrument.rs`
- Create: `crates/magic-market-core/src/value.rs`
- Create: `crates/magic-market-core/tests/values.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

- [ ] **Step 1: Write failing construction tests**

Create `tests/values.rs` covering exact invariants:

```rust
use magic_market_core::{AssetClass, CoreError, Exchange, InstrumentId, Money, Price, Quantity, Ratio};

#[test]
fn rejects_invalid_financial_values() {
    assert!(matches!(Price::new(0.0), Err(CoreError::InvalidValue { field: "price", .. })));
    assert!(Price::new(f64::NAN).is_err());
    assert!(Quantity::new(-1.0).is_err());
    assert!(Money::new(f64::INFINITY).is_err());
    assert!(Ratio::decimal(f64::NAN).is_err());
}

#[test]
fn instrument_code_is_trimmed_but_never_empty() {
    let id = InstrumentId::new(Exchange::Shanghai, " 600000 ", AssetClass::Equity).unwrap();
    assert_eq!(id.code(), "600000");
    assert!(InstrumentId::new(Exchange::Shenzhen, "   ", AssetClass::Equity).is_err());
}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `cargo test -p magic-market-core --test values`

Expected: compile failure because the exported types do not exist.

- [ ] **Step 3: Implement contextual core errors and instrument identifiers**

Define `CoreError` as `#[non_exhaustive]` with `InvalidValue { field: &'static str, value: String, reason: &'static str }`, `InvalidInstrument`, `InvalidTime`, `InvalidRequest`, and `QualityRejected { issue_count: usize }`. Define non-exhaustive `Exchange` and `AssetClass` enums and a private-field `InstrumentId` with checked `new`, `exchange`, `code`, and `asset_class` accessors. Derive `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, and `Deserialize` wherever the field types permit it.

- [ ] **Step 4: Implement checked value newtypes**

Use a private `f64` field and checked constructors. `Price` must be finite and strictly positive; `Quantity` finite and non-negative; `Money` finite; and `Ratio` finite with an explicit `RatioUnit::{Decimal, Percent}`. Provide `get()` accessors and no unchecked `From<f64>` implementation. Conversion from source floats must therefore use `TryFrom` or constructors.

- [ ] **Step 5: Export the stable facade and run tests**

In `src/lib.rs`, keep modules private and re-export only `CoreError`, `Exchange`, `AssetClass`, `InstrumentId`, `Price`, `Quantity`, `Money`, `Ratio`, and `RatioUnit`. Run:

```bash
cargo test -p magic-market-core --test values
cargo clippy -p magic-market-core --all-targets -- -D warnings
```

Expected: both commands pass with no warning.

- [ ] **Step 6: Commit checked primitives**

```bash
git add crates/magic-market-core/src/error.rs crates/magic-market-core/src/instrument.rs crates/magic-market-core/src/value.rs crates/magic-market-core/src/lib.rs crates/magic-market-core/tests/values.rs
git diff --cached --check
git commit -m "feat(core): add checked instruments and values"
```

### Task 3: Implement explicit time and request contracts

**Files:**
- Create: `crates/magic-market-core/src/time.rs`
- Create: `crates/magic-market-core/src/request.rs`
- Create: `crates/magic-market-core/tests/requests.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

- [ ] **Step 1: Write failing request validation tests**

```rust
use magic_market_core::{Adjustment, BarPeriod, BarRequest, Exchange, InstrumentId, AssetClass, TimestampMillis};

#[test]
fn range_must_be_ordered_and_limit_positive() {
    let instrument = InstrumentId::new(Exchange::Shanghai, "600000", AssetClass::Equity).unwrap();
    assert!(BarRequest::new(instrument.clone(), BarPeriod::Day, TimestampMillis::new(2), TimestampMillis::new(1), 10, Adjustment::None).is_err());
    assert!(BarRequest::new(instrument, BarPeriod::Day, TimestampMillis::new(1), TimestampMillis::new(2), 0, Adjustment::None).is_err());
}
```

- [ ] **Step 2: Run the test and verify it fails to compile**

Run: `cargo test -p magic-market-core --test requests`

Expected: unresolved imports for the request types.

- [ ] **Step 3: Implement time newtypes**

Define `TimestampMillis(i64)` as UTC Unix milliseconds and `MarketDate(u32)` as checked `YYYYMMDD`. `MarketDate::new` must validate year 1900–9999, month length, and leap years; expose `year`, `month`, `day`, and `yyyymmdd`. Keep exchange-local conversion out of core.

- [ ] **Step 4: Implement request enums and structs**

Define non-exhaustive `BarPeriod`, `Adjustment`, `Pagination`, and `SortOrder`. Implement private-field `BarRequest`, `QuoteRequest`, `MinuteRequest`, `TradeRequest`, and `PageRequest` with constructors that reject reversed ranges, zero limits/counts, and empty instrument sets. `QuoteRequest::new` does not impose a TDX-specific 60-item limit; that rule belongs in `magic-tdx-rs`.

- [ ] **Step 5: Export and verify request contracts**

Run:

```bash
cargo test -p magic-market-core --test requests
cargo test -p magic-market-core
```

Expected: all tests pass, including leap-day and reversed-range cases.

- [ ] **Step 6: Commit request contracts**

```bash
git add crates/magic-market-core/src/time.rs crates/magic-market-core/src/request.rs crates/magic-market-core/src/lib.rs crates/magic-market-core/tests/requests.rs
git diff --cached --check
git commit -m "feat(core): add time and request contracts"
```

### Task 4: Implement provenance-bearing batches and quality outcomes

**Files:**
- Create: `crates/magic-market-core/src/provenance.rs`
- Create: `crates/magic-market-core/src/batch.rs`
- Create: `crates/magic-market-core/src/quality.rs`
- Create: `crates/magic-market-core/tests/quality.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

- [ ] **Step 1: Write failing provenance and quality tests**

```rust
use magic_market_core::{BatchCompleteness, DataBatch, Price, Provenance, validate_price_series};

#[test]
fn fetched_time_never_becomes_source_time() {
    let provenance = Provenance::builder("tdx", "quotes", 1_000).request_id("r-1").build().unwrap();
    assert_eq!(provenance.source_at(), None);
    assert_eq!(provenance.fetched_at().get(), 1_000);
}

#[test]
fn large_valid_price_change_is_preserved() {
    let prices = [Price::new(10.0).unwrap(), Price::new(20.0).unwrap()];
    let report = validate_price_series(&prices);
    assert!(!report.is_blocking());
}
```

- [ ] **Step 2: Run the focused test and verify unresolved imports**

Run: `cargo test -p magic-market-core --test quality`

Expected: compile failure before these contracts are implemented.

- [ ] **Step 3: Implement provenance with an explicit builder**

`Provenance` must contain provider, operation, request id, trace id, endpoint/local source, fetched time, optional source time, requested/received counts, page statistics, adjustment label, cache status/age, and `BatchCompleteness`. The builder requires provider, operation, fetched time, and request id; it never defaults `source_at` from `fetched_at`. Define `BatchCompleteness::{Complete, Partial { missing_ranges, failed_pages }}` and `CacheStatus::{Disabled, Miss, Hit { age_millis }}`.

- [ ] **Step 4: Implement immutable `DataBatch<T>`**

Store `Vec<T>`, `Provenance`, and `QualityReport` behind private fields. `DataBatch::strict` rejects non-complete provenance; `DataBatch::best_effort` accepts partial provenance and preserves page failures. Expose slices and metadata through accessors and `into_records`; do not implement `Deref<Vec<T>>`.

- [ ] **Step 5: Implement quality issues and deterministic validators**

Define non-exhaustive `QualityIssueKind::{NonPositivePrice, DuplicateTimestamp, TimeGap, CorporateActionDiscontinuity, MissingField}` and severity `Warning | Blocking`. `validate_price_series` rejects invalid numeric values but preserves large valid changes; corporate-action and source-consistency checks require explicit evidence rather than a fixed percentage threshold. Add timestamp uniqueness/order validation and merge reports without losing issue context.

- [ ] **Step 6: Verify source-time and quality behavior**

Run:

```bash
cargo test -p magic-market-core --test quality
cargo test -p magic-market-core
```

Expected: all tests pass; add assertions for large valid changes, duplicate timestamps, complete/partial batch construction, and cache age.

- [ ] **Step 7: Commit provenance and quality contracts**

```bash
git add crates/magic-market-core/src/provenance.rs crates/magic-market-core/src/batch.rs crates/magic-market-core/src/quality.rs crates/magic-market-core/src/lib.rs crates/magic-market-core/tests/quality.rs
git diff --cached --check
git commit -m "feat(core): add provenance batches and quality reports"
```

### Task 5: Implement normalized models and capability traits

**Files:**
- Create: `crates/magic-market-core/src/model.rs`
- Create: `crates/magic-market-core/src/provider.rs`
- Create: `crates/magic-market-core/tests/providers.rs`
- Modify: `crates/magic-market-core/src/lib.rs`

- [ ] **Step 1: Write compile-time capability tests with a minimal provider**

In `tests/providers.rs`, define `BarsOnlyProvider` implementing only `HistoricalBars`, returning `DataBatch<Bar>`, and a generic assertion `fn accepts_bars<P: HistoricalBars>(_: &P) {}`. Do not implement or require `RealtimeQuotes`. Add a `compile_fail` rustdoc example showing that a bars-only provider cannot be passed to a `RealtimeQuotes` bound.

- [ ] **Step 2: Run the test and verify model/trait imports fail**

Run: `cargo test -p magic-market-core --test providers`

Expected: compile failure because `Bar` and capability traits are not defined.

- [ ] **Step 3: Implement normalized models with private fields**

Define checked models `Bar`, `Quote`, `Trade`, `Fundamental`, `CorporateAction`, `Fund`, `Block`, and `Profile`. Constructors accept only checked core values and explicit optional fields. `Bar` validates `low <= open/close <= high`, non-decreasing time meaning is validated at batch level, and source-specific unknown fields do not enter these normalized models.

- [ ] **Step 4: Implement capability-specific provider traits**

Define the exact stable traits `InstrumentProvider`, `HistoricalBars`, `RealtimeQuotes`, `MinuteData`, `Trades`, `Fundamentals`, `CorporateActions`, `FundData`, `BlockData`, and `ProfileData`. Each trait has an associated `Error: std::error::Error + Send + Sync + 'static` and one or more typed methods returning `Result<DataBatch<Model>, Self::Error>`. Use request structs rather than magic integers. Do not create a single all-capabilities supertrait.

- [ ] **Step 5: Export the facade and verify trait independence**

Run:

```bash
cargo test -p magic-market-core --test providers
cargo test -p magic-market-core --doc
cargo doc -p magic-market-core --no-deps
```

Expected: all commands pass and rustdoc reports no missing public documentation or broken links.

- [ ] **Step 6: Commit models and provider traits**

```bash
git add crates/magic-market-core/src/model.rs crates/magic-market-core/src/provider.rs crates/magic-market-core/src/lib.rs crates/magic-market-core/tests/providers.rs
git diff --cached --check
git commit -m "feat(core): define normalized models and provider traits"
```

### Task 6: Complete Phase 1 documentation and validation

**Files:**
- Modify: `README.md`
- Modify: `README.en.md`
- Modify: `crates/magic-market-core/README.md`
- Modify: `crates/magic-tdx-rs/README.md`
- Modify: `.planning/2026-07-21-magic-tdx-rs/progress.md`

- [ ] **Step 1: Document the two-crate boundary and current completion state**

The root Chinese README and English synopsis must show the virtual workspace, one-way dependency, source-vs-normalized model split, `source_at` rule, strict/default semantics, MSRV, and state that protocol/client work is not yet delivered. The core README must include compiling examples for `Price`, `InstrumentId`, `BarRequest`, `Provenance`, and a capability trait bound.

- [ ] **Step 2: Run the complete Phase 1 gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test --workspace --doc --all-features
cargo doc --workspace --all-features --no-deps
bash tools/compliance/check.sh
cargo check --workspace --all-targets
```

Expected: every command exits `0`; no default test opens an external network connection.

- [ ] **Step 3: Record exact evidence and commit the Phase 1 closeout**

Append every command, exit result, and commit SHA to the active `progress.md`, then run:

```bash
git add README.md README.en.md crates/magic-market-core/README.md crates/magic-tdx-rs/README.md .planning/2026-07-21-magic-tdx-rs/progress.md
git diff --cached --check
git commit -m "docs: close foundation phase"
```

- [ ] **Step 4: Stop for Phase 1 review**

Report the committed SHAs, validation results, and any evidence still blocked. Do not begin Phase 2 until the reviewer accepts this exit gate.
