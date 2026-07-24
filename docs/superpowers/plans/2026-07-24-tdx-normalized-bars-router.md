# Magic TDX Normalized Bars Router Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Magic TDX the provider-neutral Core `Bar` source accepted directly by `magic-market-router`, with strict source evidence and no raw `HistoricalBars` contract.

**Architecture:** Keep TDX TCP parsing and raw `get_security_bars` as the wire boundary. Replace the provider-facing and service-facing bar associated type with `magic_market_core::Bar`; normalize atomically in `adapter.rs`, then prove the contract through unit, router and live admission tests.

**Tech Stack:** Rust 1.95 stable, `magic-market-core`, `magic-tdx-rs`, `magic-market-router`, Cargo tests/Clippy/llvm-cov.

---

## File map

- `crates/magic-tdx-rs/src/protocol/parsers.rs`: reject partial packet decode.
- `crates/magic-tdx-rs/src/adapter.rs`: raw-to-Core normalization and provider impls.
- `crates/magic-tdx-rs/src/service/mod.rs`: normalized service return type.
- `crates/magic-tdx-rs/src/fund/types.rs`: correct the misleading volume-unit comment.
- `crates/magic-tdx-rs/tests/adapter.rs`: public associated-type contract.
- `crates/magic-tdx-rs/tests/internal/adapter.rs`: deterministic mapping/failure tests.
- `crates/magic-tdx-rs/tests/protocol_behaviors.rs`: packet atomicity tests.
- `crates/magic-tdx-rs/examples/live_probe.rs`: normalized evidence and unit admission.
- `crates/magic-market-router/tests/tdx_bars.rs`: direct TDX adapter/route contract.
- `docs/business_rules.md`: register strict TDX sequence and normalization rules.

### Task 1: Register strict TDX bar rules

- [ ] Add BR-013 stating that TDX bar normalization is atomic, never sorts or
  deduplicates, preserves source units, and requires record/batch evidence.
- [ ] Run `bash tools/compliance/check.sh`; expect PASS.
- [ ] Commit with `docs(tdx): register normalized bar rules`.

### Task 2: Prove and fix packet atomicity

- [ ] Add parser tests whose declared count is larger than the complete payload
  and whose next row has an invalid date; expect an explicit parse error.
- [ ] Run `cargo test -p magic-tdx-rs protocol_behaviors -- --nocapture`; expect
  the new tests to fail before implementation.
- [ ] Replace the two partial-success `break` paths in security-bar parsing with
  `TdxError::InvalidData`/protocol errors containing the declared row index.
- [ ] Run the same test target; expect PASS.
- [ ] Commit with `fix(tdx): reject partial bar packets`.

### Task 3: Normalize one complete batch

- [ ] Replace `strict_bars(source, Vec<SecurityBar>)` with
  `normalize_bars(source, request, Vec<SecurityBar>) ->
  Result<DataBatch<Bar>, TdxError>`.
- [ ] Add failing tests covering daily and five-minute timestamps, OHLC,
  unchanged lots/yuan, `Unadjusted`, `ProviderId::Tdx`, latest batch
  `source_at`, and identical record/provenance batch IDs.
- [ ] Add failing tests for empty/oversized batches, invalid component/time
  identity, duplicate/non-increasing time, NaN/Inf, non-positive prices,
  negative volume/amount, positive volume with zero amount, OHLC conflict and
  adjacent close change beyond 20%.
- [ ] Implement canonical intraday time and checked batch normalization. Build
  provenance first, read its generated batch ID, then construct every `Bar`
  with that exact ID and record source time.
- [ ] Run `cargo test -p magic-tdx-rs internal::adapter -- --nocapture`; expect
  PASS.
- [ ] Commit with `feat(tdx): normalize strict core bar batches`.

### Task 4: Replace the raw provider contract

- [ ] Change all sync/async `HistoricalBars` associated types from
  `SecurityBar` to Core `Bar`.
- [ ] Change `historical_bars_with`, `historical_bars_async_with`,
  `TdxService::bars` and `AsyncTdxService::bars` to return `DataBatch<Bar>`.
- [ ] Change public compile-time tests to require
  `HistoricalBars<Bar = magic_market_core::Bar>` and
  `AsyncHistoricalBars<Bar = magic_market_core::Bar>`.
- [ ] Run `cargo test -p magic-tdx-rs`; expect PASS.
- [ ] Run `rg -n 'HistoricalBars<Bar = .*SecurityBar|DataBatch<SecurityBar>' crates/magic-tdx-rs`;
  expect no provider/service contract matches.
- [ ] Commit with `refactor(tdx): expose one normalized bars contract`.

### Task 5: Prove router integration

- [ ] Add a compile/runtime integration test that passes `Arc<TdxHqClient>` to
  `magic_market_router::bars_source`.
- [ ] Add deterministic scripted providers proving a complete TDX batch is
  selected, an invalid TDX batch fails over to one complete Tencent-labelled
  batch, and no record/batch evidence can mix.
- [ ] Run `cargo test -p magic-market-router tdx_bars -- --nocapture`; expect
  PASS.
- [ ] Commit with `test(router): admit normalized Magic TDX bars`.

### Task 6: Correct documentation and live admission

- [ ] Correct the fund volume comment without changing values.
- [ ] Extend the TDX live probe to print normalized Core bar fields and assert
  provider, source time, fetched time, non-empty batch ID and per-record batch
  equality.
- [ ] For positive-volume rows, assert
  `amount / (volume * 100)` lies within low/high plus the documented rounding
  tolerance; fail non-zero on mismatch.
- [ ] Probe Shanghai, Shenzhen and Beijing daily bars plus a completed
  five-minute interval.
- [ ] Run `cargo run -p magic-tdx-rs --example live_probe`; expect
  `live_probe_status=passed`.
- [ ] Commit with `test(tdx): admit normalized live bars`.

### Task 7: Run release gates and coverage

- [ ] Run `cargo fmt --all -- --check`; expect PASS.
- [ ] Run `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`;
  expect PASS.
- [ ] Run `cargo test --workspace --all-targets --all-features --locked --offline -- --test-threads=1`;
  expect PASS.
- [ ] Run `bash tools/compliance/check.sh`; expect PASS.
- [ ] Run `bash tools/docs/check_links.sh`; expect PASS.
- [ ] Run the repository coverage command and require overall at least 80% and
  configured TDX critical paths at least 95%.
- [ ] Record exact outputs in release evidence; commit only generated,
  repository-tracked evidence required by policy.

